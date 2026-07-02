import argparse
import math
from pathlib import Path

import pandas as pd
from scipy import stats


METRIC_LEVEL_COL = "metric_level"
METRIC_COL = "metric_key"
VALUE_COL = "value"
LANGUAGE_COL = "programming_language"
LOC_METRIC_BY_LEVEL = {
    "file": "lines_of_code",
    "function": "function_length",
    "version": "total_lines_of_code",
}
DEFAULT_OUTPUT_PATH = Path("proportionality_report.md")
DEFAULT_SUMMARY_PATH = Path("proportionality_results.csv")
DEFAULT_INPUT_PATHS = [
    *(Path(f"metrics{index}.csv") for index in range(1, 11)),
    Path("version-metrics.csv"),
]
DEFAULT_OVER_VERSION_AGGREGATE_PATHS = [
    Path("metrics_over_versions_file.csv"),
    Path("metrics_over_versions_function.csv"),
    Path("metrics_over_versions_version.csv"),
]
MIN_OBSERVATIONS = 3
CONFIDENCE_LEVELS = {
    "90": 0.90,
    "95": 0.95,
    "99": 0.99,
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Test whether each metric is shaped by lines of code at file, "
            "function, and version levels, grouped by programming language. "
            "Function-level LOC uses function_length."
        )
    )
    parser.add_argument(
        "csv_paths",
        nargs="*",
        type=Path,
        default=None,
        help=(
            "raw metrics CSV files; defaults to metrics1.csv through metrics10.csv "
            "plus version-metrics.csv, or metrics_over_versions_<level>.csv when "
            "--over-version-aggregates is set"
        ),
    )
    parser.add_argument(
        "--over-version-aggregates",
        action="store_true",
        help=(
            "use metrics_over_versions_file.csv, metrics_over_versions_function.csv, "
            "and metrics_over_versions_version.csv as the default inputs"
        ),
    )
    parser.add_argument(
        "--statistics",
        nargs="+",
        choices=["Mean", "Median"],
        help=(
            "limit aggregate inputs to one or more statistics, for example "
            "--statistics Mean or --statistics Mean Median"
        ),
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT_PATH,
        help=f"Markdown report output path (default: {DEFAULT_OUTPUT_PATH})",
    )
    parser.add_argument(
        "--summary-csv",
        type=Path,
        default=DEFAULT_SUMMARY_PATH,
        help=f"CSV summary output path (default: {DEFAULT_SUMMARY_PATH})",
    )
    parser.add_argument(
        "--alpha",
        type=float,
        default=0.05,
        help="significance threshold for p-values (default: 0.05)",
    )
    parser.add_argument(
        "--min-observations",
        type=int,
        default=MIN_OBSERVATIONS,
        help=f"minimum paired rows needed per test (default: {MIN_OBSERVATIONS})",
    )
    args = parser.parse_args()
    if not args.csv_paths:
        args.csv_paths = (
            DEFAULT_OVER_VERSION_AGGREGATE_PATHS
            if args.over_version_aggregates
            else DEFAULT_INPUT_PATHS
        )
    return args


def load_metrics(csv_paths: list[Path]) -> pd.DataFrame:
    frames = []
    required_columns = {METRIC_LEVEL_COL, METRIC_COL, VALUE_COL}
    for input_index, csv_path in enumerate(csv_paths, start=1):
        try:
            df = pd.read_csv(csv_path)
        except Exception as exc:
            raise ValueError(f"Failed to read CSV {csv_path}: {exc}") from exc

        missing_columns = required_columns - set(df.columns)
        if missing_columns:
            raise ValueError(
                f"CSV {csv_path} is missing required columns: "
                + ", ".join(sorted(missing_columns))
            )

        missing_required = df[list(required_columns)].isna().any(axis=1)
        if missing_required.any():
            bad_rows = []
            for row_index, row in df.loc[missing_required].head(10).iterrows():
                missing = [col for col in sorted(required_columns) if pd.isna(row[col])]
                bad_rows.append(f"line {row_index + 2}: {', '.join(missing)}")
            raise ValueError(
                f"CSV {csv_path} has missing required values: " + "; ".join(bad_rows)
            )

        df = df.copy()
        if "input_index" not in df.columns:
            df["input_index"] = input_index
        if "version_number" in df.columns:
            version_numbers = pd.to_numeric(df["version_number"], errors="coerce")
            invalid_versions = version_numbers.isna() & df["version_number"].notna()
            if invalid_versions.any():
                bad_rows = [
                    f"line {row_index + 2}: {df.at[row_index, 'version_number']!r}"
                    for row_index in df.index[invalid_versions][:10]
                ]
                raise ValueError(
                    f"CSV {csv_path} has non-numeric version_number values: "
                    + "; ".join(bad_rows)
                )
            df["input_index"] = version_numbers.fillna(df["input_index"])

        values = pd.to_numeric(df[VALUE_COL], errors="coerce")
        invalid_values = values.isna() & df[VALUE_COL].notna()
        if invalid_values.any():
            bad_rows = [
                f"line {row_index + 2}: {df.at[row_index, VALUE_COL]!r}"
                for row_index in df.index[invalid_values][:10]
            ]
            raise ValueError(
                f"CSV {csv_path} has non-numeric {VALUE_COL} values: "
                + "; ".join(bad_rows)
            )
        df[VALUE_COL] = values
        frames.append(df)

    df = pd.concat(frames, ignore_index=True)
    return df.copy()


