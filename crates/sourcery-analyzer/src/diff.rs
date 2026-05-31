use anyhow::Result;
use std::{collections::BTreeSet, path::PathBuf};

use git2::{Delta, Oid, Repository};

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

pub struct FileChange {
    old_file: Option<PathBuf>,
    new_file: Option<PathBuf>,
    status: &'static str,
    old_blob_oid: Option<Oid>,
    new_blob_oid: Option<Oid>,
}

impl FileChange {
    pub fn old_file(&self) -> Option<&std::path::Path> {
        self.old_file.as_deref()
    }

    pub fn new_file(&self) -> Option<&std::path::Path> {
        self.new_file.as_deref()
    }

    pub fn status(&self) -> &'static str {
        self.status
    }

    pub fn old_blob_oid(&self) -> Option<Oid> {
        self.old_blob_oid
    }

    pub fn new_blob_oid(&self) -> Option<Oid> {
        self.new_blob_oid
    }
}

pub struct CommitDiff {
    pub new_oid: Oid,
    pub old_oid: Option<Oid>,
    files: Vec<PathBuf>,
    file_changes: Vec<FileChange>,
    changes: Vec<Change>,
    patch: Vec<u8>,
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
        let mut patch = Vec::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            patch.extend_from_slice(line.content());
            true
        })?;
        let mut files = BTreeSet::new();
        let mut file_changes = Vec::new();
        for delta in diff.deltas() {
            let old_file = delta.old_file();
            let new_file = delta.new_file();
            let old_file_buf = old_file.path().map(PathBuf::from);
            let new_file_buf = new_file.path().map(PathBuf::from);

            if let Some(path) = &old_file_buf {
                files.insert(path.to_path_buf());
            }
            if let Some(path) = &new_file_buf {
                files.insert(path.to_path_buf());
            }

            file_changes.push(FileChange {
                old_file: old_file_buf,
                new_file: new_file_buf,
                status: delta_status(delta.status()),
                old_blob_oid: oid_from_delta_file(old_file.id()),
                new_blob_oid: oid_from_delta_file(new_file.id()),
            });
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
            file_changes,
            changes,
            patch,
            files_changed,
            number_of_changes: insertions + deletions,
            insertions,
            deletions,
        })
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn file_changes(&self) -> &[FileChange] {
        &self.file_changes
    }

    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    pub fn patch(&self) -> &[u8] {
        &self.patch
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

fn delta_status(status: Delta) -> &'static str {
    match status {
        Delta::Unmodified => "unmodified",
        Delta::Added => "added",
        Delta::Deleted => "deleted",
        Delta::Modified => "modified",
        Delta::Renamed => "renamed",
        Delta::Copied => "copied",
        Delta::Ignored => "ignored",
        Delta::Untracked => "untracked",
        Delta::Typechange => "typechange",
        Delta::Unreadable => "unreadable",
        Delta::Conflicted => "conflicted",
    }
}

fn oid_from_delta_file(oid: Oid) -> Option<Oid> {
    if oid.to_string() == "0000000000000000000000000000000000000000" {
        None
    } else {
        Some(oid)
    }
}
