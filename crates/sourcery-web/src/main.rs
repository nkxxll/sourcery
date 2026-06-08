use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use clap::Parser;
use sourcery_db::{
    Codebase, Diff, DiffWithChanges, File, FileState, FilenameSearchResult, FunctionSearchResult,
    PgPool, Version, VersionFunction,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct WebArgs {
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,
    #[arg(long, default_value = "localhost:8000")]
    bind: String,
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[derive(serde::Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sourcery_web=info,axum=info".into()),
        )
        .init();

    let args = WebArgs::parse();
    let pool = sourcery_db::connect(&args.database_url)
        .await
        .context("failed to connect to postgres")?;
    let app = Router::new()
        .route("/health", get(health))
        .route("/codebases", get(list_codebases))
        .route("/codebase/{id}", get(get_codebase))
        .route("/codebase/{id}/diff", get(list_diffs_by_codebase))
        .route("/codebase/{id}/metrics", get(list_codebase_metrics))
        .route("/version/{id}", get(get_version))
        .route("/file/{file_id}", get(get_file))
        .route("/function/{function_id}", get(get_function))
        .route("/version/{id}/changed_files", get(list_version_files))
        .route("/version/{id}/files", get(list_all_version_files))
        .route("/version/{id}/files/{file_state_id}", get(get_version_file))
        .route("/version/{id}/files/search", get(search_version_filenames))
        .route("/version/{id}/diff", get(get_version_diff))
        .route("/version/{id}/diffchange", get(get_version_diff_change))
        .route("/version/{id}/callgraph", get(list_version_callgraph))
        .route("/version/{id}/treemap/files", get(list_version_treemap_files))
        .route(
            "/version/{id}/treemap/functions",
            get(list_version_treemap_functions),
        )
        .route("/version/{id}/functions", get(list_version_functions))
        .route(
            "/version/{id}/functions/{function_id}",
            get(get_version_function),
        )
        .route(
            "/version/{id}/functions/search",
            get(search_version_functions),
        )
        .with_state(AppState { pool });

    let listener = tokio::net::TcpListener::bind(&args.bind)
        .await
        .with_context(|| format!("failed to bind on {}", args.bind))?;
    tracing::debug!("web server listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn list_codebases(
    State(state): State<AppState>,
) -> Result<Json<BTreeMap<String, Vec<Codebase>>>, (StatusCode, String)> {
    let codebases = sourcery_db::list_codebases_grouped_by_language(&state.pool)
        .await
        .map_err(internal_error)?;
    Ok(Json(codebases))
}

async fn get_codebase(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Codebase>, (StatusCode, String)> {
    let codebase = sourcery_db::get_codebase_by_id(&state.pool, id)
        .await
        .map_err(internal_error)?;
    match codebase {
        Some(codebase) => Ok(Json(codebase)),
        None => Err((StatusCode::NOT_FOUND, format!("codebase {id} not found"))),
    }
}

async fn list_diffs_by_codebase(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<Diff>>, (StatusCode, String)> {
    let diffs = sourcery_db::list_diffs_by_codebase(&state.pool, id)
        .await
        .map_err(internal_error)?;
    Ok(Json(diffs))
}

async fn list_codebase_metrics(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<Version>>, (StatusCode, String)> {
    let versions = sourcery_db::list_versions_by_codebase(&state.pool, id)
        .await
        .map_err(internal_error)?;
    Ok(Json(versions))
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    tracing::error!(error = %error, "database request failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error".to_string(),
    )
}

#[derive(serde::Deserialize)]
struct PageQuery {
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default)]
    offset: u32,
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    50
}

fn bounded_limit(limit: u32) -> i32 {
    limit.min(500) as i32
}

async fn get_version_or_not_found(
    pool: &PgPool,
    id: Uuid,
) -> Result<Version, (StatusCode, String)> {
    let version = sourcery_db::get_version_by_id_optional(pool, id)
        .await
        .map_err(internal_error)?;
    version.ok_or_else(|| (StatusCode::NOT_FOUND, format!("version {id} not found")))
}

async fn get_version(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Version>, (StatusCode, String)> {
    let version = get_version_or_not_found(&state.pool, id).await?;
    Ok(Json(version))
}

async fn get_file(
    Path(file_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<FileState>, (StatusCode, String)> {
    let file = sourcery_db::get_file_state_by_id(&state.pool, file_id)
        .await
        .map_err(internal_error)?;
    match file {
        Some(file) => Ok(Json(file)),
        None => Err((StatusCode::NOT_FOUND, format!("file {file_id} not found"))),
    }
}

async fn get_function(
    Path(function_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<VersionFunction>, (StatusCode, String)> {
    let function = sourcery_db::get_version_function_by_id(&state.pool, function_id)
        .await
        .map_err(internal_error)?;
    match function {
        Some(function) => Ok(Json(function)),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("function {function_id} not found"),
        )),
    }
}

async fn get_version_diff(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Diff>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let diff = sourcery_db::get_diff_by_version(&state.pool, id)
        .await
        .map_err(internal_error)?;
    Ok(Json(diff))
}

async fn list_version_files(
    Path(id): Path<Uuid>,
    Query(query): Query<PageQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<File>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let files = sourcery_db::list_files_by_version_paginated(
        &state.pool,
        id,
        i64::from(query.limit),
        i64::from(query.offset),
    )
    .await
    .map_err(internal_error)?;
    Ok(Json(files))
}

async fn list_all_version_files(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<FileState>>, (StatusCode, String)> {
    let files = sourcery_db::list_all_files_states(&state.pool, id)
        .await
        .map_err(internal_error)?;
    Ok(Json(files))
}

async fn get_version_file(
    Path((id, file_state_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
) -> Result<Json<FileState>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let file = sourcery_db::get_file_state_by_id_for_version(&state.pool, id, file_state_id)
        .await
        .map_err(internal_error)?;
    match file {
        Some(file) => Ok(Json(file)),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("file {file_state_id} not found for version {id}"),
        )),
    }
}

async fn list_version_functions(
    Path(id): Path<Uuid>,
    Query(query): Query<PageQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<VersionFunction>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let functions = sourcery_db::list_functions_by_version_paginated(
        &state.pool,
        id,
        i64::from(query.limit),
        i64::from(query.offset),
    )
    .await
    .map_err(internal_error)?;
    Ok(Json(functions))
}

async fn list_version_callgraph(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<VersionFunction>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let mut functions = sourcery_db::list_all_functions(&state.pool, id)
        .await
        .map_err(internal_error)?;
    for function in &mut functions {
        normalize_callgraph_metrics(function);
    }
    Ok(Json(functions))
}

async fn list_version_treemap_files(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<FileState>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let files = sourcery_db::list_all_files_states(&state.pool, id)
        .await
        .map_err(internal_error)?;
    Ok(Json(files))
}

async fn list_version_treemap_functions(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<VersionFunction>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let functions = sourcery_db::list_all_functions(&state.pool, id)
        .await
        .map_err(internal_error)?;
    Ok(Json(functions))
}

fn normalize_callgraph_metrics(function: &mut VersionFunction) {
    let Some(metrics) = function.metrics.as_object_mut() else {
        return;
    };

    let mut normalized = Vec::new();
    let mut seen_keys = BTreeSet::new();
    let mut resolved_names = BTreeSet::new();
    let mut unresolved_names = BTreeSet::new();

    if let Some(calls) = metrics
        .get("function_calls")
        .and_then(serde_json::Value::as_array)
    {
        for call in calls {
            let name = call.get("name").and_then(serde_json::Value::as_str);
            let Some(name) = name else {
                continue;
            };
            let file = call
                .get("file")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let line = call
                .get("line")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default();
            let column = call
                .get("column")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default();
            let definition_found = call
                .get("definition_found")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);

            if !definition_found || file.is_empty() || line == 0 || column == 0 {
                unresolved_names.insert(name.to_string());
                continue;
            }

            let key = format!("{file}:{name}:{line}:{column}");
            if !seen_keys.insert(key.clone()) {
                continue;
            }
            resolved_names.insert(name.to_string());
            normalized.push(serde_json::json!({
                "key": key,
                "name": name,
                "file": file,
                "line": line,
                "column": column,
                "definition_found": true,
            }));
        }
    }

    if let Some(names) = metrics
        .get("functions_called")
        .and_then(serde_json::Value::as_array)
    {
        for name in names.iter().filter_map(serde_json::Value::as_str) {
            unresolved_names.insert(name.to_string());
        }
    }

    for name in unresolved_names {
        if resolved_names.contains(&name) || !seen_keys.insert(name.clone()) {
            continue;
        }
        normalized.push(serde_json::json!({
            "key": name,
            "name": name,
            "file": "",
            "line": 0,
            "column": 0,
            "definition_found": false,
        }));
    }

    metrics.insert("function_calls".to_string(), normalized.into());
}

async fn get_version_function(
    Path((id, function_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
) -> Result<Json<VersionFunction>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let function =
        sourcery_db::get_version_function_by_id_for_version(&state.pool, id, function_id)
            .await
            .map_err(internal_error)?;
    match function {
        Some(function) => Ok(Json(function)),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("function {function_id} not found for version {id}"),
        )),
    }
}

async fn search_version_filenames(
    Path(id): Path<Uuid>,
    Query(query): Query<SearchQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<FilenameSearchResult>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let files = sourcery_db::search_version_filenames(
        &state.pool,
        id,
        &query.q,
        bounded_limit(query.limit),
    )
    .await
    .map_err(internal_error)?;
    Ok(Json(files))
}

async fn search_version_functions(
    Path(id): Path<Uuid>,
    Query(query): Query<SearchQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<FunctionSearchResult>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let functions = sourcery_db::search_version_functions(
        &state.pool,
        id,
        &query.q,
        bounded_limit(query.limit),
    )
    .await
    .map_err(internal_error)?;
    Ok(Json(functions))
}

async fn get_version_diff_change(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<DiffWithChanges>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let diffchange = sourcery_db::get_diff_with_changes_by_version(&state.pool, id)
        .await
        .map_err(internal_error)?;
    Ok(Json(diffchange))
}
