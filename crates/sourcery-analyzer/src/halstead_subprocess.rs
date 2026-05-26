use anyhow::Result;
use std::path::PathBuf;
use std::collections::HashMap;
use crate::processor::{HalsteadMetrics, FunctionAnalysis};

/// Result of halstead metrics computation from a subprocess
#[derive(Debug, Clone)]
pub struct HalsteadSubprocessResult {
    /// Maps function index to halstead metrics
    pub function_metrics: HashMap<usize, HalsteadMetrics>,
    /// File-level aggregated metrics
    pub file_metrics: HalsteadMetrics,
}

/// Request to halstead subprocess
#[derive(Debug, serde::Serialize)]
pub struct HalsteadRequest {
    pub file_path: PathBuf,
    pub language: String,
    pub source: String,
    pub functions: Vec<HalsteadFunctionRequest>,
}

#[derive(Debug, serde::Serialize)]
pub struct HalsteadFunctionRequest {
    pub index: usize,
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Response from halstead subprocess
#[derive(Debug, serde::Deserialize)]
pub struct HalsteadResponse {
    pub file_metrics: HalsteadMetricsJson,
    pub function_metrics: HashMap<usize, HalsteadMetricsJson>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct HalsteadMetricsJson {
    pub unique_operators: usize,
    pub unique_operands: usize,
    pub operands: usize,
    pub operators: usize,
}

impl From<HalsteadMetricsJson> for HalsteadMetrics {
    fn from(json: HalsteadMetricsJson) -> Self {
        HalsteadMetrics {
            unique_operators: json.unique_operators,
            unique_operands: json.unique_operands,
            operands: json.operands,
            operators: json.operators,
        }
    }
}

/// Spawn a subprocess to compute halstead metrics.
/// 
/// This is a placeholder implementation. In production, this would:
/// 1. Serialize the request to JSON
/// 2. Spawn a language-specific subprocess (go or ocaml binary)
/// 3. Send the request over stdin
/// 4. Parse the JSON response
/// 5. Map results back to functions
pub async fn spawn_halstead_metrics_process(
    _file_path: &PathBuf,
    _language: &str,
    _source: &str,
    _functions: &[FunctionAnalysis],
) -> Result<HalsteadSubprocessResult> {
    // For now, return a placeholder that indicates the subprocess feature is not yet implemented.
    // In Phase 2, we would:
    // 1. Check if the subprocess binary is available
    // 2. Spawn it with the appropriate language parameter
    // 3. Send serialized request
    // 4. Parse and return results
    
    tracing::debug!("halstead metrics subprocess not yet implemented; skipping");
    
    // Return empty result - will be populated when subprocess is available
    Ok(HalsteadSubprocessResult {
        function_metrics: HashMap::new(),
        file_metrics: HalsteadMetrics::default(),
    })
}

/// Apply halstead metrics from subprocess result to functions.
pub fn apply_halstead_to_functions(
    functions: &mut [FunctionAnalysis],
    result: &HalsteadSubprocessResult,
) {
    for (index, func) in functions.iter_mut().enumerate() {
        if let Some(metrics) = result.function_metrics.get(&index) {
            func.halstead = Some(*metrics);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halstead_metrics_json_conversion() {
        let json = HalsteadMetricsJson {
            unique_operators: 5,
            unique_operands: 10,
            operands: 20,
            operators: 15,
        };
        
        let metrics: HalsteadMetrics = json.into();
        assert_eq!(metrics.unique_operators, 5);
        assert_eq!(metrics.unique_operands, 10);
        assert_eq!(metrics.operands, 20);
        assert_eq!(metrics.operators, 15);
    }

    #[test]
    fn test_apply_halstead_to_functions() {
        let mut functions = vec![
            FunctionAnalysis {
                function_name: "func1".into(),
                name: Default::default(),
                definition: Default::default(),
                definition_line_span: Default::default(),
                definition_position_range: Default::default(),
                function_length: 10,
                cyclomatic: 1,
                cyclomatic_match_as_single_branch: 1,
                functions_called: vec![],
                references: vec![],
                enriched_calls: vec![],
                halstead: None,
            },
        ];

        let mut function_metrics = HashMap::new();
        function_metrics.insert(
            0,
            HalsteadMetrics {
                unique_operators: 5,
                unique_operands: 10,
                operands: 20,
                operators: 15,
            },
        );

        let result = HalsteadSubprocessResult {
            function_metrics,
            file_metrics: HalsteadMetrics::default(),
        };

        apply_halstead_to_functions(&mut functions, &result);
        assert!(functions[0].halstead.is_some());
        assert_eq!(functions[0].halstead.unwrap().unique_operators, 5);
    }
}
