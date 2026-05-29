/// this is just a command line application that fires the sql queries so I can look at the results
use clap::{Parser, Subcommand};
use sourcery_db::{connect, get_codebase_by_id, list_codebases, list_versions_by_codebase};
use uuid::Uuid;

#[derive(Parser)]
pub struct CommandLine {
    #[command(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    /// data about one codebase
    Codebase { id: String },
    /// list of all codebases
    Codebases,
    /// list of metrics for one codebase
    CodebaseMetrics { id: String },
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
    }
    Ok(())
}
