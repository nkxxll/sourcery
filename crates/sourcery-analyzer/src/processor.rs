use std::{
    collections::{HashMap, HashSet},
    ops::Range as ByteRange,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use ecow::EcoString;
use sourcery_lsp_client::{Position, Range as LspRange, SharedSocket};
use tracing::{debug, info, warn};
use tree_sitter::{Node, Tree};
use url::Url;

use crate::language::{CodeByteSpan, LanguageConfig, ProgrammingLanguage};

pub struct ProcessorSource {
    source: EcoString,
    new_line_map: NewLineMap,
    file: PathBuf,
}

impl ProcessorSource {
    pub fn from_path(path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path)?;
        Ok(Self::from_text(source, path.to_path_buf()))
    }

    pub fn from_text(source: impl Into<EcoString>, file: impl Into<PathBuf>) -> Self {
        let source = source.into();
        let new_line_map = NewLineMap::new(&source);
        Self {
            source,
            new_line_map,
            file: file.into(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn file(&self) -> &PathBuf {
        &self.file
    }

    pub fn new_line_map(&self) -> &NewLineMap {
        &self.new_line_map
    }
}

pub struct Processor<'processor> {
    lc: &'processor LanguageConfig,
    source: ProcessorSource,
    socket: Option<SharedSocket>,
    uri: Url,
}

impl<'processor> Processor<'processor> {
    pub fn new(
        lc: &'processor LanguageConfig,
        path: &Path,
        socket: SharedSocket,
        uri: Url,
    ) -> Result<Self> {
        let source = ProcessorSource::from_path(path)?;
        // load the file into memory on the ls side
        Ok(Self {
            lc,
            source,
            socket: Some(socket),
            uri,
        })
    }

    pub fn from_source_input(lc: &'processor LanguageConfig, source: ProcessorSource) -> Self {
        // Try to canonicalize the path (makes it absolute and resolves symlinks).
        // If canonicalize fails (e.g. in tests the file may not exist), fall back to
        // constructing an absolute path from the current directory.
        let file_path = match source.file.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                if source.file.is_absolute() {
                    source.file.clone()
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(&source.file)
                }
            }
        };
        Self {
            lc,
            source,
            socket: None,
            uri: Url::from_file_path(&file_path).expect("url failed in test"),
        }
    }

    pub async fn close_language_server_file(&mut self) {
        let path = self.source.file();
        debug!(file = %path.display(), "closing language server file");
        self.socket.as_mut().unwrap().close_document(path).await;
        debug!(file = %path.display(), "closed language server file");
    }

    pub fn source(&self) -> &str {
        self.source.source()
    }

    pub fn new_line_map(&self) -> &NewLineMap {
        self.source.new_line_map()
    }

    /// Compute all LOC metrics from the syntax analysis and source.
    fn compute_loc_metrics(
        &self,
        syntax_functions: &[FunctionAnalysis],
        comments: &[CommentAnalysis],
    ) -> (u64, u64, u64, u64, u64, u64) {
        let source = self.source.source();
        let new_line_map = self.source.new_line_map();
        let lines_of_code = new_line_map.line_count() as u64;
        let bracket_lines_of_code = Self::bracket_lines(source);
        let blank_lines = self.blank_lines();
        let comment_lines_of_code = comments.iter().map(|comment| comment.lines).sum::<usize>() as u64;
        let effective_lines_of_code =
            lines_of_code.saturating_sub(comment_lines_of_code).saturating_sub(blank_lines);
        let total_cyclomatic = syntax_functions.iter().map(|func| func.cyclomatic).sum::<u64>();

        (
            lines_of_code,
            blank_lines,
            bracket_lines_of_code,
            comment_lines_of_code,
            effective_lines_of_code,
            total_cyclomatic,
        )
    }

    /// Compute syntax analysis: extract all structural information from source.
    pub fn compute_syntax_analysis(&self, ast_processor: &AstProcessor) -> Result<SyntaxAnalysis> {
        debug!(file = %self.source.file().display(), "starting syntax analysis");
        let ast_analysis = ast_processor.analyze_tree()?;

        let (lines_of_code, blank_lines, bracket_lines_of_code, comment_lines_of_code, effective_lines_of_code, total_cyclomatic) =
            self.compute_loc_metrics(&ast_analysis.functions, &ast_analysis.comments);

        let syntax = SyntaxAnalysis {
            lines_of_code,
            blank_lines,
            bracket_lines_of_code,
            comment_lines_of_code,
            effective_lines_of_code,
            total_cyclomatic,
            functions: ast_analysis.functions,
            comments: ast_analysis.comments,
        };

        debug!(
            file = %self.source.file().display(),
            lines_of_code = syntax.lines_of_code,
            effective_lines_of_code = syntax.effective_lines_of_code,
            comment_lines_of_code = syntax.comment_lines_of_code,
            bracket_lines_of_code = syntax.bracket_lines_of_code,
            total_cyclomatic = syntax.total_cyclomatic,
            functions = syntax.functions.len(),
            "finished syntax analysis"
        );
        Ok(syntax)
    }

    /// Synchronous analysis without enrichment (for testing).
    #[allow(dead_code)]
    fn analyze(&self, ast_processor: &AstProcessor) -> Result<Analysis> {
        debug!(file = %self.source.file().display(), "starting ast analysis");
        let syntax = self.compute_syntax_analysis(ast_processor)?;
        let analysis = Self::combine_analysis(self.source.file().clone(), syntax, None, None);
        debug!(
            file = %analysis.file.display(),
            lines_of_code = analysis.lines_of_code,
            effective_lines_of_code = analysis.effective_lines_of_code,
            comment_lines_of_code = analysis.comment_lines_of_code,
            bracket_lines_of_code = analysis.bracket_lines_of_code,
            total_cyclomatic = analysis.total_cyclomatic,
            "finished ast analysis"
        );
        Ok(analysis)
    }


    pub async fn analyze_with_enrichted_stats(&mut self) -> Result<Analysis> {
        info!(
            file = %self.source.file().display(),
            "starting analysis with parallel lsp enrichment and halstead computation"
        );
        let ast_processor = AstProcessor::new(
            self.lc,
            self.source.source(),
            self.source.file().to_path_buf(),
            self.uri.clone(),
        );

        // Phase 1: Compute syntax analysis (required before parallel work)
        let syntax = self.compute_syntax_analysis(&ast_processor)?;

        debug!(
            file = %self.source.file().display(),
            functions = syntax.functions.len(),
            "starting parallel tasks for lsp enrichment and halstead computation"
        );

        // Phase 2 & 3: Run LSP enrichment and halstead computation in parallel
        let socket_opt = self.socket.take();

        let lsp_future = async {
            if let Some(mut sock) = socket_opt {
                debug!(file = %self.source.file().display(), "starting lsp enrichment");
                match ast_processor.enricht_analysis(syntax.functions.clone(), &mut sock).await {
                    Ok(enriched) => {
                        debug!(file = %self.source.file().display(), "finished lsp enrichment");
                        Some(enriched)
                    }
                    Err(e) => {
                        warn!(file = %self.source.file().display(), error = ?e, "lsp enrichment failed");
                        None
                    }
                }
            } else {
                None
            }
        };

        let halstead_future = async {
            debug!(file = %self.source.file().display(), "starting halstead metrics computation");
            match crate::halstead_subprocess::spawn_halstead_metrics_process(
                self.source.file(),
                &self.lc.language.to_string(),
                self.source.source(),
                &syntax.functions,
            )
            .await
            {
                Ok(result) => {
                    debug!(file = %self.source.file().display(), "finished halstead metrics computation");
                    Some(result)
                }
                Err(e) => {
                    warn!(file = %self.source.file().display(), error = ?e, "halstead metrics computation failed");
                    None
                }
            }
        };

        // Wait for both tasks to complete
        let (lsp_result, halstead_result) = tokio::join!(lsp_future, halstead_future);

        debug!(
            file = %self.source.file().display(),
            "finished parallel tasks"
        );

        // Combine all analysis results
        let analysis = Self::combine_analysis(
            self.source.file().clone(),
            syntax,
            lsp_result,
            halstead_result,
        );

        info!(
            file = %analysis.file.display(),
            "finished analysis with lsp enrichment and halstead metrics"
        );
        Ok(analysis)
    }

    /// Combine syntax analysis, LSP enrichment, and halstead metrics into final Analysis.
    fn combine_analysis(
        file: PathBuf,
        syntax: SyntaxAnalysis,
        enriched_functions: Option<Vec<FunctionAnalysis>>,
        halstead_result: Option<crate::halstead_subprocess::HalsteadSubprocessResult>,
    ) -> Analysis {
        let mut functions = syntax.functions;

        // Apply LSP enrichment if available
        if let Some(enriched) = enriched_functions {
            functions = enriched;
        }

        // Apply halstead metrics if available
        if let Some(halstead) = halstead_result {
            crate::halstead_subprocess::apply_halstead_to_functions(&mut functions, &halstead);
        }

        Analysis {
            file,
            functions,
            comments: syntax.comments,
            lines_of_code: syntax.lines_of_code,
            blank_lines: syntax.blank_lines,
            bracket_lines_of_code: syntax.bracket_lines_of_code,
            comment_lines_of_code: syntax.comment_lines_of_code,
            effective_lines_of_code: syntax.effective_lines_of_code,
            total_cyclomatic: syntax.total_cyclomatic,
        }
    }

    fn bracket_lines(source: &str) -> u64 {
        let mut res = 0;
        for line in source.lines() {
            match line.trim() {
                "[" | "]" | "{" | "}" | "(" | ")" | ";;" | ";" => res += 1,
                _ => continue,
            }
        }
        res
    }

    fn blank_lines(&self) -> u64 {
        let mut res = 0;
        let mut last: Option<usize> = None;
        for newline in &self.source.new_line_map.newline_offsets {
            if let Some(l) = last
                && l == newline - 1
            {
                // there is nothing in the line a blank line is here defined as a line that holds no other character
                res += 1;
            }
            last = Some(*newline);
        }
        res
    }
}

