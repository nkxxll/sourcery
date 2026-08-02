use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use tracing::debug;

use crate::processor::{Analysis, CodePosition, FunctionCall};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FunctionDefinition {
    file: PathBuf,
    name: String,
    start: CodePosition,
    end: CodePosition,
}

struct FunctionAndReferences {
    definition: FunctionDefinition,
    total_indegree: usize,
    total_outdegree: usize,
    unique_outdegree: usize,
    function_references: Vec<FunctionCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallgraphFunctionMetrics {
    pub file: PathBuf,
    pub name: String,
    pub start: CodePosition,
    pub end: CodePosition,
    pub total_indegree: usize,
    pub unique_indegree: usize,
    pub total_outdegree: usize,
    pub unique_outdegree: usize,
}

pub struct CallgraphCsvSample {
    pub codebase_id: String,
    pub codebase_name: String,
    pub programming_language: String,
    pub version_id: String,
    pub version_number: i32,
    pub sample_number: i32,
    pub sample_commit_hash: String,
    pub functions: Vec<CallgraphFunctionMetrics>,
}

type MetricDefinition = (
    &'static str,
    &'static str,
    fn(&CallgraphFunctionMetrics) -> usize,
);

const FUNCTION_METRICS: &[MetricDefinition] = &[
    ("indegree", "Function Indegree", |metrics| {
        metrics.total_indegree
    }),
    ("unique_indegree", "Function Unique Indegree", |metrics| {
        metrics.unique_indegree
    }),
    ("outdegree", "Function Outdegree", |metrics| {
        metrics.total_outdegree
    }),
    ("unique_outdegree", "Function Unique Outdegree", |metrics| {
        metrics.unique_outdegree
    }),
];

const FILE_METRICS: &[MetricDefinition] = &[
    (
        "total_indegree_per_file",
        "Total Indegree/File",
        |metrics| metrics.total_indegree,
    ),
    (
        "total_unique_indegree_per_file",
        "Total Unique Indegree/File",
        |metrics| metrics.unique_indegree,
    ),
    (
        "total_outdegree_per_file",
        "Total Outdegree/File",
        |metrics| metrics.total_outdegree,
    ),
    (
        "total_unique_outdegree_per_file",
        "Total Unique Outdegree/File",
        |metrics| metrics.unique_outdegree,
    ),
    ("mean_indegree_per_file", "Mean Indegree/File", |metrics| {
        metrics.total_indegree
    }),
    (
        "mean_unique_indegree_per_file",
        "Mean Unique Indegree/File",
        |metrics| metrics.unique_indegree,
    ),
    (
        "mean_outdegree_per_file",
        "Mean Outdegree/File",
        |metrics| metrics.total_outdegree,
    ),
    (
        "mean_unique_outdegree_per_file",
        "Mean Unique Outdegree/File",
        |metrics| metrics.unique_outdegree,
    ),
];

/// Formats call graph metrics using the same long-form file/function rows as the database export.
pub fn output_csv(samples: &[CallgraphCsvSample]) -> String {
    let mut output = String::from(
        "metric_key,metric_label,codebase_id,codebase_name,programming_language,version_id,version_number,sample_number,sample_commit_hash,metric_level,file_path,function_name,function_start_line,function_end_line,value\n",
    );

    for sample in samples {
        let mut functions_by_file = BTreeMap::<&Path, Vec<&CallgraphFunctionMetrics>>::new();
        for function in &sample.functions {
            functions_by_file
                .entry(&function.file)
                .or_default()
                .push(function);
        }

        for (file, file_functions) in functions_by_file {
            for (key, label, value) in FILE_METRICS {
                let total = file_functions
                    .iter()
                    .map(|function| value(function))
                    .sum::<usize>();
                let value = if key.starts_with("mean_") {
                    total as f64 / file_functions.len() as f64
                } else {
                    total as f64
                };
                output.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},file,{},,,,{}\n",
                    csv_field(key),
                    csv_field(label),
                    csv_field(&sample.codebase_id),
                    csv_field(&sample.codebase_name),
                    csv_field(&sample.programming_language),
                    csv_field(&sample.version_id),
                    sample.version_number,
                    sample.sample_number,
                    csv_field(&sample.sample_commit_hash),
                    csv_field(&file.display().to_string()),
                    value,
                ));
            }

