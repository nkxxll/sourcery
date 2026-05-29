use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use ecow::EcoString;
use git2::Oid;
use regex::Regex;
use serde_json::json;
use sourcery_db::{Codebase, PgPool};
use sourcery_lsp_client::Server;
use std::sync::OnceLock;
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{debug, info, warn};

use crate::{
    diff::CommitDiff,
    git_handler::{CommitInfo, SourceRepository},
    language::{LanguageConfig, ProgrammingLanguage},
    processor::{AggregatedFileMetrics, Analysis, FileMetrics, NewLineMap, Processor},
    progress::Progress,
};

static FIX_REGEX: OnceLock<Regex> = OnceLock::new();
const DEFAULT_DATABASE_URL: &str = "postgres://localhost:5432/postgres";

pub mod diff;
pub mod git_handler;
pub mod halstead_subprocess;
pub mod language;
pub mod processor;
pub mod progress;
pub use sourcery_db as db;

pub async fn analyze_git_repository(
    url: &str,
    programming_language: Option<ProgrammingLanguage>,
) -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
    analyze_git_repository_with_database(url, programming_language, &database_url).await
}

fn guess_repo_language(url: &str) -> Result<ProgrammingLanguage> {
    if url.to_lowercase().contains("go") {
        Ok(ProgrammingLanguage::Golang)
    } else if url.to_lowercase().contains("ocaml") {
        Ok(ProgrammingLanguage::Ocaml)
    } else if url.to_lowercase().contains("haskell") {
        Ok(ProgrammingLanguage::Haskell)
    } else if url.to_lowercase().contains("python") {
        Ok(ProgrammingLanguage::Python)
    } else {
        Err(anyhow!(
            "the programming language could not be detected by the url you have to provide it manually"
        ))
    }
}

struct State {
    pub sr: SourceRepository,
    pub codebase: Codebase,
    pub progress: Progress,
    pub commits: Vec<Oid>,
    pub current_aggregate: AggregatedFileMetrics,
    pub current_file_metrics_by_path: HashMap<EcoString, FileMetrics>,
}

impl State {
    async fn new(
        url: &str,
        pool: &PgPool,
        programming_language: Option<ProgrammingLanguage>,
    ) -> Result<Self> {
        let sr = SourceRepository::new(url)?;
        let codebase_name = SourceRepository::get_repo_base_name(url);
        let codebase = if let Some(pl) = programming_language {
            let programming_language_str = pl.to_string();
            db::insert_codebase(&pool, &codebase_name, url, &programming_language_str).await?
        } else {
            let language = guess_repo_language(url)?.to_string();
            db::insert_codebase(&pool, &codebase_name, url, &language).await?
        };
        let commits = Self::gather_commits(&sr);
        let number_of_commits = commits.len();
        info!("Found {number_of_commits} commits.");
        let progress = Progress::new(number_of_commits as u64, None);
        progress.start_print();
        let current_aggregate = AggregatedFileMetrics::default();
        let current_file_metrics_by_path = HashMap::new();
        Ok(Self {
            sr,
            codebase,
            progress,
            commits,
            current_aggregate,
            current_file_metrics_by_path,
        })
    }

    fn commit_info(&self, oid: &Oid) -> CommitInfo {
        match self
            .sr
            .find_commit(&oid)
            .with_context(|| format!("failed to find commit {oid}"))
        {
            Ok(commit) => {
                let message = commit.message().unwrap_or("").to_string();
                let author_name = commit.author().name().unwrap_or_default().to_string();
                let author_email = commit.author().email().unwrap_or_default().to_string();
                let committed_at = DateTime::<Utc>::from_timestamp(commit.time().seconds(), 0);
                let is_fix = is_fix(&message);
                let commit_hash = oid.to_string();
                CommitInfo {
                    author_name,
                    author_email,
                    message: message,
                    committed_at,
                    is_fix,
                    hash: commit_hash,
                }
            }
            Err(err) => {
                warn!("could not find commit {err}");
                CommitInfo::default()
            }
        }
    }

    fn gather_commits(sr: &SourceRepository) -> Vec<Oid> {
        let mut res = Vec::new();
        for commit_oid in sr.into_iter() {
            let oid = match commit_oid {
                Ok(oid) => oid,
                Err(err) => {
                    warn!(error = %err, "failed to read commit oid");
                    continue;
                }
            };
            res.push(oid);
        }
        res
    }
}

