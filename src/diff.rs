use anyhow::Result;
use std::path::PathBuf;

use git2::{Oid, Repository};

/// how to get a diff of two commits oids
/// needed are the files that are changed the number of changes measured after
/// delete 1
/// add 1
/// edit 2 (one delete one add)
/// and the ranges of change if possible and the corresponding files
/// this can be read from the standard diff format or maybe the information is present from libgit2
///
pub struct Change {
    old_file: Option<PathBuf>,
    new_file: Option<PathBuf>,
    old_line_span: (usize, usize),
    new_line_span: (usize, usize),
}

impl Change {
    pub fn new(
        old_file: Option<PathBuf>,
        new_file: Option<PathBuf>,
        old_file_hunk_start: usize,
        old_file_hunk_end: usize,
        new_file_hunk_start: usize,
        new_file_hunk_end: usize,
    ) -> Self {
        Change {
            old_file,
            new_file,
            old_line_span: (old_file_hunk_start, old_file_hunk_end),
            new_line_span: (new_file_hunk_start, new_file_hunk_end),
        }
    }
}

pub struct CommitDiff {
    changes: Vec<Change>,
    files_changed: usize,
    number_of_changes: usize,
    insertions: usize,
    deletions: usize,
}

impl CommitDiff {
    pub fn new(repo: &Repository, old_commit_oid: &Oid, new_commit_oid: &Oid) -> Result<Self> {
        let old_commit = repo.find_commit(*old_commit_oid)?;
        let new_commit = repo.find_commit(*new_commit_oid)?;

        let tree1 = old_commit.tree()?;
        let tree2 = new_commit.tree()?;

        let diff = repo.diff_tree_to_tree(Some(&tree1), Some(&tree2), None)?;

        let stats = diff.stats()?;
        let insertions = stats.insertions();
        let deletions = stats.deletions();
        let files_changed = stats.files_changed();
        let mut changes = Vec::new();
        diff.foreach(
            &mut |_delta, _progress| true,
            None,
            Some(&mut |delta, hunk| {
                // todo fill up the changes hashmap
                let new_file = delta.new_file();
                let old_file = delta.old_file();

                let new_file_buf = new_file.path().map(|p| PathBuf::from(p));
                let old_file_buf = old_file.path().map(|p| PathBuf::from(p));

                println!("file: {:?}", delta.new_file().path());
                let change = Change::new(
                    new_file_buf,
                    old_file_buf,
                    hunk.old_start() as usize,
                    hunk.old_lines() as usize,
                    hunk.new_start() as usize,
                    hunk.new_lines() as usize,
                );
                changes.push(change);
                println!(
                    "hunk: old {}+{}, new {}+{}",
                    hunk.old_start(),
                    hunk.old_lines(),
                    hunk.new_start(),
                    hunk.new_lines()
                );
                true
            }),
            Some(&mut |_delta, _hunk, _line| {
                // optional: lines inside the hunk
                true
            }),
        );
        Ok(CommitDiff {
            changes,
            files_changed,
            number_of_changes: insertions + deletions,
            insertions,
            deletions,
        })
    }
}
