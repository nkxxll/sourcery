use std::{fs, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, ValueEnum};
use tree_sitter::{Node, Parser as TreeSitterParser};

struct State {
    nodes: usize,
    functions: usize,
    function_lines: usize,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Language {
    Go,
    Ocaml,
}

impl Language {
    fn tree_sitter_language(self) -> tree_sitter::Language {
        match self {
            Language::Go => tree_sitter_go::LANGUAGE.into(),
            Language::Ocaml => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
        }
    }
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(value_enum)]
    language: Language,
    path: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let source = fs::read_to_string(&args.path)
        .with_context(|| format!("failed to read {}", args.path.display()))?;
    let tree = parse(args.language, &source)?;

    let root = tree.root_node();

    let mut state = State {
        nodes: 0,
        functions: 0,
        function_lines: 0,
    };

    (state.functions, state.function_lines) = count_functions(root, args.language);
    (state.functions, state.function_lines) = count_functions(root, args.language);

    let mut cursor = root.walk();
    root.children(&mut cursor).for_each(|n| {
        if n.kind() == "function_declaration"
            || n.kind() == "method_declaration"
            || n.kind() == "value_definition"
        {
            walk_tree(&mut state, n);
        }
    });
    println!("nodes: {}", state.nodes);
    println!("functions: {}", state.functions);
    println!("function_lines: {}", state.function_lines);
    println!("function_lines/nodes: {}/{} = {} ", state.nodes, state.function_lines, state.nodes as f64 / state.function_lines as f64);

    Ok(())
}

fn count_functions(node: Node, language: Language) -> (usize, usize) {
    match language {
        Language::Go => count_functions_go(node),
        Language::Ocaml => count_functions_ocaml(node),
    }
}

fn count_functions_go(node: Node) -> (usize, usize) {
    let mut cursor = node.walk();

    let function_lines: Vec<usize> = node
        .children(&mut cursor)
        .filter_map(|c| {
            if c.kind() == "function_declaration" || c.kind() == "method_declaration" {
                Some(count_function_lines(c))
            } else {
                None
            }
        })
        .collect();

    (function_lines.len(), function_lines.into_iter().sum())
}

fn count_functions_ocaml(node: Node) -> (usize, usize) {
    let mut cursor = node.walk();

    let function_lines: Vec<usize> = node
        .children(&mut cursor)
        .filter_map(|c| {
            if c.kind() == "value_definition" {
                Some(count_function_lines(c))
            } else {
                None
            }
        })
        .collect();

    (function_lines.len(), function_lines.into_iter().sum())
}

fn count_function_lines(node: Node) -> usize {
    node.end_position().row - node.start_position().row + 1
}

fn parse(language: Language, source: &str) -> Result<tree_sitter::Tree> {
    let mut parser = TreeSitterParser::new();
    parser.set_language(&language.tree_sitter_language())?;
    parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("tree-sitter returned no parse tree"))
}

fn walk_tree(state: &mut State, node: Node) {
    // Scaffold: replace this with whatever call/function extraction you want.
    // print_node(node, source, depth);
    state.nodes += 1;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree(state, child);
    }
}

