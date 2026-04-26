use std::ops::Range;
use std::path::Path;

use anyhow::{Result, anyhow};
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_go;
use tree_sitter_ocaml;
use tree_sitter_python;

/// sets up structures for the languages and language specific analysis metadata

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgrammingLanguage {
    Python,
    Ocaml,
    Haskell,
    Golang,
}

pub struct LanguageConfig {
    language: ProgrammingLanguage,
    pub function_nodes: Vec<String>,
    pub function_name_field: String,
    pub comment_nodes: Vec<String>,
    pub control_flow_nodes: Vec<String>,
    pub boolean_operators: Vec<String>,
    pub match_construct_nodes: Vec<String>,
    pub match_arm_nodes: Vec<String>,
}

impl ProgrammingLanguage {
    pub fn detect_language(path: &Path, content: Option<&str>) -> Option<Self> {
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            if let Some(language) = Self::from_extension(extension) {
                return Some(language);
            }
        }

        content.and_then(Self::from_content)
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "py" | "pyw" | "pyi" => Some(ProgrammingLanguage::Python),
            "ml" | "mli" => Some(ProgrammingLanguage::Ocaml),
            "hs" | "lhs" => Some(ProgrammingLanguage::Haskell),
            "go" => Some(ProgrammingLanguage::Golang),
            _ => None,
        }
    }

    fn from_content(content: &str) -> Option<Self> {
        let first_non_empty_line = content.lines().find(|line| !line.trim().is_empty())?;
        let lowered = first_non_empty_line.to_ascii_lowercase();

        if lowered.starts_with("#!") {
            if lowered.contains("python") {
                return Some(ProgrammingLanguage::Python);
            }
            if lowered.contains("runhaskell") || lowered.contains("ghc") {
                return Some(ProgrammingLanguage::Haskell);
            }
            if lowered.contains("ocaml") {
                return Some(ProgrammingLanguage::Ocaml);
            }
            if lowered.contains("go") {
                return Some(ProgrammingLanguage::Golang);
            }
        }

        let source = content.trim_start();
        if source.starts_with("package ")
            && (source.contains("\nfunc ") || source.contains("\nimport ("))
        {
            return Some(ProgrammingLanguage::Golang);
        }

        if (source.starts_with("module ") && source.contains(" where"))
            || source.contains("\nimport qualified ")
        {
            return Some(ProgrammingLanguage::Haskell);
        }

        if source.starts_with("let ") && source.contains(" =") && source.contains(";;") {
            return Some(ProgrammingLanguage::Ocaml);
        }
        if source.contains("match ") && source.contains(" with") && source.contains("->") {
            return Some(ProgrammingLanguage::Ocaml);
        }

        if source.starts_with("def ")
            || source.starts_with("class ")
            || source.contains("\ndef ")
            || source.contains("\nclass ")
            || source.contains("\nimport ")
            || source.starts_with("import ")
        {
            return Some(ProgrammingLanguage::Python);
        }

        None
    }

    fn ts_language(&self) -> tree_sitter::Language {
        match self {
            ProgrammingLanguage::Python => tree_sitter_python::LANGUAGE.into(),
            ProgrammingLanguage::Ocaml => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
            ProgrammingLanguage::Haskell => todo!("this language is not implemented yet!"),
            ProgrammingLanguage::Golang => tree_sitter_go::LANGUAGE.into(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct CodeSpan {
    start: usize,
    end: usize,
}

impl CodeSpan {
    pub fn with_location(&self, content: &str) -> Result<String> {
        let name = self.get_content(content)?;
        Ok(format!("{}:{}", name, self.start))
    }

    pub fn get_content(&self, content: &str) -> Result<String> {
        if self.end > content.len() {
            return Err(anyhow!("end of span is longer than content length"));
        }
        Ok(content[self.start..self.end].to_string())
    }

    pub fn new(start: usize, end: usize) -> Self {
        if start >= end {
            panic!("start must be less than end");
        }
        Self { start, end }
    }
}

impl From<CodeSpan> for Range<usize> {
    fn from(span: CodeSpan) -> Self {
        span.start..span.end
    }
}

impl LanguageConfig {
    pub fn new(language: ProgrammingLanguage) -> Self {
        let (
            function_nodes,
            function_name_field,
            comment_nodes,
            control_flow_nodes,
            boolean_operators,
            match_construct_nodes,
            match_arm_nodes,
        ) = match language {
            ProgrammingLanguage::Python => (
                vec!["function_definition".to_string()],
                "name".to_string(),
                vec!["comment".to_string()],
                vec![
                    "if_statement".to_string(),
                    "elif_clause".to_string(),
                    "for_statement".to_string(),
                    "while_statement".to_string(),
                    "except_clause".to_string(),
                    "conditional_expression".to_string(),
                    "match_statement".to_string(),
                ],
                vec!["boolean_operator".to_string()],
                vec!["match_statement".to_string()],
                vec!["case_clause".to_string()],
            ),
            ProgrammingLanguage::Ocaml => (
                vec!["let_binding".to_string()],
                "pattern".to_string(),
                vec!["comment".to_string()],
                vec![
                    "if_expression".to_string(),
                    "for_expression".to_string(),
                    "while_expression".to_string(),
                    "try_expression".to_string(),
                    "match_expression".to_string(),
                ],
                vec!["and_operator".to_string(), "or_operator".to_string()],
                vec!["match_expression".to_string()],
                vec!["match_case".to_string()],
            ),
            ProgrammingLanguage::Haskell => todo!("this language is not implemented yet!"),
            ProgrammingLanguage::Golang => (
                vec![
                    "function_declaration".to_string(),
                    "method_declaration".to_string(),
                ],
                "name".to_string(),
                vec!["comment".to_string()],
                vec![
                    "if_statement".to_string(),
                    "for_statement".to_string(),
                    "expression_switch_statement".to_string(),
                    "type_switch_statement".to_string(),
                    "select_statement".to_string(),
                ],
                vec!["&&".to_string(), "||".to_string()],
                vec![
                    "expression_switch_statement".to_string(),
                    "type_switch_statement".to_string(),
                    "select_statement".to_string(),
                ],
                vec![
                    "expression_case".to_string(),
                    "type_case".to_string(),
                    "communication_case".to_string(),
                    "default_case".to_string(),
                ],
            ),
        };

        Self {
            language,
            function_nodes,
            function_name_field,
            comment_nodes,
            control_flow_nodes,
            boolean_operators,
            match_construct_nodes,
            match_arm_nodes,
        }
    }

    pub fn get_tree(&self, path: &Path) -> Result<(Tree, String)> {
        let source_code = std::fs::read_to_string(path)?;
        let tree = self.parse_tree(&source_code)?;
        Ok((tree, source_code))
    }

    pub fn parse_tree(&self, source_code: &str) -> Result<Tree> {
        let mut parser = Parser::new();
        parser.set_language(&self.language.ts_language())?;
        let tree = parser.parse(source_code, None);
        Ok(tree.expect("has to be a tree"))
    }

    pub fn function_name_span(&self, function_node: Node) -> Option<CodeSpan> {
        let name_node = function_node.child_by_field_name(&self.function_name_field)?;
        Some(Self::node_span(name_node))
    }

    pub fn node_span(node: Node) -> CodeSpan {
        CodeSpan::new(node.start_byte(), node.end_byte())
    }

    pub fn is_doc_string_node(&self, node: Node) -> bool {
        match self.language {
            ProgrammingLanguage::Python => Self::is_python_docstring(node),
            _ => false,
        }
    }

    fn is_python_docstring(node: Node) -> bool {
        let Some(parent) = node.parent() else {
            return false;
        };
        if parent.kind() != "expression_statement" {
            return false;
        }

        let Some(grandparent) = parent.parent() else {
            return false;
        };

        match grandparent.kind() {
            "module" => true,
            "block" => grandparent.parent().is_some_and(|scope| {
                matches!(scope.kind(), "function_definition" | "class_definition")
            }),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LanguageConfig, ProgrammingLanguage};
    use std::path::Path;

    #[test]
    fn detects_by_extension() {
        let language = ProgrammingLanguage::detect_language(Path::new("script.py"), None);

        assert_eq!(language, Some(ProgrammingLanguage::Python));
    }

    #[test]
    fn detects_python_by_shebang_without_extension() {
        let content = "#!/usr/bin/env python3\nprint('hello')\n";
        let language = ProgrammingLanguage::detect_language(Path::new("tool"), Some(content));

        assert_eq!(language, Some(ProgrammingLanguage::Python));
    }

    #[test]
    fn detects_golang_by_content_without_extension() {
        let content = "package main\n\nfunc main() {}\n";
        let language = ProgrammingLanguage::detect_language(Path::new("main"), Some(content));

        assert_eq!(language, Some(ProgrammingLanguage::Golang));
    }

    #[test]
    fn parses_golang_source_with_language_config() {
        let source = r#"
package main

func add(a int, b int) int {
    if a > b && b > 0 {
        return a
    }
    return b
}
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Golang);
        let tree = profile.parse_tree(source).unwrap();

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parses_ocaml_source_with_language_config() {
        let source = r#"
let add a b =
  if a > b && b > 0 then
    a
  else
    b
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Ocaml);
        let tree = profile.parse_tree(source).unwrap();

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn returns_none_for_unknown_content_and_extension() {
        let content = "just plain text";
        let language = ProgrammingLanguage::detect_language(Path::new("notes.txt"), Some(content));

        assert_eq!(language, None);
    }
}
