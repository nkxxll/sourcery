use anyhow::Result;
use std::{collections::BTreeSet, path::PathBuf};

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

    pub fn old_file(&self) -> Option<&std::path::Path> {
        self.old_file.as_deref()
    }

    pub fn new_file(&self) -> Option<&std::path::Path> {
        self.new_file.as_deref()
    }

    pub fn old_line_span(&self) -> (usize, usize) {
        self.old_line_span
    }

    pub fn new_line_span(&self) -> (usize, usize) {
        self.new_line_span
    }
}

pub struct CommitDiff {
    pub new_oid: Oid,
    pub old_oid: Option<Oid>,
    files: Vec<PathBuf>,
    changes: Vec<Change>,
    files_changed: usize,
    number_of_changes: usize,
    insertions: usize,
    deletions: usize,
}

impl CommitDiff {
    pub fn new(
        repo: &Repository,
        old_commit_oid: Option<&Oid>,
        new_commit_oid: &Oid,
    ) -> Result<Self> {
        let new_commit = repo.find_commit(*new_commit_oid)?;
        let new_tree = new_commit.tree()?;
        let old_tree = if let Some(oid) = old_commit_oid {
            Some(repo.find_commit(*oid)?.tree()?)
        } else {
            None
        };
        let diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), None)?;

        let stats = diff.stats()?;
        let insertions = stats.insertions();
        let deletions = stats.deletions();
        let files_changed = stats.files_changed();
        let mut files = BTreeSet::new();
        for delta in diff.deltas() {
            if let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) {
                files.insert(path.to_path_buf());
            }
        }
        let mut changes = Vec::new();
        diff.foreach(
            &mut |_delta, _progress| true,
            None,
            Some(&mut |delta, hunk| {
                let new_file = delta.new_file();
                let old_file = delta.old_file();

                let new_file_buf = new_file.path().map(|p| PathBuf::from(p));
                let old_file_buf = old_file.path().map(|p| PathBuf::from(p));

                let change = Change::new(
                    old_file_buf,
                    new_file_buf,
                    hunk.old_start() as usize,
                    hunk.old_start() as usize + hunk.old_lines() as usize,
                    hunk.new_start() as usize,
                    hunk.new_start() as usize + hunk.new_lines() as usize,
                );
                changes.push(change);
                true
            }),
            Some(&mut |_delta, _hunk, _line| {
                // optional: lines inside the hunk
                true
            }),
        )?;
        Ok(CommitDiff {
            new_oid: *new_commit_oid,
            old_oid: old_commit_oid.copied(),
            files: files.into_iter().collect(),
            changes,
            files_changed,
            number_of_changes: insertions + deletions,
            insertions,
            deletions,
        })
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    pub fn files_changed(&self) -> usize {
        self.files_changed
    }

    pub fn number_of_changes(&self) -> usize {
        self.number_of_changes
    }

    pub fn insertions(&self) -> usize {
        self.insertions
    }

    pub fn deletions(&self) -> usize {
        self.deletions
    }

    pub fn pretty_print(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("files changed: {}", self.files_changed));
        lines.push(format!("insertions: {}", self.insertions));
        lines.push(format!("deletions: {}", self.deletions));
        lines.push(format!("total line changes: {}", self.number_of_changes));
        lines.push("files:".to_string());
        for file in &self.files {
            lines.push(format!("  - {}", file.display()));
        }
        lines.push("hunks:".to_string());
        for change in &self.changes {
            let old_file = change
                .old_file
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".to_string());
            let new_file = change
                .new_file
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".to_string());
            lines.push(format!(
                "  - {}:{}..{} -> {}:{}..{}",
                old_file,
                change.old_line_span.0,
                change.old_line_span.1,
                new_file,
                change.new_line_span.0,
                change.new_line_span.1
            ));
        }
        lines.join("\n")
    }
}