def filter_statistics(df: pd.DataFrame, statistics: list[str] | None) -> pd.DataFrame:
    if not statistics:
        return df
    if "statistic" not in df.columns:
        raise ValueError("--statistics requires input data with a statistic column")
    return df[df["statistic"].isin(statistics)].copy()


def identity_columns(df: pd.DataFrame, level: str) -> list[str]:
    candidates = [
        "input_index",
        "panel",
        "statistic",
        "codebase_id",
        "version_id",
        "sample_number",
        "programming_language",
        "file_path",
        "function_name",
        "function_start_line",
        "function_end_line",
    ]
    cols = [col for col in candidates if col in df.columns]
    if level == "version":
        return [
            col
            for col in cols
            if col
            in {
                "input_index",
                "panel",
                "statistic",
                "codebase_id",
                "version_id",
                "sample_number",
                LANGUAGE_COL,
            }
        ]
    if level == "file":
        return [col for col in cols if not col.startswith("function_") and col != "function_name"]
    return cols


def build_pairs(level_df: pd.DataFrame, level: str, metric: str) -> pd.DataFrame:
    loc_metric = LOC_METRIC_BY_LEVEL[level]
    key_cols = identity_columns(level_df, level)
    loc_df = (
        level_df[level_df[METRIC_COL] == loc_metric][key_cols + [VALUE_COL]]
        .rename(columns={VALUE_COL: "loc"})
        .drop_duplicates(subset=key_cols)
    )
    metric_df = (
        level_df[level_df[METRIC_COL] == metric][key_cols + [VALUE_COL]]
        .rename(columns={VALUE_COL: "metric_value"})
        .drop_duplicates(subset=key_cols)
    )
    pairs = loc_df.merge(metric_df, on=key_cols, how="inner")
    return pairs.dropna(subset=["loc", "metric_value"]).copy()


def safe_corr(method, x: pd.Series, y: pd.Series) -> tuple[float, float]:
    if x.nunique() < 2 or y.nunique() < 2:
        return math.nan, math.nan
    result = method(x, y)
    return float(result.statistic), float(result.pvalue)


def correlation_stats(prefix: str, x: pd.Series, y: pd.Series) -> dict[str, float]:
    pearson_r, pearson_p = safe_corr(stats.pearsonr, x, y)
    spearman_r, spearman_p = safe_corr(stats.spearmanr, x, y)
    return {
        f"{prefix}_pearson_r": pearson_r,
        f"{prefix}_pearson_p": pearson_p,
        f"{prefix}_spearman_r": spearman_r,
        f"{prefix}_spearman_p": spearman_p,
    }


def model_correlation_stats(
    x: pd.Series, y: pd.Series, min_observations: int
) -> dict[str, float]:
    empty = pd.Series(dtype=float)
    result = {
        **correlation_stats("linear", x, y),
        **correlation_stats("origin", x, y),
    }

    log_log_positive = (x > 0) & (y > 0)
    log_log_x = x[log_log_positive].map(math.log)
    log_log_y = y[log_log_positive].map(math.log)
    if len(log_log_x) < min_observations:
        log_log_x = empty
        log_log_y = empty
    result.update(correlation_stats("log_log", log_log_x, log_log_y))

    exponential_positive = y > 0
    exponential_x = x[exponential_positive]
    exponential_y = y[exponential_positive].map(math.log)
    if len(exponential_x) < min_observations:
        exponential_x = empty
        exponential_y = empty
    result.update(correlation_stats("exponential", exponential_x, exponential_y))
    return result


