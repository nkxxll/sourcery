use std::path::PathBuf;

use anyhow::Result;
use git2::{Oid, Repository, Revwalk, Sort};

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

    pub fn checkout_commit(self: &Self, oid: Oid) -> Result<()> {
        let repo = &self.repo;
        let object = repo.find_object(oid, None)?;
        repo.checkout_tree(&object, None)?;
        repo.set_head_detached(oid)?;
        tracing::info!("Checked out commit {}", oid);
        Ok(())
    }
}

impl<'a> IntoIterator for &'a SourceRepository {
    type Item = Result<git2::Oid, git2::Error>;
    type IntoIter = Revwalk<'a>;

    fn into_iter(self) -> Revwalk<'a> {
        self.iter().expect("failed to create revwalk")
    }
}
