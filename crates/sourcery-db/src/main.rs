/// this is just a command line application that fires the sql queries so I can look at the results
use clap::{Parser, Subcommand};
use sourcery_db::{list_codebases, list_codebase_metrics, connect};
use uuid::Uuid;

#[derive(Parser)]
pub struct CommandLine {
    #[command(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    /// list of all codebases
    Codebases,
    /// list of metrics for one codebase
    CodebaseMetrics { id: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CommandLine::parse();
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable not set");
    let pool = connect(&database_url).await?;
    
    match args.subcommand {
        SubCommand::Codebases => {
            let codebases = list_codebases(&pool).await?;
            println!("{}", serde_json::to_string_pretty(&codebases)?);
        }
        SubCommand::CodebaseMetrics { id } => {
            let codebase_id = Uuid::parse_str(&id)?;
            let metrics = list_codebase_metrics(&pool, codebase_id).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
    }
    Ok(())
}