pub async fn analyze_git_repository_with_database(
    url: &str,
    programming_language: Option<ProgrammingLanguage>,
    database_url: &str,
) -> Result<()> {
    let semaphore = Arc::new(Semaphore::new(100));
    let pool = db::connect(database_url).await?;

    let mut state = State::new(url, &pool, programming_language).await?;
    let pl = programming_language.expect("should be determined by now");
    let (binary, args) = pl.lsp();
    let mut server = Server::new(&state.sr.dest_dir, binary, args);
    let mainloop = server.run_main_loop();
    server.initialize().await;
    info!(
        repository = url,
        lsp_binary = binary,
        commits = state.commits.len(),
        "starting repository analysis"
    );

    let mut previous_oid = None;
    let commits = state.commits.clone();
    for oid in commits {
        state.progress.next();
        state.sr.checkout_commit(&oid)?;
        let commit_info = state.commit_info(&oid);

        let commit_diff = state.sr.commit_diff(previous_oid.as_ref(), &oid)?;
        let stored_commit =
            store_commit_snapshot(&pool, &state, &commit_info, &commit_diff).await?;
        store_diff_file_changes(&pool, &stored_commit.diff, &commit_diff).await?;

        let old_metrics_by_path =
            gather_old_metrics(commit_diff.files(), &state.current_file_metrics_by_path);
        let old_metrics = AggregatedFileMetrics::from_file_metrics_map(&old_metrics_by_path);
        debug!(
            previous_files = state.current_aggregate.files,
            files = old_metrics.files,
            total_lines_of_code = old_metrics.total_lines_of_code,
            total_effective_lines_of_code = old_metrics.total_effective_lines_of_code,
            total_comment_lines_of_code = old_metrics.total_comment_lines_of_code,
            total_cyclomatic = old_metrics.total_cyclomatic,
            "aggregated old metrics for changed files"
        );

        store_diff_line_changes(&pool, &stored_commit.diff, &commit_diff).await?;

        let new_metrics_by_path = analyze_and_store_changed_files(
            &pool,
            &server,
            semaphore.clone(),
            &state,
            &commit_info,
            &stored_commit.version,
            &commit_diff,
        )
        .await?;

        update_version_metrics(
            &pool,
            &mut state,
            &stored_commit.version,
            &commit_diff,
            &stored_commit.diff.id.to_string(),
            old_metrics,
            new_metrics_by_path,
        )
        .await?;

        previous_oid = Some(oid);
    }
    server.shutdown(mainloop).await;
    info!(repository = url, "finished repository analysis");
    Ok(())
}

struct StoredCommit {
    version: db::Version,
    diff: db::Diff,
}

struct StoredFileAnalysis {
    file: db::File,
    metrics: FileMetrics,
}

async fn store_commit_snapshot(
    pool: &PgPool,
    state: &State,
    commit_info: &CommitInfo,
    commit_diff: &CommitDiff,
) -> Result<StoredCommit> {
    let old_commit_hash = commit_diff.old_oid.as_ref().map(ToString::to_string);
    let new_commit_hash = commit_diff.new_oid.to_string();
    let pretty_diff = commit_diff.pretty_print();

    let version = db::insert_version(
        pool,
        state.codebase.id,
        &commit_info.hash,
        &commit_info.message,
        &commit_info.author_name,
        &commit_info.author_email,
        commit_info.committed_at,
        Some(commit_info.is_fix),
        &json!({}),
    )
    .await?;
    debug!(
        version_id = %version.id,
        commit = %commit_info.hash,
        old_commit_hash = ?old_commit_hash,
        "stored commit version"
    );
    let diff = db::insert_diff(
           pool,
           version.id,
           old_commit_hash.as_deref(),
           &new_commit_hash,
           usize_to_i32(commit_diff.files_changed(), "files_changed")?,
           usize_to_i32(commit_diff.insertions(), "insertions")?,
           usize_to_i32(commit_diff.deletions(), "deletions")?,
           usize_to_i32(commit_diff.number_of_changes(), "changed_lines")?,
           Some(&pretty_diff),
           &json!({}),
       )
       .await?;



    Ok(StoredCommit { version, diff })
}

async fn store_diff_file_changes(
    pool: &PgPool,
    diff: &db::Diff,
    commit_diff: &CommitDiff,
) -> Result<()> {
    db::delete_file_changes_by_diff(pool, diff.id).await?;

    for file_change in commit_diff.file_changes() {
        let old_path = file_change
            .old_file()
            .map(|path| path.display().to_string());
        let new_path = file_change
            .new_file()
            .map(|path| path.display().to_string());
        let old_blob_oid = file_change.old_blob_oid().map(|oid| oid.to_string());
        let new_blob_oid = file_change.new_blob_oid().map(|oid| oid.to_string());

        db::insert_file_change(
            pool,
            diff.id,
            old_path.as_deref(),
            new_path.as_deref(),
            file_change.status(),
            old_blob_oid.as_deref(),
            new_blob_oid.as_deref(),
            &json!({}),
        )
        .await?;
    }

    Ok(())
}

