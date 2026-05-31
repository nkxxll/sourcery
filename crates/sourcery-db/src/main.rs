/// this is just a command line application that fires the sql queries so I can look at the results
use clap::{Parser, Subcommand};
use sourcery_db::{
    connect, get_codebase_by_id, get_diff_by_version, get_diff_with_changes_by_version, get_version_by_commit, get_version_by_id, list_all_files_states, list_all_functions, list_codebases, list_files_by_version, list_functions_by_version, list_versions_by_codebase
};
use uuid::Uuid;

#[derive(Parser)]
pub struct CommandLine {
    #[command(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    /// data about one codebase
    Codebase {
        id: String,
    },
    /// list of all codebases
    Codebases,
    /// list of metrics for one codebase
    CodebaseMetrics {
        id: String,
    },
    Version {
        version_id: String,
    },
    VersionFiles {
        version_id: String,
    },
    VersionByCommit {
        codebase_id: String,
        commit_hash: String,
    },
    CurrentFiles {
        version_id: String,
    },
    Diff {
        version_id: String,
    },
    DiffChange {
        version_id: String,
    },
    Functions {
        version_id: String,
    },
    AllFunctions {
        version_id: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CommandLine::parse();
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
    let pool = connect(&database_url).await?;

    match args.subcommand {
        SubCommand::Codebase { id } => {
            let codebase_id = Uuid::parse_str(&id)?;
            let metrics = get_codebase_by_id(&pool, codebase_id).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        SubCommand::Codebases => {
            let codebases = list_codebases(&pool).await?;
            println!("{}", serde_json::to_string_pretty(&codebases)?);
        }
        SubCommand::CodebaseMetrics { id } => {
            let codebase_id = Uuid::parse_str(&id)?;
            let metrics = list_versions_by_codebase(&pool, codebase_id).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        SubCommand::Version { version_id } => {
            let id = Uuid::parse_str(&version_id)?;
            let metrics = get_version_by_id(&pool, id).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        SubCommand::VersionFiles { version_id } => {
            let id = Uuid::parse_str(&version_id)?;
            let files = list_files_by_version(&pool, id).await?;
            println!("{}", serde_json::to_string_pretty(&files)?);
        }
        SubCommand::CurrentFiles { version_id } => {
            let version_id = Uuid::parse_str(&version_id)?;
            let files = list_all_files_states(&pool, version_id).await?;
            println!("{}", serde_json::to_string_pretty(&files)?);
        }
        SubCommand::VersionByCommit {
            codebase_id,
            commit_hash,
        } => {
            let id = Uuid::parse_str(&codebase_id)?;
            let metrics = get_version_by_commit(&pool, id, &commit_hash).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        SubCommand::Diff { version_id } => {
            let id = Uuid::parse_str(&version_id)?;
            let metrics = get_diff_by_version(&pool, id).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        SubCommand::DiffChange { version_id } => {
            let id = Uuid::parse_str(&version_id)?;
            let metrics = get_diff_with_changes_by_version(&pool, id).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        SubCommand::Functions { version_id } => {
            let id = Uuid::parse_str(&version_id)?;
            let metrics = list_functions_by_version(&pool, id).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        SubCommand::AllFunctions { version_id } => {
            let id = Uuid::parse_str(&version_id)?;
            let metrics = list_all_functions(&pool, id).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
    }
    Ok(())
}
