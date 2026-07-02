# Scientific Assessment of LOC Proportionality

## Purpose

This document describes how `proportionality.py` investigates whether software metrics are
proportional to lines of code (LOC), summarizes the results produced from the current data, and
interprets what those results imply.

The analysis uses the current generated output in `proportionality_over_versions_results.csv`,
produced from `metrics1.csv` through `metrics10.csv`.

## Research Question

The central question is whether each metric behaves as a proportional function of LOC at the same
measurement level.

A strictly proportional relationship has the form:

```text
metric_value = constant * LOC
```

Under strict proportionality, larger code units should have metric values that increase linearly
with LOC, and the relationship should approximately pass through the origin. In practical terms, if
LOC is zero, the metric should also be zero.

## Data Preparation

The script reads one or more raw metrics CSV files. Each input row must contain at least:

| Column         | Role                                                                        |
| -------------- | --------------------------------------------------------------------------- |
| `metric_level` | Defines whether the metric is measured at file, function, or version level. |
| `metric_key`   | Identifies the metric.                                                      |
| `value`        | Numeric value of the metric.                                                |

The script validates the data before analysis. Missing required fields, non-numeric metric values, and non-numeric `version_number` values cause a hard error. This avoids silently dropping malformed rows and makes data problems explicit.

The LOC reference metric depends on the metric level:

| Metric level | LOC reference metric  |
| ------------ | --------------------- |
| `file`       | `lines_of_code`       |
| `function`   | `function_length`     |
| `version`    | `total_lines_of_code` |

For each non-LOC metric, the script pairs each metric value with the corresponding LOC value using identity columns such as input index, codebase, version, sample number, programming language, file path, and function location where applicable. Analyses are then performed separately by `programming_language`.

## Statistical Method

For each metric, language, and metric level, the script computes several complementary measures.

| Measure                           | Purpose                                                                                                |
| --------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Pearson correlation               | Measures linear association between LOC and the metric.                                                |
| Spearman correlation              | Measures monotonic association, including nonlinear monotonic patterns.                                |
| Ordinary least squares regression | Estimates `metric_value = slope * LOC + intercept`.                                                    |
| Linear R2                         | Measures how much metric variation is explained by the fitted linear model.                            |
| Linear slope p-value              | Tests whether the linear slope differs from zero.                                                      |
| Intercept p-value                 | Tests whether the fitted intercept differs from zero.                                                  |
| Through-origin R2                 | Measures fit quality when the model is forced through zero.                                            |
| Log-log regression                | Estimates scaling behavior using `log(metric_value)` versus `log(LOC)` for positive values.            |
| Confidence intervals              | Reports 90%, 95%, and 99% t-based intervals for the linear slope, linear intercept, and log-log slope. |

The fitted linear model is:

```text
metric_value = linear_slope * LOC + linear_intercept
```

The residual degrees of freedom are `n - 2`, because the model estimates two parameters: slope and intercept.

The log-log model is:

```text
log(metric_value) = log_log_slope * log(LOC) + intercept
```

The log-log slope is useful for identifying scaling behavior. A log-log slope near 1 is consistent with approximately linear scaling. Values above 1 suggest superlinear growth, and values below 1 suggest sublinear growth.

## Classification Rule

The script classifies each metric-language-level combination using the following heuristic rule at `alpha = 0.05`:

| Verdict                                 | Rule                                                                                                              |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `insufficient data`                     | Fewer than the minimum required paired observations, or no valid linear R2.                                       |
| `no significant linear relationship`    | Linear slope p-value is not significant.                                                                          |
| `strong proportional evidence`          | Significant linear slope, `linear_r2 >= 0.8`, `intercept_p >= 0.05`, and `origin_r2 >= 0.75`.                     |
| `LOC-shaped, not strictly proportional` | Significant linear slope and `linear_r2 >= 0.5`, but the stricter proportionality criteria are not all satisfied. |
| `weak LOC relationship`                 | Significant linear slope but `linear_r2 < 0.5`.                                                                   |

This rule is an exploratory classification, not a formal proof of proportionality. In particular,
`intercept_p >= 0.05` means the analysis did not detect evidence that the intercept differs from
zero; it does not prove that the intercept is exactly zero.

## Results

The current run produced 88 analyzed metric-language-level combinations. The generated output
contains file-level and function-level tests. No version-level result rows were produced in the
current result CSV.

### Overall Verdict Counts

| Verdict                                 | Count |
| --------------------------------------- | ----: |
| `weak LOC relationship`                 |    44 |
| `LOC-shaped, not strictly proportional` |    40 |
| `no significant linear relationship`    |     2 |
| `strong proportional evidence`          |     2 |