            for function in file_functions {
                for (key, label, value) in FUNCTION_METRICS {
                    output.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},function,{},{},{},{},{}\n",
                        csv_field(key),
                        csv_field(label),
                        csv_field(&sample.codebase_id),
                        csv_field(&sample.codebase_name),
                        csv_field(&sample.programming_language),
                        csv_field(&sample.version_id),
                        sample.version_number,
                        sample.sample_number,
                        csv_field(&sample.sample_commit_hash),
                        csv_field(&function.file.display().to_string()),
                        csv_field(&function.name),
                        function.start.line,
                        function.end.line,
                        value(function),
                    ));
                }
            }
        }
    }

    output
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Caller {
    Function(FunctionDefinition),
    TopLevel(PathBuf),
}

/// Accumulates call graph information for every file in one repository version.
///
/// All analyses must be inserted before calculating unique indegree because LSP
/// references can point to callers in any file in the repository.
#[derive(Default)]
pub struct CallgraphData {
    function_index: BTreeMap<PathBuf, Vec<FunctionDefinition>>,
    function_list_with_references: Vec<FunctionAndReferences>,
}

impl CallgraphData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts the definitions and references found while analyzing one file.
    pub fn insert(&mut self, analysis: &Analysis) {
        let file = normalize_path(&analysis.file);

        for function in &analysis.functions {
            let definition = FunctionDefinition {
                file: file.clone(),
                name: function.function_name.to_string(),
                start: function.definition_position_range.start,
                end: function.definition_position_range.end,
            };

            let definitions = self.function_index.entry(file.clone()).or_default();
            if !definitions.contains(&definition) {
                definitions.push(definition.clone());
                definitions.sort_by(|left, right| {
                    left.start
                        .cmp(&right.start)
                        .then_with(|| right.end.cmp(&left.end))
                        .then_with(|| left.name.cmp(&right.name))
                });
            }

            let unique_outdegree = function
                .syntax_function_calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<BTreeSet<_>>()
                .len();
            let function_data = FunctionAndReferences {
                definition: definition.clone(),
                total_indegree: function.references.len(),
                total_outdegree: function.syntax_function_calls.len(),
                unique_outdegree,
                function_references: function.references.clone(),
            };

            if let Some(existing) = self
                .function_list_with_references
                .iter_mut()
                .find(|entry| entry.definition == definition)
            {
                debug!(
                    file = %definition.file.display(),
                    function = %definition.name,
                    start_line = definition.start.line,
                    start_column = definition.start.column,
                    end_line = definition.end.line,
                    end_column = definition.end.column,
                    existing_total_indegree = existing.total_indegree,
                    existing_total_outdegree = existing.total_outdegree,
                    existing_unique_outdegree = existing.unique_outdegree,
                    existing_reference_count = existing.function_references.len(),
                    replacement_total_indegree = function_data.total_indegree,
                    replacement_total_outdegree = function_data.total_outdegree,
                    replacement_unique_outdegree = function_data.unique_outdegree,
                    replacement_reference_count = function_data.function_references.len(),
                    "replaced duplicate function analysis"
                );
                *existing = function_data;
            } else {
                self.function_list_with_references.push(function_data);
            }
        }
    }

    /// Calculates unique indegree as the number of distinct enclosing callers.
    ///
    /// Repeated calls from one function count once. A call inside nested
    /// functions belongs to the innermost definition. References outside a
    /// function are grouped into one top-level caller per file.
    pub fn calculate_unique_indegree(&self) -> Vec<CallgraphFunctionMetrics> {
        let mut metrics = self
            .function_list_with_references
            .iter()
            .map(|function| {
                let unique_indegree = function
                    .function_references
                    .iter()
                    .map(|reference| self.caller_for_reference(reference))
                    .collect::<BTreeSet<_>>()
                    .len();

                CallgraphFunctionMetrics {
                    file: function.definition.file.clone(),
                    name: function.definition.name.clone(),
                    start: function.definition.start,
                    end: function.definition.end,
                    total_indegree: function.total_indegree,
                    unique_indegree,
                    total_outdegree: function.total_outdegree,
                    unique_outdegree: function.unique_outdegree,
                }
            })
            .collect::<Vec<_>>();

        metrics.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then_with(|| left.start.cmp(&right.start))
                .then_with(|| left.end.cmp(&right.end))
                .then_with(|| left.name.cmp(&right.name))
        });
        metrics
    }

    fn caller_for_reference(&self, reference: &FunctionCall) -> Caller {
        let file = normalize_path(&reference.file);
        let Some(definitions) = self.function_index.get(&file) else {
            return Caller::TopLevel(file);
        };

        definitions
            .iter()
            .filter(|definition| contains_position(definition, reference.pos))
            .max_by(|left, right| {
                left.start
                    .cmp(&right.start)
                    .then_with(|| right.end.cmp(&left.end))
            })
            .cloned()
            .map(Caller::Function)
            .unwrap_or(Caller::TopLevel(file))
    }
}

