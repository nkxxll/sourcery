use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use clap::Parser;
use sourcery_db::{Codebase, Diff, PgPool};
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
        .route("/codebases/{id}", get(get_codebase))
        .route("/codebases/{id}/diff", get(list_diffs_by_codebase))
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

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    tracing::error!(error = %error, "database request failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error".to_string(),
    )
}