### Verdicts by Metric Level

| Metric level | LOC-shaped, not strictly proportional | No significant linear relationship | Strong proportional evidence | Weak LOC relationship |
| ------------ | ------------------------------------: | ---------------------------------: | ---------------------------: | --------------------: |
| `file`       |                                    23 |                                  0 |                            0 |                    25 |
| `function`   |                                    17 |                                  2 |                            2 |                    19 |

### Verdicts by Language

| Language | LOC-shaped, not strictly proportional | No significant linear relationship | Strong proportional evidence | Weak LOC relationship |
| -------- | ------------------------------------: | ---------------------------------: | ---------------------------: | --------------------: |
| Golang   |                                    26 |                                  1 |                            0 |                    17 |
| Ocaml    |                                    14 |                                  1 |                            2 |                    27 |

### Strong Proportional Evidence

Only two metric combinations met the full strict proportionality criteria. Both were function-level Ocaml Halstead metrics:

| Metric level | Language | Metric            |      n | Linear R2 | Origin R2 | Linear slope | 95% slope CI           | Intercept | 95% intercept CI      | Intercept p-value | Log-log slope | 95% log-log slope CI |
| ------------ | -------- | ----------------- | -----: | --------: | --------: | -----------: | ---------------------- | --------: | --------------------- | ----------------: | ------------: | -------------------- |
| function     | Ocaml    | `halstead_bugs`   | 156635 |    0.9662 |    0.9662 |     0.011261 | [0.011250, 0.011271]   | -0.001093 | [-0.002710, 0.000524] |            0.1852 |        0.8988 | [0.8958, 0.9019]     |
| function     | Ocaml    | `halstead_volume` | 156635 |    0.9662 |    0.9662 |    33.782106 | [33.750798, 33.813414] | -3.279178 | [-8.129758, 1.571403] |            0.1852 |        0.8988 | [0.8958, 0.9019]     |

These metrics have high linear R2, high through-origin R2, and confidence intervals for the intercept that include zero. They therefore satisfy the script's operational definition of strong proportional evidence.

### LOC-Shaped but Not Strictly Proportional

Forty combinations were classified as LOC-shaped but not strictly proportional. These metrics usually show a clear linear relationship with LOC, but they fail at least one strict proportionality condition. Common reasons include a statistically nonzero intercept, a weaker through-origin fit, or scaling behavior that does not cleanly match strict proportionality.

Examples with high linear R2 include:

| Metric level | Language | Metric                                  |      n | Linear R2 | Origin R2 | Linear slope | 95% slope CI         |  Intercept | Intercept p-value | Log-log slope |
| ------------ | -------- | --------------------------------------- | -----: | --------: | --------: | -----------: | -------------------- | ---------: | ----------------: | ------------: |
| file         | Golang   | `effective_lines_of_code_with_brackets` |  13126 |    0.9849 |    0.9844 |     0.853505 | [0.851700, 0.855311] |  -8.970647 |         2.46e-108 |        1.0359 |
| function     | Ocaml    | `halstead_calculated_length`            | 156635 |    0.9848 |    0.9848 |     9.902348 | [9.896253, 9.908443] |   5.871140 |          3.84e-34 |        0.7216 |
| function     | Ocaml    | `halstead_unique_operands`              | 156635 |    0.9790 |    0.9717 |     0.698949 | [0.698443, 0.699456] |   9.378858 |               0.0 |        0.5204 |
| function     | Ocaml    | `halstead_vocabulary`                   | 156635 |    0.9740 |    0.9562 |     0.701671 | [0.701104, 0.702239] |  14.718174 |               0.0 |        0.5114 |
| file         | Golang   | `effective_lines_of_code`               |  13126 |    0.9592 |    0.9569 |     0.736147 | [0.733550, 0.738745] | -15.875994 |         2.31e-161 |        1.0302 |

These examples show that high R2 alone is not sufficient for strict proportionality. Several metrics are strongly LOC-shaped but have intercepts that are statistically distinguishable from zero because the sample sizes are large and standard errors are small.

### Weak LOC Relationships

Forty-four combinations were classified as weak LOC relationships. These metrics may still have statistically significant slopes, but LOC explains less than half of the observed variance under the linear model.

Examples near the upper boundary of the weak class include:

| Metric level | Language | Metric                                 |     n | Linear R2 | Origin R2 | Linear slope | 95% slope CI               |     Intercept | Intercept p-value | Log-log slope |
| ------------ | -------- | -------------------------------------- | ----: | --------: | --------: | -----------: | -------------------------- | ------------: | ----------------: | ------------: |
| function     | Golang   | `maintainability_index_three_property` | 99867 |    0.4843 |  -16.3528 |    -0.485134 | [-0.488238, -0.482029]     |    113.991457 |               0.0 |       -0.2266 |
| file         | Golang   | `total_halstead_calculated_length`     | 13126 |    0.4689 |    0.4555 |     7.957808 | [7.812900, 8.102716]       |   -586.269957 |          6.59e-73 |        0.9570 |
| function     | Golang   | `maintainability_index_visual_studio`  | 99867 |    0.4686 |  -16.4054 |    -0.278368 | [-0.280207, -0.276530]     |     66.567967 |               0.0 |       -0.2266 |
| function     | Golang   | `halstead_effort`                      | 99867 |    0.4656 |    0.4221 |  1202.700227 | [1194.708893, 1210.691562] | -14644.267083 |               0.0 |        1.6559 |

The weak category includes several maintainability-index metrics. Their negative slopes indicate that maintainability scores tend to decrease as LOC increases, but the relationship is not proportional in the positive multiplicative sense used by this analysis.

### No Significant Linear Relationship

Two combinations did not show a statistically significant linear slope:

| Metric level | Language | Metric     |      n | Linear R2 | Linear p-value | Pearson r | Spearman r |
| ------------ | -------- | ---------- | -----: | --------: | -------------: | --------: | ---------: |
| function     | Golang   | `indegree` |  99867 |  0.000037 |         0.0543 |   -0.0061 |     0.1004 |
| function     | Ocaml    | `indegree` | 165282 |  0.000003 |         0.4969 |    0.0017 |     0.1575 |

At function level, `indegree` is therefore not meaningfully explained by function length in this dataset.

## Interpretation

The main result is that LOC is often associated with metric values, but strict proportionality is rare.

Most metrics fall into one of two categories. Some are LOC-shaped, meaning they increase or decrease systematically with LOC and have moderate to high linear R2. Others have weak relationships, meaning LOC alone does not explain enough variation to treat the metric as primarily size-driven.

Only function-level Ocaml `halstead_bugs` and `halstead_volume` satisfy the full strict proportionality criteria. This means that, for these two metrics in this dataset, the evidence is consistent with a near-through-origin linear relationship between function length and the metric value.

The high number of LOC-shaped but not strictly proportional results is important. It indicates that many metrics are size-sensitive, but they are not merely rescaled versions of LOC. They may include fixed offsets, language-specific structure, code-style effects, library conventions, nesting, operator/operand diversity, or other design properties that do not grow exactly in direct proportion to LOC.

The maintainability metrics should not be interpreted as proportional to LOC. They often have negative slopes and poor through-origin behavior, which is expected for index-like scores. Such metrics are bounded or transformed summaries rather than additive counts.

The function-level `indegree` result suggests that dependency structure at the function level is not captured by function length alone. This is consistent with the idea that graph connectivity is a structural property rather than a size property.

## Scientific Caveats

The analysis is best interpreted as exploratory statistical evidence rather than definitive proof.

Several caveats apply:

| Caveat                                                   | Consequence                                                                            |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Many metrics are tested                                  | P-values are not adjusted for multiple comparisons.                                    |
| Very large sample sizes                                  | Small deviations from zero intercept can become statistically significant.             |
| Linear regression assumptions are not checked            | Residual normality, heteroscedasticity, and influential outliers may affect inference. |
| Pairing depends on identity columns                      | Results depend on how corresponding LOC and metric rows are matched.                   |
| Non-significant intercept is not proof of zero intercept | It only means the script did not detect a statistically significant nonzero intercept. |
| Version-level rows are absent in the current output      | No conclusion should be drawn about version-level proportionality from this run.       |

## Conclusion

The current evidence supports the claim that many software metrics are influenced by LOC, but it does not support the stronger claim that most metrics are strictly proportional to LOC.

Strict proportionality appears only for a small subset of Halstead metrics at Ocaml function level. Many other metrics are LOC-shaped, meaning size is an important explanatory factor, but not the whole explanation. Metrics related to maintainability and dependency structure show especially weak or non-proportional behavior, suggesting that they capture properties beyond simple code size.

The practical implication is that LOC should be treated as an important baseline covariate when interpreting many software metrics. However, replacing those metrics with LOC, or assuming they are simple multiples of LOC, would discard meaningful information for most metrics in this dataset.
