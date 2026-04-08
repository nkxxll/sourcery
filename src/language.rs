use std::path::Path;

use anyhow::{Result, anyhow};
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator, Tree};
use tree_sitter_python;

use tracing::warn;
/// sets up structures for the languages with all the treesitter specific queries
/// and other language specific stuff

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
}

pub struct LanguageConfig {
    language: ProgrammingLanguage,
    queries: LanguageQueries,
}

impl ProgrammingLanguage {
    fn ts_language(&self) -> tree_sitter::Language {
        match self {
            ProgrammingLanguage::Python => tree_sitter_python::LANGUAGE.into(),
            _ => todo!("this language is not implemented yet!"),
        }
    }
}

pub struct CodeSpan {
    start: usize,
    end: usize,
}

impl CodeSpan {
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
        let queries = match language {
            ProgrammingLanguage::Python => {
                let query = Query::new(
                    &tree_sitter_python::LANGUAGE.into(),
                    r#"(
                  (function_definition
                    name: (identifier) @name
                  ) @definition
                )"#,
                )
                .expect("query error python function");
                let name_idx = query
                    .capture_index_for_name("name")
                    .expect("query must have @name capture");
                let definition_idx = query
                    .capture_index_for_name("definition")
                    .expect("query must have @definition capture");

                LanguageQueries {
                    functions: query,
                    function_name_index: name_idx,
                    function_definition_index: definition_idx,
                }
            }
            _ => todo!("this language is not implemented yet!"),
        };
        Self { language, queries }
    }

    pub fn get_tree(&self, path: &Path) -> Result<(Tree, String)> {
        let source_code = std::fs::read_to_string(path)?;
        let mut parser = Parser::new();
        parser.set_language(&self.language.ts_language())?;
        let tree = parser.parse(&source_code, None);
        Ok((tree.expect("has to be a tree"), source_code))
    }

    pub fn get_functions(&self, tree: &Tree, code: &str) -> Result<Vec<FunctionPosition>> {
        let query = &self.queries.functions;
        let mut cursor = QueryCursor::new();
        let root = tree.root_node();
        let mut matches = cursor.matches(&query, root, code.as_bytes());

        let mut functions = Vec::new();

        while let Some(m) = matches.next() {
            let name_index = self.queries.function_name_index;
            let definition_index = self.queries.function_definition_index;
            let name_capture = &m.captures.iter().find(|c| c.index == name_index);
            let definiton_capture = &m.captures.iter().find(|c| c.index == definition_index);

            let (Some(name_cap), Some(def_cap)) = (name_capture, definiton_capture) else {
                warn!("one of funciton name or definition could not be found");
                continue;
            };

            let name_span = Self::get_node_pos(name_cap.node);
            let definition_span = Self::get_node_pos(def_cap.node);

            functions.push(FunctionPosition::new(name_span, definition_span));
        }
        Ok(functions)
    }

    fn get_node_pos(node: Node) -> CodeSpan {
        CodeSpan::new(node.start_byte(), node.end_byte())
    }
}
