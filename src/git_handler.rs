use std::path::{Path, PathBuf};

use anyhow::Result;
use git2::{Commit, Oid, Repository, Revwalk, Sort};

const REPOSITORIES_DIRECTORY: &str = "toanalyze";
const ANALYTICS_DIRECTORY: &str = "analytics";

pub struct SourceRepository {
    pub url: String,
    repo: Repository,
    pub dest_dir: PathBuf,
    pub analytics_dir: PathBuf,
}

impl SourceRepository {
    pub fn new(url: &str) -> Result<Self> {
        let dest_dir = Self::get_dest_directory(url);
        let analytics_dir = Self::get_analytics_directory(url);
        let repo = if dest_dir.is_dir() {
            Repository::open(&dest_dir)?
        } else {
            Repository::clone(url, &dest_dir)?
        };
        Ok(SourceRepository {
            url: url.to_string(),
            repo,
            dest_dir,
            analytics_dir,
        })
    }

    pub fn find_commit(&self, oid: &Oid) -> Result<Commit<'_>> {
        Ok(self.repo.find_commit(*oid)?)
    }

    pub fn commit_diff(
        &self,
        old_commit_oid: Option<&Oid>,
        new_commit_oid: &Oid,
    ) -> Result<crate::diff::CommitDiff> {
        crate::diff::CommitDiff::new(&self.repo, old_commit_oid, new_commit_oid)
    }

    pub fn is_path_ignored(self: &Self, path: &Path) -> Result<bool> {
        Ok(self.repo.is_path_ignored(path)?)
    }

    pub fn from_path(path: PathBuf) -> Result<Self> {
        let repo = Repository::open(&path)?;
        Ok(SourceRepository {
            url: path.display().to_string(),
            repo,
            // this works as long as this is a test
            dest_dir: path.clone(),
            analytics_dir: path,
        })
    }

    fn get_repo_base_name(url: &str) -> String {
        url.trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .expect("invalid repository URL")
            .into()
    }

    fn get_dest_directory(url: &str) -> PathBuf {
        let name = Self::get_repo_base_name(url);
        let cwd = std::env::current_dir().expect("failed to get current directory");
        cwd.join(REPOSITORIES_DIRECTORY).join(name)
    }

    fn get_analytics_directory(url: &str) -> PathBuf {
        let name = Self::get_repo_base_name(url);
        let cwd = std::env::current_dir().expect("failed to get current directory");
        cwd.join(ANALYTICS_DIRECTORY).join(name)
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
