use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
pub use sqlx::PgPool;
use uuid::Uuid;

// Models

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
    pub is_fix: Option<bool>,
    pub diff: Option<String>,
    pub metrics: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Diff {
    pub id: Uuid,
    pub version_id: Uuid,
    pub new_commit_hash: String,
    pub old_commit_hash: Option<String>,
    pub files_changed: i32,
    pub insertions: i32,
    pub deletions: i32,
    pub changed_lines: i32,
    pub summary: Option<String>,
    pub metrics: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct File {
    pub id: Uuid,
    pub version_id: Uuid,
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Change {
    pub id: Uuid,
    pub diff_id: Uuid,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub old_start_line: i32,
    pub old_end_line: i32,
    pub new_start_line: i32,
    pub new_end_line: i32,
    pub metrics: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// Connection

pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;
    Ok(pool)
}

// Codebases

pub async fn insert_codebase(pool: &PgPool, name: &str, url: &str) -> Result<Codebase> {
    let row = sqlx::query_as::<_, Codebase>(
        "INSERT INTO codebases (name, url) VALUES ($1, $2)
         ON CONFLICT (name) DO UPDATE SET
             url = EXCLUDED.url
         RETURNING *",
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

// Versions

#[allow(clippy::too_many_arguments)]
pub async fn insert_version(
    pool: &PgPool,
    codebase_id: Uuid,
    commit_hash: &str,
    message: &str,
    author_name: &str,
    author_email: &str,
    committed_at: Option<DateTime<Utc>>,
    is_fix: Option<bool>,
    diff: Option<&str>,
    metrics: &serde_json::Value,
) -> Result<Version> {
    let row = sqlx::query_as::<_, Version>(
        "INSERT INTO versions (codebase_id, commit_hash, message, author_name, author_email, committed_at, is_fix, diff, metrics)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (codebase_id, commit_hash) DO UPDATE SET
             message = EXCLUDED.message,
             author_name = EXCLUDED.author_name,
             author_email = EXCLUDED.author_email,
             committed_at = EXCLUDED.committed_at,
             is_fix = EXCLUDED.is_fix,
             diff = EXCLUDED.diff,
             metrics = EXCLUDED.metrics
         RETURNING *",
    )
    .bind(codebase_id)
    .bind(commit_hash)
    .bind(message)
    .bind(author_name)
    .bind(author_email)
    .bind(committed_at)
    .bind(is_fix)
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

// Diffs

#[allow(clippy::too_many_arguments)]
pub async fn insert_diff(
    pool: &PgPool,
    version_id: Uuid,
    old_commit_hash: Option<&str>,
    new_commit_hash: &str,
    files_changed: i32,
    insertions: i32,
    deletions: i32,
    changed_lines: i32,
    summary: Option<&str>,
    metrics: &serde_json::Value,
) -> Result<Diff> {
    let row = sqlx::query_as::<_, Diff>(
        "INSERT INTO diffs (version_id, old_commit_hash, new_commit_hash, files_changed, insertions, deletions, changed_lines, summary, metrics)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (version_id) DO UPDATE SET
             old_commit_hash = EXCLUDED.old_commit_hash,
             new_commit_hash = EXCLUDED.new_commit_hash,
             files_changed = EXCLUDED.files_changed,
             insertions = EXCLUDED.insertions,
             deletions = EXCLUDED.deletions,
             changed_lines = EXCLUDED.changed_lines,
             summary = EXCLUDED.summary,
             metrics = EXCLUDED.metrics
         RETURNING *",
    )
    .bind(version_id)
    .bind(old_commit_hash)
    .bind(new_commit_hash)
    .bind(files_changed)
    .bind(insertions)
    .bind(deletions)
    .bind(changed_lines)
    .bind(summary)
    .bind(metrics)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_diff_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Diff>> {
    let row = sqlx::query_as::<_, Diff>("SELECT * FROM diffs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_diff_by_version(pool: &PgPool, version_id: Uuid) -> Result<Option<Diff>> {
    let row = sqlx::query_as::<_, Diff>("SELECT * FROM diffs WHERE version_id = $1")
        .bind(version_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_diffs_by_codebase(pool: &PgPool, codebase_id: Uuid) -> Result<Vec<Diff>> {
    let rows = sqlx::query_as::<_, Diff>(
        "SELECT d.*
         FROM diffs d
         JOIN versions v ON v.id = d.version_id
         WHERE v.codebase_id = $1
         ORDER BY d.created_at",
    )
    .bind(codebase_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete_diff(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM diffs WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// Files

pub async fn insert_file(
    pool: &PgPool,
    version_id: Uuid,
    path: &str,
    language: Option<&str>,
    metrics: &serde_json::Value,
) -> Result<File> {
    let row = sqlx::query_as::<_, File>(
        "INSERT INTO files (version_id, path, language, metrics)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (version_id, path) DO UPDATE SET
             language = EXCLUDED.language,
             metrics = EXCLUDED.metrics
         RETURNING *",
    )
    .bind(version_id)
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
    let rows = sqlx::query_as::<_, File>("SELECT * FROM files WHERE version_id = $1 ORDER BY path")
        .bind(version_id)
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

// Functions

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

// Changes

#[allow(clippy::too_many_arguments)]
pub async fn insert_change(
    pool: &PgPool,
    diff_id: Uuid,
    old_path: Option<&str>,
    new_path: Option<&str>,
    old_start_line: i32,
    old_end_line: i32,
    new_start_line: i32,
    new_end_line: i32,
    metrics: &serde_json::Value,
) -> Result<Change> {
    let row = sqlx::query_as::<_, Change>(
        "INSERT INTO changes (diff_id, old_path, new_path, old_start_line, old_end_line, new_start_line, new_end_line, metrics)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING *",
    )
    .bind(diff_id)
    .bind(old_path)
    .bind(new_path)
    .bind(old_start_line)
    .bind(old_end_line)
    .bind(new_start_line)
    .bind(new_end_line)
    .bind(metrics)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_change_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Change>> {
    let row = sqlx::query_as::<_, Change>("SELECT * FROM changes WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_changes_by_diff(pool: &PgPool, diff_id: Uuid) -> Result<Vec<Change>> {
    let rows = sqlx::query_as::<_, Change>(
        "SELECT * FROM changes
         WHERE diff_id = $1
         ORDER BY created_at, old_start_line, new_start_line",
    )
    .bind(diff_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete_change(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM changes WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
