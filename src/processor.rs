use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Read},
};

use crate::language::{CodeSpan, LanguageConfig};
use tracing::warn;
use tree_sitter::{Node, Tree};

use anyhow::Result;

pub struct LinesOfCodeProcessor;

#[derive(Debug)]
pub struct FunctionLocResult {
    pub name: String,
    pub start_line: i32,
    pub end_line: i32,
    pub loc: usize,
}

#[derive(Debug)]
pub struct FunctionAnalysis {
    pub name: CodeSpan,
    pub definition: CodeSpan,
    pub cyclomatic: u64,
    pub cyclomatic_match_as_single_branch: u64,
}

#[derive(Debug)]
pub struct AstAnalysis {
    pub functions: Vec<FunctionAnalysis>,
    pub comments: Vec<CodeSpan>,
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
    comments: Vec<CodeSpan>,
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

const CHUNK_SIZE: usize = 1 << 16; // 64KB

impl LinesOfCodeProcessor {
    /// counts the lines of content
    /// plus one because the content is without the last line break
    pub fn lines_of_code_content(content: &str) -> Result<u64> {
        if content.is_empty() {
            return Ok(0);
        }
        let mut reader = BufReader::new(content.as_bytes());
        Ok(1 + Self::count_lines_from_reader(&mut reader)?)
    }

    pub fn effective_lines_of_code_content(content: &str) -> Result<u64> {
        if content.is_empty() {
            return Ok(0);
        }
        let mut reader = BufReader::new(content.as_bytes());
        Ok(1 + Self::count_effective_lines_from_reader(&mut reader)?)
    }

    pub fn count_effective_lines_from_reader<R: BufRead>(reader: &mut R) -> Result<u64> {
        let mut count = 0;
        let mut line = String::new();
        loop {
            let bytes_read = reader.read_line(&mut line)?;
            if bytes_read == 0 {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                line.clear();
                continue;
            }
            line.clear();
            count += 1;
        }
        Ok(count)
    }

    pub fn count_lines_from_reader<R: Read>(reader: &mut R) -> Result<u64> {
        let mut buffer = [0u8; CHUNK_SIZE];
        let mut count = 0;

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            count += bytecount::count(&buffer[..bytes_read], b'\n') as u64;
        }

        Ok(count)
    }
}

impl AstProcessor {
    pub fn analyze_tree(tree: &Tree, profile: &LanguageConfig) -> AstAnalysis {
        let classifier = NodeKindClassifier::from_language(profile);
        let mut state = AstTraversalState::default();
        Self::traverse(tree.root_node(), profile, &classifier, &mut state);

        AstAnalysis {
            functions: state.functions,
            comments: state.comments,
        }
    }

    fn traverse(
        node: Node,
        profile: &LanguageConfig,
        classifier: &NodeKindClassifier<'_>,
        state: &mut AstTraversalState,
    ) {
        let kind = node.kind();
        let mut entered_function = false;

        if classifier.function_nodes.contains(kind) {
            let Some(name_span) = profile.function_name_span(node) else {
                warn!("function node without expected name field: {}", kind);
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    Self::traverse(child, profile, classifier, state);
                }
                return;
            };
            let function_index = state.functions.len();
            state.functions.push(FunctionAnalysis {
                name: name_span,
                definition: LanguageConfig::node_span(node),
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
            state.comments.push(LanguageConfig::node_span(node));
        }

        for frame in &mut state.function_stack {
            frame.cyclomatic_counts.add_from_kind(kind, classifier);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::traverse(child, profile, classifier, state);
        }

        if entered_function {
            let Some(frame) = state.function_stack.pop() else {
                return;
            };
            let function = &mut state.functions[frame.function_index];
            function.cyclomatic = frame.cyclomatic_counts.cyclomatic();
            function.cyclomatic_match_as_single_branch =
                frame.cyclomatic_counts.cyclomatic_match_as_single_branch();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AstProcessor, LinesOfCodeProcessor};
    use crate::language::{LanguageConfig, ProgrammingLanguage};
    use std::io::Cursor;
    use tree_sitter::Parser;

    fn parse_python(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .expect("python language must load");
        parser
            .parse(source, None)
            .expect("python source must parse")
    }

    #[test]
    fn count_lines_from_reader_counts_newlines() {
        let content = "first line\nsecond line\nthird line\n";
        let mut reader = Cursor::new(content.as_bytes());

        let line_breaks = LinesOfCodeProcessor::count_lines_from_reader(&mut reader).unwrap();

        assert_eq!(line_breaks, 3);
    }

    #[test]
    fn lines_of_code_content_counts_lines_without_final_newline() {
        let content = "first line\nsecond line\nthird line";

        let lines = LinesOfCodeProcessor::lines_of_code_content(content).unwrap();

        assert_eq!(lines, 3);
    }

    #[test]
    fn count_effective_lines_ignores_blank_and_whitespace_only_lines() {
        let content = "first line\n\n   \nsecond line\n\t\nthird line\n";
        let mut reader = Cursor::new(content.as_bytes());

        let effective =
            LinesOfCodeProcessor::count_effective_lines_from_reader(&mut reader).unwrap();

        assert_eq!(effective, 3);
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
        let tree = parse_python(source);
        let profile = LanguageConfig::new(ProgrammingLanguage::Python);

        let metrics = AstProcessor::analyze_tree(&tree, &profile);

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
        let tree = parse_python(source);
        let profile = LanguageConfig::new(ProgrammingLanguage::Python);

        let metrics = AstProcessor::analyze_tree(&tree, &profile);

        assert_eq!(metrics.functions.len(), 1);
        assert_eq!(metrics.functions[0].cyclomatic, 1);
        assert_eq!(metrics.functions[0].cyclomatic_match_as_single_branch, 1);
    }
}