fn contains_position(definition: &FunctionDefinition, position: CodePosition) -> bool {
    definition.start <= position && position < definition.end
}

fn normalize_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(current_dir) = std::env::current_dir() {
        current_dir.join(path)
    } else {
        path.to_path_buf()
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{CallgraphCsvSample, CallgraphData, output_csv};
    use crate::{
        halstead_subprocess::HalsteadMetrics,
        language::CodeByteSpan,
        processor::{
            Analysis, CodeLineSpan, CodePosition, CodePositionRange, FunctionAnalysis, FunctionCall,
        },
    };
    use std::path::{Path, PathBuf};

    fn position(line: usize, column: usize) -> CodePosition {
        CodePosition { line, column }
    }

    fn reference(file: &str, line: usize, column: usize) -> FunctionCall {
        FunctionCall {
            name: "target".into(),
            pos: position(line, column),
            file: PathBuf::from(file),
        }
    }

    fn function(
        name: &str,
        start: CodePosition,
        end: CodePosition,
        references: Vec<FunctionCall>,
        calls: &[&str],
        file: &str,
    ) -> FunctionAnalysis {
        FunctionAnalysis {
            function_name: name.into(),
            name: CodeByteSpan::new(0, 1),
            definition: CodeByteSpan::new(0, 1),
            definition_line_span: CodeLineSpan {
                start_line: start.line,
                end_line: end.line,
            },
            definition_position_range: CodePositionRange { start, end },
            function_length: end.line - start.line,
            cyclomatic: 1,
            cyclomatic_match_as_single_branch: 1,
            syntax_function_calls: calls
                .iter()
                .enumerate()
                .map(|(index, name)| FunctionCall {
                    name: (*name).into(),
                    pos: position(start.line, start.column + index),
                    file: PathBuf::from(file),
                })
                .collect(),
            references,
            enriched_calls: Vec::new(),
            halstead: None,
            maintainability_index: None,
            function_arguments: Vec::new(),
        }
    }

    fn analysis(file: &str, functions: Vec<FunctionAnalysis>) -> Analysis {
        Analysis {
            file: PathBuf::from(file),
            functions,
            comments: Vec::new(),
            lines_of_code: 0,
            blank_lines: 0,
            bracket_lines_of_code: 0,
            comment_lines_of_code: 0,
            effective_lines_of_code_with_brackets: 0,
            effective_lines_of_code: 0,
            total_cyclomatic: 0,
            total_halstead: HalsteadMetrics::default(),
            maintainability_index: None,
        }
    }

    fn metric<'a>(
        metrics: &'a [super::CallgraphFunctionMetrics],
        file: &str,
        name: &str,
    ) -> &'a super::CallgraphFunctionMetrics {
        metrics
            .iter()
            .find(|metric| metric.file.ends_with(Path::new(file)) && metric.name == name)
            .unwrap()
    }

    #[test]
    fn insert_populates_all_direct_degree_metrics() {
        let mut graph = CallgraphData::new();
        graph.insert(&analysis(
            "/repo/target.go",
            vec![function(
                "target",
                position(1, 1),
                position(4, 2),
                vec![reference("/repo/caller.go", 2, 2)],
                &["one", "one", "two"],
                "/repo/target.go",
            )],
        ));

        let metrics = graph.calculate_unique_indegree();
        let target = metric(&metrics, "target.go", "target");
        assert_eq!(target.total_indegree, 1);
        assert_eq!(target.unique_indegree, 1);
        assert_eq!(target.total_outdegree, 3);
        assert_eq!(target.unique_outdegree, 2);
    }

    #[test]
    fn unique_indegree_uses_the_innermost_enclosing_function() {
        let mut graph = CallgraphData::new();
        graph.insert(&analysis(
            "/repo/caller.go",
            vec![
                function(
                    "outer",
                    position(1, 1),
                    position(10, 2),
                    Vec::new(),
                    &[],
                    "/repo/caller.go",
                ),
                function(
                    "inner",
                    position(4, 3),
                    position(7, 4),
                    Vec::new(),
                    &[],
                    "/repo/caller.go",
                ),
            ],
        ));
        graph.insert(&analysis(
            "/repo/target.go",
            vec![function(
                "target",
                position(1, 1),
                position(2, 2),
                vec![reference("/repo/caller.go", 5, 5)],
                &[],
                "/repo/target.go",
            )],
        ));

        let metrics = graph.calculate_unique_indegree();
        assert_eq!(metric(&metrics, "target.go", "target").unique_indegree, 1);
    }

    #[test]
    fn same_named_callers_in_different_files_are_distinct() {
        let mut graph = CallgraphData::new();
        for file in ["/repo/one.go", "/repo/two.go"] {
            graph.insert(&analysis(
                file,
                vec![function(
                    "caller",
                    position(1, 1),
                    position(3, 2),
                    Vec::new(),
                    &[],
                    file,
                )],
            ));
        }
        graph.insert(&analysis(
            "/repo/target.go",
            vec![function(
                "target",
                position(1, 1),
                position(2, 2),
                vec![
                    reference("/repo/one.go", 2, 2),
                    reference("/repo/two.go", 2, 2),
                ],
                &[],
                "/repo/target.go",
            )],
        ));

        let metrics = graph.calculate_unique_indegree();
        assert_eq!(metric(&metrics, "target.go", "target").unique_indegree, 2);
    }

    #[test]
    fn top_level_references_are_distinct_per_file() {
        let mut graph = CallgraphData::new();
        graph.insert(&analysis(
            "/repo/target.go",
            vec![function(
                "target",
                position(1, 1),
                position(2, 2),
                vec![
                    reference("/repo/one.go", 20, 1),
                    reference("/repo/one.go", 21, 1),
                    reference("/repo/two.go", 20, 1),
                ],
                &[],
                "/repo/target.go",
            )],
        ));

        let metrics = graph.calculate_unique_indegree();
        assert_eq!(metric(&metrics, "target.go", "target").unique_indegree, 2);
    }

    #[test]
    fn csv_contains_function_metrics_and_file_means() {
        let mut graph = CallgraphData::new();
        graph.insert(&analysis(
            "/repo/a,file.go",
            vec![
                function(
                    "one",
                    position(1, 1),
                    position(3, 2),
                    vec![reference("/repo/caller.go", 1, 1)],
                    &["a", "a"],
                    "/repo/a,file.go",
                ),
                function(
                    "two",
                    position(5, 1),
                    position(8, 2),
                    vec![
                        reference("/repo/caller.go", 1, 1),
                        reference("/repo/other.go", 1, 1),
                    ],
                    &["a", "b"],
                    "/repo/a,file.go",
                ),
            ],
        ));

        let csv = output_csv(&[CallgraphCsvSample {
            codebase_id: "codebase-id".into(),
            codebase_name: "example".into(),
            programming_language: "Go".into(),
            version_id: "version-id".into(),
            version_number: 7,
            sample_number: 3,
            sample_commit_hash: "abc123".into(),
            functions: graph.calculate_unique_indegree(),
        }]);

        assert!(csv.starts_with(
            "metric_key,metric_label,codebase_id,codebase_name,programming_language,version_id,version_number,sample_number,sample_commit_hash,metric_level,file_path,function_name,function_start_line,function_end_line,value\n"
        ));
        assert!(csv.contains(
            "mean_indegree_per_file,Mean Indegree/File,codebase-id,example,Go,version-id,7,3,abc123,file,\"/repo/a,file.go\",,,,1.5\n"
        ));
        assert!(csv.contains(
            "total_outdegree_per_file,Total Outdegree/File,codebase-id,example,Go,version-id,7,3,abc123,file,\"/repo/a,file.go\",,,,4\n"
        ));
        assert!(
            csv.contains("outdegree,Function Outdegree,codebase-id,example,Go,version-id,7,3,abc123,function,\"/repo/a,file.go\",one,1,3,2\n")
        );
    }
}