#[derive(Debug, Clone)]
pub struct CommentAnalysis {
    pub comment_span: CodeByteSpan,
    pub comment_line_span: CodeLineSpan,
    /// Length in lines gathered by the newline map.
    pub lines: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HalsteadMetrics {
    pub unique_operators: usize,
    pub unique_operands: usize,
    pub operands: usize,
    pub operators: usize,
}

#[derive(Debug, Clone)]
pub struct SyntaxAnalysis {
    /// LOC metrics computed from the entire file
    pub lines_of_code: u64,
    pub blank_lines: u64,
    pub bracket_lines_of_code: u64,
    pub comment_lines_of_code: u64,
    pub effective_lines_of_code: u64,
    pub total_cyclomatic: u64,

    /// Syntax-level function data (no LSP or halstead yet)
    pub functions: Vec<FunctionAnalysis>,

    /// Comment positions
    pub comments: Vec<CommentAnalysis>,
}

#[derive(Debug, Clone)]
pub struct FunctionAnalysis {
    pub function_name: EcoString,
    pub name: CodeByteSpan,
    pub definition: CodeByteSpan,
    pub definition_line_span: CodeLineSpan,
    pub definition_position_range: CodePositionRange,
    pub function_length: usize,
    pub cyclomatic: u64,
    pub cyclomatic_match_as_single_branch: u64,
    pub functions_called: Vec<FunctionCall>,
    pub references: Vec<FunctionCall>,
    pub enriched_calls: Vec<FunctionCall>,
    pub halstead: Option<HalsteadMetrics>,
}

#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub name: EcoString,
    pub pos: CodePosition,
    pub file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CodePosition {
    pub line: usize,
    pub column: usize,
}

impl CodePosition {
    fn to_lsp_position(self) -> Position {
        Position {
            line: self.line.saturating_sub(1) as u32,
            character: self.column.saturating_sub(1) as u32,
        }
    }

