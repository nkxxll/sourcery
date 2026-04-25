use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Read},
    ops::Range,
};

use crate::language::{CodeSpan, LanguageConfig};
use tree_sitter::Node;

use anyhow::Result;

pub struct LinesOfCodeProcessor;

#[derive(Debug)]
pub struct FunctionLocResult {
    pub name: String,
    pub start_line: i32,
    pub end_line: i32,
    pub loc: usize,
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

pub struct CyclomaticComplexityProcessor;

#[derive(Default)]
struct CyclomaticTraversalState {
    control_flow: u64,
    match_constructs: u64,
    match_arms: u64,
    boolean_operators: u64,
}

impl CyclomaticComplexityProcessor {
    /// inspired from https://github.com/StrangeDaysTech/arborist
    /// Compute cyclomatic complexity for a function body.
    ///
    /// Starts at 1 (base path). Each decision point adds +1:
    /// if, else if, for, while, do-while, match/switch arm,
    /// catch/except, &&, ||, ternary operator.
    ///
    /// Note: `else` is NOT a decision point and does not increment cyclomatic complexity.
    pub fn compute_cyclomatic(
        node: &Node,
        profile: &LanguageConfig,
        match_range: Option<CodeSpan>,
    ) -> u64 {
        let counts = Self::collect_cyclomatic_counts(node, profile, match_range);

        // Base path + all decision points.
        1 + counts.control_flow + counts.match_arms + counts.boolean_operators
    }

    /// Variant of cyclomatic complexity where each match/switch construct counts as one branch,
    /// independent from how many case arms it contains.
    pub fn compute_cyclomatic_match_as_single_branch(
        node: &Node,
        profile: &LanguageConfig,
        match_range: Option<CodeSpan>,
    ) -> u64 {
        let counts = Self::collect_cyclomatic_counts(node, profile, match_range);

        1 + counts.control_flow + counts.match_constructs + counts.boolean_operators
    }

    fn collect_cyclomatic_counts(
        node: &Node,
        profile: &LanguageConfig,
        match_range: Option<CodeSpan>,
    ) -> CyclomaticTraversalState {
        let match_constructs: HashSet<&str> = profile
            .match_construct_nodes
            .iter()
            .map(String::as_str)
            .collect();

        let control_flow: HashSet<&str> = profile
            .control_flow_nodes
            .iter()
            .map(String::as_str)
            .filter(|kind| !match_constructs.contains(kind))
            .collect();

        let match_arms: HashSet<&str> =
            profile.match_arm_nodes.iter().map(String::as_str).collect();
        let boolean_operators: HashSet<&str> = profile
            .boolean_operators
            .iter()
            .map(String::as_str)
            .collect();

        let mut state = CyclomaticTraversalState::default();
        let range = match_range.map(Into::into);
        Self::traverse_cyclomatic(
            *node,
            &control_flow,
            &match_constructs,
            &match_arms,
            &boolean_operators,
            range.as_ref(),
            &mut state,
        );

        state
    }

    fn traverse_cyclomatic(
        node: Node,
        control_flow: &HashSet<&str>,
        match_constructs: &HashSet<&str>,
        match_arms: &HashSet<&str>,
        boolean_operators: &HashSet<&str>,
        range: Option<&Range<usize>>,
        state: &mut CyclomaticTraversalState,
    ) {
        if let Some(range) = range {
            if !Self::node_overlaps_range(node, range) {
                return;
            }
        }

        let should_count = range.is_none_or(|range| Self::node_within_range(node, range));
        if should_count {
            let kind = node.kind();
            if control_flow.contains(kind) {
                state.control_flow += 1;
            }
            if match_constructs.contains(kind) {
                state.match_constructs += 1;
            }
            if match_arms.contains(kind) {
                state.match_arms += 1;
            }
            if boolean_operators.contains(kind) {
                state.boolean_operators += 1;
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::traverse_cyclomatic(
                child,
                control_flow,
                match_constructs,
                match_arms,
                boolean_operators,
                range,
                state,
            );
        }
    }

    fn node_within_range(node: Node, range: &Range<usize>) -> bool {
        node.start_byte() >= range.start && node.end_byte() <= range.end
    }

    fn node_overlaps_range(node: Node, range: &Range<usize>) -> bool {
        node.end_byte() > range.start && node.start_byte() < range.end
    }
}

#[cfg(test)]
mod tests {
    use super::{CyclomaticComplexityProcessor, LinesOfCodeProcessor};
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
    fn cyclomatic_complexity_is_one_for_straight_line_python_function() {
        let source = r#"
def identity(value):
    result = value + 1
    return result
"#;
        let tree = parse_python(source);
        let profile = LanguageConfig::new(ProgrammingLanguage::Python);

        let complexity =
            CyclomaticComplexityProcessor::compute_cyclomatic(&tree.root_node(), &profile, None);

        assert_eq!(complexity, 1);
    }

    #[test]
    fn cyclomatic_complexity_counts_python_decision_points() {
        let source = r#"
def analyze(x, values):
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

        let complexity =
            CyclomaticComplexityProcessor::compute_cyclomatic(&tree.root_node(), &profile, None);

        assert_eq!(complexity, 10);
    }

    #[test]
    fn cyclomatic_complexity_match_counts_construct_once() {
        let source = r#"
def analyze(x, values):
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

        let complexity = CyclomaticComplexityProcessor::compute_cyclomatic_match_as_single_branch(
            &tree.root_node(),
            &profile,
            None,
        );

        assert_eq!(complexity, 9);
    }
}
