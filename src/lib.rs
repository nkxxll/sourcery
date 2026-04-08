use std::sync::Arc;

use tokio::{sync::Semaphore, task::JoinSet};
use walkdir::WalkDir;

use crate::{
    git_handler::SourceRepository, language::LanguageConfig, processor::LinesOfCodeProcessor,
};
use anyhow::Result;

pub mod db;
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
    for commit in sr.into_iter() {
        let repo_walker = WalkDir::new(&sr.dest_dir);
        sr.checkout_commit(commit.expect("this should be a commit"))?;
        for entry in repo_walker.into_iter().filter_map(|e| e.ok()) {
            let path = entry.path().to_path_buf();
            if entry.file_type().is_file() && !sr.is_ignored_file(&path, "py")? {
                {
                    let permit = semaphore.clone().acquire_owned().await.unwrap();
                    let path = path.clone();
                    join_set.spawn(async move {
                        let _permit = permit; // use the permit here
                        let loc_file = LinesOfCodeProcessor::lines_of_code_file(&path)?;
                        dbg!(&path, loc_file);
                        // TODO: return the right type
                        Ok(())
                    })
                };
                {
                    let permit = semaphore.clone().acquire_owned().await.unwrap();
                    let path = path.clone();
                    let lc = lc.clone();
                    join_set.spawn(async move {
                        let _permit = permit; // use the permit here
                        let (tree, source) = lc.get_tree(&path)?;
                        let functions = lc.get_functions(&tree, &source)?;
                        for func in functions {
                            let name = func.name.get_content(&source)?;
                            let definition = func.definition.get_content(&source)?;
                            let loc_function = LinesOfCodeProcessor::lines_of_code_content(&definition)?;
                            dbg!(&path, &name, loc_function);
                        }

                        // TODO: return the right type
                        Ok(())
                    });
                }
            }
        }
        while let Some(_) = join_set.join_next().await {}
    }
    Ok(())
}
