use clap::{Parser, Subcommand};
use sourcery_analyzer::{
    analyze_git_repository_with_database, analyze_repo_samples, analyze_repo_version,
    analyze_single_file, language::ProgrammingLanguage,
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
    Version {
        path: String,
        programming_language: Option<ProgrammingLanguage>,
    },
    Sample {
        path: String,
        #[arg(long, default_value = "10")]
        samples: usize,
        programming_language: Option<ProgrammingLanguage>,
    },
    File {
        path: String,
        #[arg(long, default_value = "stats.txt")]
        outfile: String,
        programming_language: Option<ProgrammingLanguage>,
        #[arg(long)]
        root_dir: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sourcery_analyzer=info,sourcery_lsp_client=info".into()),
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
        SubCommand::Version {
            path,
            programming_language,
        } => {
            analyze_repo_version(path, programming_language).await?;
        }
        SubCommand::Sample {
            path,
            samples,
            programming_language,
        } => {
            analyze_repo_samples(path, samples, programming_language).await?;
        }
        SubCommand::File {
            path,
            outfile,
            programming_language,
            root_dir,
        } => {
            analyze_single_file(path, outfile, programming_language, root_dir).await?;
        }
    }

    Ok(())
}