def confidence_intervals(
    column_prefix: str,
    estimate: float,
    standard_error: float,
    degrees_of_freedom: int,
) -> dict[str, float]:
    intervals = {}
    for label, confidence_level in CONFIDENCE_LEVELS.items():
        margin_probability = (1 + confidence_level) / 2
        critical_value = (
            stats.t.ppf(margin_probability, degrees_of_freedom)
            if degrees_of_freedom > 0
            else math.nan
        )
        margin_of_error = (
            critical_value * standard_error
            if standard_error and standard_error > 0 and not math.isnan(critical_value)
            else math.nan
        )
        intervals[f"{column_prefix}_ci{label}_low"] = float(estimate - margin_of_error)
        intervals[f"{column_prefix}_ci{label}_high"] = float(estimate + margin_of_error)
    return intervals


def regression_stats(x: pd.Series, y: pd.Series) -> dict[str, float]:
    if x.nunique() < 2 or y.nunique() < 2:
        return {
            "linear_slope": math.nan,
            "linear_intercept": math.nan,
            **confidence_intervals("linear_slope", math.nan, math.nan, 0),
            **confidence_intervals("linear_intercept", math.nan, math.nan, 0),
            "linear_r2": math.nan,
            "linear_p": math.nan,
            "intercept_p": math.nan,
            "origin_slope": math.nan,
            **confidence_intervals("origin_slope", math.nan, math.nan, 0),
            "origin_r2": math.nan,
        }

    linear = stats.linregress(x, y)
    predicted_values = linear.slope * x + linear.intercept
    residuals = y - predicted_values
    degrees_of_freedom = len(x) - 2
    x_mean = x.mean()
    x_sum_of_squares = ((x - x_mean) ** 2).sum()
    residual_std = (
        math.sqrt((residuals**2).sum() / degrees_of_freedom)
        if degrees_of_freedom > 0
        else math.nan
    )
    intercept_standard_error = (
        residual_std * math.sqrt((1 / len(x)) + (x_mean**2 / x_sum_of_squares))
        if x_sum_of_squares > 0 and not math.isnan(residual_std)
        else math.nan
    )
    intercept_p = (
        2 * stats.t.sf(abs(linear.intercept / intercept_standard_error), degrees_of_freedom)
        if intercept_standard_error
        and intercept_standard_error > 0
        and degrees_of_freedom > 0
        else math.nan
    )

    origin_slope = float((x * y).sum() / (x * x).sum()) if (x * x).sum() else math.nan
    origin_predicted_values = origin_slope * x
    origin_sum_of_squared_errors = float(((y - origin_predicted_values) ** 2).sum())
    origin_degrees_of_freedom = len(x) - 1
    origin_residual_std = (
        math.sqrt(origin_sum_of_squared_errors / origin_degrees_of_freedom)
        if origin_degrees_of_freedom > 0
        else math.nan
    )
    origin_slope_standard_error = (
        origin_residual_std / math.sqrt(float((x * x).sum()))
        if (x * x).sum() > 0 and not math.isnan(origin_residual_std)
        else math.nan
    )
    total_sum_of_squares = float(((y - y.mean()) ** 2).sum())
    origin_r2 = (
        1 - (origin_sum_of_squared_errors / total_sum_of_squares)
        if total_sum_of_squares
        else math.nan
    )

    return {
        "linear_slope": float(linear.slope),
        "linear_intercept": float(linear.intercept),
        **confidence_intervals(
            "linear_slope",
            float(linear.slope),
            float(linear.stderr),
            degrees_of_freedom,
        ),
        **confidence_intervals(
            "linear_intercept",
            float(linear.intercept),
            float(intercept_standard_error),
            degrees_of_freedom,
        ),
        "linear_r2": float(linear.rvalue**2),
        "linear_p": float(linear.pvalue),
        "intercept_p": float(intercept_p),
        "origin_slope": origin_slope,
        **confidence_intervals(
            "origin_slope",
            origin_slope,
            float(origin_slope_standard_error),
            origin_degrees_of_freedom,
        ),
        "origin_r2": origin_r2,
    }


