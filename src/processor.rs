use std::io::{BufRead, BufReader, Read};

use crate::{language::LanguageConfig, languages::LanguageProfile};
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
    pub fn compute_cyclomatic(node: &Node, source: &[u8], profile: &dyn LanguageProfile) -> u64 {
        let mut complexity: u64 = 1; // base path
        Self::walk_cyclomatic(node, source, profile, &mut complexity);
        complexity
    }

    fn walk_cyclomatic(
        node: &Node,
        _source: &[u8],
        profile: LanguageConfig,
        complexity: &mut u64,
    ) {
        let kind = node.kind();
        let control_flow = profile.control_flow_nodes;
        let boolean_ops = profile.boolean_operators;
        let match_constructs = profile.match_construct_nodes;
        let match_arms = profile.match_arm_nodes;

        // Count control flow nodes as decision points, but NOT any else clauses
        // and NOT match/switch constructs (those are counted per-arm instead).
        if control_flow.contains(&kind) && !match_constructs.contains(&kind) {
            *complexity += 1;
        }

        // Each match/switch arm is a separate decision point (SonarSource/McCabe).
        if match_arms.contains(&kind) {
            *complexity += 1;
        }

        if boolean_ops.contains(&kind) {
            *complexity += 1;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_cyclomatic(&child, _source, profile, complexity);
        }
    }
}