async fn store_diff_line_changes(
    pool: &PgPool,
    diff: &db::Diff,
    commit_diff: &CommitDiff,
) -> Result<()> {
    db::delete_changes_by_diff(pool, diff.id).await?;

    for change in commit_diff.changes() {
        let old_span = change.old_line_span();
        let new_span = change.new_line_span();
        let old_path = change.old_file().map(|path| path.display().to_string());
        let new_path = change.new_file().map(|path| path.display().to_string());
        let change_metrics = json!({
            "old_span_length": old_span.1.saturating_sub(old_span.0),
            "new_span_length": new_span.1.saturating_sub(new_span.0),
        });

        db::insert_change(
            pool,
            diff.id,
            old_path.as_deref(),
            new_path.as_deref(),
            usize_to_i32(old_span.0, "old_start_line")?,
            usize_to_i32(old_span.1, "old_end_line")?,
            usize_to_i32(new_span.0, "new_start_line")?,
            usize_to_i32(new_span.1, "new_end_line")?,
            &change_metrics,
        )
        .await?;
    }

    Ok(())
}

async fn analyze_and_store_changed_files(
    pool: &PgPool,
    server: &Server,
    semaphore: Arc<Semaphore>,
    state: &State,
    commit_info: &CommitInfo,
    version: &db::Version,
    commit_diff: &CommitDiff,
) -> Result<HashMap<EcoString, FileMetrics>> {
    let mut join_set: JoinSet<Result<(EcoString, StoredFileAnalysis)>> =
        tokio::task::JoinSet::new();
    let paths_to_analyze: Vec<(PathBuf, PathBuf)> = commit_diff
        .files()
        .iter()
        .map(|path| (path.clone(), state.sr.dest_dir.join(path)))
        .collect();

    for (relative_path, absolute_path) in paths_to_analyze {
        let Some(lc) = filter_map_language_config(state, &absolute_path) else {
            continue;
        };
        debug!(
            commit = %commit_info.hash,
            file = %relative_path.display(),
            "queueing file analysis task"
        );

        let permit = semaphore.clone().acquire_owned().await?;
        let pool = pool.clone();
        let version = version.clone();
        let mut socket = server.socket();
        let relative_path_clone = relative_path.clone();
        join_set.spawn(async move {
            let _permit = permit;
            debug!(
                version_id = %version.id,
                file = %relative_path.display(),
                "starting file analysis task"
            );

            let uri = socket.open_document(&absolute_path).await;
            let mut processor = Processor::new(&lc, &absolute_path, socket, uri)?;
            let analysis = processor.analyze_with_enrichted_stats().await?;

            let file_path = EcoString::from(relative_path_clone.display().to_string());
            let file_analysis = store_file_analysis(
                &pool,
                &version,
                &file_path,
                &lc.language,
                &analysis,
                processor.source(),
                processor.new_line_map(),
            )
            .await?;

            processor.close_language_server_file().await;
            debug!(
                version_id = %version.id,
                file = %file_path,
                "finished file analysis task"
            );

            Ok((file_path, file_analysis))
        });
    }

    let mut analyses_by_path = HashMap::new();
    let mut metrics_by_path = HashMap::new();
    while let Some(task_result) = join_set.join_next().await {
        let (path, analysis) = task_result??;
        debug!(
            commit = %commit_info.hash,
            file = %path,
            "collected analyzed file metrics"
        );
        metrics_by_path.insert(path.clone(), analysis.metrics);
        analyses_by_path.insert(path, analysis);
    }

    store_file_states(
        pool,
        &state.codebase,
        version,
        commit_diff,
        &analyses_by_path,
    )
    .await?;

    Ok(metrics_by_path)
}

