# Correlation Pairing

`correlation.py` compares lines of code against other metrics by building pairs from the same raw observation.

## Spearman Correlation

Spearman correlation is order-insensitive. The rows do not need to be sorted by time or version for the result to make sense.

It answers this question:

```text
When LOC is larger, is the other metric usually larger too?
```

For example, for LOC versus cyclomatic complexity, Spearman checks whether larger code observations usually also have larger cyclomatic complexity values.

It does not answer this question:

```text
Does this metric increase over time?
```

To measure whether a metric increases over time, correlate `version_number` or `input_index` with that metric instead.

## Pair Construction

The script does not compare averaged language/version points for file and function metrics. It pairs raw rows that describe the same observation, then compares LOC against the selected metric inside that observation.

### Version Level

Version-level rows come from `version-metrics.csv`.

Rows are paired when these columns match:

- `input_index`
- `codebase_id`
- `version_id`
- `sample_number`
- `programming_language`

Example pair:

```text
same project + same version + same sample
total_lines_of_code = 12000
total_cyclomatic = 900
```

This becomes:

```text
(LOC=12000, metric=900)
```

### File Level

File-level rows come from the raw `metrics*.csv` files.

Rows are paired when these columns match:

- `input_index`
- `codebase_id`
- `version_id`
- `sample_number`
- `programming_language`
- `file_path`

Example pair:

```text
same project + same version + same sample + same file
lines_of_code = 120
total_cyclomatic = 18
```

This becomes:

```text
(LOC=120, metric=18)
```

### Function Level

Function-level rows come from the raw `metrics*.csv` files.

Rows are paired when these columns match:

- `input_index`
- `codebase_id`
- `version_id`
- `sample_number`
- `programming_language`
- `file_path`
- `function_name`
- `function_start_line`
- `function_end_line`

Example pair:

```text
same project + same version + same sample + same file + same function location
function_length = 40
cyclomatic = 7
```

This becomes:

```text
(LOC=40, metric=7)
```

## Model Fitting

After pairs are built, the script calculates Spearman correlation and tries the configured regression models.

Models that require invalid transformations are skipped. For example:

- Power-law models are skipped when LOC or the metric contains zero or negative values.
- Logarithmic-growth models are skipped when LOC contains zero or negative values.
- Exponential-growth models are skipped when the metric contains zero or negative values.
- Any model with too few observations or no variation is skipped.

Skipped models are written with `NaN` model values, so they are not selected as the best model.
