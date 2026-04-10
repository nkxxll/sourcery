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
