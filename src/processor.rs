use std::io::{BufRead, BufReader, Read};

use crate::language::LanguageConfig;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

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
        let mut reader = BufReader::new(content.as_bytes());
        Ok(1 + Self::count_lines_from_reader(&mut reader)?)
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

impl CyclomaticComplexityProcessor {
    /// inspired from https://github.com/StrangeDaysTech/arborist
    /// Compute cyclomatic complexity for a function body.
    ///
    /// Starts at 1 (base path). Each decision point adds +1:
    /// if, else if, for, while, do-while, match/switch arm,
    /// catch/except, &&, ||, ternary operator.
    ///
    /// Note: `else` is NOT a decision point and does not increment cyclomatic complexity.
    pub fn compute_cyclomatic(node: &Node, source: &[u8], profile: &LanguageConfig) -> u64 {
        let match_constructs = &profile.match_construct_nodes;
        let control_flow: Vec<String> = profile
            .control_flow_nodes
            .iter()
            .filter(|kind| !match_constructs.contains(*kind))
            .cloned()
            .collect();

        // Base path + all decision points.
        1 + Self::count_keyword_matches(node, source, &control_flow)
            + Self::count_keyword_matches(node, source, &profile.match_arm_nodes)
            + Self::count_keyword_matches(node, source, &profile.boolean_operators)
    }

    fn count_keyword_matches(node: &Node, source: &[u8], kinds: &[String]) -> u64 {
        if kinds.is_empty() {
            return 0;
        }

        let query_text = kinds
            .iter()
            .map(|kind| format!("{} @decision", Self::query_pattern_for_kind(kind)))
            .collect::<Vec<_>>()
            .join("\n");

        let query = match Query::new(&node.language(), &query_text) {
            Ok(query) => query,
            Err(_) => return 0,
        };

        let mut count: u64 = 0;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, *node, source);
        while matches.next().is_some() {
            count += 1;
        }
        count
    }

    fn query_pattern_for_kind(kind: &str) -> String {
        if kind
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            format!("({kind})")
        } else {
            let escaped = kind.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{escaped}\"")
        }
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
        parser.parse(source, None).expect("python source must parse")
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

        let effective = LinesOfCodeProcessor::count_effective_lines_from_reader(&mut reader).unwrap();

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

        let complexity = CyclomaticComplexityProcessor::compute_cyclomatic(
            &tree.root_node(),
            source.as_bytes(),
            &profile,
        );

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

        let complexity = CyclomaticComplexityProcessor::compute_cyclomatic(
            &tree.root_node(),
            source.as_bytes(),
            &profile,
        );

        assert_eq!(complexity, 10);
    }
}