    fn from_lsp_position(position: Position) -> Self {
        Self {
            line: position.line as usize + 1,
            column: position.character as usize + 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CodePositionRange {
    pub start: CodePosition,
    pub end: CodePosition,
}

impl CodePositionRange {
    fn from_lsp_range(range: LspRange) -> Self {
        Self {
            start: CodePosition::from_lsp_position(range.start),
            end: CodePosition::from_lsp_position(range.end),
        }
    }
}

#[derive(Debug)]
pub struct Analysis {
    pub file: PathBuf,
    pub functions: Vec<FunctionAnalysis>,
    pub comments: Vec<CommentAnalysis>,
    pub lines_of_code: u64,
    pub blank_lines: u64,
    pub bracket_lines_of_code: u64,
    pub comment_lines_of_code: u64,
    pub effective_lines_of_code: u64,
    pub total_cyclomatic: u64,
}
impl Analysis {
    pub(crate) fn pretty_print(&self, source: &str) -> String {
        // reduce perf hit by growing
        let mut res = String::with_capacity(1024);

        res.push_str(&format!("lines_of_code: {}\n", self.lines_of_code));
        res.push_str(&format!("blank_lines: {}\n", self.blank_lines));
        res.push_str(&format!(
            "bracket_lines_of_code: {}\n",
            self.bracket_lines_of_code
        ));
        res.push_str(&format!(
            "comment_lines_of_code: {}\n",
            self.comment_lines_of_code
        ));
        res.push_str(&format!(
            "effective_lines_of_code: {}\n",
            self.effective_lines_of_code
        ));
        res.push_str(&format!("total_cyclomatic: {}\n", self.total_cyclomatic));

        res.push_str("functions:\n");
        res.push_str(&self.print_call_graph());
        for function in &self.functions {
            let name = &function.function_name;
            res.push_str(&format!(
                "  - {}:{}..{} length={} cyclomatic={} cyclomatic_match_as_single_branch={}\n",
                name,
                function.definition_line_span.start_line,
                function.definition_line_span.end_line,
                function.function_length,
                function.cyclomatic,
                function.cyclomatic_match_as_single_branch,
            ));
        }

        res.push_str("comments:\n");
        for comment in &self.comments {
            let snippet = comment
                .comment_span
                .get_content(source)
                .map(|content| content.replace("\n", "\\n"))
                .map(|content| {
                    const MAX_CHARS: usize = 80;
                    if content.chars().count() > MAX_CHARS {
                        let truncated: EcoString = content.chars().take(MAX_CHARS).collect();
                        format!("{truncated}...").into()
                    } else {
                        content
                    }
                })
                .unwrap_or_else(|_| "<invalid span>".into());
            res.push_str(&format!(
                "  - {}..{} length={} text={:?}\n",
                comment.comment_line_span.start_line,
                comment.comment_line_span.end_line,
                comment.lines,
                snippet,
            ));
        }

        res
    }

    fn print_call_graph(&self) -> String {
        let mut res = String::with_capacity(512);
        res.push_str("\n\ndigraph CallGraph {\n");
        for function in &self.functions {
            let name = &function.function_name;
            let line = function.definition_position_range.start.line;
            let column = function.definition_position_range.start.column;
            let calls = if function.enriched_calls.is_empty() {
                &function.functions_called
            } else {
                &function.enriched_calls
            };
            for called in calls {
                let called_name = &called.name;
                let called_line = &called.pos.line;
                let called_col = &called.pos.column;
                let called_file = &called.file.to_string_lossy();
                res.push_str(&format!(
                    "\t\"{name}:{line}:{column}\" -> \"{called_file}:{called_name}:{called_line}:{called_col}\"\n"
                ));
            }
        }
        res.push_str("}\n\n");
        res
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FileMetrics {
    pub lines_of_code: u64,
    pub effective_lines_of_code: u64,
    pub comment_lines_of_code: u64,
    pub bracket_lines_of_code: u64,
    pub total_cyclomatic: u64,
}

impl FileMetrics {
    pub fn from_json(metrics: &serde_json::Value) -> Self {
        Self {
            lines_of_code: metrics
                .get("lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            effective_lines_of_code: metrics
                .get("effective_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            comment_lines_of_code: metrics
                .get("comment_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            bracket_lines_of_code: metrics
                .get("bracket_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_cyclomatic: metrics
                .get("total_cyclomatic")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AggregatedFileMetrics {
    pub files: u64,
    pub total_lines_of_code: u64,
    pub total_effective_lines_of_code: u64,
    pub total_comment_lines_of_code: u64,
    pub total_bracket_lines_of_code: u64,
    pub total_cyclomatic: u64,
}

impl AggregatedFileMetrics {
    pub fn add_file_metrics(&mut self, metrics: &FileMetrics) {
        self.files += 1;
        self.total_lines_of_code += metrics.lines_of_code;
        self.total_effective_lines_of_code += metrics.effective_lines_of_code;
        self.total_comment_lines_of_code += metrics.comment_lines_of_code;
        self.total_bracket_lines_of_code += metrics.bracket_lines_of_code;
        self.total_cyclomatic += metrics.total_cyclomatic;
    }

    fn mean(total: u64, files: u64) -> f64 {
        if files == 0 {
            0.0
        } else {
            total as f64 / files as f64
        }
    }

    pub fn from_json(metrics: &serde_json::Value) -> Self {
        Self {
            files: metrics
                .get("files")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_lines_of_code: metrics
                .get("total_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_effective_lines_of_code: metrics
                .get("total_effective_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_comment_lines_of_code: metrics
                .get("total_comment_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_bracket_lines_of_code: metrics
                .get("total_bracket_lines_of_code")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            total_cyclomatic: metrics
                .get("total_cyclomatic")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "files": self.files,
            "total_lines_of_code": self.total_lines_of_code,
            "total_effective_lines_of_code": self.total_effective_lines_of_code,
            "total_comment_lines_of_code": self.total_comment_lines_of_code,
            "total_bracket_lines_of_code": self.total_bracket_lines_of_code,
            "total_cyclomatic": self.total_cyclomatic,
            "mean_lines_of_code_per_file": Self::mean(self.total_lines_of_code, self.files),
            "mean_effective_lines_of_code_per_file": Self::mean(self.total_effective_lines_of_code, self.files),
            "mean_comment_lines_of_code_per_file": Self::mean(self.total_comment_lines_of_code, self.files),
            "mean_bracket_lines_of_code_per_file": Self::mean(self.total_bracket_lines_of_code, self.files),
            "mean_cyclomatic_complexity_per_file": Self::mean(self.total_cyclomatic, self.files),
        })
    }

    pub fn from_file_metrics_map(file_metrics_by_path: &HashMap<EcoString, FileMetrics>) -> Self {
        let mut aggregated = Self::default();
        for metrics in file_metrics_by_path.values() {
            aggregated.add_file_metrics(metrics);
        }
        aggregated
    }

    pub fn reconcile(
        previous: AggregatedFileMetrics,
        old_metrics: AggregatedFileMetrics,
        new_metrics: AggregatedFileMetrics,
    ) -> AggregatedFileMetrics {
        AggregatedFileMetrics {
            files: previous
                .files
                .saturating_sub(old_metrics.files)
                .saturating_add(new_metrics.files),
            total_lines_of_code: previous
                .total_lines_of_code
                .saturating_sub(old_metrics.total_lines_of_code)
                .saturating_add(new_metrics.total_lines_of_code),
            total_effective_lines_of_code: previous
                .total_effective_lines_of_code
                .saturating_sub(old_metrics.total_effective_lines_of_code)
                .saturating_add(new_metrics.total_effective_lines_of_code),
            total_comment_lines_of_code: previous
                .total_comment_lines_of_code
                .saturating_sub(old_metrics.total_comment_lines_of_code)
                .saturating_add(new_metrics.total_comment_lines_of_code),
            total_bracket_lines_of_code: previous
                .total_bracket_lines_of_code
                .saturating_sub(old_metrics.total_bracket_lines_of_code)
                .saturating_add(new_metrics.total_bracket_lines_of_code),
            total_cyclomatic: previous
                .total_cyclomatic
                .saturating_sub(old_metrics.total_cyclomatic)
                .saturating_add(new_metrics.total_cyclomatic),
        }
    }
}

#[derive(Debug)]
pub struct AstAnalysis {
    pub functions: Vec<FunctionAnalysis>,
    pub comments: Vec<CommentAnalysis>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CodeLineSpan {
    pub start_line: usize,
    pub end_line: usize,
}

impl CodeLineSpan {
    pub fn line_length(&self) -> usize {
        return self.end_line - self.start_line;
    }
}

#[derive(Debug)]
pub struct NewLineMap {
    newline_offsets: Vec<usize>,
    source_len: usize,
    ends_with_newline: bool,
}

impl NewLineMap {
    pub fn new(source: &str) -> Self {
        let newline_offsets = source
            .as_bytes()
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'\n').then_some(index))
            .collect();

        Self {
            newline_offsets,
            source_len: source.len(),
            ends_with_newline: source.ends_with('\n'),
        }
    }

    pub fn line_count(&self) -> usize {
        if self.source_len == 0 {
            return 0;
        }
        if self.ends_with_newline {
            self.newline_offsets.len()
        } else {
            self.newline_offsets.len() + 1
        }
    }

    pub fn position(&self, byte: usize) -> Option<Position> {
        let line_rest = self.get_line_and_rest(byte);
        line_rest.map(|(line, rest)| Position {
            line: (line - 1) as u32,
            character: (rest - 1) as u32,
        })
    }

    pub fn byte_offset(&self, position: Position) -> Option<usize> {
        let line = position.line as usize + 1;
        let character = position.character as usize;

        if line == 0 || line > self.line_count() {
            return None;
        }

        let line_start = if line == 1 {
            0
        } else {
            self.newline_offsets.get(line - 2).copied()? + 1
        };

        let line_end = self
            .newline_offsets
            .get(line - 1)
            .copied()
            .unwrap_or(self.source_len);

        let byte = line_start + character;
        (byte < line_end).then_some(byte)
    }

    pub fn get_line_and_rest(&self, byte: usize) -> Option<(usize, usize)> {
        if self.source_len == 0 || byte >= self.source_len {
            return None;
        }

        match self.newline_offsets.binary_search(&byte) {
            Ok(i) | Err(i) => {
                let line = i + 1;
                let rest = if i == 0 {
                    byte + 1
                } else {
                    byte - self.newline_offsets[i - 1]
                };
                Some((line, rest))
            }
        }
    }

    pub fn get_line(&self, byte: usize) -> Option<usize> {
        let line_rest = self.get_line_and_rest(byte);
        line_rest.map(|(line, _)| line)
    }

    pub fn get_code_line_span(&self, code_byte_span: &CodeByteSpan) -> Result<CodeLineSpan> {
        let span: ByteRange<usize> = (*code_byte_span).into();
        if span.end > self.source_len {
            return Err(anyhow!(
                "span end {} exceeds source length {}",
                span.end,
                self.source_len
            ));
        }
        let start_line = self
            .get_line(span.start)
            .ok_or_else(|| anyhow!("span start {} is out of bounds", span.start))?;
        let end_inclusive = span.end - 1;
        let end_line = self
            .get_line(end_inclusive)
            .ok_or_else(|| anyhow!("span end {} is out of bounds", end_inclusive))?;

        Ok(CodeLineSpan {
            start_line,
            end_line,
        })
    }

    pub fn count_lines(&self, code_byte_span: &CodeByteSpan) -> Result<usize> {
        let code_line_span = self.get_code_line_span(code_byte_span)?;
        Ok((code_line_span.end_line - code_line_span.start_line) + 1)
    }
}

#[derive(Default, Clone, Copy)]
struct CyclomaticCounts {
    control_flow: u64,
    match_constructs: u64,
    match_arms: u64,
    boolean_operators: u64,
}

impl CyclomaticCounts {
    fn add_from_node(
        &mut self,
        node: Node,
        profile: &LanguageConfig,
        classifier: &NodeKindClassifier<'_>,
    ) {
        let kind = node.kind();

        if classifier.control_flow.contains(kind) {
            self.control_flow += 1;
        }
        if classifier.match_constructs.contains(kind) {
            self.match_constructs += 1;
        }
        if classifier.match_arms.contains(kind) {
            self.match_arms += Self::match_arm_complexity(node, profile);
        }
        if classifier.boolean_operators.contains(kind) {
            self.boolean_operators += 1;
        }
    }

    fn match_arm_complexity(node: Node, profile: &LanguageConfig) -> u64 {
        if profile.language != ProgrammingLanguage::Ocaml || node.kind() != "match_case" {
            return 1;
        }

        let pattern_complexity = node
            .child_by_field_name("pattern")
            .map(Self::ocaml_pattern_complexity)
            .unwrap_or(1);
        let guard_complexity = u64::from(Self::has_named_child_kind(node, "guard"));

        pattern_complexity + guard_complexity
    }

    fn ocaml_pattern_complexity(pattern: Node) -> u64 {
        if pattern.kind() == "parenthesized_pattern" {
            return Self::first_named_child(pattern)
                .map(Self::ocaml_pattern_complexity)
                .unwrap_or(1);
        }

        if pattern.kind() != "tuple_pattern" {
            return 1;
        }

        let mut cursor = pattern.walk();
        pattern
            .children(&mut cursor)
            .filter(|child| child.is_named())
            .count()
            .max(1) as u64
    }

    fn first_named_child(node: Node) -> Option<Node> {
        let mut cursor = node.walk();
        node.children(&mut cursor).find(|child| child.is_named())
    }

    fn has_named_child_kind(node: Node, kind: &str) -> bool {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .any(|child| child.is_named() && child.kind() == kind)
    }

    fn cyclomatic(self) -> u64 {
        1 + self.control_flow + self.match_arms + self.boolean_operators
    }

    fn cyclomatic_match_as_single_branch(self) -> u64 {
        1 + self.control_flow + self.match_constructs + self.boolean_operators
    }
}

struct FunctionFrame {
    function_index: usize,
    cyclomatic_counts: CyclomaticCounts,
    function_calls: Vec<FunctionCall>,
}

#[derive(Default)]
struct AstTraversalState {
    functions: Vec<FunctionAnalysis>,
    comments: Vec<CommentAnalysis>,
    function_stack: Vec<FunctionFrame>,
}

struct NodeKindClassifier<'a> {
    function_nodes: HashSet<&'a str>,
    comment_nodes: HashSet<&'a str>,
    control_flow: HashSet<&'a str>,
    match_constructs: HashSet<&'a str>,
    match_arms: HashSet<&'a str>,
    boolean_operators: HashSet<&'a str>,
    function_call: HashSet<&'a str>,
}

impl<'a> NodeKindClassifier<'a> {
    fn from_language(profile: &'a LanguageConfig) -> Self {
        let function_nodes = profile.function_nodes.iter().copied().collect();
        let comment_nodes = profile.comment_nodes.iter().copied().collect();
        let match_constructs: HashSet<&str> =
            profile.match_construct_nodes.iter().copied().collect();
        let control_flow = profile
            .control_flow_nodes
            .iter()
            .copied()
            .filter(|kind| !match_constructs.contains(kind))
            .collect();
        let match_arms = profile.match_arm_nodes.iter().copied().collect();
        let boolean_operators = profile.boolean_operators.iter().copied().collect();
        let function_call = profile.function_call_nodes.iter().copied().collect();
        Self {
            function_nodes,
            comment_nodes,
            control_flow,
            match_constructs,
            match_arms,
            boolean_operators,
            function_call,
        }
    }
}

pub struct AstProcessor<'processor> {
    tree: Tree,
    profile: &'processor LanguageConfig,
    new_line_map: NewLineMap,
    source: &'processor str,
    file: PathBuf,
    uri: Url,
}

impl<'processor> AstProcessor<'processor> {
    pub fn new(
        profile: &'processor LanguageConfig,
        source: &'processor str,
        file: PathBuf,
        uri: Url,
    ) -> Self {
        Self {
            tree: profile.parse_tree(source).expect("could not parse tree"),
            profile,
            new_line_map: NewLineMap::new(source),
            source,
            file,
            uri,
        }
    }

    pub async fn enricht_analysis(
        &self,
        mut function_analysis: Vec<FunctionAnalysis>,
        socket: &mut SharedSocket,
    ) -> Result<Vec<FunctionAnalysis>> {
        info!(
            file = %self.file.display(),
            functions = function_analysis.len(),
            "starting lsp enrichment loop"
        );
        for (index, fa) in function_analysis.iter_mut().enumerate() {
            let range = fa
                .name
                .to_range(&self.new_line_map)
                .expect("could not translate codebytespan to range");
            let function_name = fa.function_name.clone();
            debug!(
                file = %self.file.display(),
                function = %function_name,
                function_index = index,
                line = range.start.line,
                character = range.start.character,
                "starting lsp calls for function"
            );
            let mut ref_socket = socket.clone();
            let ref_fut =
                self.find_references((function_name.clone(), range.start), &mut ref_socket);
            let call_positions = fa
                .functions_called
                .iter()
                .map(|f| (f.name.clone(), f.pos.to_lsp_position()));
            let call_fut = self.get_enriched_calls(call_positions.collect(), socket);
            let (references, calls) = tokio::join!(ref_fut, call_fut);
            let references = references?;
            let calls = calls?;
            debug!(
                file = %self.file.display(),
                function = %function_name,
                function_index = index,
                references = references.as_ref().map_or(0, Vec::len),
                calls = calls.len(),
                "finished lsp calls for function"
            );
            fa.references = references.unwrap_or_default();
            fa.enriched_calls = calls;
        }
        info!(
            file = %self.file.display(),
            enriched_functions = function_analysis.len(),
            "finished lsp enrichment loop"
        );
        Ok(function_analysis)
    }

    async fn get_enriched_calls(
        &self,
        call_posisions: Vec<(EcoString, Position)>,
        socket: &mut SharedSocket,
    ) -> Result<Vec<FunctionCall>> {
        let mut res_vec = Vec::new();
        for (name, call) in call_posisions {
            let uri = self.uri.clone();
            // todo this needs to be async for better perf
            debug!(
                file = %self.file.display(),
                function = %name,
                line = call.line,
                character = call.character + 2,
                "requesting goto_definition"
            );
            let res = socket.goto_definition(uri, call).await?;
            debug!(
                file = %self.file.display(),
                function = %name,
                definitions = res.len(),
                "received goto_definition response"
            );
            let func_calls: Vec<FunctionCall> = res
                .iter()
                .map(|location| {
                    let lsp_range: LspRange = location.range.into();
                    let path = PathBuf::from(location.uri.path());
                    debug!(path = %path.display(), range = %lsp_range, "location range in function call definition");
                    // only debugging
                    let pos = CodePosition::from_lsp_position(lsp_range.start);
                    FunctionCall {
                        name: name.clone(),
                        pos,
                        file: path,
                    }
                })
                .collect();

            let mut func_calls = func_calls.into_iter();
            let func_call = func_calls
                .next()
                .expect("there should be at least one definition of a function");
            if func_calls.next().is_some() {
                tracing::warn!("more than one definition for function taking first");
            }
            res_vec.push(func_call);
        }
        Ok(res_vec)
    }

    async fn find_references(
        &self,
        func: (EcoString, Position),
        socket: &mut SharedSocket,
    ) -> Result<Option<Vec<FunctionCall>>> {
        let (name, call) = func;
        let line = call.line;
        let character = call.character + 2;
        let uri = SharedSocket::project_path_to_uri(&self.file)?;
        debug!(
            file = %self.file.display(),
            function = %name,
            line,
            character,
            "requesting find_references"
        );
        let res = socket.find_references(uri, call).await?;
        debug!(
            file = %self.file.display(),
            function = %name,
            references = res.as_ref().map_or(0, Vec::len),
            "received find_references response"
        );
        Ok(match res {
            Some(res) => Some(
                res.iter()
                    .map(|location| {
                        let lsp_range: LspRange = location.range.into();
                        let path = PathBuf::from(location.uri.path());
                        FunctionCall {
                            name: name.clone(),
                            pos: CodePosition::from_lsp_position(lsp_range.start),
                            file: path,
                        }
                    })
                    .collect(),
            ),
            None => {
                tracing::info!(
                    "function: {} at {}:{}:{}\nis not used in the codebase",
                    name,
                    line,
                    character,
                    self.file.to_string_lossy(),
                );
                None
            }
        })
    }

    pub fn analyze_tree(&self) -> Result<AstAnalysis> {
        let classifier = NodeKindClassifier::from_language(self.profile);
        let mut state = AstTraversalState::default();
        self.traverse(self.tree.root_node(), &classifier, &mut state)?;

        Ok(AstAnalysis {
            functions: state.functions,
            comments: state.comments,
        })
    }

    fn traverse(
        &self,
        node: Node,
        classifier: &NodeKindClassifier<'_>,
        state: &mut AstTraversalState,
    ) -> Result<()> {
        let kind = node.kind();
        let mut entered_function = false;

        if classifier.function_nodes.contains(kind) {
            let Some(name_span) = self.profile.function_name_span(node) else {
                warn!("function node without expected name field: {}", kind);
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.traverse(child, classifier, state)?;
                }
                return Ok(());
            };
            let function_index = state.functions.len();
            let definition_span = LanguageConfig::node_span(node);
            let definition_line_span = self.new_line_map.get_code_line_span(&definition_span)?;
            let definition_byte_range: ByteRange<usize> = definition_span.into();
            let definition_lsp_range =
                definition_span
                    .to_range(&self.new_line_map)
                    .ok_or_else(|| {
                        anyhow!(
                            "could not translate function definition span {}..{} to lsp range",
                            definition_byte_range.start,
                            definition_byte_range.end
                        )
                    })?;
            state.functions.push(FunctionAnalysis {
                function_name: name_span.get_content(self.source)?,
                name: name_span,
                definition: definition_span,
                definition_line_span: definition_line_span,
                definition_position_range: CodePositionRange::from_lsp_range(definition_lsp_range),
                function_length: definition_line_span.line_length(),
                cyclomatic: 1,
                cyclomatic_match_as_single_branch: 1,
                functions_called: Vec::new(),
                references: Vec::new(),
                enriched_calls: Vec::new(),
                halstead: None,
            });
            state.function_stack.push(FunctionFrame {
                function_index,
                cyclomatic_counts: CyclomaticCounts::default(),
                function_calls: Vec::new(),
            });
            entered_function = true;
        }

        if classifier.comment_nodes.contains(kind) || self.profile.is_doc_string_node(node) {
            let comment_span = LanguageConfig::node_span(node);
            let length = self.new_line_map.count_lines(&comment_span)?;
            state.comments.push(CommentAnalysis {
                comment_span,
                comment_line_span: self.new_line_map.get_code_line_span(&comment_span)?,
                lines: length,
            });
        }

        if let Some(frame) = &mut state.function_stack.last_mut() {
            frame
                .cyclomatic_counts
                .add_from_node(node, self.profile, classifier);
            if classifier.function_call.contains(kind) {
                let name = self.get_function_call(node, self.source, &self.file)?;
                frame.function_calls.push(name);
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.traverse(child, classifier, state)?;
        }

        if entered_function {
            let Some(frame) = state.function_stack.pop() else {
                return Ok(());
            };
            let function = &mut state.functions[frame.function_index];
            function.cyclomatic = frame.cyclomatic_counts.cyclomatic();
            function.cyclomatic_match_as_single_branch =
                frame.cyclomatic_counts.cyclomatic_match_as_single_branch();
            function.functions_called = frame.function_calls;
        }

        Ok(())
    }

    fn get_function_call(&self, node: Node, source: &str, file: &PathBuf) -> Result<FunctionCall> {
        if let Some(field) = node.child_by_field_name("function") {
            // Some grammars (notably OCaml) wrap the callable in a parenthesized expression.
            // Use the wrapped callable node so byte/column mapping targets the symbol itself.
            let call_target = if field.kind() == "parenthesized_expression" {
                CyclomaticCounts::first_named_child(field).unwrap_or(field)
            } else {
                field
            };
            let position = self
                .new_line_map
                .get_line_and_rest(call_target.start_byte())
                .ok_or_else(|| anyhow!("could not translate function call position"))?;
            let name = EcoString::from(call_target.utf8_text(source.as_bytes())?);
            let func_call = FunctionCall {
                name,
                pos: CodePosition {
                    line: position.0,
                    column: position.1,
                },
                file: file.clone(),
            };
            return Ok(func_call);
        }
        Err(anyhow::anyhow!("field not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AggregatedFileMetrics, AstProcessor, FileMetrics, NewLineMap, Processor, ProcessorSource,
    };
    use crate::language::{CodeByteSpan, LanguageConfig, ProgrammingLanguage};
    use ecow::EcoString;
    use std::collections::HashMap;
    use url::Url;

    #[test]
    fn newline_map_counts_lines_without_trailing_newline() {
        let content = "first line\nsecond line\nthird line";
        let map = NewLineMap::new(content);

        assert_eq!(map.line_count(), 3);
    }

    #[test]
    fn newline_map_counts_lines_with_trailing_newline() {
        let content = "first line\nsecond line\nthird line\n";
        let map = NewLineMap::new(content);

        assert_eq!(map.line_count(), 3);
    }

    #[test]
    fn newline_map_counts_lines_for_code_span() {
        let content = "first line\nsecond line\nthird line";
        let map = NewLineMap::new(content);
        let start = content.find("second").unwrap();
        let end = content.find("third").unwrap() + "third line".len();
        let span = CodeByteSpan::new(start, end);

        let lines = map.count_lines(&span).unwrap();

        assert_eq!(lines, 2);
    }

    #[test]
    fn newline_map_reports_position_for_second_line_start() {
        let content = "first\nsecond";
        let map = NewLineMap::new(content);
        let byte = content.find("second").unwrap();

        let line_and_rest = map.get_line_and_rest(byte).unwrap();
        let position = map.position(byte).unwrap();

        assert_eq!(line_and_rest, (2, 1));
        assert_eq!(position.line, 1);
        assert_eq!(position.character, 0);
    }

    #[test]
    fn processor_uses_newline_map_for_file_and_comment_lines() {
        let source = "package main\n\n// module comment";
        let profile = LanguageConfig::new(ProgrammingLanguage::Golang);
        let file = std::env::current_dir().unwrap().join("test.go");
        let uri = Url::from_file_path(&file).expect("url failed in test");
        let source_input = ProcessorSource::from_text(source, file.clone());
        let processor = Processor::from_source_input(&profile, source_input);
        let ast_processor = AstProcessor::new(&profile, source, file, uri);

        let analysis = processor.analyze(&ast_processor).unwrap();

        assert_eq!(analysis.lines_of_code, 3);
        assert_eq!(analysis.comment_lines_of_code, 1);
        assert_eq!(analysis.effective_lines_of_code, 1);
    }

    #[test]
    fn one_pass_analysis_collects_functions_comments_and_cyclomatic() {
        let source = r#"package main

// analyze function
// with multi-line comment
func analyze(x int, values []int) int {
    // function comment
    if x > 10 && x < 20 {
        return 1
    } else if x == 0 {
        return 2
    }

    for _, value := range values {
        if value%2 == 0 {
            return 3
        }
    }

    for x > 0 {
        x--
    }

    switch x {
    case 1:
        return 4
    case 2:
        return 5
    }

    if x < 0 {
        return 6
    }
    return 7
}
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Golang);
        let file = std::env::current_dir().unwrap().join("test.go");
        let uri = Url::from_file_path(&file).expect("url failed in test");
        let source_input = ProcessorSource::from_text(source, file.clone());
        let processor = Processor::from_source_input(&profile, source_input);
        let ast_processor = AstProcessor::new(&profile, source, file, uri);

        let analysis = processor.analyze(&ast_processor).unwrap();

        assert_eq!(analysis.functions.len(), 1);
        assert_eq!(analysis.comments.len(), 3);
        assert_eq!(analysis.functions[0].cyclomatic, 10);
        assert_eq!(analysis.functions[0].cyclomatic_match_as_single_branch, 9);
    }

    #[test]
    fn one_pass_analysis_keeps_straight_line_function_at_one() {
        let source = r#"package main

func identity(value int) int {
    result := value + 1
    return result
}
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Golang);
        let file = std::env::current_dir().unwrap().join("test.go");
        let uri = Url::from_file_path(&file).expect("url failed in test");
        let source_input = ProcessorSource::from_text(source, file.clone());
        let processor = Processor::from_source_input(&profile, source_input);
        let ast_processor = AstProcessor::new(&profile, source, file, uri);

        let analysis = processor.analyze(&ast_processor).unwrap();

        assert_eq!(analysis.functions.len(), 1);
        assert_eq!(analysis.functions[0].cyclomatic, 1);
        assert_eq!(analysis.functions[0].cyclomatic_match_as_single_branch, 1);
    }

    #[test]
    fn ocaml_match_counts_tuple_patterns_and_guards() {
        let source = r#"
let merge a b =
  match (a, b) with
  | ([], []) -> []
  | (x :: xs, y :: ys) when x > y -> merge xs ys
  | _ -> failwith "unsupported"
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Ocaml);
        let file = std::env::current_dir().unwrap().join("test.ml");
        let uri = Url::from_file_path(&file).expect("url failed in test");
        let source_input = ProcessorSource::from_text(source, file.clone());
        let processor = Processor::from_source_input(&profile, source_input);
        let ast_processor = AstProcessor::new(&profile, source, file, uri);

        let analysis = processor.analyze(&ast_processor).unwrap();

        assert_eq!(analysis.functions.len(), 1);
        assert_eq!(analysis.functions[0].cyclomatic, 7);
        assert_eq!(analysis.functions[0].cyclomatic_match_as_single_branch, 2);
    }

    #[test]
    fn ocaml_parenthesized_callee_uses_inner_symbol_position() {
        let source = r#"
let run value =
  (helper) value
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Ocaml);
        let file = std::env::current_dir().unwrap().join("test.ml");
        let uri = Url::from_file_path(&file).expect("url failed in test");
        let source_input = ProcessorSource::from_text(source, file.clone());
        let processor = Processor::from_source_input(&profile, source_input);
        let ast_processor = AstProcessor::new(&profile, source, file, uri);

        let analysis = processor.analyze(&ast_processor).unwrap();
        let call = &analysis.functions[0].functions_called[0];

        assert_eq!(call.name.as_ref(), "helper");
        assert_eq!(call.pos.line, 3);
        assert_eq!(call.pos.column, 4);
    }

    #[test]
    fn aggregated_file_metrics_sums_file_metrics_map() {
        let old_metrics = HashMap::from([
            (
                EcoString::from("src/a.rs"),
                FileMetrics {
                    lines_of_code: 10,
                    effective_lines_of_code: 8,
                    comment_lines_of_code: 2,
                    bracket_lines_of_code: 1,
                    total_cyclomatic: 3,
                },
            ),
            (
                EcoString::from("src/b.rs"),
                FileMetrics {
                    lines_of_code: 20,
                    effective_lines_of_code: 15,
                    comment_lines_of_code: 5,
                    bracket_lines_of_code: 3,
                    total_cyclomatic: 7,
                },
            ),
        ]);

        let aggregated = AggregatedFileMetrics::from_file_metrics_map(&old_metrics);

        assert_eq!(aggregated.files, 2);
        assert_eq!(aggregated.total_lines_of_code, 30);
        assert_eq!(aggregated.total_effective_lines_of_code, 23);
        assert_eq!(aggregated.total_comment_lines_of_code, 7);
        assert_eq!(aggregated.total_bracket_lines_of_code, 4);
        assert_eq!(aggregated.total_cyclomatic, 10);
    }

    #[test]
    fn aggregated_file_metrics_reconciles_previous_with_old_and_new_metrics() {
        let previous = AggregatedFileMetrics {
            files: 3,
            total_lines_of_code: 60,
            total_effective_lines_of_code: 48,
            total_comment_lines_of_code: 12,
            total_bracket_lines_of_code: 9,
            total_cyclomatic: 18,
        };
        let old_metrics = AggregatedFileMetrics {
            files: 2,
            total_lines_of_code: 35,
            total_effective_lines_of_code: 28,
            total_comment_lines_of_code: 7,
            total_bracket_lines_of_code: 6,
            total_cyclomatic: 10,
        };
        let new_metrics = AggregatedFileMetrics {
            files: 2,
            total_lines_of_code: 30,
            total_effective_lines_of_code: 26,
            total_comment_lines_of_code: 4,
            total_bracket_lines_of_code: 5,
            total_cyclomatic: 9,
        };

        let reconciled = AggregatedFileMetrics::reconcile(previous, old_metrics, new_metrics);

        assert_eq!(reconciled.files, 3);
        assert_eq!(reconciled.total_lines_of_code, 55);
        assert_eq!(reconciled.total_effective_lines_of_code, 46);
        assert_eq!(reconciled.total_comment_lines_of_code, 9);
        assert_eq!(reconciled.total_bracket_lines_of_code, 8);
        assert_eq!(reconciled.total_cyclomatic, 17);
    }

    #[test]
    fn test_bracket_counting() {
        let source1 = r"let some_function =
     let first = 1 in
     let second = 2 in
 ;;
 let () = some_function;;
 ";
        let source2 = r#"package main
 import "fmt"

 func main() {
     fmt.println("hello world")
     fmt.println(
         "some string that is very long"
     )
 }
 "#;
        let brackets1 = Processor::bracket_lines(source1);
        let brackets2 = Processor::bracket_lines(source2);

        assert_eq!(brackets1, 1);
        assert_eq!(brackets2, 2);
    }

    #[test]
    fn compute_syntax_analysis_creates_correct_metrics() {
        let source = r#"package main

import "fmt"

// Module comment
func analyze(x int, values []int) int {
    // Function comment
    if x > 10 && x < 20 {
        return 1
    } else if x == 0 {
        return 2
    }
    return 3
}
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Golang);
        let file = std::env::current_dir().unwrap().join("test.go");
        let uri = Url::from_file_path(&file).expect("url failed in test");
        let source_input = ProcessorSource::from_text(source, file.clone());
        let processor = Processor::from_source_input(&profile, source_input);
        let ast_processor = AstProcessor::new(&profile, source, file, uri);

        let syntax = processor.compute_syntax_analysis(&ast_processor).unwrap();

        // Verify LOC metrics
        assert!(syntax.lines_of_code > 0);
        assert!(syntax.comment_lines_of_code > 0);
        assert!(syntax.functions.len() > 0);
        assert!(syntax.comments.len() > 0);

        // Verify function extraction
        assert_eq!(syntax.functions[0].cyclomatic, 4);
        assert_eq!(syntax.functions[0].cyclomatic_match_as_single_branch, 4);

        // Verify halstead is initially None
        assert!(syntax.functions[0].halstead.is_none());
    }

    #[test]
    fn combine_analysis_merges_syntax_correctly() {
        let source = r#"
package main

import "fmt"

func main() {
    someNumber := 10
    someOtherNumber := 11
    fmt.Println("Some text", someNumber + someOtherNumber)
}
"#;
        let profile = LanguageConfig::new(ProgrammingLanguage::Golang);
        let file = std::env::current_dir().unwrap().join("test.go");
        let uri = Url::from_file_path(&file).expect("url failed in test");
        let source_input = ProcessorSource::from_text(source, file.clone());
        let processor = Processor::from_source_input(&profile, source_input);
        let ast_processor = AstProcessor::new(&profile, source, file.clone(), uri);

        let syntax = processor.compute_syntax_analysis(&ast_processor).unwrap();
        let analysis = Processor::combine_analysis(file.clone(), syntax, None, None);

        // Verify Analysis contains all SyntaxAnalysis data
        assert_eq!(analysis.lines_of_code, 10);  // 9 lines + 1 for blank line metric
        assert_eq!(analysis.functions.len(), 1);
        assert_eq!(analysis.file, file);
        assert!(analysis.functions[0].halstead.is_none());
    }
}
