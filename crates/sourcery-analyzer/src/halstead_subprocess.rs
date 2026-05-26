use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::processor::FunctionAnalysis;

const OCAML_HALSTEAD: &str = "ocamlhalstead";
const GO_HALSTEAD: &str = "gohalstead";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Root {
    pub totals: HalsteadMetrics,
    pub functions: Vec<Functions>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Functions {
    name: String,
    metrics: HalsteadMetrics,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct HalsteadMetrics {
    pub unique_operators: usize,
    pub unique_operands: usize,
    pub operands: usize,
    pub operators: usize,
}

pub struct HalsteadService {}

impl HalsteadService {
    fn generate_input(file: &Path, functions: Vec<FunctionAnalysis>) -> String {
        let absolute_file_path = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
        let mut sb = String::with_capacity(512);
        sb.push_str(absolute_file_path.to_string_lossy().as_ref());
        for function in functions {
            sb.push('\n');
            sb.push_str(&Self::function_to_input_line(&function));
        }
        sb.push('\n');
        sb
    }

    fn function_to_input_line(function: &FunctionAnalysis) -> String {
        let name = function.function_name.to_string();
        let start_line = function.definition_line_span.start_line;
        let end_line = function.definition_line_span.end_line;
        format!("{}:{}:{}", name, start_line, end_line)
    }

    fn setup_process() {
        let child =
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

        let input = HalsteadService::generate_input(&Path::new("myfile.go"), functions);
        let expected = format!("myfile.go\nfoo:1:2\n");
        assert_eq!(input, expected);
    }
}
