use std::collections::HashMap;

use git2::{Diff, Oid};

use crate::language::CodeSpan;

/// how to get a diff of two commits oids
/// needed are the files that are changed the number of changes measured after
/// delete 1
/// add 1
/// edit 2 (one delete one add)
/// and the ranges of change if possible and the corresponding files
/// this can be read from the standard diff format or maybe the information is present from libgit2

struct CommitDiff {
    changed_files: Vec<PathBuf>,
    changes: HashMap<String, Vec<CodeSpan>>,
    files_changed: usize,
    number_of_changes: usize,
    insertions: usize,
    deletions: usize,
}

impl CommitDiff {
    pub fn new(diff: Diff) -> Self {
        // DiffStats describing the hunk data
        // stats = diff.stats()
        // stats.insertions()
        // stats.deletions()
        // stats.files_changed()
        // Deltas an iterator over diffs in a delta
        // diff.deltas()
        // DeltaStats
        // for deltastat in deltas
        //   deltastat.nfiles()
        //   deltastat.old_file()
        //   deltastat.new_file()
        //   deltastat.diffflags() -> to see whether we have a text file so NOT_BINARY
        //
        let stats = diff.stats()?;
        let insertions = stats.insertions();
        let deletions = stats.deletions();
        let files_changed = stats.files_changed();
        let mut changed_files = Vec::new();
        let mut changes = HashMap::new();
        diff.foreach(
            &mut |_delta, _progress| true,
            None,
            Some(&mut |delta, hunk| {
                // todo fill up the changes hashmap
                let new_file = delta.new_file();
                let old_file = delta.old_file();
                let new_file_name = new_file.path().map_or("<path not found>", |path| {
                    path.to_str()
                        .unwrap_or("<path could not be converted to string>")
                });
                let old_file_name = old_file.path().map_or("<path not found>", |path| {
                    path.to_str()
                        .unwrap_or("<path could not be converted to string>")
                });

                changed_files.push(new_file.into());
                println!("file: {:?}", delta.new_file().path());
                println!(
                    "hunk: old {}+{}, new {}+{}",
                    hunk.old_start(),
                    hunk.old_lines(),
                    hunk.new_start(),
                    hunk.new_lines()
                );
                true
            }),
            Some(&mut |_delta, _hunk, line| {
                // optional: lines inside the hunk
                true
            }),
        );
        CommitDiff {
            changed_files,
            changes,
            files_changed,
            number_of_changes: insertions + deletions,
            insertions,
            deletions,
        }
    }
}
