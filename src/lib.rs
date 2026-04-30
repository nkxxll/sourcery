use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    sync::Arc,
};

use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{debug, warn};

use crate::{
    git_handler::SourceRepository,
    language::{LanguageConfig, ProgrammingLanguage},
    processor::{AstProcessor, LinesOfCodeProcessor},
};
use anyhow::Result;
use regex::Regex;
use std::sync::OnceLock;

static FIX_REGEX: OnceLock<Regex> = OnceLock::new();

pub mod db;
pub mod diff;
pub mod git_handler;
pub mod language;
pub mod processor;

pub async fn analyze_git_repository(url: &str) -> Result<()> {
    // get a semaphore that tracks open files so that the fs is not overwhelmed
    let semaphore = Arc::new(Semaphore::new(100));

    // catch all the task at the end
    let mut join_set: JoinSet<Result<()>> = tokio::task::JoinSet::new();

    // the old commit

    let sr = SourceRepository::new(url)?;
    let diff_output_path = sr.dest_dir.join("commit_diffs.txt");
    let mut diff_output = BufWriter::new(File::create(&diff_output_path)?);
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
        if let Ok(commit) = sr.find_commit(&oid) {
            match commit.message() {
                Some(message) if is_fix(message) => debug!(?oid, "fix commit"),
                Some(_) => debug!(?oid, "non-fix commit"),
                None => warn!(?oid, "there is no message on this commit"),
            }
        } else {
            warn!(?oid, "there is no commit found for the oid");
        }

        let commit_diff = sr.commit_diff(previous_oid.as_ref(), &oid)?;
        let diff_range = previous_oid
            .as_ref()
            .map(|prev_oid| format!("{prev_oid}..{oid}"))
            .unwrap_or_else(|| format!("root..{oid}"));
        writeln!(diff_output, "=== {diff_range} ===")?;
        writeln!(diff_output, "{}\n", commit_diff.pretty_print())?;

        let paths_to_analyze: Vec<PathBuf> = commit_diff
            .files()
            .iter()
            .map(|path| sr.dest_dir.join(path))
            .collect();

        for path in paths_to_analyze {
            if !path.is_file() {
                continue;
            }
            if sr.is_ignored_file(&path, "py")? {
                continue;
            }

            let Some(language) = ProgrammingLanguage::detect_language(&path, None) else {
                continue;
            };
            if matches!(language, ProgrammingLanguage::Haskell) {
                debug!(
                    ?path,
                    ?language,
                    "skipping file because no language configuration is available"
                );
                continue;
            }

            let permit = semaphore.clone().acquire_owned().await?;
            join_set.spawn(async move {
                let _permit = permit;
                let source = std::fs::read_to_string(&path)?;
                let lc = LanguageConfig::new(language);

                let loc_file = LinesOfCodeProcessor::lines_of_code_content(&source)?;
                let loc_effect_file =
                    LinesOfCodeProcessor::effective_lines_of_code_content(&source)?;
                debug!(?path, loc_file, loc_effect_file);

                let tree = lc.parse_tree(&source)?;
                let metrics = AstProcessor::analyze_tree(&tree, &lc);
                for func in metrics.functions {
                    // It is common that function/method names are repeated because of namespaces,
                    // so include location to make function names unique within a file.
                    let unique_name = func.name.with_location(&source)?;
                    let definition = func.definition.get_content(&source)?;
                    let loc_function = LinesOfCodeProcessor::lines_of_code_content(&definition)?;
                    let cyclomatic_function = func.cyclomatic;
                    debug!(?path, ?unique_name, loc_function, cyclomatic_function);
                }

                for comment in metrics.comments {
                    let content = comment.get_content(&source)?;
                    debug!(
                        ?content,
                        comment_loc = LinesOfCodeProcessor::lines_of_code_content(&content)?
                    );
                }

                Ok(())
            });
        }
        while let Some(task_result) = join_set.join_next().await {
            task_result??;
        }
        previous_oid = Some(oid);
    }
    diff_output.flush()?;
    Ok(())
}

fn is_fix(message: &str) -> bool {
    let re = FIX_REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)\b(fix(e[sd])?|bugfix(es)?|hotfix(es)?|patch(ed|es)?|resolve[sd]?|correct(ed)?|repair(ed|s)?)\b"
        )
        .unwrap()
    });

    re.is_match(message)
}
