use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use git2::{Delta, DiffFindOptions, DiffOptions, Oid, Repository, RepositoryOpenFlags, Sort};
use sourcery_db::{Codebase, PgPool, connect, list_codebases_by_commit};

#[derive(Debug, Parser)]
#[command(about = "Count how often files changed along a repository's mainline history")]
struct Cli {
    /// Database URL. Defaults to DATABASE_URL, including values loaded from .env.
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,

    /// Revision to start from, for example HEAD, main, or refs/heads/main.
    #[arg(short = 'r', long, default_value = "HEAD")]
    rev: String,

    /// CSV output path.
    #[arg(short, long, default_value = "file-change-counts.csv")]
    output: PathBuf,

    /// Only include repositories whose database codebase language matches this value.
    #[arg(short, long)]
    language: Option<String>,

    /// Git repository directories, or directories containing git repositories, to analyze.
    #[arg(value_name = "REPO", default_value = ".")]
    repos: Vec<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let database_url = cli
        .database_url
        .context("DATABASE_URL must be set or passed with --database-url")?;
    let pool = connect(&database_url).await?;
    let mut rows = Vec::new();

    let repo_paths = expand_repo_paths(&cli.repos)?;
    let language = cli.language.as_deref().map(normalize_language);

    for repo_path in &repo_paths {
        let repo = open_repo_at(repo_path)
            .with_context(|| format!("failed to open repository at {}", repo_path.display()))?;
        let start = resolve_revision(&repo, &cli.rev)?;
        let Some(codebase) = resolve_codebase(&pool, repo_path, start, language.as_deref()).await?
        else {
            continue;
        };
        let counts = count_file_changes(&repo, start)?;

        for (file, change_count) in counts {
            rows.push(ChangeCountRow {
                codebase_id: codebase.id.to_string(),
                codebase_name: codebase.name.clone(),
                file,
                change_count,
            });
        }
    }

    write_csv(&cli.output, &rows)
        .with_context(|| format!("failed to write CSV to {}", cli.output.display()))?;

    Ok(())
}

fn expand_repo_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut repos = Vec::new();

    for path in paths {
        if is_repo_dir(path) {
            repos.push(path.clone());
            continue;
        }

        let entries = fs::read_dir(path)
            .with_context(|| format!("failed to read directory {}", path.display()))?;
        let before = repos.len();
        for entry in entries {
            let entry = entry?;
            let child_path = entry.path();
            if entry.file_type()?.is_dir() && is_repo_dir(&child_path) {
                repos.push(child_path);
            }
        }

        if repos.len() == before {
            bail!(
                "{} is not a git repository and contains no git repositories",
                path.display()
            );
        }
    }

    Ok(repos)
}

fn is_repo_dir(path: &Path) -> bool {
    open_repo_at(path).is_ok()
}

fn open_repo_at(path: &Path) -> Result<Repository, git2::Error> {
    Repository::open_ext(path, RepositoryOpenFlags::NO_SEARCH, Vec::<PathBuf>::new())
}

fn normalize_language(language: &str) -> String {
    match language.trim().to_ascii_lowercase().as_str() {
        "go" => "golang".to_string(),
        language => language.to_string(),
    }
}

#[derive(Debug)]
struct ChangeCountRow {
    codebase_id: String,
    codebase_name: String,
    file: String,
    change_count: usize,
}

fn resolve_revision(repo: &Repository, rev: &str) -> Result<Oid> {
    let object = repo
        .revparse_single(rev)
        .with_context(|| format!("failed to resolve revision {rev}"))?;
    Ok(object.peel_to_commit()?.id())
}

async fn resolve_codebase(
    pool: &PgPool,
    repo_path: &Path,
    head: Oid,
    language: Option<&str>,
) -> Result<Option<Codebase>> {
    let commit_hash = head.to_string();
    let mut codebases = list_codebases_by_commit(pool, &commit_hash)
        .await
        .with_context(|| format!("failed to look up codebase for commit {commit_hash}"))?;

    let found_codebases = !codebases.is_empty();
    if let Some(language) = language {
        codebases.retain(|codebase| normalize_language(&codebase.programming_language) == language);
    }

    match codebases.as_slice() {
        [] if found_codebases && language.is_some() => Ok(None),
        [] => bail!(
            "no codebase found in database for {} at HEAD commit {commit_hash}",
            repo_path.display()
        ),
        [codebase] => Ok(Some(codebase.clone())),
        _ => disambiguate_codebase(repo_path, &codebases)
            .map(Some)
            .with_context(|| {
                format!(
                    "multiple codebases found in database for {} at HEAD commit {commit_hash}",
                    repo_path.display()
                )
            }),
    }
}

