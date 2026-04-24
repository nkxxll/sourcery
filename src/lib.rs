use std::{
    fs::File,
    io::{BufReader, Read},
    sync::Arc,
};

use tokio::{sync::Semaphore, task::JoinSet};
use tracing::warn;
use walkdir::WalkDir;

use crate::{
    git_handler::SourceRepository,
    language::LanguageConfig,
    processor::{CyclomaticComplexityProcessor, LinesOfCodeProcessor},
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
    let mut join_set: JoinSet<Result<(), anyhow::Error>> = tokio::task::JoinSet::new();

    // TODO: get the language config here
    let lc = Arc::new(LanguageConfig::new(language::ProgrammingLanguage::Python));

    let sr = SourceRepository::new(url)?;
    for commit_oid in sr.into_iter() {
        let repo_walker = WalkDir::new(&sr.dest_dir);
        eprintln!("=== commit ===");
        dbg!(&commit_oid);
        let oid = commit_oid.expect("this should be a commit");
        sr.checkout_commit(&oid)?;
        if let Ok(commit) = &sr.find_commit(&oid) {
            if let Some(message) = commit.message() {
                if is_fix(message) {
                    dbg!("this is a fix");
                } else {
                    dbg!("this is not a fix");
                }
            } else {
                warn!("there is no message on this commit");
            }
        } else {
            warn!("there is no commit found for the oid!");
        }
        for entry in repo_walker.into_iter().filter_map(|e| e.ok()) {
            let path = entry.path().to_path_buf();
            if entry.file_type().is_file() && !sr.is_ignored_file(&path, "py")? {
                let permit = semaphore.clone().acquire_owned().await.unwrap();
                let path = path.clone();
                let lc = lc.clone();
                join_set.spawn(async move {
                    let _permit = permit; // use the permit here
                    let file = File::open(&path)?;
                    let mut file_reader = BufReader::new(file);
                    let mut source = String::new();
                    file_reader.read_to_string(&mut source)?;

                    let loc_file = LinesOfCodeProcessor::lines_of_code_content(&source)?;

                    let loc_effect_file =
                        LinesOfCodeProcessor::effective_lines_of_code_content(&source)?;
                    dbg!(&path, loc_file, loc_effect_file);

                    let tree = lc.parse_tree(&source)?;
                    let functions = lc.get_functions(&tree, &source)?;
                    for func in functions {
                        // it is common that because of some kind of
                        // namespace the same funciton/method name could
                        // occure multiple times in a file so we have to
                        // encode the locations with the name
                        let unique_name = func.name.with_location(&source)?;
                        let definition = func.definition.get_content(&source)?;
                        let loc_function =
                            LinesOfCodeProcessor::lines_of_code_content(&definition)?;
                        let cyclomatic_function = CyclomaticComplexityProcessor::compute_cyclomatic(
                            &tree.root_node(),
                            source.as_bytes(),
                            &lc,
                            Some(func.definition),
                        );
                        dbg!(&path, &unique_name, loc_function, cyclomatic_function);
                    }

                    let comments = lc.get_comments(&tree, &source)?;
                    for comment in comments {
                        let content = comment.get_content(&source)?;
                        dbg!(
                            &content,
                            LinesOfCodeProcessor::lines_of_code_content(&content)?
                        );
                    }

                    // TODO: return the right type
                    Ok(())
                });
            }
        }
        while let Some(_) = join_set.join_next().await {}
    }
    Ok(())
}

fn is_fix(message: &str) -> bool {
    let re = FIX_REGEX.get_or_init(|| {
            Regex::new(
                r"(?i)\b(fix(e[sd])?|bugfix(es)?|hotfix(es)?|patch(ed|es)?|resolve[sd]?|correct(ed)?|repair(ed|s)?)\b"
            ).unwrap()
        });

    re.is_match(message)
}
