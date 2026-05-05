use clap::Parser;
use sourcery_analyzer::analyze_git_repository_with_database;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CommandLineInterface {
    url: String,
    #[arg(
        long,
        env = "DATABASE_URL",
        default_value = "postgres://localhost:5432/postgres"
    )]
    database_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CommandLineInterface::parse();
    analyze_git_repository_with_database(&args.url, &args.database_url).await?;
    Ok(())
}