def log_log_stats(x: pd.Series, y: pd.Series, min_observations: int) -> dict[str, float]:
    positive = (x > 0) & (y > 0)
    positive_x = x[positive]
    positive_y = y[positive]
    log_x = positive_x.map(math.log)
    log_y = positive_y.map(math.log)
    if len(log_x) < min_observations or log_x.nunique() < 2 or log_y.nunique() < 2:
        return {
            "log_log_n": int(len(log_x)),
            "log_log_slope": math.nan,
            **confidence_intervals("log_log_slope", math.nan, math.nan, 0),
            "log_log_r2": math.nan,
            "log_log_raw_r2": math.nan,
            "log_log_slope_eq_1_p": math.nan,
        }

    result = stats.linregress(log_x, log_y)
    degrees_of_freedom = len(log_x) - 2
    predicted_y = (result.slope * log_x + result.intercept).map(math.exp)
    sum_of_squared_errors = float(((positive_y - predicted_y) ** 2).sum())
    total_sum_of_squares = float(((positive_y - positive_y.mean()) ** 2).sum())
    raw_r2 = (
        1 - (sum_of_squared_errors / total_sum_of_squares)
        if total_sum_of_squares
        else math.nan
    )
    slope_eq_1_p = (
        2 * stats.t.sf(abs((result.slope - 1) / result.stderr), degrees_of_freedom)
        if result.stderr and result.stderr > 0 and degrees_of_freedom > 0
        else math.nan
    )
    return {
        "log_log_n": int(len(log_x)),
        "log_log_slope": float(result.slope),
        **confidence_intervals(
            "log_log_slope",
            float(result.slope),
            float(result.stderr),
            degrees_of_freedom,
        ),
        "log_log_r2": float(result.rvalue**2),
        "log_log_raw_r2": raw_r2,
        "log_log_slope_eq_1_p": float(slope_eq_1_p),
    }


def exponential_stats(x: pd.Series, y: pd.Series, min_observations: int) -> dict[str, float]:
    positive = y > 0
    exp_x = x[positive]
    positive_y = y[positive]
    log_y = positive_y.map(math.log)
    if len(exp_x) < min_observations or exp_x.nunique() < 2 or log_y.nunique() < 2:
        return {
            "exponential_n": int(len(exp_x)),
            "exponential_slope": math.nan,
            **confidence_intervals("exponential_slope", math.nan, math.nan, 0),
            "exponential_r2": math.nan,
            "exponential_raw_r2": math.nan,
        }

    result = stats.linregress(exp_x, log_y)
    degrees_of_freedom = len(exp_x) - 2
    predicted_y = (result.slope * exp_x + result.intercept).map(math.exp)
    sum_of_squared_errors = float(((positive_y - predicted_y) ** 2).sum())
    total_sum_of_squares = float(((positive_y - positive_y.mean()) ** 2).sum())
    raw_r2 = (
        1 - (sum_of_squared_errors / total_sum_of_squares)
        if total_sum_of_squares
        else math.nan
    )
    return {
        "exponential_n": int(len(exp_x)),
        "exponential_slope": float(result.slope),
        **confidence_intervals(
            "exponential_slope",
            float(result.slope),
            float(result.stderr),
            degrees_of_freedom,
        ),
        "exponential_r2": float(result.rvalue**2),
        "exponential_raw_r2": raw_r2,
    }


def classify(row: pd.Series, min_observations: int, alpha: float) -> str:
    if row["n"] < min_observations or math.isnan(row["linear_r2"]):
        return "insufficient data"
    if row["linear_p"] >= alpha:
        return "no significant linear relationship"
    if row["linear_r2"] >= 0.8 and row["intercept_p"] >= alpha and row["origin_r2"] >= 0.75:
        return "strong proportional evidence"
    if row["linear_r2"] >= 0.5:
        return "LOC-shaped, not strictly proportional"
    return "weak LOC relationship"


def star_rating(r2: float) -> int:
    if pd.isna(r2):
        return 0
    if r2 >= 0.99:
        return 3
    if r2 >= 0.95:
        return 2
    if r2 >= 0.8:
        return 1
    return 0


