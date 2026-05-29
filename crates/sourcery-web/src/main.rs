use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use clap::Parser;
use sourcery_db::{Codebase, Diff, DiffWithChanges, File, PgPool, Version, VersionFunction};
use std::collections::BTreeMap;
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
        .route("/version/{id}/diff", get(get_version_diff))
        .route("/version/{id}/files", get(list_version_files))
        .route("/version/{id}/functions", get(list_version_functions))
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

fn default_limit() -> u32 {
    50
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

async fn get_version_diff(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<DiffWithChanges>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let diff = sourcery_db::get_diff_with_changes_by_version(&state.pool, id)
        .await
        .map_err(internal_error)?;
    match diff {
        Some(diff) => Ok(Json(diff)),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("diff for version {id} not found"),
        )),
    }
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

async fn list_version_functions(
    Path(id): Path<Uuid>,
    Query(query): Query<PageQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<VersionFunction>>, (StatusCode, String)> {
    get_version_or_not_found(&state.pool, id).await?;
    let functions = sourcery_db::list_functions_by_version(
        &state.pool,
        id,
        i64::from(query.limit),
        i64::from(query.offset),
    )
    .await
    .map_err(internal_error)?;
    Ok(Json(functions))
}
