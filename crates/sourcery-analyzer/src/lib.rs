use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use regex::Regex;
use serde_json::json;
use std::sync::OnceLock;
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{debug, warn};

use crate::{
    git_handler::SourceRepository,
    language::{LanguageConfig, ProgrammingLanguage},
    processor::Processor,
};

static FIX_REGEX: OnceLock<Regex> = OnceLock::new();
const DEFAULT_DATABASE_URL: &str = "postgres://localhost:5432/postgres";

pub mod diff;
pub mod git_handler;
pub mod language;
pub mod processor;
pub use sourcery_db as db;

pub async fn analyze_git_repository(url: &str) -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
    analyze_git_repository_with_database(url, &database_url).await
}

pub async fn analyze_git_repository_with_database(url: &str, database_url: &str) -> Result<()> {
    // Track open files so the filesystem is not overwhelmed.
    let semaphore = Arc::new(Semaphore::new(100));
    let mut join_set: JoinSet<Result<()>> = tokio::task::JoinSet::new();

    let sr = SourceRepository::new(url)?;
    let pool = db::connect(database_url).await?;
    let codebase_name = SourceRepository::get_repo_base_name(url);
    let codebase = match db::get_codebase_by_name(&pool, &codebase_name).await? {
        Some(codebase) => codebase,
        None => db::insert_codebase(&pool, &codebase_name, url).await?,
    };

    let mut commit_oids = Vec::new();
    for commit_oid in sr.into_iter() {
        let oid = match commit_oid {
            Ok(oid) => oid,
            Err(err) => {
                warn!(error = %err, "failed to read commit oid");
                continue;
            }
        };
        commit_oids.push(oid);
    }

    let mut previous_oid = None;
    for oid in commit_oids {
        sr.checkout_commit(&oid)?;
        let commit = sr
            .find_commit(&oid)
            .with_context(|| format!("failed to find commit {oid}"))?;
        let message = commit.message().unwrap_or("").to_string();
        let is_fix_commit = is_fix(&message);
        if is_fix_commit {
            debug!(?oid, "fix commit");
        } else {
            debug!(?oid, "non-fix commit");
        }

        let author = commit.author();
        let author_name = author.name().unwrap_or("").to_string();
        let author_email = author.email().unwrap_or("").to_string();
        let commit_hash = oid.to_string();

        let commit_diff = sr.commit_diff(previous_oid.as_ref(), &oid)?;
        let old_commit_hash = previous_oid.as_ref().map(ToString::to_string);
        let pretty_diff = commit_diff.pretty_print();

        let version_metrics = json!({});
        let version = db::insert_version(
            &pool,
            codebase.id,
            &commit_hash,
            &message,
            &author_name,
            &author_email,
            None,
            Some(is_fix_commit),
            Some(&pretty_diff),
            &version_metrics,
        )
        .await?;
        debug!(
            version_id = %version.id,
            commit = %commit_hash,
            old_commit_hash = ?old_commit_hash,
            "stored commit version"
        );

        let files_changed = i32::try_from(commit_diff.files_changed())
            .context("files_changed is larger than i32")?;
        let insertions =
            i32::try_from(commit_diff.insertions()).context("insertions is larger than i32")?;
        let deletions =
            i32::try_from(commit_diff.deletions()).context("deletions is larger than i32")?;
        let changed_lines = i32::try_from(commit_diff.number_of_changes())
            .context("changed_lines is larger than i32")?;
        let diff_metrics = json!({});
        let diff = db::insert_diff(
            &pool,
            version.id,
            old_commit_hash.as_deref(),
            &commit_hash,
            files_changed,
            insertions,
            deletions,
            changed_lines,
            Some(&pretty_diff),
            &diff_metrics,
        )
        .await?;

        for change in commit_diff.changes() {
            let old_span = change.old_line_span();
            let new_span = change.new_line_span();
            let old_path = change.old_file().map(|path| path.display().to_string());
            let new_path = change.new_file().map(|path| path.display().to_string());
            let old_start_line =
                i32::try_from(old_span.0).context("old_start_line is larger than i32")?;
            let old_end_line =
                i32::try_from(old_span.1).context("old_end_line is larger than i32")?;
            let new_start_line =
                i32::try_from(new_span.0).context("new_start_line is larger than i32")?;
            let new_end_line =
                i32::try_from(new_span.1).context("new_end_line is larger than i32")?;
            let change_metrics = json!({
                "old_span_length": old_span.1.saturating_sub(old_span.0),
                "new_span_length": new_span.1.saturating_sub(new_span.0),
            });
            db::insert_change(
                &pool,
                diff.id,
                old_path.as_deref(),
                new_path.as_deref(),
                old_start_line,
                old_end_line,
                new_start_line,
                new_end_line,
                &change_metrics,
            )
            .await?;
        }

        let paths_to_analyze: Vec<(PathBuf, PathBuf)> = commit_diff
            .files()
            .iter()
            .map(|path| (path.clone(), sr.dest_dir.join(path)))
            .collect();

        for (relative_path, absolute_path) in paths_to_analyze {
            if !absolute_path.is_file() {
                debug!(?absolute_path, "ignored file because is not a file");
                continue;
            }
            let Some(language) = ProgrammingLanguage::detect_language(&absolute_path, None) else {
                debug!(
                    ?absolute_path,
                    "ignored file because could not detect language"
                );
                continue;
            };
            let lc = LanguageConfig::new(language);
            if sr.is_ignored_file(&absolute_path, &lc.extensions)? {
                debug!(
                    ?absolute_path,
                    ?language,
                    "ignored file because extension does not match"
                );
                continue;
            }

            if matches!(language, ProgrammingLanguage::Haskell) {
                debug!(
                    ?absolute_path,
                    ?language,
                    "skipping file because no language configuration is available"
                );
                continue;
            }

            let permit = semaphore.clone().acquire_owned().await?;
            let pool = pool.clone();
            let version_id = version.id;
            join_set.spawn(async move {
                let _permit = permit;
                let processor = Processor::new(&lc, &absolute_path)?;
                let analysis = processor.analyze()?;
                let source = processor.source();
                let metrics = &analysis.ast_analysis;
                let file_path = relative_path.display().to_string();
                let language_name = format!("{language:?}").to_ascii_lowercase();

                let file_metrics = json!({
                    "lines_of_code": analysis.lines_of_code,
                    "effective_lines_of_code": analysis.effective_lines_of_code,
                    "comment_lines_of_code": analysis.comment_lines_of_code,
                    "total_cyclomatic": analysis.total_cyclomatic,
                });
                let file = db::insert_file(
                    &pool,
                    version_id,
                    &file_path,
                    Some(&language_name),
                    &file_metrics,
                )
                .await?;

                for func in &metrics.functions {
                    // Function names may repeat by namespace, so include location.
                    let unique_name = func.name.with_location(source)?;
                    let start_line = i32::try_from(func.definition_line_span.start_line)
                        .context("function start_line is larger than i32")?;
                    let end_line = i32::try_from(func.definition_line_span.end_line)
                        .context("function end_line is larger than i32")?;
                    let function_metrics = json!({
                        "function_length": func.function_length,
                        "cyclomatic": func.cyclomatic,
                        "cyclomatic_match_as_single_branch": func.cyclomatic_match_as_single_branch,
                    });
                    db::insert_function(
                        &pool,
                        file.id,
                        &unique_name,
                        start_line,
                        end_line,
                        &function_metrics,
                    )
                    .await?;
                }
                Ok(())
            });
        }
        while let Some(task_result) = join_set.join_next().await {
            task_result??;
        }
        previous_oid = Some(oid);
    }
    Ok(())
}

fn is_fix(message: &str) -> bool {
    let re = FIX_REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)\b(fix(e[sd])?|bugfix(es)?|hotfix(es)?|patch(ed|es)?|resolve[sd]?|correct(ed)?|repair(ed|s)?)\b",
        )
        .unwrap()
    });

    re.is_match(message)
}
