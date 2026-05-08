use std::{collections::HashMap, collections::HashSet, ops::Range, path::Path};

use anyhow::{Result, anyhow};
use tracing::warn;
use tree_sitter::{Node, Tree};

use crate::language::{CodeByteSpan, LanguageConfig};

pub struct Processor<'processor> {
    lc: &'processor LanguageConfig,
    source: String,
    new_line_map: NewLineMap,
}

impl<'processor> Processor<'processor> {
    pub fn new(lc: &'processor LanguageConfig, path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path)?;
        Ok(Self::from_source(lc, source))
    }

    pub fn from_source(lc: &'processor LanguageConfig, source: impl Into<String>) -> Self {
        let source = source.into();
        let new_line_map = NewLineMap::new(&source);
        Self {
            lc,
            source,
            new_line_map,
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn new_line_map(&self) -> &NewLineMap {
        &self.new_line_map
    }

    /// analyze is where all analysis happens for a single file.
    pub fn analyze(&self) -> Result<Analysis> {
        let tree = self.lc.parse_tree(&self.source)?;
        let ast_analysis = AstProcessor::analyze_tree(&tree, self.lc, &self.new_line_map)?;
        let lines_of_code = self.new_line_map.line_count() as u64;
        let comment_lines_of_code = ast_analysis
            .comments
            .iter()
            .map(|comment| comment.length)
            .sum::<usize>() as u64;
        let effective_lines_of_code = lines_of_code.saturating_sub(comment_lines_of_code);
        let total_cyclomatic = ast_analysis
            .functions
            .iter()
            .map(|func| func.cyclomatic)
            .sum::<u64>();

        Ok(Analysis {
            ast_analysis,
            lines_of_code,
            effective_lines_of_code,
            comment_lines_of_code,
            total_cyclomatic,
        })
    }
}

#[derive(Debug)]
pub struct CommentAnalysis {
    pub comment_span: CodeByteSpan,
    pub comment_line_span: CodeLineSpan,
    /// Length in lines gathered by the newline map.
    pub length: usize,
}

#[derive(Debug)]
pub struct FunctionAnalysis {
    pub name: CodeByteSpan,
    pub definition: CodeByteSpan,
    pub definition_line_span: CodeLineSpan,
    pub function_length: usize,
    pub cyclomatic: u64,
    pub cyclomatic_match_as_single_branch: u64,
}

#[derive(Debug)]
pub struct Analysis {
    pub ast_analysis: AstAnalysis,
    pub lines_of_code: u64,
    pub effective_lines_of_code: u64,
    pub comment_lines_of_code: u64,
    pub total_cyclomatic: u64,
}
impl Analysis {
    pub(crate) fn pretty_print(&self, source: &str) -> String {
        // reduce perf hit by growing
        let mut res = String::with_capacity(1024);

        res.push_str(&format!("lines_of_code: {}\n", self.lines_of_code));
        res.push_str(&format!(
            "effective_lines_of_code: {}\n",
            self.effective_lines_of_code
        ));
        res.push_str(&format!(
            "comment_lines_of_code: {}\n",
            self.comment_lines_of_code
        ));
        res.push_str(&format!("total_cyclomatic: {}\n", self.total_cyclomatic));

        res.push_str("functions:\n");
        for function in &self.ast_analysis.functions {
            let name = function
                .name
                .get_content(source)
                .unwrap_or_else(|_| "<invalid span>".to_string());
            res.push_str(&format!(
                "  - {}:{}..{} length={} cyclomatic={} cyclomatic_match_as_single_branch={}\n",
                name,
                function.definition_line_span.start_line,
                function.definition_line_span.end_line,
                function.function_length,
                function.cyclomatic,
                function.cyclomatic_match_as_single_branch,
            ));
        }

        res.push_str("comments:\n");
        for comment in &self.ast_analysis.comments {
            let snippet = comment
                .comment_span
                .get_content(source)
                .map(|content| content.replace('\n', "\\n"))
                .map(|content| {
                    const MAX_CHARS: usize = 80;
                    if content.chars().count() > MAX_CHARS {
                        let truncated: String = content.chars().take(MAX_CHARS).collect();
                        format!("{truncated}...")
                    } else {
                        content
                    }
                })
                .unwrap_or_else(|_| "<invalid span>".to_string());
            res.push_str(&format!(
                "  - {}..{} length={} text={:?}\n",
                comment.comment_line_span.start_line,
                comment.comment_line_span.end_line,
                comment.length,
                snippet,
            ));
        }

        res
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FileMetrics {
    pub lines_of_code: u64,
    pub effective_lines_of_code: u64,
    pub comment_lines_of_code: u64,
    pub total_cyclomatic: u64,
}

impl FileMetrics {
    pub fn from_json(metrics: &serde_json::Value) -> Self {
        Self {
            lines_of_code: metrics
                .get("lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            effective_lines_of_code: metrics
                .get("effective_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            comment_lines_of_code: metrics
                .get("comment_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_cyclomatic: metrics
                .get("total_cyclomatic")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AggregatedFileMetrics {
    pub files: u64,
    pub total_lines_of_code: u64,
    pub total_effective_lines_of_code: u64,
    pub total_comment_lines_of_code: u64,
    pub total_cyclomatic: u64,
}

impl AggregatedFileMetrics {
    pub fn add_file_metrics(&mut self, metrics: &FileMetrics) {
        self.files += 1;
        self.total_lines_of_code += metrics.lines_of_code;
        self.total_effective_lines_of_code += metrics.effective_lines_of_code;
        self.total_comment_lines_of_code += metrics.comment_lines_of_code;
        self.total_cyclomatic += metrics.total_cyclomatic;
    }

    fn mean(total: u64, files: u64) -> f64 {
        if files == 0 {
            0.0
        } else {
            total as f64 / files as f64
        }
    }

    pub fn from_json(metrics: &serde_json::Value) -> Self {
        Self {
            files: metrics
                .get("files")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_lines_of_code: metrics
                .get("total_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_effective_lines_of_code: metrics
                .get("total_effective_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_comment_lines_of_code: metrics
                .get("total_comment_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_cyclomatic: metrics
                .get("total_cyclomatic")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "files": self.files,
            "total_lines_of_code": self.total_lines_of_code,
            "total_effective_lines_of_code": self.total_effective_lines_of_code,
            "total_comment_lines_of_code": self.total_comment_lines_of_code,
            "total_cyclomatic": self.total_cyclomatic,
            "mean_lines_of_code_per_file": Self::mean(self.total_lines_of_code, self.files),
            "mean_effective_lines_of_code_per_file": Self::mean(self.total_effective_lines_of_code, self.files),
            "mean_comment_lines_of_code_per_file": Self::mean(self.total_comment_lines_of_code, self.files),
            "mean_cyclomatic_complexity_per_file": Self::mean(self.total_cyclomatic, self.files),
        })
    }

    pub fn from_file_metrics_map(file_metrics_by_path: &HashMap<String, FileMetrics>) -> Self {
        let mut aggregated = Self::default();
        for metrics in file_metrics_by_path.values() {
            aggregated.add_file_metrics(metrics);
        }
        aggregated
    }

    pub fn reconcile(
        previous: AggregatedFileMetrics,
        old_metrics: AggregatedFileMetrics,
        new_metrics: AggregatedFileMetrics,
    ) -> AggregatedFileMetrics {
        AggregatedFileMetrics {
            files: previous
                .files
                .saturating_sub(old_metrics.files)
                .saturating_add(new_metrics.files),
            total_lines_of_code: previous
                .total_lines_of_code
                .saturating_sub(old_metrics.total_lines_of_code)
                .saturating_add(new_metrics.total_lines_of_code),
            total_effective_lines_of_code: previous
                .total_effective_lines_of_code
                .saturating_sub(old_metrics.total_effective_lines_of_code)
                .saturating_add(new_metrics.total_effective_lines_of_code),
            total_comment_lines_of_code: previous
                .total_comment_lines_of_code
                .saturating_sub(old_metrics.total_comment_lines_of_code)
                .saturating_add(new_metrics.total_comment_lines_of_code),
            total_cyclomatic: previous
                .total_cyclomatic
                .saturating_sub(old_metrics.total_cyclomatic)
                .saturating_add(new_metrics.total_cyclomatic),
        }
    }
}

#[derive(Debug)]
pub struct AstAnalysis {
    pub functions: Vec<FunctionAnalysis>,
    pub comments: Vec<CommentAnalysis>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeLineSpan {
    pub start_line: usize,
    pub end_line: usize,
}

impl CodeLineSpan {
    pub fn line_length(&self) -> usize {
        return self.end_line - self.start_line;
    }
}

#[derive(Debug)]
pub struct NewLineMap {
    newline_offsets: Vec<usize>,
    source_len: usize,
    ends_with_newline: bool,
}

impl NewLineMap {
    pub fn new(source: &str) -> Self {
        let newline_offsets = source
            .as_bytes()
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'\n').then_some(index))
            .collect();

        Self {
            newline_offsets,
            source_len: source.len(),
            ends_with_newline: source.ends_with('\n'),
        }
    }

    pub fn line_count(&self) -> usize {
        if self.source_len == 0 {
            return 0;
        }
        if self.ends_with_newline {
            self.newline_offsets.len()
        } else {
            self.newline_offsets.len() + 1
        }
    }

    pub fn get_line(&self, byte: usize) -> Option<usize> {
        if self.source_len == 0 || byte >= self.source_len {
            return None;
        }

        match self.newline_offsets.binary_search(&byte) {
            Ok(i) | Err(i) => Some(i),
        }
    }

    pub fn get_code_line_span(&self, code_byte_span: &CodeByteSpan) -> Result<CodeLineSpan> {
        let span: Range<usize> = (*code_byte_span).into();
        if span.end > self.source_len {
            return Err(anyhow!(
                "span end {} exceeds source length {}",
                span.end,
                self.source_len
            ));
        }
        let start_line = self
            .get_line(span.start)
            .ok_or_else(|| anyhow!("span start {} is out of bounds", span.start))?;
        let end_inclusive = span.end - 1;
        let end_line = self
            .get_line(end_inclusive)
            .ok_or_else(|| anyhow!("span end {} is out of bounds", end_inclusive))?;

        Ok(CodeLineSpan {
            start_line,
            end_line,
        })
    }

    pub fn count_lines(&self, code_byte_span: &CodeByteSpan) -> Result<usize> {
        let code_line_span = self.get_code_line_span(code_byte_span)?;
        Ok((code_line_span.end_line - code_line_span.start_line) + 1)
    }
}

pub struct AstProcessor;

#[derive(Default, Clone, Copy)]
struct CyclomaticCounts {
    control_flow: u64,
    match_constructs: u64,
    match_arms: u64,
    boolean_operators: u64,
}

impl CyclomaticCounts {
    fn add_from_kind(&mut self, kind: &str, classifier: &NodeKindClassifier<'_>) {
        if classifier.control_flow.contains(kind) {
            self.control_flow += 1;
        }
        if classifier.match_constructs.contains(kind) {
            self.match_constructs += 1;
        }
        if classifier.match_arms.contains(kind) {
            self.match_arms += 1;
        }
        if classifier.boolean_operators.contains(kind) {
            self.boolean_operators += 1;
        }
    }

    fn cyclomatic(self) -> u64 {
        1 + self.control_flow + self.match_arms + self.boolean_operators
    }

    fn cyclomatic_match_as_single_branch(self) -> u64 {
        1 + self.control_flow + self.match_constructs + self.boolean_operators
    }
}

#[derive(Default)]
struct AstTraversalState {
    functions: Vec<FunctionAnalysis>,
    comments: Vec<CommentAnalysis>,
    function_stack: Vec<FunctionFrame>,
}

struct FunctionFrame {
    function_index: usize,
    cyclomatic_counts: CyclomaticCounts,
}

struct NodeKindClassifier<'a> {
    function_nodes: HashSet<&'a str>,
    comment_nodes: HashSet<&'a str>,
    control_flow: HashSet<&'a str>,
    match_constructs: HashSet<&'a str>,
    match_arms: HashSet<&'a str>,
    boolean_operators: HashSet<&'a str>,
}

impl<'a> NodeKindClassifier<'a> {
    fn from_language(profile: &'a LanguageConfig) -> Self {
        let function_nodes = profile.function_nodes.iter().map(String::as_str).collect();
        let comment_nodes = profile.comment_nodes.iter().map(String::as_str).collect();
        let match_constructs: HashSet<&str> = profile
            .match_construct_nodes
            .iter()
            .map(String::as_str)
            .collect();
        let control_flow = profile
            .control_flow_nodes
            .iter()
            .map(String::as_str)
            .filter(|kind| !match_constructs.contains(kind))
            .collect();
        let match_arms = profile.match_arm_nodes.iter().map(String::as_str).collect();
        let boolean_operators = profile
            .boolean_operators
            .iter()
            .map(String::as_str)
            .collect();

        Self {
            function_nodes,
            comment_nodes,
            control_flow,
            match_constructs,
            match_arms,
            boolean_operators,
        }
    }
}

impl AstProcessor {
    pub fn analyze_tree(
        tree: &Tree,
        profile: &LanguageConfig,
        new_line_map: &NewLineMap,
    ) -> Result<AstAnalysis> {
        let classifier = NodeKindClassifier::from_language(profile);
        let mut state = AstTraversalState::default();
        Self::traverse(
            tree.root_node(),
            profile,
            &classifier,
            new_line_map,
            &mut state,
        )?;

        Ok(AstAnalysis {
            functions: state.functions,
            comments: state.comments,
        })
    }

    fn traverse(
        node: Node,
        profile: &LanguageConfig,
        classifier: &NodeKindClassifier<'_>,
        new_line_map: &NewLineMap,
        state: &mut AstTraversalState,
    ) -> Result<()> {
        let kind = node.kind();
        let mut entered_function = false;

        if classifier.function_nodes.contains(kind) {
            let Some(name_span) = profile.function_name_span(node) else {
                warn!("function node without expected name field: {}", kind);
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    Self::traverse(child, profile, classifier, new_line_map, state)?;
                }
                return Ok(());
            };
            let function_index = state.functions.len();
            let definition_span = LanguageConfig::node_span(node);
            let definition_line_span = new_line_map.get_code_line_span(&definition_span)?;
            state.functions.push(FunctionAnalysis {
                name: name_span,
                definition: definition_span,
                definition_line_span: definition_line_span,
                function_length: definition_line_span.line_length(),
                cyclomatic: 1,
                cyclomatic_match_as_single_branch: 1,
            });
            state.function_stack.push(FunctionFrame {
                function_index,
                cyclomatic_counts: CyclomaticCounts::default(),
            });
            entered_function = true;
        }

        if classifier.comment_nodes.contains(kind) || profile.is_doc_string_node(node) {
            let comment_span = LanguageConfig::node_span(node);
            let length = new_line_map.count_lines(&comment_span)?;
            state.comments.push(CommentAnalysis {
                comment_span,
                comment_line_span: new_line_map.get_code_line_span(&comment_span)?,
                length,
            });
        }

        for frame in &mut state.function_stack {
            frame.cyclomatic_counts.add_from_kind(kind, classifier);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::traverse(child, profile, classifier, new_line_map, state)?;
        }

        if entered_function {
            let Some(frame) = state.function_stack.pop() else {
                return Ok(());
            };
            let function = &mut state.functions[frame.function_index];
            function.cyclomatic = frame.cyclomatic_counts.cyclomatic();
            function.cyclomatic_match_as_single_branch =
                frame.cyclomatic_counts.cyclomatic_match_as_single_branch();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AggregatedFileMetrics, FileMetrics, NewLineMap, Processor};
    use crate::language::{CodeByteSpan, LanguageConfig, ProgrammingLanguage};
    use std::collections::HashMap;

    #[test]
    fn newline_map_counts_lines_without_trailing_newline() {
        let content = "first line\nsecond line\nthird line";
        let map = NewLineMap::new(content);

        assert_eq!(map.line_count(), 3);
    }

    #[test]
    fn newline_map_counts_lines_with_trailing_newline() {
        let content = "first line\nsecond line\nthird line\n";
        let map = NewLineMap::new(content);

        assert_eq!(map.line_count(), 3);
    }

    #[test]
    fn newline_map_counts_lines_for_code_span() {
        let content = "first line\nsecond line\nthird line";
        let map = NewLineMap::new(content);
        let start = content.find("second").unwrap();
        let end = content.find("third").unwrap() + "third line".len();
        let span = CodeByteSpan::new(start, end);

        let lines = map.count_lines(&span).unwrap();

        assert_eq!(lines, 2);
    }

    #[test]
    fn processor_uses_newline_map_for_file_and_comment_lines() {
        let source = "# module comment";
        let profile = LanguageConfig::new(ProgrammingLanguage::Python);
        let processor = Processor::from_source(&profile, source);

        let analysis = processor.analyze().unwrap();

        assert_eq!(analysis.lines_of_code, 1);
        assert_eq!(analysis.comment_lines_of_code, 1);
        assert_eq!(analysis.effective_lines_of_code, 0);
    }

    #[test]
    fn one_pass_analysis_collects_functions_comments_and_cyclomatic() {
        let source = r#"
"""module docs"""
# module comment
def analyze(x, values):
    """function docs"""
    if x > 10 and x < 20:
        return 1
    elif x == 0:
        return 2

    for value in values:
        if value % 2 == 0:
            return 3

    while x > 0:
        x -= 1

    match x:
        case 1:
            return 4
        case _:
            return 5

    return 6 if x < 0 else 7
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Python);
        let processor = Processor::from_source(&profile, source);

        let metrics = processor.analyze().unwrap().ast_analysis;

        assert_eq!(metrics.functions.len(), 1);
        assert_eq!(metrics.comments.len(), 3);
        assert_eq!(metrics.functions[0].cyclomatic, 10);
        assert_eq!(metrics.functions[0].cyclomatic_match_as_single_branch, 9);
    }

    #[test]
    fn one_pass_analysis_keeps_straight_line_function_at_one() {
        let source = r#"
def identity(value):
    result = value + 1
    return result
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Python);
        let processor = Processor::from_source(&profile, source);

        let metrics = processor.analyze().unwrap().ast_analysis;

        assert_eq!(metrics.functions.len(), 1);
        assert_eq!(metrics.functions[0].cyclomatic, 1);
        assert_eq!(metrics.functions[0].cyclomatic_match_as_single_branch, 1);
    }

    #[test]
    fn aggregated_file_metrics_sums_file_metrics_map() {
        let old_metrics = HashMap::from([
            (
                "src/a.rs".to_string(),
                FileMetrics {
                    lines_of_code: 10,
                    effective_lines_of_code: 8,
                    comment_lines_of_code: 2,
                    total_cyclomatic: 3,
                },
            ),
            (
                "src/b.rs".to_string(),
                FileMetrics {
                    lines_of_code: 20,
                    effective_lines_of_code: 15,
                    comment_lines_of_code: 5,
                    total_cyclomatic: 7,
                },
            ),
        ]);

        let aggregated = AggregatedFileMetrics::from_file_metrics_map(&old_metrics);

        assert_eq!(aggregated.files, 2);
        assert_eq!(aggregated.total_lines_of_code, 30);
        assert_eq!(aggregated.total_effective_lines_of_code, 23);
        assert_eq!(aggregated.total_comment_lines_of_code, 7);
        assert_eq!(aggregated.total_cyclomatic, 10);
    }

    #[test]
    fn aggregated_file_metrics_reconciles_previous_with_old_and_new_metrics() {
        let previous = AggregatedFileMetrics {
            files: 3,
            total_lines_of_code: 60,
            total_effective_lines_of_code: 48,
            total_comment_lines_of_code: 12,
            total_cyclomatic: 18,
        };
        let old_metrics = AggregatedFileMetrics {
            files: 2,
            total_lines_of_code: 35,
            total_effective_lines_of_code: 28,
            total_comment_lines_of_code: 7,
            total_cyclomatic: 10,
        };
        let new_metrics = AggregatedFileMetrics {
            files: 2,
            total_lines_of_code: 30,
            total_effective_lines_of_code: 26,
            total_comment_lines_of_code: 4,
            total_cyclomatic: 9,
        };

        let reconciled = AggregatedFileMetrics::reconcile(previous, old_metrics, new_metrics);

        assert_eq!(reconciled.files, 3);
        assert_eq!(reconciled.total_lines_of_code, 55);
        assert_eq!(reconciled.total_effective_lines_of_code, 46);
        assert_eq!(reconciled.total_comment_lines_of_code, 9);
        assert_eq!(reconciled.total_cyclomatic, 17);
    }
}
