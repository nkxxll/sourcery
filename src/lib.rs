use walkdir::WalkDir;

use crate::git_handler::SourceRepository;
use anyhow::Result;

mod db;
mod git_handler;

pub fn analyze_git_repository(url: &str) -> Result<()> {
    let sr = SourceRepository::new(url)?;
    for commit in sr.into_iter() {
        let repo_walker = WalkDir::new(&sr.dest_dir);
        sr.checkout_commit(commit.expect("this should be a commit"))?;
        for entry in repo_walker.into_iter() {
            todo!("do the metrics getting here");
        }
    }
    Ok(())
}