def best_regression_model(row: dict[str, object]) -> dict[str, object]:
    models = [
        (
            "linear",
            row["linear_r2"],
            row["linear_slope_ci95_low"],
            row["linear_slope_ci95_high"],
        ),
        (
            "through_origin_linear",
            row["origin_r2"],
            row["origin_slope_ci95_low"],
            row["origin_slope_ci95_high"],
        ),
        (
            "log_log",
            row["log_log_raw_r2"],
            row["log_log_slope_ci95_low"],
            row["log_log_slope_ci95_high"],
        ),
        (
            "exponential",
            row["exponential_raw_r2"],
            row["exponential_slope_ci95_low"],
            row["exponential_slope_ci95_high"],
        ),
    ]
    valid_models = [model for model in models if not pd.isna(model[1])]
    if not valid_models:
        return {
            "best_regression_model": "",
            "best_regression_r2": math.nan,
            "best_regression_slope_ci95_low": math.nan,
            "best_regression_slope_ci95_high": math.nan,
            "best_regression_star_rating": 0,
        }

    name, r2, ci95_low, ci95_high = max(valid_models, key=lambda model: model[1])
    return {
        "best_regression_model": name,
        "best_regression_r2": r2,
        "best_regression_slope_ci95_low": ci95_low,
        "best_regression_slope_ci95_high": ci95_high,
        "best_regression_star_rating": star_rating(r2),
    }


def analyze(df: pd.DataFrame, min_observations: int, alpha: float) -> pd.DataFrame:
    rows = []
    for level, loc_metric in LOC_METRIC_BY_LEVEL.items():
        level_df = df[df[METRIC_LEVEL_COL] == level]
        if level_df.empty or loc_metric not in set(level_df[METRIC_COL]):
            continue

        if LANGUAGE_COL not in level_df.columns:
            raise ValueError(f"input data must contain {LANGUAGE_COL}")
        languages = sorted(level_df[LANGUAGE_COL].dropna().unique())

        for metric in sorted(level_df[METRIC_COL].dropna().unique()):
            if metric == loc_metric:
                continue
            pairs = build_pairs(level_df, level, metric)

            for language in languages:
                language_pairs = pairs[pairs[LANGUAGE_COL] == language]
                if len(language_pairs) < min_observations:
                    regression = regression_stats(
                        pd.Series(dtype=float), pd.Series(dtype=float)
                    )
                    log_log = log_log_stats(
                        pd.Series(dtype=float),
                        pd.Series(dtype=float),
                        min_observations,
                    )
                    exponential = exponential_stats(
                        pd.Series(dtype=float),
                        pd.Series(dtype=float),
                        min_observations,
                    )
                    model_correlations = model_correlation_stats(
                        pd.Series(dtype=float),
                        pd.Series(dtype=float),
                        min_observations,
                    )
                    rows.append(
                        {
                            "metric_level": level,
                            LANGUAGE_COL: language,
                            "loc_metric": loc_metric,
                            "metric_key": metric,
                            "n": len(language_pairs),
                            "pearson_r": math.nan,
                            "pearson_p": math.nan,
                            "spearman_r": math.nan,
                            "spearman_p": math.nan,
                            **model_correlations,
                            **regression,
                            **log_log,
                            **exponential,
                            **best_regression_model(
                                {**regression, **log_log, **exponential}
                            ),
                            "verdict": "insufficient data",
                        }
                    )
                    continue

                x = language_pairs["loc"].astype(float)
                y = language_pairs["metric_value"].astype(float)
                pearson_r, pearson_p = safe_corr(stats.pearsonr, x, y)
                spearman_r, spearman_p = safe_corr(stats.spearmanr, x, y)
                model_correlations = model_correlation_stats(x, y, min_observations)
                row = {
                    "metric_level": level,
                    LANGUAGE_COL: language,
                    "loc_metric": loc_metric,
                    "metric_key": metric,
                    "n": len(language_pairs),
                    "pearson_r": pearson_r,
                    "pearson_p": pearson_p,
                    "spearman_r": spearman_r,
                    "spearman_p": spearman_p,
                    **model_correlations,
                    **regression_stats(x, y),
                    **log_log_stats(x, y, min_observations),
                    **exponential_stats(x, y, min_observations),
                }
                row.update(best_regression_model(row))
                row["verdict"] = classify(pd.Series(row), min_observations, alpha)
                rows.append(row)

    if not rows:
        return pd.DataFrame()
    return pd.DataFrame(rows).sort_values(
        ["metric_level", LANGUAGE_COL, "verdict", "metric_key"]
    )