fn disambiguate_codebase(repo_path: &Path, codebases: &[Codebase]) -> Result<Codebase> {
    let repo_path = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    let path_matches: Vec<_> = codebases
        .iter()
        .filter(|codebase| {
            let codebase_path = PathBuf::from(&codebase.url)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(&codebase.url));
            codebase_path == repo_path
        })
        .collect();
    if let Some(codebase) = single_match(&path_matches) {
        return Ok(codebase.clone());
    }

    let name_matches: Vec<_> = codebases
        .iter()
        .filter(|codebase| is_likely_codebase_name(repo_path.as_path(), &codebase.name))
        .collect();
    if let Some(codebase) = single_match(&name_matches) {
        return Ok(codebase.clone());
    }

    let candidates = codebases
        .iter()
        .map(|codebase| format!("{} ({})", codebase.name, codebase.id))
        .collect::<Vec<_>>()
        .join(", ");
    bail!("could not choose one codebase; candidates: {candidates}")
}

fn single_match<'a>(matches: &[&'a Codebase]) -> Option<&'a Codebase> {
    if matches.len() == 1 {
        Some(matches[0])
    } else {
        None
    }
}

fn is_likely_codebase_name(repo_path: &Path, name: &str) -> bool {
    let Some(base_name) = repo_path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    name == base_name
        || name == format!("{base_name} (version analysis)")
        || (name.starts_with(&format!("{base_name} (sample analysis: "))
            && name.ends_with(" samples)"))
}

fn count_file_changes(repo: &Repository, start: Oid) -> Result<HashMap<String, usize>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push(start)?;
    revwalk.simplify_first_parent()?;
    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

    let mut commits = revwalk.collect::<Result<Vec<_>, _>>()?;
    commits.reverse();

    let mut counts = HashMap::new();
    for oid in commits {
        let commit = repo.find_commit(oid)?;
        let new_tree = commit.tree()?;
        let old_tree = if commit.parent_count() == 0 {
            None
        } else {
            Some(commit.parent(0)?.tree()?)
        };

        let mut diff_options = DiffOptions::new();
        let mut diff =
            repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut diff_options))?;
        let mut find_options = DiffFindOptions::new();
        find_options.renames(true);
        diff.find_similar(Some(&mut find_options))?;

        let mut changed_paths = BTreeSet::new();
        for delta in diff.deltas() {
            if delta.status() == Delta::Renamed {
                add_renamed_delta(
                    &mut counts,
                    &mut changed_paths,
                    delta.old_file().path(),
                    delta.new_file().path(),
                );
            } else {
                add_delta_paths(
                    &mut changed_paths,
                    delta.status(),
                    delta.old_file().path(),
                    delta.new_file().path(),
                );
            }
        }

        for path in changed_paths {
            *counts.entry(path).or_insert(0) += 1;
        }
    }

    Ok(counts)
}

fn add_renamed_delta(
    counts: &mut HashMap<String, usize>,
    paths: &mut BTreeSet<String>,
    old_path: Option<&Path>,
    new_path: Option<&Path>,
) {
    let Some(new_path) = path_string(new_path) else {
        add_path(paths, old_path);
        return;
    };

    if let Some(old_path) = path_string(old_path)
        && old_path != new_path
        && let Some(old_count) = counts.remove(&old_path)
    {
        *counts.entry(new_path.clone()).or_insert(0) += old_count;
    }

    paths.insert(new_path);
}

fn add_delta_paths(
    paths: &mut BTreeSet<String>,
    status: Delta,
    old_path: Option<&Path>,
    new_path: Option<&Path>,
) {
    match status {
        Delta::Deleted => add_path(paths, old_path),
        Delta::Renamed | Delta::Copied => {
            add_path(paths, old_path);
            add_path(paths, new_path);
        }
        _ => add_path(paths, new_path.or(old_path)),
    }
}

fn add_path(paths: &mut BTreeSet<String>, path: Option<&Path>) {
    if let Some(path) = path_string(path) {
        paths.insert(path);
    }
}

fn path_string(path: Option<&Path>) -> Option<String> {
    path.map(|path| path.to_string_lossy().into_owned())
}

fn write_csv(output: &Path, rows: &[ChangeCountRow]) -> Result<()> {
    let file = File::create(output)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "codebase_id,codebase_name,file,change_count")?;

    let mut rows: Vec<_> = rows.iter().collect();
    rows.sort_by(|left, right| {
        left.codebase_id
            .cmp(&right.codebase_id)
            .then_with(|| right.change_count.cmp(&left.change_count))
            .then_with(|| left.file.cmp(&right.file))
    });

    for row in rows {
        writeln!(
            writer,
            "{},{},{},{}",
            csv_escape(&row.codebase_id),
            csv_escape(&row.codebase_name),
            csv_escape(&row.file),
            row.change_count
        )?;
    }

    Ok(())
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
