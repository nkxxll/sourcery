use std::ops::Range;
use std::path::Path;

use anyhow::{Result, anyhow};
use clap::ValueEnum;
use ecow::EcoString;
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_go;
use tree_sitter_ocaml;
use tree_sitter_python;

/// sets up structures for the languages and language specific analysis metadata

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProgrammingLanguage {
    Python,
    Ocaml,
    Haskell,
    Golang,
}

impl ToString for ProgrammingLanguage {
    fn to_string(&self) -> String {
        match self {
            ProgrammingLanguage::Python => "Python".to_string(),
            ProgrammingLanguage::Ocaml => "Ocaml".to_string(),
            ProgrammingLanguage::Haskell => "Haskell".to_string(),
            ProgrammingLanguage::Golang => "Golang".to_string(),
        }
    }
}

pub struct LanguageConfig {
    pub language: ProgrammingLanguage,
    pub function_nodes: Vec<EcoString>,
    pub function_name_field: EcoString,
    pub comment_nodes: Vec<EcoString>,
    pub control_flow_nodes: Vec<EcoString>,
    pub boolean_operators: Vec<EcoString>,
    pub match_construct_nodes: Vec<EcoString>,
    pub match_arm_nodes: Vec<EcoString>,
    pub extensions: Vec<EcoString>,
    pub function_call_nodes: Vec<EcoString>,
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
pub struct CodeByteSpan {
    start: usize,
    end: usize,
}

impl CodeByteSpan {
    pub fn with_location(&self, content: &str) -> Result<EcoString> {
        let name = self.get_content(content)?;
        Ok(format!("{}:{}", name, self.start).into())
    }

    pub fn get_content(&self, content: &str) -> Result<EcoString> {
        if self.end > content.len() {
            return Err(anyhow!("end of span is longer than content length"));
        }
        Ok(content[self.start..self.end].into())
    }

    pub fn new(start: usize, end: usize) -> Self {
        if start >= end {
            panic!("start must be less than end");
        }
        Self { start, end }
    }

    pub fn from_node(node: Node) -> Self {
        let start = node.start_byte();
        let end = node.end_byte();
        Self::new(start, end)
    }
}

impl From<CodeByteSpan> for Range<usize> {
    fn from(span: CodeByteSpan) -> Self {
        span.start..span.end
    }
}

impl LanguageConfig {
    pub fn new(language: ProgrammingLanguage) -> Self {
        fn eco_vec(values: &[&str]) -> Vec<EcoString> {
            values.iter().map(|value| (*value).into()).collect()
        }

        let (
            function_nodes,
            function_name_field,
            comment_nodes,
            control_flow_nodes,
            boolean_operators,
            match_construct_nodes,
            match_arm_nodes,
            extensions,
            function_call_nodes,
        ) = match language {
            ProgrammingLanguage::Python => (
                eco_vec(&["function_definition"]),
                EcoString::from("name"),
                eco_vec(&["comment"]),
                eco_vec(&[
                    "if_statement",
                    "elif_clause",
                    "for_statement",
                    "while_statement",
                    "except_clause",
                    "conditional_expression",
                    "match_statement",
                ]),
                eco_vec(&["boolean_operator"]),
                eco_vec(&["match_statement"]),
                eco_vec(&["case_clause"]),
                eco_vec(&["py"]),
                eco_vec(&["call"]),
            ),
            ProgrammingLanguage::Ocaml => (
                eco_vec(&["let_binding"]),
                EcoString::from("pattern"),
                eco_vec(&["comment"]),
                eco_vec(&[
                    "if_expression",
                    "for_expression",
                    "while_expression",
                    "try_expression",
                    "match_expression",
                ]),
                eco_vec(&["and_operator", "or_operator"]),
                eco_vec(&["match_expression"]),
                eco_vec(&["match_case"]),
                eco_vec(&["ml", "mli"]),
                eco_vec(&["application_expression"]),
            ),
            ProgrammingLanguage::Haskell => todo!("this language is not implemented yet!"),
            ProgrammingLanguage::Golang => (
                eco_vec(&["function_declaration", "method_declaration"]),
                EcoString::from("name"),
                eco_vec(&["comment"]),
                eco_vec(&[
                    "if_statement",
                    "for_statement",
                    "expression_switch_statement",
                    "type_switch_statement",
                    "select_statement",
                ]),
                eco_vec(&["&&", "||"]),
                eco_vec(&[
                    "expression_switch_statement",
                    "type_switch_statement",
                    "select_statement",
                ]),
                eco_vec(&[
                    "expression_case",
                    "type_case",
                    "communication_case",
                    "default_case",
                ]),
                eco_vec(&["go"]),
                eco_vec(&["call_expression"]),
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
            extensions,
            function_call_nodes,
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

    pub fn function_name_span(&self, function_node: Node) -> Option<CodeByteSpan> {
        let name_node = function_node.child_by_field_name(self.function_name_field.as_str())?;
        Some(Self::node_span(name_node))
    }

    pub fn node_span(node: Node) -> CodeByteSpan {
        CodeByteSpan::new(node.start_byte(), node.end_byte())
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