async fn store_file_analysis(
    pool: &PgPool,
    version: &db::Version,
    file_path: &EcoString,
    language: &ProgrammingLanguage,
    analysis: &Analysis,
    source: &str,
    newline_map: &NewLineMap,
) -> Result<StoredFileAnalysis> {
    let language_name = format!("{language:?}").to_ascii_lowercase();
    let file_metrics = FileMetrics {
        lines_of_code: analysis.lines_of_code,
        effective_lines_of_code: analysis.effective_lines_of_code,
        comment_lines_of_code: analysis.comment_lines_of_code,
        bracket_lines_of_code: analysis.bracket_lines_of_code,
        total_cyclomatic: analysis.total_cyclomatic,
    };
    let file = db::insert_file(
        pool,
        version.id,
        file_path,
        Some(&language_name),
        &file_metrics_json(&file_metrics),
    )
    .await?;
    db::delete_functions_by_file(pool, file.id).await?;

    for func in &analysis.functions {
        let functions_called: Vec<String> = func
            .functions_called
            .iter()
            .map(|call| call.name.to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let function_calls: Vec<serde_json::Value> = func
            .functions_called
            .iter()
            .map(|call| {
                json!({
                    "name": call.name.to_string(),
                    "file": call.file.display().to_string(),
                    "line": call.pos.line,
                    "column": call.pos.column,
                })
            })
            .collect();
        let outdegree = u64::try_from(functions_called.len()).context("outdegree exceeds u64")?;

        let references: Vec<String> = analysis
            .functions
            .iter()
            .filter(|candidate| candidate.function_name != func.function_name)
            .filter(|candidate| {
                candidate
                    .functions_called
                    .iter()
                    .any(|called| called.name == func.function_name)
            })
            .map(|candidate| candidate.function_name.to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let indegree = u64::try_from(references.len()).context("indegree exceeds u64")?;

        // Function names may repeat by namespace, so include location.
        let unique_name = func.name.with_location(source, newline_map)?;
        let function_metrics = json!({
            "function_length": func.function_length,
            "cyclomatic": func.cyclomatic,
            "cyclomatic_match_as_single_branch": func.cyclomatic_match_as_single_branch,
            "definition_position_range": {
                "start": {
                    "line": func.definition_position_range.start.line,
                    "column": func.definition_position_range.start.column,
                },
                "end": {
                    "line": func.definition_position_range.end.line,
                    "column": func.definition_position_range.end.column,
                },
            },
            "functions_called": functions_called,
            "function_calls": function_calls,
            "references": references,
            "indegree": indegree,
            "outdegree": outdegree,
        });

        db::insert_function(
            pool,
            file.id,
            unique_name.as_str(),
            usize_to_i32(func.definition_line_span.start_line, "function start_line")?,
            usize_to_i32(func.definition_line_span.end_line, "function end_line")?,
            &function_metrics,
        )
        .await?;
    }

    Ok(StoredFileAnalysis {
        file,
        metrics: file_metrics,
    })
}

async fn store_file_states(
    pool: &PgPool,
    codebase: &Codebase,
    version: &db::Version,
    commit_diff: &CommitDiff,
    analyses_by_path: &HashMap<EcoString, StoredFileAnalysis>,
) -> Result<()> {
    let mut states = Vec::new();
    let total_files = commit_diff.file_changes().len();
    let mut processed = 0;

    info!(
        version_id = %version.id,
        total_files = total_files,
        "starting batch file state storage"
    );

    for file_change in commit_diff.file_changes() {
        let old_path = file_change
            .old_file()
            .map(|path| path.display().to_string());
        let new_path = file_change
            .new_file()
            .map(|path| path.display().to_string());
        let status = file_change.status();

        if let Some(path) = old_path.as_deref() {
            let path_removed = matches!(status, "deleted" | "renamed") || new_path.is_none();
            let path_replaced = new_path.as_deref().is_some_and(|new_path| new_path != path);
            if path_removed || path_replaced {
                states.push(db::FileStateInsert {
                    codebase_id: codebase.id,
                    version_id: version.id,
                    path: path.to_string(),
                    file_id: None,
                    status: status.to_string(),
                    exists: false,
                    source_path: None,
                    metrics: json!({}),
                });
            }
        }

        let Some(path) = new_path.as_deref() else {
            processed += 1;
            continue;
        };
        let Some(analysis) = analyses_by_path.get(path) else {
            processed += 1;
            continue;
        };
        states.push(db::FileStateInsert {
            codebase_id: codebase.id,
            version_id: version.id,
            path: path.to_string(),
            file_id: Some(analysis.file.id),
            status: status.to_string(),
            exists: true,
            source_path: old_path
                .as_deref()
                .filter(|old_path| *old_path != path)
                .map(|s| s.to_string()),
            metrics: file_metrics_json(&analysis.metrics),
        });
        processed += 1;
    }

    debug!(
        version_id = %version.id,
        collected_states = states.len(),
        processed_files = processed,
        "collected all file states for batch insert"
    );

    let rows_affected = db::batch_upsert_file_states(pool, states).await?;

    info!(
        version_id = %version.id,
        rows_affected = rows_affected,
        "completed batch file state storage"
    );

    Ok(())
}

async fn update_version_metrics(
    pool: &PgPool,
    state: &mut State,
    version: &db::Version,
    commit_diff: &CommitDiff,
    diff_id: &str,
    old_metrics: AggregatedFileMetrics,
    new_metrics_by_path: HashMap<EcoString, FileMetrics>,
) -> Result<()> {
    let new_metrics = AggregatedFileMetrics::from_file_metrics_map(&new_metrics_by_path);
    let version_metrics =
        AggregatedFileMetrics::reconcile(state.current_aggregate, old_metrics, new_metrics);
    db::update_version_metrics(pool, version.id, &version_metrics.to_json(), diff_id).await?;

    for changed_path in commit_diff.files() {
        state
            .current_file_metrics_by_path
            .remove(changed_path.display().to_string().as_str());
    }
    state
        .current_file_metrics_by_path
        .extend(new_metrics_by_path);
    state.current_aggregate = version_metrics;

    Ok(())
}

fn file_metrics_json(metrics: &FileMetrics) -> serde_json::Value {
    json!({
        "lines_of_code": metrics.lines_of_code,
        "effective_lines_of_code": metrics.effective_lines_of_code,
        "comment_lines_of_code": metrics.comment_lines_of_code,
        "bracket_lines_of_code": metrics.bracket_lines_of_code,
        "total_cyclomatic": metrics.total_cyclomatic,
    })
}

fn usize_to_i32(value: usize, name: &str) -> Result<i32> {
    i32::try_from(value).with_context(|| format!("{name} is larger than i32"))
}

fn gather_old_metrics(
    changed_paths: &[PathBuf],
    current_file_metrics_by_path: &HashMap<EcoString, FileMetrics>,
) -> HashMap<EcoString, FileMetrics> {
    if changed_paths.is_empty() {
        return HashMap::new();
    }

    let mut seen = HashSet::new();
    changed_paths
        .into_iter()
        .filter_map(|path| {
            let path = EcoString::from(path.display().to_string());
            if !seen.insert(path.clone()) {
                return None;
            }
            current_file_metrics_by_path
                .get(&path)
                .copied()
                .map(|metrics| (path, metrics))
        })
        .collect()
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

fn filter_map_language_config(state: &State, absolute_path: &PathBuf) -> Option<LanguageConfig> {
    if !absolute_path.is_file() {
        debug!(?absolute_path, "ignored file because is not a file");
        return None;
    }
    let Some(language) = ProgrammingLanguage::detect_language(absolute_path, None) else {
        debug!(
            ?absolute_path,
            "ignored file because could not detect language"
        );
        return None;
    };
    let lc = LanguageConfig::new(language);
    if state
        .sr
        .is_ignored_file(absolute_path, &lc.extensions)
        .unwrap_or(true)
    {
        debug!(
            ?absolute_path,
            ?language,
            "ignored file because extension does not match"
        );
        return None;
    }

    if matches!(language, ProgrammingLanguage::Haskell) {
        debug!(
            ?absolute_path,
            ?language,
            "skipping file because no language configuration is available"
        );
        return None;
    }
    Some(lc)
}

pub async fn analyze_single_file(
    path: String,
    outfile: String,
    programming_language: Option<ProgrammingLanguage>,
    root_dir: Option<String>,
) -> Result<()> {
    let path = PathBuf::from(path);
    let lc = match programming_language {
        Some(pl) => LanguageConfig::new(pl),
        None => {
            let pl = ProgrammingLanguage::from_extension(
                &path
                    .extension()
                    .expect("could not get extension")
                    .to_string_lossy()
                    .to_string(),
            )
            .expect("could not guess programming language from extension");
            LanguageConfig::new(pl)
        }
    };
    let (binary, args) = lc.language.lsp();
    let root = root_dir.map(|r| PathBuf::from(&r)).unwrap_or(
        path.parent()
            .expect("could not find partent dir of file")
            .into(),
    );
    let mut server = Server::new(root, binary, args);
    let mainloop = server.run_main_loop();
    server.initialize().await;
    let mut socket = server.socket();
    let uri = socket.open_document(&path).await;
    let mut processor = Processor::new(&lc, &path, socket, uri)?;
    let analysis = processor.analyze_with_enrichted_stats().await?;
    let source = processor.source();
    let pretty_metrics = analysis.pretty_print(source);

    fs::write(outfile, pretty_metrics)?;
    server.shutdown(mainloop).await;
    Ok(())
}
