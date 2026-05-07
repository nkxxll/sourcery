use clap::Parser;
use sourcery_analyzer::{analyze_git_repository_with_database, language::ProgrammingLanguage};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CommandLineInterface {
    url: String,
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,
    programming_language: Option<ProgrammingLanguage>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    let args = CommandLineInterface::parse();
    analyze_git_repository_with_database(&args.url, args.programming_language, &args.database_url)
        .await?;
    Ok(())
}
