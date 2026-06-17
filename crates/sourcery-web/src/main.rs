use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use clap::Parser;
use sourcery_db::{
    AnalysisMetricSample, AnalysisVersionSample, Codebase, Diff, DiffWithChanges, File, FileState,
    FileStateWithFunctionCount, FilenameSearchResult, FunctionSearchResult,
    GithubIssueTimelineEvent, PgPool, Version, VersionFunction,
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

#[derive(serde::Serialize)]
struct VersionDashboardResponse {
    #[serde(flatten)]
    version: Version,
    codebase_name: String,
    total_files: i64,
    total_functions: i64,
}

#[derive(serde::Serialize)]
struct FileDetailResponse {
    #[serde(flatten)]
    file: FileStateWithFunctionCount,
    codebase_url: String,
    commit_hash: String,
}

#[derive(serde::Serialize)]
struct FunctionDetailResponse {
    #[serde(flatten)]
    function: VersionFunction,
    codebase_url: String,
    commit_hash: String,
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
        .route("/analysis/versions", get(list_analysis_versions))
        .route("/analysis/metrics", get(list_analysis_metrics))
        .route("/codebase/{id}", get(get_codebase))
        .route("/codebase/{id}/diff", get(list_diffs_by_codebase))
        .route("/codebase/{id}/metrics", get(list_codebase_metrics))
        .route("/github/issues/timeline", get(list_github_issue_timeline))
        .route("/version/{id}", get(get_version))
        .route("/version/{id}/sample", get(get_sample_version))
        .route("/file/{file_id}", get(get_file))
        .route("/function/{function_id}", get(get_function))
        .route("/version/{id}/changed_files", get(list_version_files))
        .route("/version/{id}/files", get(list_all_version_files))
        .route("/version/{id}/sample/files", get(list_sample_version_files))
        .route("/version/{id}/files/{file_state_id}", get(get_version_file))
        .route("/version/{id}/files/search", get(search_version_filenames))
        .route("/version/{id}/diff", get(get_version_diff))
        .route("/version/{id}/diffchange", get(get_version_diff_change))
        .route("/version/{id}/callgraph", get(list_version_callgraph))
        .route(
            "/version/{id}/sample/callgraph",
            get(list_sample_version_callgraph),
        )
        .route(
            "/version/{id}/treemap/files",
            get(list_version_treemap_files),
        )
        .route(
            "/version/{id}/sample/treemap/files",
            get(list_sample_version_treemap_files),
        )
        .route(
            "/version/{id}/treemap/functions",
            get(list_version_treemap_functions),
        )
        .route(
            "/version/{id}/sample/treemap/functions",
            get(list_sample_version_treemap_functions),
        )
        .route("/version/{id}/functions", get(list_version_functions))
        .route(
            "/version/{id}/sample/functions",
            get(list_sample_version_functions),
        )
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

#[derive(serde::Deserialize)]
struct GithubIssueTimelineQuery {
    repo: String,
    issue: Option<i32>,
    event: Option<String>,
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default)]
    offset: u32,
}

#[derive(serde::Deserialize)]
struct AnalysisVersionsQuery {
    codebase_ids: String,
}

