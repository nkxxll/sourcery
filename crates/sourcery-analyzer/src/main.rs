use clap::Parser;
use sourcery_analyzer::analyze_git_repository;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CommandLineInterface {
    url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CommandLineInterface::parse();
    analyze_git_repository(&args.url).await?;
    Ok(())
}
