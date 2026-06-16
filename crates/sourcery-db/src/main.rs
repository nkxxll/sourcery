/// this is just a command line application that fires the sql queries so I can look at the results
use clap::{Parser, Subcommand};
use sourcery_db::{
    connect, count_snapshot_version_files_and_functions, count_version_files_and_functions,
    delete_codebase, get_codebase_by_id, get_codebase_by_name, get_diff_by_version,
    get_diff_with_changes_by_version, get_version_by_commit, get_version_by_id,
    get_version_by_sample_number, list_all_files_states, list_all_functions,
    list_analysis_metric_samples, list_codebases, list_files_by_version, list_functions_by_version,
    list_snapshot_file_states, list_snapshot_functions, list_versions_by_codebase,
    search_version_filenames, search_version_functions,
};
use uuid::Uuid;

const ANALYSIS_METRICS: [(&str, &str); 6] = [
    ("mean_outdegree_per_file", "Mean Outdegree/File"),
    ("mean_indegree_per_file", "Mean Indegree/File"),
    (
        "mean_cyclomatic_per_function_per_file",
        "Mean Function Cyclomatic/File",
    ),
    ("lines_of_code", "Lines Of Code/File"),
    ("effective_lines_of_code", "Effective LOC/File"),
    ("total_cyclomatic", "Total Cyclomatic/File"),
];

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
    /// delete one codebase and descendants by name
    DeleteCodebase {
        name: String,
    },
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
    VersionBySample {
        codebase_id: String,
        sample_number: i32,
    },
    CurrentFiles {
        version_id: String,
    },
    SnapshotFiles {
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
    SnapshotFunctions {
        version_id: String,
    },
    SearchFilenames {
        version_id: String,
        query: String,
        #[arg(long, default_value_t = 50)]
        limit: i32,
    },
    SearchFunctions {
        version_id: String,
        query: String,
        #[arg(long, default_value_t = 50)]
        limit: i32,
    },
    FileFunctionCount {
        version_id: String,
    },
    SnapshotFileFunctionCount {
        version_id: String,
    },
    /// CSV rows for the metrics shown by react-frontend/src/routes/analysis.tsx
    AnalysisMetricsCsv {
        #[arg(long, value_delimiter = ',', required = true)]
        codebase_ids: Vec<String>,
        #[arg(long, default_value_t = 10)]
        version: i64,
        #[arg(long, value_delimiter = ',')]
        metrics: Vec<String>,
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
        SubCommand::DeleteCodebase { name } => {
            let Some(codebase) = get_codebase_by_name(&pool, &name).await? else {
                anyhow::bail!("codebase not found: {name}");
            };

            let deleted = delete_codebase(&pool, codebase.id).await?;
            if !deleted {
                anyhow::bail!("failed to delete codebase: {name}");
            }

            println!("{}", serde_json::to_string_pretty(&codebase)?);
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
        SubCommand::SnapshotFiles { version_id } => {
            let version_id = Uuid::parse_str(&version_id)?;
            let files = list_snapshot_file_states(&pool, version_id).await?;
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
        SubCommand::VersionBySample {
            codebase_id,
            sample_number,
        } => {
            let id = Uuid::parse_str(&codebase_id)?;
            let metrics = get_version_by_sample_number(&pool, id, sample_number).await?;
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
        SubCommand::SnapshotFunctions { version_id } => {
            let id = Uuid::parse_str(&version_id)?;
            let metrics = list_snapshot_functions(&pool, id).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }
        SubCommand::SearchFilenames {
            version_id,
            query,
            limit,
        } => {
            let id = Uuid::parse_str(&version_id)?;
            let results = search_version_filenames(&pool, id, &query, limit).await?;
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        SubCommand::SearchFunctions {
            version_id,
            query,
            limit,
        } => {
            let id = Uuid::parse_str(&version_id)?;
            let results = search_version_functions(&pool, id, &query, limit).await?;
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        SubCommand::FileFunctionCount { version_id } => {
            let id = Uuid::parse_str(&version_id)?;
            let results = count_version_files_and_functions(&pool, id).await?;
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        SubCommand::SnapshotFileFunctionCount { version_id } => {
            let id = Uuid::parse_str(&version_id)?;
            let results = count_snapshot_version_files_and_functions(&pool, id).await?;
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        SubCommand::AnalysisMetricsCsv {
            codebase_ids,
            version,
            metrics,
        } => {
            if !(1..=10).contains(&version) {
                anyhow::bail!("version must be between 1 and 10");
            }

            let codebase_ids = codebase_ids
                .iter()
                .map(|id| Uuid::parse_str(id))
                .collect::<Result<Vec<_>, _>>()?;
            let selected_metrics = selected_analysis_metrics(&metrics)?;

            println!(
                "metric_key,metric_label,codebase_id,codebase_name,programming_language,version_id,version_number,sample_number,file_path,value"
            );
            for (metric_key, metric_label) in selected_metrics {
                let rows =
                    list_analysis_metric_samples(&pool, &codebase_ids, version, metric_key).await?;
                for row in rows {
                    println!(
                        "{},{},{},{},{},{},{},{},{},{}",
                        csv_field(metric_key),
                        csv_field(metric_label),
                        csv_field(&row.codebase_id.to_string()),
                        csv_field(&row.codebase_name),
                        csv_field(&row.programming_language),
                        csv_field(&row.version_id.to_string()),
                        row.version_number,
                        row.sample_number,
                        csv_field(&row.file_path),
                        row.value,
                    );
                }
            }
        }
    }
    Ok(())
}

fn selected_analysis_metrics(
    metrics: &[String],
) -> anyhow::Result<Vec<(&'static str, &'static str)>> {
    if metrics.is_empty() {
        return Ok(ANALYSIS_METRICS.to_vec());
    }

    metrics
        .iter()
        .map(|metric| {
            ANALYSIS_METRICS
                .iter()
                .copied()
                .find(|(key, _)| key == metric)
                .ok_or_else(|| anyhow::anyhow!("unsupported analysis metric {metric}"))
        })
        .collect()
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
