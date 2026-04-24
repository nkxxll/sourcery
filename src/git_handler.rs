use std::path::{Path, PathBuf};

use anyhow::Result;
use git2::{Commit, Oid, Repository, Revwalk, Sort};

pub struct SourceRepository {
    pub url: String,
    repo: Repository,
    pub dest_dir: PathBuf,
}

impl SourceRepository {
    pub fn new(url: &str) -> Result<Self> {
        let dest_dir = Self::get_dest_directory(url);
        let repo = if dest_dir.is_dir() {
            Repository::open(&dest_dir)?
        } else {
            Repository::clone(url, &dest_dir)?
        };
        Ok(SourceRepository {
            url: url.to_string(),
            repo,
            dest_dir,
        })
    }

    pub fn find_commit(&self, oid: &Oid) -> Result<Commit> {
        Ok(self.repo.find_commit(*oid)?)
    }

    pub fn is_path_ignored(self: &Self, path: &Path) -> Result<bool> {
        Ok(self.repo.is_path_ignored(path)?)
    }

    pub fn from_path(path: PathBuf) -> Result<Self> {
        let repo = Repository::open(&path)?;
        Ok(SourceRepository {
            url: path.display().to_string(),
            repo,
            dest_dir: path,
        })
    }

    fn get_dest_directory(url: &str) -> PathBuf {
        let name = url
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .expect("invalid repository URL");
        let cwd = std::env::current_dir().expect("failed to get current directory");
        cwd.join(name)
    }

    /// Returns a preconfigured `Revwalk` iterator over first-parent commits.
    pub fn iter(&self) -> Result<Revwalk<'_>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.simplify_first_parent()?;
        revwalk.push_head()?;
        revwalk.set_sorting(Sort::REVERSE | Sort::TOPOLOGICAL)?; // root to head
        Ok(revwalk)
    }

    pub fn checkout_commit(self: &Self, oid: &Oid) -> Result<()> {
        let repo = &self.repo;
        let object = repo.find_object(*oid, None)?;
        repo.checkout_tree(&object, None)?;
        repo.set_head_detached(*oid)?;
        tracing::info!("Checked out commit {}", oid);
        Ok(())
    }

    pub fn is_ignored_file(self: &Self, path: &Path, extension: &str) -> Result<bool> {
        if self.is_path_ignored(path)? {
            return Ok(true);
        }
        // and some other options
        match path.extension() {
            Some(ex) => {
                if ex == extension {
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            None => Ok(true),
        }
    }
}

impl<'a> IntoIterator for &'a SourceRepository {
    type Item = Result<git2::Oid, git2::Error>;
    type IntoIter = Revwalk<'a>;

    fn into_iter(self) -> Revwalk<'a> {
        self.iter().expect("failed to create revwalk")
    }
}
