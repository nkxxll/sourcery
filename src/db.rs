use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ── Models ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Codebase {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Version {
    pub id: Uuid,
    pub codebase_id: Uuid,
    pub commit_hash: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub committed_at: Option<DateTime<Utc>>,
    pub diff: Option<String>,
    pub metrics: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Directory {
    pub id: Uuid,
    pub version_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub path: String,
    pub metrics: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct File {
    pub id: Uuid,
    pub version_id: Uuid,
    pub directory_id: Uuid,
    pub path: String,
    pub language: Option<String>,
    pub metrics: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Function {
    pub id: Uuid,
    pub file_id: Uuid,
    pub name: String,
    pub start_line: i32,
    pub end_line: i32,
    pub metrics: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// ── Connection ──────────────────────────────────────────────────────

pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

// ── Codebases ───────────────────────────────────────────────────────

pub async fn insert_codebase(pool: &PgPool, name: &str, url: &str) -> Result<Codebase> {
    let row = sqlx::query_as::<_, Codebase>(
        "INSERT INTO codebases (name, url) VALUES ($1, $2) RETURNING *",
    )
    .bind(name)
    .bind(url)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_codebase_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Codebase>> {
    let row = sqlx::query_as::<_, Codebase>("SELECT * FROM codebases WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_codebase_by_name(pool: &PgPool, name: &str) -> Result<Option<Codebase>> {
    let row = sqlx::query_as::<_, Codebase>("SELECT * FROM codebases WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_codebases(pool: &PgPool) -> Result<Vec<Codebase>> {
    let rows = sqlx::query_as::<_, Codebase>("SELECT * FROM codebases ORDER BY created_at")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn delete_codebase(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM codebases WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Versions ────────────────────────────────────────────────────────

pub async fn insert_version(
    pool: &PgPool,
    codebase_id: Uuid,
    commit_hash: &str,
    message: &str,
    author_name: &str,
    author_email: &str,
    committed_at: Option<DateTime<Utc>>,
    diff: Option<&str>,
    metrics: &serde_json::Value,
) -> Result<Version> {
    let row = sqlx::query_as::<_, Version>(
        "INSERT INTO versions (codebase_id, commit_hash, message, author_name, author_email, committed_at, diff, metrics)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING *",
    )
    .bind(codebase_id)
    .bind(commit_hash)
    .bind(message)
    .bind(author_name)
    .bind(author_email)
    .bind(committed_at)
    .bind(diff)
    .bind(metrics)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_version_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Version>> {
    let row = sqlx::query_as::<_, Version>("SELECT * FROM versions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_version_by_commit(
    pool: &PgPool,
    codebase_id: Uuid,
    commit_hash: &str,
) -> Result<Option<Version>> {
    let row = sqlx::query_as::<_, Version>(
        "SELECT * FROM versions WHERE codebase_id = $1 AND commit_hash = $2",
    )
    .bind(codebase_id)
    .bind(commit_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_versions_by_codebase(pool: &PgPool, codebase_id: Uuid) -> Result<Vec<Version>> {
    let rows = sqlx::query_as::<_, Version>(
        "SELECT * FROM versions WHERE codebase_id = $1 ORDER BY created_at",
    )
    .bind(codebase_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete_version(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM versions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Directories ─────────────────────────────────────────────────────

pub async fn insert_directory(
    pool: &PgPool,
    version_id: Uuid,
    parent_id: Option<Uuid>,
    path: &str,
    metrics: &serde_json::Value,
) -> Result<Directory> {
    let row = sqlx::query_as::<_, Directory>(
        "INSERT INTO directories (version_id, parent_id, path, metrics)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(version_id)
    .bind(parent_id)
    .bind(path)
    .bind(metrics)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_directory_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Directory>> {
    let row = sqlx::query_as::<_, Directory>("SELECT * FROM directories WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_directories_by_version(
    pool: &PgPool,
    version_id: Uuid,
) -> Result<Vec<Directory>> {
    let rows = sqlx::query_as::<_, Directory>(
        "SELECT * FROM directories WHERE version_id = $1 ORDER BY path",
    )
    .bind(version_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_subdirectories(pool: &PgPool, parent_id: Uuid) -> Result<Vec<Directory>> {
    let rows = sqlx::query_as::<_, Directory>(
        "SELECT * FROM directories WHERE parent_id = $1 ORDER BY path",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete_directory(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM directories WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Files ───────────────────────────────────────────────────────────

pub async fn insert_file(
    pool: &PgPool,
    version_id: Uuid,
    directory_id: Uuid,
    path: &str,
    language: Option<&str>,
    metrics: &serde_json::Value,
) -> Result<File> {
    let row = sqlx::query_as::<_, File>(
        "INSERT INTO files (version_id, directory_id, path, language, metrics)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(version_id)
    .bind(directory_id)
    .bind(path)
    .bind(language)
    .bind(metrics)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_file_by_id(pool: &PgPool, id: Uuid) -> Result<Option<File>> {
    let row = sqlx::query_as::<_, File>("SELECT * FROM files WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_files_by_version(pool: &PgPool, version_id: Uuid) -> Result<Vec<File>> {
    let rows =
        sqlx::query_as::<_, File>("SELECT * FROM files WHERE version_id = $1 ORDER BY path")
            .bind(version_id)
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

pub async fn list_files_by_directory(pool: &PgPool, directory_id: Uuid) -> Result<Vec<File>> {
    let rows =
        sqlx::query_as::<_, File>("SELECT * FROM files WHERE directory_id = $1 ORDER BY path")
            .bind(directory_id)
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

pub async fn delete_file(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM files WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Functions ───────────────────────────────────────────────────────

pub async fn insert_function(
    pool: &PgPool,
    file_id: Uuid,
    name: &str,
    start_line: i32,
    end_line: i32,
    metrics: &serde_json::Value,
) -> Result<Function> {
    let row = sqlx::query_as::<_, Function>(
        "INSERT INTO functions (file_id, name, start_line, end_line, metrics)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(file_id)
    .bind(name)
    .bind(start_line)
    .bind(end_line)
    .bind(metrics)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_function_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Function>> {
    let row = sqlx::query_as::<_, Function>("SELECT * FROM functions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_functions_by_file(pool: &PgPool, file_id: Uuid) -> Result<Vec<Function>> {
    let rows = sqlx::query_as::<_, Function>(
        "SELECT * FROM functions WHERE file_id = $1 ORDER BY start_line",
    )
    .bind(file_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete_function(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM functions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
