use clap::{Parser, Subcommand};
use sourcery_analyzer::{
    analyze_git_repository_with_database, analyze_single_file, language::ProgrammingLanguage,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CommandLineInterface {
    #[command(subcommand)]
    command: SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    Repo {
        url: String,
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        programming_language: Option<ProgrammingLanguage>,
    },
    File {
        path: String,
        #[arg(long, default_value = "stats.txt")]
        outfile: String,
        programming_language: Option<ProgrammingLanguage>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sourcery_analyzer=info".into()),
        )
        .init();
    let cli = CommandLineInterface::parse();

    match cli.command {
        SubCommand::Repo {
            url,
            database_url,
            programming_language,
        } => {
            analyze_git_repository_with_database(&url, programming_language, &database_url).await?;
        }
        SubCommand::File {
            path,
            outfile,
            programming_language,
        } => {
            analyze_single_file(path, outfile, programming_language)?;
        }
    }

    Ok(())
}
