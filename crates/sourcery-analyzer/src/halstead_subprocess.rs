use std::{path::Path, process::Stdio};
use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{io::AsyncWriteExt, process::Command};

use crate::{language::ProgrammingLanguage, processor::FunctionAnalysis};

const OCAML_HALSTEAD: &str = "ocamlhalstead";
const GO_HALSTEAD: &str = "gohalstead";

#[derive(Debug, Default, Serialize, Deserialize)]
/// JSON payload returned by the halstead helper process.
pub struct HalsteadMetricsResponse {
    pub totals: HalsteadMetrics,
    pub functions: Vec<Functions>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
/// Per-function metrics returned by the helper process.
pub struct Functions {
    name: String,
    metrics: HalsteadMetrics,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Halstead counts for a single file or function.
pub struct HalsteadMetrics {
    pub unique_operators: usize,
    pub unique_operands: usize,
    pub operands: usize,
    pub operators: usize,
}

impl fmt::Display for HalsteadMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unique_operators={} unique_operands={} operators={} operands={}",
            self.unique_operators, self.unique_operands, self.operators, self.operands
        )
    }
}

pub struct HalsteadService<'halstead_service> {
    file: &'halstead_service Path,
    functions: &'halstead_service Vec<FunctionAnalysis>,
    language: ProgrammingLanguage,
}

impl<'halstead_service> HalsteadService<'halstead_service> {
    /// Creates a service that prepares input, runs the helper, and applies the response.
    pub fn new(
        file: &'halstead_service Path,
        functions: &'halstead_service Vec<FunctionAnalysis>,
        language: ProgrammingLanguage,
    ) -> Self {
        HalsteadService {
            file,
            functions,
            language,
        }
    }
    /// Builds the helper input format: file path followed by `name:start:end` lines.
    fn generate_input(&self) -> String {
        let absolute_file_path = &self
            .file
            .canonicalize()
            .unwrap_or_else(|_| self.file.to_path_buf());
        let mut sb = String::with_capacity(512);
        sb.push_str(absolute_file_path.to_string_lossy().as_ref());
        for function in self.functions {
            sb.push('\n');
            sb.push_str(&Self::function_to_input_line(&function));
        }
        sb.push('\n');
        sb
    }

    /// Formats one function into the helper's expected `name:start:end` line.
    fn function_to_input_line(function: &FunctionAnalysis) -> String {
        let name = function.function_name.to_string();
        let start_line = function.definition_line_span.start_line;
        let end_line = function.definition_line_span.end_line;
        format!("{}:{}:{}", name, start_line, end_line)
    }

    /// Runs the external halstead binary and returns its stdout as UTF-8 text.
    async fn spawn_process(&self) -> Result<String> {
        let program = match self.language {
            ProgrammingLanguage::Golang => GO_HALSTEAD,
            ProgrammingLanguage::Ocaml => OCAML_HALSTEAD,
            _ => todo!("this programming language has no halstead process"),
        };
        let mut child = Command::new(program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("could not spawn halstead process");
        let mut stdin = child.stdin.take().expect("could not take stdin");

        let input = self.generate_input();
        stdin.write_all(input.as_bytes()).await?;
        drop(stdin);
        let output: String = String::from_utf8(
            child
                .wait_with_output()
                .await
                .expect("failed waiting for output")
                .stdout,
        )?;
        Ok(output)
    }

    /// Enriches the owned function list with halstead metrics and returns the result.
    pub async fn compute_halstead_metrics(&self) -> Result<HalsteadMetricsResponse> {
        let output = self.spawn_process().await?;
        let response: HalsteadMetricsResponse = serde_json::from_str(output.as_ref())?;
        Ok(response)
    }
}

/// Runs halstead analysis and returns the totals plus the enriched functions.
pub async fn compute_halstead_metrics(
    file: &Path,
    functions: &Vec<FunctionAnalysis>,
    language: ProgrammingLanguage,
) -> Result<HalsteadMetricsResponse> {
    HalsteadService::new(file, functions, language)
        .compute_halstead_metrics()
        .await
}

/// Returns a new function list with halstead metrics merged in.
pub fn apply_halstead_to_functions(
    functions: &mut Vec<FunctionAnalysis>,
    halstead: &HalsteadMetricsResponse,
) {
    for function in functions {
        if let Some(function_halstead) = halstead
            .functions
            .iter()
            .find(|f| f.name == function.function_name)
        {
            function.halstead = Some(function_halstead.metrics);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_generate_input_format() {
        let functions = vec![FunctionAnalysis {
            function_name: "foo".into(),
            name: crate::language::CodeByteSpan::new(0, 1),
            definition: crate::language::CodeByteSpan::new(0, 1),
            definition_line_span: crate::processor::CodeLineSpan {
                start_line: 1,
                end_line: 2,
            },
            definition_position_range: crate::processor::CodePositionRange::default(),
            function_length: 1,
            cyclomatic: 0,
            cyclomatic_match_as_single_branch: 0,
            functions_called: vec![],
            references: vec![],
            enriched_calls: vec![],
            halstead: None,
        }];

        let file = Path::new("myfile.go");
        let hal_serv = HalsteadService::new(&file, &functions, ProgrammingLanguage::Golang);
        let input = hal_serv.generate_input();
        let expected = format!("myfile.go\nfoo:1:2\n");
        assert_eq!(input, expected);
    }
}
