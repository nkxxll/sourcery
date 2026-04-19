use std::ops::Range;
use std::path::Path;

use anyhow::{Result, anyhow};
use tree_sitter::{Node, Parser, Query, QueryCursor, QueryMatch, StreamingIterator, Tree};
use tree_sitter_python;

use tracing::warn;
/// sets up structures for the languages with all the treesitter specific queries
/// and other language specific stuff

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgrammingLanguage {
    Python,
    Ocaml,
    Haskell,
    Golang,
}

pub enum QueryType {
    Functions,
}

struct LanguageQueries {
    functions: Query,
    function_name_index: u32,
    function_definition_index: u32,
    comments: Query,
    comment_index: u32,
}

pub struct LanguageConfig {
    language: ProgrammingLanguage,
    queries: LanguageQueries,
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
            _ => todo!("this language is not implemented yet!"),
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

pub struct FunctionPosition {
    pub name: CodeSpan,
    pub definition: CodeSpan,
}

impl FunctionPosition {
    pub fn new(name: CodeSpan, definition: CodeSpan) -> Self {
        Self { name, definition }
    }
}

impl LanguageConfig {
    pub fn new(language: ProgrammingLanguage) -> Self {
        let (
            queries,
            control_flow_nodes,
            boolean_operators,
            match_construct_nodes,
            match_arm_nodes,
        ) = match language {
            ProgrammingLanguage::Python => {
                let comment_query = Query::new(
                    &tree_sitter_python::LANGUAGE.into(),
                    r#"(comment) @comment
                    (module
                      (expression_statement
                        (string) @comment))
                    (function_definition
                      body: (block
                        (expression_statement
                          (string) @comment)))
                    (class_definition
                      body: (block
                        (expression_statement
                          (string) @comment)))
                    "#,
                )
                .expect("query error python comments");
                let function_query = Query::new(
                    &tree_sitter_python::LANGUAGE.into(),
                    r#"(
                  (function_definition
                    name: (identifier) @name
                  ) @definition
                )"#,
                )
                .expect("query error python function");
                let name_idx = function_query
                    .capture_index_for_name("name")
                    .expect("query must have @name capture");
                let definition_idx = function_query
                    .capture_index_for_name("definition")
                    .expect("query must have @definition capture");
                let comment_idx = comment_query
                    .capture_index_for_name("comment")
                    .expect("comment query must have @comment capture");

                (
                    LanguageQueries {
                        functions: function_query,
                        function_name_index: name_idx,
                        function_definition_index: definition_idx,
                        comments: comment_query,
                        comment_index: comment_idx,
                    },
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
                )
            }
            _ => todo!("this language is not implemented yet!"),
        };
        Self {
            language,
            queries,
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

    fn collect_matches<T>(
        &self,
        query: &Query,
        tree: &Tree,
        code: &str,
        mut map: impl FnMut(&QueryMatch<'_, '_>) -> Option<T>,
    ) -> Vec<T> {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), code.as_bytes());
        let mut out = Vec::new();

        while let Some(m) = matches.next() {
            if let Some(value) = map(&m) {
                out.push(value);
            }
        }
        out
    }

    pub fn get_comments(&self, tree: &Tree, code: &str) -> Result<Vec<CodeSpan>> {
        let query = &self.queries.comments;
        let comment_index = self.queries.comment_index;
        let comments = self.collect_matches(query, tree, code, |m| {
            let capture = m.captures.iter().find(|c| c.index == comment_index)?;
            Some(Self::get_node_pos(capture.node))
        });

        Ok(comments)
    }

    pub fn get_functions(&self, tree: &Tree, code: &str) -> Result<Vec<FunctionPosition>> {
        let query = &self.queries.functions;
        let name_index = self.queries.function_name_index;
        let definition_index = self.queries.function_definition_index;
        let functions = self.collect_matches(query, tree, code, |m| {
            let name_capture = m.captures.iter().find(|c| c.index == name_index);
            let definition_capture = m.captures.iter().find(|c| c.index == definition_index);

            let (Some(name_cap), Some(def_cap)) = (name_capture, definition_capture) else {
                warn!("one of funciton name or definition could not be found");
                return None;
            };

            let name_span = Self::get_node_pos(name_cap.node);
            let definition_span = Self::get_node_pos(def_cap.node);

            Some(FunctionPosition::new(name_span, definition_span))
        });

        Ok(functions)
    }

    fn get_node_pos(node: Node) -> CodeSpan {
        CodeSpan::new(node.start_byte(), node.end_byte())
    }
}

#[cfg(test)]
mod tests {
    use super::ProgrammingLanguage;
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
    fn returns_none_for_unknown_content_and_extension() {
        let content = "just plain text";
        let language = ProgrammingLanguage::detect_language(Path::new("notes.txt"), Some(content));

        assert_eq!(language, None);
    }
}