#[derive(serde::Deserialize)]
struct AnalysisMetricsQuery {
    codebase_ids: String,
    version: i64,
    metric: String,
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

async fn get_codebase_or_not_found(
    pool: &PgPool,
    id: Uuid,
) -> Result<Codebase, (StatusCode, String)> {
    let codebase = sourcery_db::get_codebase_by_id(pool, id)
        .await
        .map_err(internal_error)?;
    codebase.ok_or_else(|| (StatusCode::NOT_FOUND, format!("codebase {id} not found")))
}

async fn list_github_issue_timeline(
    Query(query): Query<GithubIssueTimelineQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<GithubIssueTimelineEvent>>, (StatusCode, String)> {
    let rows = sourcery_db::list_github_issue_timeline_events(
        &state.pool,
        &query.repo,
        query.issue,
        query.event.as_deref(),
        i64::from(query.limit),
        i64::from(query.offset),
    )
    .await
    .map_err(internal_error)?;
    Ok(Json(rows))
}

async fn list_analysis_versions(
    Query(query): Query<AnalysisVersionsQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<AnalysisVersionSample>>, (StatusCode, String)> {
    let codebase_ids = parse_codebase_ids(&query.codebase_ids)?;
    let rows = sourcery_db::list_analysis_version_samples(&state.pool, &codebase_ids)
        .await
        .map_err(internal_error)?;
    Ok(Json(rows))
}

async fn list_analysis_metrics(
    Query(query): Query<AnalysisMetricsQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<AnalysisMetricSample>>, (StatusCode, String)> {
    let codebase_ids = parse_codebase_ids(&query.codebase_ids)?;
    if !(1..=10).contains(&query.version) {
        return Err((
            StatusCode::BAD_REQUEST,
            "version must be between 1 and 10".to_string(),
        ));
    }
    if !is_analysis_metric(&query.metric) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("unsupported analysis metric {}", query.metric),
        ));
    }

    let rows = sourcery_db::list_analysis_metric_samples(
        &state.pool,
        &codebase_ids,
        query.version,
        &query.metric,
    )
    .await
    .map_err(internal_error)?;
    Ok(Json(rows))
}

fn parse_codebase_ids(value: &str) -> Result<Vec<Uuid>, (StatusCode, String)> {
    let ids = value
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| {
            id.parse::<Uuid>()
                .map_err(|_| (StatusCode::BAD_REQUEST, format!("invalid codebase id {id}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "at least one codebase id is required".to_string(),
        ));
    }
    Ok(ids)
}

fn is_analysis_metric(metric: &str) -> bool {
    matches!(
        metric,
        "lines_of_code"
            | "effective_lines_of_code_with_brackets"
            | "effective_lines_of_code"
            | "comment_lines_of_code"
            | "bracket_lines_of_code"
            | "total_cyclomatic"
            | "maintainability_index_three_property"
            | "maintainability_index_four_property"
            | "maintainability_index_visual_studio"
            | "maintainability_index_comment_percentage"
            | "total_halstead_unique_operators"
            | "total_halstead_unique_operands"
            | "total_halstead_operators"
            | "total_halstead_operands"
            | "total_halstead_length"
            | "total_halstead_vocabulary"
            | "total_halstead_calculated_length"
            | "total_halstead_volume"
            | "total_halstead_difficulty"
            | "total_halstead_effort"
            | "total_halstead_time_seconds"
            | "total_halstead_bugs"
            | "mean_outdegree_per_file"
            | "mean_indegree_per_file"
            | "mean_cyclomatic_per_function_per_file"
            | "function_length"
            | "cyclomatic"
            | "cyclomatic_match_as_single_branch"
            | "indegree"
            | "outdegree"
            | "halstead_unique_operators"
            | "halstead_unique_operands"
            | "halstead_operators"
            | "halstead_operands"
            | "halstead_length"
            | "halstead_vocabulary"
            | "halstead_calculated_length"
            | "halstead_volume"
            | "halstead_difficulty"
            | "halstead_effort"
            | "halstead_time_seconds"
            | "halstead_bugs"
    )
}

async fn get_version(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<VersionDashboardResponse>, (StatusCode, String)> {
    let version = get_version_or_not_found(&state.pool, id).await?;
    let codebase = get_codebase_or_not_found(&state.pool, version.codebase_id).await?;
    let counts = sourcery_db::count_version_files_and_functions(&state.pool, id)
        .await
        .map_err(internal_error)?;
    let total_files = metric_i64(&version.metrics, "files").unwrap_or(counts.total_files);
    Ok(Json(VersionDashboardResponse {
        version,
        codebase_name: codebase.name,
        total_files,
        total_functions: counts.total_functions,
    }))
}

async fn get_sample_version(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<VersionDashboardResponse>, (StatusCode, String)> {
    let version = get_version_or_not_found(&state.pool, id).await?;
    let codebase = get_codebase_or_not_found(&state.pool, version.codebase_id).await?;
    let counts = sourcery_db::count_snapshot_version_files_and_functions(&state.pool, id)
        .await
        .map_err(internal_error)?;
    Ok(Json(VersionDashboardResponse {
        version,
        codebase_name: codebase.name,
        total_files: counts.total_files,
        total_functions: counts.total_functions,
    }))
}

fn metric_i64(metrics: &serde_json::Value, key: &str) -> Option<i64> {
    let value = metrics.get(key)?;
    if let Some(value) = value.as_i64() {
        return Some(value);
    }
    if let Some(value) = value.as_u64() {
        return i64::try_from(value).ok();
    }
    value.as_str()?.parse().ok()
}

async fn get_file(
    Path(file_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<FileDetailResponse>, (StatusCode, String)> {
    let file = sourcery_db::get_file_state_by_id(&state.pool, file_id)
        .await
        .map_err(internal_error)?;
    match file {
        Some(file) => {
            let version = get_version_or_not_found(&state.pool, file.version_id).await?;
            let codebase = get_codebase_or_not_found(&state.pool, file.codebase_id).await?;
            Ok(Json(FileDetailResponse {
                file: file_state_with_function_count(&state.pool, file).await?,
                codebase_url: codebase.url,
                commit_hash: version.commit_hash,
            }))
        }
        None => {
            let file = sourcery_db::get_file_by_id(&state.pool, file_id)
                .await
                .map_err(internal_error)?;
            match file {
                Some(file) => {
                    let version = get_version_or_not_found(&state.pool, file.version_id).await?;
                    let codebase =
                        get_codebase_or_not_found(&state.pool, version.codebase_id).await?;
                    Ok(Json(FileDetailResponse {
                        file: file_state_with_function_count(
                            &state.pool,
                            snapshot_file_state(file, version.codebase_id),
                        )
                        .await?,
                        codebase_url: codebase.url,
                        commit_hash: version.commit_hash,
                    }))
                }
                None => Err((StatusCode::NOT_FOUND, format!("file {file_id} not found"))),
            }
        }
    }
}

fn snapshot_file_state(file: File, codebase_id: Uuid) -> FileState {
    FileState {
        id: file.id,
        codebase_id,
        version_id: file.version_id,
        path: file.path,
        file_id: Some(file.id),
        status: "analyzed".to_string(),
        exists: true,
        source_path: None,
        metrics: file.metrics,
        created_at: file.created_at,
    }
}

async fn get_function(
    Path(function_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<FunctionDetailResponse>, (StatusCode, String)> {
    let function = sourcery_db::get_version_function_by_id(&state.pool, function_id)
        .await
        .map_err(internal_error)?;
    match function {
        Some(function) => {
            let version = get_version_or_not_found(&state.pool, function.version_id).await?;
            let codebase = get_codebase_or_not_found(&state.pool, version.codebase_id).await?;
            Ok(Json(FunctionDetailResponse {
                function,
                codebase_url: codebase.url,
                commit_hash: version.commit_hash,
            }))
        }
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

async fn list_sample_version_files(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<FileState>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let files = sourcery_db::list_snapshot_file_states(&state.pool, id)
        .await
        .map_err(internal_error)?;
    Ok(Json(files))
}

async fn get_version_file(
    Path((id, file_state_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
) -> Result<Json<FileStateWithFunctionCount>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let file = sourcery_db::get_file_state_by_id_for_version(&state.pool, id, file_state_id)
        .await
        .map_err(internal_error)?;
    match file {
        Some(file) => Ok(Json(
            file_state_with_function_count(&state.pool, file).await?,
        )),
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

async fn list_sample_version_functions(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<VersionFunction>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let functions = sourcery_db::list_snapshot_functions(&state.pool, id)
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

async fn list_sample_version_callgraph(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<VersionFunction>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let mut functions = sourcery_db::list_snapshot_functions(&state.pool, id)
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
) -> Result<Json<Vec<FileStateWithFunctionCount>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let mut files = sourcery_db::list_all_file_states_with_function_counts(&state.pool, id)
        .await
        .map_err(internal_error)?;
    for file in &mut files {
        add_file_function_count_metric(file);
    }
    Ok(Json(files))
}

async fn list_sample_version_treemap_files(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<FileStateWithFunctionCount>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let mut files = sourcery_db::list_snapshot_file_states_with_function_counts(&state.pool, id)
        .await
        .map_err(internal_error)?;
    for file in &mut files {
        add_file_function_count_metric(file);
    }
    Ok(Json(files))
}

fn add_file_function_count_metric(file: &mut FileStateWithFunctionCount) {
    if !file.metrics.is_object() {
        file.metrics = serde_json::json!({});
    }
    let Some(metrics) = file.metrics.as_object_mut() else {
        return;
    };
    metrics.insert("functions".to_string(), file.total_functions.into());
}

async fn file_state_with_function_count(
    pool: &PgPool,
    file: FileState,
) -> Result<FileStateWithFunctionCount, (StatusCode, String)> {
    let total_functions = match file.file_id {
        Some(file_id) => sourcery_db::count_functions_by_file(pool, file_id)
            .await
            .map_err(internal_error)?,
        None => 0,
    };
    let mut file = FileStateWithFunctionCount {
        id: file.id,
        codebase_id: file.codebase_id,
        version_id: file.version_id,
        path: file.path,
        file_id: file.file_id,
        status: file.status,
        exists: file.exists,
        source_path: file.source_path,
        metrics: file.metrics,
        created_at: file.created_at,
        total_functions,
    };
    add_file_function_count_metric(&mut file);
    Ok(file)
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

async fn list_sample_version_treemap_functions(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<VersionFunction>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let functions = sourcery_db::list_snapshot_functions(&state.pool, id)
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