def fmt(value: object) -> str:
    if pd.isna(value):
        return ""
    if isinstance(value, float):
        return f"{value:.4g}"
    return str(value)


def write_report(results: pd.DataFrame, output_path: Path, alpha: float) -> None:
    lines = [
        "# LOC Proportionality Report",
        "",
        "This tests whether each metric is shaped by LOC at the same metric level.",
        "Each test is run separately per `programming_language`.",
        "Function-level LOC uses `function_length`; file-level LOC uses `lines_of_code`; version-level LOC uses `total_lines_of_code`.",
        "",
        "Strong proportional evidence requires significant linear slope, high linear R2, intercept not significantly different from zero, and good through-origin fit.",
        f"Alpha: `{alpha}`.",
        "",
    ]

    if results.empty:
        lines.extend(["No analyzable metric pairs found.", ""])
        output_path.write_text("\n".join(lines))
        return

    for level in ["file", "function", "version"]:
        level_results = results[results["metric_level"] == level]
        if level_results.empty:
            continue
        lines.extend([f"## {level.title()} Level", ""])
        for language, language_df in level_results.groupby(LANGUAGE_COL, sort=True):
            lines.extend([f"### {language}", ""])
            for verdict, verdict_df in language_df.groupby("verdict", sort=False):
                lines.extend([f"#### {verdict}", ""])
                cols = [
                    "metric_key",
                    "n",
                    "pearson_r",
                    "spearman_r",
                    "linear_pearson_r",
                    "linear_spearman_r",
                    "origin_pearson_r",
                    "origin_spearman_r",
                    "log_log_pearson_r",
                    "log_log_spearman_r",
                    "exponential_pearson_r",
                    "exponential_spearman_r",
                    "best_regression_model",
                    "best_regression_r2",
                    "best_regression_slope_ci95_low",
                    "best_regression_slope_ci95_high",
                    "best_regression_star_rating",
                    "linear_r2",
                    "origin_r2",
                    "linear_slope",
                    "linear_slope_ci90_low",
                    "linear_slope_ci90_high",
                    "linear_slope_ci95_low",
                    "linear_slope_ci95_high",
                    "linear_slope_ci99_low",
                    "linear_slope_ci99_high",
                    "linear_intercept",
                    "linear_intercept_ci90_low",
                    "linear_intercept_ci90_high",
                    "linear_intercept_ci95_low",
                    "linear_intercept_ci95_high",
                    "linear_intercept_ci99_low",
                    "linear_intercept_ci99_high",
                    "intercept_p",
                    "log_log_slope",
                    "log_log_slope_ci90_low",
                    "log_log_slope_ci90_high",
                    "log_log_slope_ci95_low",
                    "log_log_slope_ci95_high",
                    "log_log_slope_ci99_low",
                    "log_log_slope_ci99_high",
                    "log_log_r2",
                    "log_log_raw_r2",
                    "exponential_slope",
                    "exponential_slope_ci90_low",
                    "exponential_slope_ci90_high",
                    "exponential_slope_ci95_low",
                    "exponential_slope_ci95_high",
                    "exponential_slope_ci99_low",
                    "exponential_slope_ci99_high",
                    "exponential_r2",
                    "exponential_raw_r2",
                ]
                lines.append("| " + " | ".join(cols) + " |")
                lines.append("| " + " | ".join(["---"] * len(cols)) + " |")
                for _, row in verdict_df[cols].iterrows():
                    lines.append("| " + " | ".join(fmt(row[col]) for col in cols) + " |")
                lines.append("")

    output_path.write_text("\n".join(lines))


if __name__ == "__main__":
    args = parse_args()
    metrics_df = load_metrics(args.csv_paths)
    metrics_df = filter_statistics(metrics_df, args.statistics)
    results_df = analyze(metrics_df, args.min_observations, args.alpha)
    results_df.to_csv(args.summary_csv, index=False)
    write_report(results_df, args.output, args.alpha)
    print(f"Wrote proportionality summary to {args.summary_csv}")
    print(f"Wrote proportionality report to {args.output}")
