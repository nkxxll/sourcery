use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use ecow::EcoString;
use git2::{BranchType, Commit, Oid, Repository, Revwalk, Sort};

const REPOSITORIES_DIRECTORY: &str = "toanalyze";

pub struct SourceRepository {
    pub url: EcoString,
    repo: Repository,
    pub dest_dir: PathBuf,
    pub cwd: PathBuf,
}

fn ensure_present(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    Ok(())
}

impl SourceRepository {
    fn try_push_ref_tip(repo: &Repository, revwalk: &mut Revwalk<'_>, refname: &str) -> bool {
        let Ok(reference) = repo.find_reference(refname) else {
            return false;
        };

        let oid = if let Ok(resolved) = reference.resolve() {
            resolved.target()
        } else {
            reference.target()
        };

        match oid {
            Some(oid) => revwalk.push(oid).is_ok(),
            None => false,
        }
    }

    fn push_detached_fallback(repo: &Repository, revwalk: &mut Revwalk<'_>) -> Result<()> {
        if Self::try_push_ref_tip(repo, revwalk, "refs/remotes/origin/HEAD")
            || Self::try_push_ref_tip(repo, revwalk, "refs/heads/main")
            || Self::try_push_ref_tip(repo, revwalk, "refs/heads/master")
        {
            return Ok(());
        }

        for branch in repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            let Some(name) = branch.get().name() else {
                continue;
            };
            if Self::try_push_ref_tip(repo, revwalk, name) {
                return Ok(());
            }
        }

        revwalk.push_head()?;
        Ok(())
    }

    pub fn new(url: &str) -> Result<Self> {
        let (cwd, dest_dir) = Self::setup_directories(url)?;
        let clone_repo = || {
            Repository::clone(url, &dest_dir).map_err(|e| {
                anyhow!(
                    "failed to clone repository {url} into {} (cwd: {}): {e}",
                    dest_dir.display(),
                    cwd.display(),
                )
            })
        };

        let repo = if dest_dir.exists() {
            match Repository::open(&dest_dir) {
                Ok(repo) => repo,
                Err(_open_err)
                    if dest_dir.is_dir() && fs::read_dir(&dest_dir)?.next().is_none() =>
                {
                    clone_repo()?
                }
                Err(open_err) => {
                    return Err(anyhow!(
                        "failed to open repository at {} (cwd: {}): {open_err}",
                        dest_dir.display(),
                        cwd.display()
                    ));
                }
            }
        } else {
            let repo_parent = dest_dir.parent().ok_or_else(|| {
                anyhow!(
                    "failed to resolve parent directory for repository path {}",
                    dest_dir.display()
                )
            })?;
            ensure_present(repo_parent)?;
            clone_repo()?
        };
        Ok(SourceRepository {
            url: url.into(),
            repo,
            dest_dir,
            cwd,
        })
    }

    fn setup_directories(url: &str) -> Result<(PathBuf, PathBuf)> {
        let cwd = std::env::current_dir()?;
        let dest_dir = Self::get_dest_directory(url, &cwd);
        Ok((cwd, dest_dir))
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
        let cwd = std::env::current_dir()?;
        let repo = Repository::open(&path)?;
        Ok(SourceRepository {
            url: path.display().to_string().into(),
            repo,
            // this works as long as this is a test
            dest_dir: path.clone(),
            cwd,
        })
    }

    pub(crate) fn get_repo_base_name(url: &str) -> EcoString {
        url.trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .expect("invalid repository URL")
            .into()
    }

    fn get_dest_directory(url: &str, cwd: &Path) -> PathBuf {
        let name = Self::get_repo_base_name(url);
        cwd.join(REPOSITORIES_DIRECTORY).join(name.as_str())
    }

    /// Returns a preconfigured `Revwalk` iterator over first-parent commits.
    pub fn iter(&self) -> Result<Revwalk<'_>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.simplify_first_parent()?;
        if self.repo.head_detached()? {
            Self::push_detached_fallback(&self.repo, &mut revwalk)?;
        } else {
            revwalk.push_head()?;
        }
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

    pub fn is_ignored_file(self: &Self, path: &Path, extension: &[EcoString]) -> Result<bool> {
        if self.is_path_ignored(path)? {
            return Ok(true);
        }
        // and some other options
        match path.extension() {
            Some(ex) => {
                if extension.iter().any(|x| {
                    let ext = ex.to_str().expect("conversion failed");
                    x.as_str() == ext
                }) {
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
