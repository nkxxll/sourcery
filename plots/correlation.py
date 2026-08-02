import argparse
import math
from pathlib import Path
from sys import stderr

import matplotlib.pyplot as plt
from matplotlib.lines import Line2D
import numpy as np
import pandas as pd
from scipy import stats

from plot_callgraph_results import (
    METRICS_BY_LEVEL as UPDATED_DEGREE_METRICS_BY_LEVEL,
    VERSION_METRICS as UPDATED_VERSION_METRICS,
    build_version_metrics,
    load_results,
    load_size_metrics,
)

VALUE_COL = "value"
METRIC_KEY = "metric_key"
LANGUAGE_COL = "programming_language"
VERSION_COL = "input_index"
VERSION_NUMBER_COL = "version_number"
METRIC_LEVEL_COL = "metric_level"
LOC_METRIC_VERSION = "total_lines_of_code"
LOC_METRIC_FILE = "lines_of_code"
LOC_METRIC_FUNCTION = "function_length"
MIN_OBSERVATIONS = 3
SPEARMAN_THRESHOLD = 0.8
R2_THRESHOLD = 0.8
VERSION_MODEL_PLOTS_DIR = Path("version_metric_model_plots")
DEFAULT_GRID_POINT_COLOR = "#1f77b4"
OCAML_GRID_POINT_COLOR = "#ff7f0e"
SPLOT_VARIABLES = {
    # Add a (label, metric_key) pair here when a variable should be included.
    # None means that the metric is not available at that analysis level.
    "version": [
        ("Lines of Code", LOC_METRIC_VERSION),
        ("Cyclomatic Complexity", "total_cyclomatic"),
        ("Halstead Operators", "total_halstead_operators"),
        ("Halstead Operands", "total_halstead_operands"),
        ("Maintainability index", "total_four_property_maintainability_index"),
        ("Indegree", None),
        ("Outdegree", None),
    ],
    "file": [
        ("Lines of Code", LOC_METRIC_FILE),
        ("Cyclomatic Complexity", "total_cyclomatic"),
        ("Halstead Operators", "total_halstead_operators"),
        ("Halstead Operands", "total_halstead_operands"),
        ("Maintainability index", "maintainability_index_four_property"),
        ("Indegree", "mean_indegree_per_file"),
        ("Outdegree", "mean_outdegree_per_file"),
    ],
    "function": [
        ("Lines of Code", LOC_METRIC_FUNCTION),
        ("Cyclomatic Complexity", "cyclomatic"),
        ("Halstead Operators", "halstead_operators"),
        ("Halstead Operands", "halstead_operands"),
        ("Maintainability index", "maintainability_index_four_property"),
        ("Indegree", "indegree"),
        ("Outdegree", "outdegree"),
    ],
}
MODEL_NAMES = [
    "linear",
    "sqrt",
    "log1p",
    "power_law",
    "logarithmic_growth",
    "exponential_growth",
]


def raw_metrics_csv_paths() -> list[Path]:
    return sorted(
        path
        for path in Path(".").glob("metrics*.csv")
        if path.stem.removeprefix("metrics").isdigit()
    )


def filter_metrics_keys(
    df: pd.DataFrame, metrics_keys: list[str] | None
) -> pd.DataFrame:
    if not metrics_keys:
        return df
    if METRIC_KEY not in df.columns:
        raise ValueError(
            "metric filtering requires input data with a metric_key column"
        )
    return df[df[METRIC_KEY].isin(metrics_keys)].copy()


def check_for_invalid_values(df, csv_path):
    values = pd.to_numeric(df[VALUE_COL], errors="coerce")
    invalid_values = values.isna() & df[VALUE_COL].notna()
    if invalid_values.any():
        bad_rows = [
            f"line {row_index + 2}: {df.at[row_index, VALUE_COL]!r}"
            for row_index in df.index[invalid_values][:10]
        ]
        raise ValueError(
            f"CSV {csv_path} has non-numeric {VALUE_COL} values: " + "; ".join(bad_rows)
        )
    return values


def load_metrics_csv(csv_path):
    try:
        df = pd.read_csv(csv_path)
    except Exception as exc:
        raise ValueError(f"Failed to read CSV {csv_path}: {exc}") from exc

    values = check_for_invalid_values(
        df, csv_path
    )  # csv path only for the error messages this is not nice but it is what
    # it is now

    df[VALUE_COL] = values  # basically converted to numberic values

    if VERSION_COL not in df.columns:
        df[VERSION_COL] = math.nan
    if VERSION_NUMBER_COL in df.columns:
        version_numbers = pd.to_numeric(df[VERSION_NUMBER_COL], errors="coerce")
        invalid_versions = version_numbers.isna() & df[VERSION_NUMBER_COL].notna()
        if invalid_versions.any():
            bad_rows = [
                f"line {row_index + 2}: {df.at[row_index, VERSION_NUMBER_COL]!r}"
                for row_index in df.index[invalid_versions][:10]
            ]
            raise ValueError(
                f"CSV {csv_path} has non-numeric {VERSION_NUMBER_COL} values: "
                + "; ".join(bad_rows)
            )
        df[VERSION_COL] = version_numbers.fillna(df[VERSION_COL])

    return df


def load_raw_metrics(csv_paths: list[Path]) -> pd.DataFrame:
    if not csv_paths:
        raise ValueError("no raw metrics*.csv files found")
    return pd.concat(
        [load_metrics_csv(csv_path) for csv_path in csv_paths], ignore_index=True
    )


def metric_matrices_by_language(df: pd.DataFrame) -> dict[str, pd.DataFrame]:
    required_columns = {LANGUAGE_COL, VERSION_COL, METRIC_KEY, VALUE_COL}
    missing_columns = required_columns - set(df.columns)
    if missing_columns:
        raise ValueError(
            "input data is missing required columns: "
            + ", ".join(sorted(missing_columns))
        )

    if "statistic" in df.columns:
        df = df[df["statistic"] == "Mean"].copy()
    if "panel" in df.columns:
        df = df[df["panel"] == df[LANGUAGE_COL]].copy()

    matrices = {}
    for language, language_df in df.groupby(LANGUAGE_COL, sort=True):
        matrix = language_df.pivot_table(
            index=VERSION_COL,
            columns=METRIC_KEY,
            values=VALUE_COL,
            aggfunc="first",
        ).sort_index()
        matrices[str(language)] = matrix
    return matrices


def identity_columns(df: pd.DataFrame, level_name: str) -> list[str]:
    candidates = [
        VERSION_COL,
        "codebase_id",
        "version_id",
        "sample_number",
        LANGUAGE_COL,
        "file_path",
        "function_name",
        "function_start_line",
        "function_end_line",
    ]
    cols = [col for col in candidates if col in df.columns]
    if level_name == "version":
        return [
            col
            for col in cols
            if col
            in {
                VERSION_COL,
                "codebase_id",
                "version_id",
                "sample_number",
                LANGUAGE_COL,
            }
        ]
    if level_name == "file":
        return [
            col
            for col in cols
            if not col.startswith("function_") and col != "function_name"
        ]
    return cols


def build_metric_pairs(
    df: pd.DataFrame, level_name: str, lines_of_code_key: str, metric_key: str
) -> pd.DataFrame:
    key_cols = identity_columns(df, level_name)
    loc = (
        df[df[METRIC_KEY] == lines_of_code_key][key_cols + [VALUE_COL]]
        .rename(columns={VALUE_COL: "loc"})
        .drop_duplicates(subset=key_cols)
    )
    metric = (
        df[df[METRIC_KEY] == metric_key][key_cols + [VALUE_COL]]
        .rename(columns={VALUE_COL: "metric_value"})
        .drop_duplicates(subset=key_cols)
    )
    paired = loc.merge(metric, on=key_cols, how="inner")
    return paired.dropna(subset=["loc", "metric_value"]).copy()


def safe_spearmanr(x: pd.Series, y: pd.Series) -> dict[str, float]:
    paired = pd.concat([x, y], axis=1).dropna()
    paired.columns = ["loc", "metric_value"]
    return safe_spearmanr_for_pairs(paired)


def safe_spearmanr_for_pairs(paired: pd.DataFrame) -> dict[str, float]:
    paired = paired[np.isfinite(paired["loc"]) & np.isfinite(paired["metric_value"])]

    model_results = linearized_model_results(paired)
    if not enough_values_for_regression(paired):
        return {
            "n": int(len(paired)),
            "spearman_r": math.nan,
            "spearman_p": math.nan,
            **model_results,
        }

    spearman = stats.spearmanr(paired["loc"], paired["metric_value"])
    return {
        "n": int(len(paired)),
        "spearman_r": float(spearman.statistic),
        "spearman_p": float(spearman.pvalue),
        **model_results,
    }


def enough_values_for_regression(paired: pd.DataFrame) -> bool:
    return (
        len(paired) >= MIN_OBSERVATIONS
        and paired["loc"].nunique() >= 2
        and paired["metric_value"].nunique() >= 2
    )


def nan_model_result() -> dict[str, float]:
    return {
        "r": math.nan,
        "r2": math.nan,
        "target_r2": math.nan,
        "p": math.nan,
        "slope": math.nan,
        "intercept": math.nan,
    }


def linearized_model_results(paired: pd.DataFrame) -> dict[str, float]:
    results = {}
    for model_name in MODEL_NAMES:
        model_result = linearized_model_result(paired, model_name)
        results[f"{model_name}_r"] = model_result["r"]
        results[f"{model_name}_r2"] = model_result["r2"]
        results[f"{model_name}_target_r2"] = model_result["target_r2"]
        results[f"{model_name}_p"] = model_result["p"]
        results[f"{model_name}_slope"] = model_result["slope"]
        results[f"{model_name}_intercept"] = model_result["intercept"]
    return results


def linearized_model_result(paired: pd.DataFrame, model_name: str) -> dict[str, float]:
    if not enough_values_for_regression(paired):
        return nan_model_result()

    loc = paired["loc"].to_numpy(dtype=float)
    metric = paired["metric_value"].to_numpy(dtype=float)

    if model_name == "linear":
        regression_x = loc
        regression_y = metric
    elif model_name == "sqrt":
        if np.any(loc < 0):
            return nan_model_result()
        regression_x = np.sqrt(loc)
        regression_y = metric
    elif model_name == "log1p":
        if np.any(loc <= -1):
            return nan_model_result()
        regression_x = np.log1p(loc)
        regression_y = metric
    elif model_name == "power_law":
        if np.any(loc <= 0) or np.any(metric <= 0):
            return nan_model_result()
        regression_x = np.log(loc)
        regression_y = np.log(metric)
    elif model_name == "logarithmic_growth":
        if np.any(loc <= 0):
            return nan_model_result()
        regression_x = np.log(loc)
        regression_y = metric
    elif model_name == "exponential_growth":
        if np.any(metric <= 0):
            return nan_model_result()
        regression_x = loc
        regression_y = np.log(metric)
    else:
        raise ValueError(f"unknown model {model_name}")

    if not np.all(np.isfinite(regression_x)) or len(np.unique(regression_x)) < 2:
        return nan_model_result()
    if not np.all(np.isfinite(regression_y)):
        return nan_model_result()

    regression = stats.linregress(regression_x, regression_y)
    predictions = model_prediction(
        model_name, loc, float(regression.slope), float(regression.intercept)
    )
    return {
        "r": float(regression.rvalue),
        "r2": original_scale_r2(metric, predictions),
        "target_r2": float(regression.rvalue * regression.rvalue),
        "p": float(regression.pvalue),
        "slope": float(regression.slope),
        "intercept": float(regression.intercept),
    }


def original_scale_r2(actual: np.ndarray, predicted: np.ndarray) -> float:
    finite = np.isfinite(actual) & np.isfinite(predicted)
    if finite.sum() < MIN_OBSERVATIONS:
        return math.nan

    actual = actual[finite]
    predicted = predicted[finite]
    total_sum_squares = np.sum((actual - actual.mean()) ** 2)
    if total_sum_squares == 0:
        return math.nan

    residual_sum_squares = np.sum((actual - predicted) ** 2)
    return float(1 - residual_sum_squares / total_sum_squares)


def panic(message: str):
    print(message, file=stderr)
    exit(1)


def spearman_for_metric_language(
    df: pd.DataFrame, lines_of_code_key: str, level_name: str
) -> pd.DataFrame:
    required_columns = {LANGUAGE_COL, VERSION_COL, METRIC_KEY, VALUE_COL}
    missing_columns = required_columns - set(df.columns)
    if missing_columns:
        raise ValueError(
            "input data is missing required columns: "
            + ", ".join(sorted(missing_columns))
        )

    rows = []
    for language, language_df in df.groupby(LANGUAGE_COL, sort=True):
        metric_keys = sorted(language_df[METRIC_KEY].dropna().unique())
        if lines_of_code_key not in metric_keys:
            panic("lines of code key not in columns")

        for metric_key in metric_keys:
            if metric_key == lines_of_code_key:
                continue
            paired = build_metric_pairs(
                language_df, level_name, lines_of_code_key, metric_key
            )
            rows.append(
                {
                    LANGUAGE_COL: language,
                    "metric_key": metric_key,
                    **safe_spearmanr_for_pairs(paired),
                }
            )

    if not rows:
        return pd.DataFrame()
    return pd.DataFrame(rows).sort_values([LANGUAGE_COL, "metric_key"])


def best_model_for_row(row: pd.Series) -> dict[str, float | str]:
    best_model = ""
    best_r = math.nan
    best_p = math.nan
    best_r2 = math.nan
    best_target_r2 = math.nan
    best_slope = math.nan
    best_intercept = math.nan

    for model_name in MODEL_NAMES:
        model_r2 = row[f"{model_name}_r2"]
        if pd.isna(model_r2):
            continue

        if pd.isna(best_r2) or model_r2 > best_r2:
            best_model = model_name
            best_r = row[f"{model_name}_r"]
            best_p = row[f"{model_name}_p"]
            best_r2 = model_r2
            best_target_r2 = row[f"{model_name}_target_r2"]
            best_slope = row[f"{model_name}_slope"]
            best_intercept = row[f"{model_name}_intercept"]

    return {
        "best_model": best_model,
        "best_model_r": best_r,
        "best_model_r2": best_r2,
        "best_model_target_r2": best_target_r2,
        "best_model_p": best_p,
        "best_model_slope": best_slope,
        "best_model_intercept": best_intercept,
    }


def metric_selection_table(result: pd.DataFrame) -> pd.DataFrame:
    if result.empty:
        return result

    best_models = result.apply(best_model_for_row, axis=1, result_type="expand")
    table = pd.concat([result, best_models], axis=1)
    table["passed_spearman_threshold"] = table["spearman_r"].abs() >= SPEARMAN_THRESHOLD
    return table[
        [
            LANGUAGE_COL,
            "metric_key",
            "n",
            "spearman_r",
            "spearman_p",
            "passed_spearman_threshold",
            "best_model",
            "best_model_r",
            "best_model_r2",
            "best_model_target_r2",
            "best_model_p",
            "best_model_slope",
            "best_model_intercept",
        ]
    ]


def write_metric_selection_tables(result: pd.DataFrame, level_name: str):
    table = metric_selection_table(result)
    if table.empty:
        table.to_csv(f"metrics_over_0_8_r2_{level_name}.csv", index=False)
        table.to_csv(f"metrics_under_0_8_r2_{level_name}.csv", index=False)
        return

    model_was_calculated = table["best_model_r2"].notna()
    model_is_over_threshold = table["best_model_r2"] >= R2_THRESHOLD

    over_threshold = table[model_was_calculated & model_is_over_threshold]
    under_threshold = table[~model_was_calculated | ~model_is_over_threshold]

    over_threshold.to_csv(f"metrics_over_0_8_r2_{level_name}.csv", index=False)
    under_threshold.to_csv(f"metrics_under_0_8_r2_{level_name}.csv", index=False)


def model_prediction(
    model_name: str, loc: np.ndarray, slope: float, intercept: float
) -> np.ndarray:
    if model_name == "linear":
        return intercept + slope * loc
    if model_name == "sqrt":
        return intercept + slope * np.sqrt(loc)
    if model_name == "log1p":
        return intercept + slope * np.log1p(loc)
    if model_name == "power_law":
        return np.exp(intercept) * np.power(loc, slope)
    if model_name == "logarithmic_growth":
        return intercept + slope * np.log(loc)
    if model_name == "exponential_growth":
        return np.exp(intercept + slope * loc)
    raise ValueError(f"unknown model {model_name}")


def safe_filename(value: str) -> str:
    return "".join(char if char.isalnum() else "_" for char in value).strip("_").lower()


def metric_matrix_for_splot(
    df: pd.DataFrame, level_name: str, variables: list[tuple[str, str | None]]
) -> tuple[pd.DataFrame, list[tuple[str, str]]]:
    """Return one row per analyzed entity and the configured metrics available."""
    if "statistic" in df.columns:
        df = df[df["statistic"] == "Mean"].copy()
    if "panel" in df.columns:
        df = df[df["panel"] == df[LANGUAGE_COL]].copy()

    available = [
        (label, key)
        for label, key in variables
        if key is not None and key in set(df[METRIC_KEY].dropna())
    ]
    if len(available) < 2:
        raise ValueError(f"fewer than two configured metrics available for {level_name}")

    key_cols = identity_columns(df, level_name)
    selected_keys = [key for _, key in available]
    matrix = (
        df[df[METRIC_KEY].isin(selected_keys)]
        .pivot_table(
            index=key_cols,
            columns=METRIC_KEY,
            values=VALUE_COL,
            aggfunc="first",
        )
        .rename(columns=dict((key, label) for label, key in available))
    )
    return matrix, available


def plot_metric_splot(
    df: pd.DataFrame,
    level_name: str,
    variables: list[tuple[str, str | None]] | None = None,
    output_path: Path | None = None,
) -> Path:
    """Write a scatterplot matrix for the configured variables at one level."""
    variables = variables or SPLOT_VARIABLES[level_name]
    matrix, available = metric_matrix_for_splot(df, level_name, variables)
    labels = [label for label, _ in available]
    languages = (
        matrix.index.get_level_values(LANGUAGE_COL)
        if LANGUAGE_COL in matrix.index.names
        else pd.Series("all", index=matrix.index)
    )
    colors = {
        language: (OCAML_GRID_POINT_COLOR if language == "Ocaml" else DEFAULT_GRID_POINT_COLOR)
        for language in pd.unique(languages)
    }

    output_path = output_path or Path(f"correlation_splot_{level_name}.png")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    size = len(labels)
    fig, axes = plt.subplots(size, size, figsize=(2.6 * size, 2.6 * size), squeeze=False)
    for row, y_label in enumerate(labels):
        for column, x_label in enumerate(labels):
            ax = axes[row, column]
            if row == column:
                for language, color in colors.items():
                    values = matrix.loc[languages == language, y_label].dropna()
                    if not values.empty:
                        ax.hist(values, bins="auto", alpha=0.45, color=color, label=str(language))
            else:
                for language, color in colors.items():
                    points = matrix.loc[languages == language, [x_label, y_label]].dropna()
                    ax.scatter(points[x_label], points[y_label], s=8, alpha=0.45, color=color)
            if row == size - 1:
                ax.set_xlabel(x_label, rotation=35, ha="right")
            else:
                ax.set_xlabel("")
            if column == 0:
                ax.set_ylabel(y_label)
            else:
                ax.set_ylabel("")
            ax.grid(True, alpha=0.25)

    fig.suptitle(f"{level_name.title()} metric relationships")
    fig.tight_layout(rect=(0, 0, 1, 0.97))
    fig.savefig(output_path, dpi=200, bbox_inches="tight")
    plt.close(fig)
    return output_path


def plot_metric_language_grid(
    df: pd.DataFrame,
    level_name: str,
    lines_of_code_key: str,
    output_path: Path | None = None,
) -> Path:
    """Write a grid with one LOC-versus-metric scatter plot per cell."""
    required_columns = {LANGUAGE_COL, METRIC_KEY, VALUE_COL}
    missing_columns = required_columns - set(df.columns)
    if missing_columns:
        raise ValueError(
            "input data is missing required columns: "
            + ", ".join(sorted(missing_columns))
        )

    languages = sorted(df[LANGUAGE_COL].dropna().unique())
    metric_keys = sorted(
        metric_key
        for metric_key in df[METRIC_KEY].dropna().unique()
        if metric_key != lines_of_code_key
    )
    if not languages or not metric_keys:
        raise ValueError(f"no metrics or languages available for {level_name} grid")

    output_path = output_path or Path(
        f"correlation_metric_language_grid_{level_name}.png"
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)

    figure_width = max(6, 4 * len(languages))
    figure_height = max(4, 2.8 * len(metric_keys))
    fig, axes = plt.subplots(
        len(metric_keys),
        len(languages),
        figsize=(figure_width, figure_height),
        sharex=True,
        squeeze=False,
    )

    for column, language in enumerate(languages):
        axes[0, column].set_title(str(language))

    for row, metric_key in enumerate(metric_keys):
        for column, language in enumerate(languages):
            ax = axes[row, column]
            language_df = df[df[LANGUAGE_COL] == language]
            paired = build_metric_pairs(
                language_df, level_name, lines_of_code_key, metric_key
            )
            paired = paired[
                np.isfinite(paired["loc"]) & np.isfinite(paired["metric_value"])
            ]

            if paired.empty:
                ax.text(
                    0.5,
                    0.5,
                    "no paired data",
                    ha="center",
                    va="center",
                    transform=ax.transAxes,
                    color="0.45",
                )
            else:
                ax.scatter(
                    paired["loc"],
                    paired["metric_value"],
                    s=10,
                    alpha=0.55,
                    color=(
                        OCAML_GRID_POINT_COLOR
                        if language == "Ocaml"
                        else DEFAULT_GRID_POINT_COLOR
                    ),
                )

            ax.set_ylabel(str(metric_key) if column == 0 else "")
            ax.grid(True, alpha=0.3)

    for ax in axes[-1, :]:
        ax.set_xlabel(lines_of_code_key)

    legend_handles = [
        Line2D(
            [],
            [],
            color=(
                OCAML_GRID_POINT_COLOR
                if language == "Ocaml"
                else DEFAULT_GRID_POINT_COLOR
            ),
            marker="o",
            linestyle="None",
            markersize=6,
            label=str(language),
        )
        for language in languages
    ]
    fig.suptitle(f"{level_name.title()} metrics by language")
    fig.legend(
        handles=legend_handles,
        loc="upper center",
        bbox_to_anchor=(0.5, 0.96),
        ncol=len(legend_handles),
    )
    fig.tight_layout(rect=(0, 0, 1, 0.93))
    fig.savefig(output_path, dpi=200, bbox_inches="tight")
    plt.close(fig)
    return output_path


def plot_version_metric_models(
    df: pd.DataFrame,
    result: pd.DataFrame,
    output_dir: Path = VERSION_MODEL_PLOTS_DIR,
) -> None:
    table = metric_selection_table(result)
    if table.empty:
        return

    table = table[table["best_model_r2"] >= R2_THRESHOLD].copy()
    if table.empty:
        return

    output_dir.mkdir(parents=True, exist_ok=True)

    for row in table.itertuples(index=False):
        language = getattr(row, LANGUAGE_COL)
        metric_key = row.metric_key
        language_df = df[df[LANGUAGE_COL] == language]
        paired = build_metric_pairs(
            language_df, "version", LOC_METRIC_VERSION, metric_key
        )
        paired = paired[
            np.isfinite(paired["loc"]) & np.isfinite(paired["metric_value"])
        ].copy()
        if len(paired) < 2:
            continue

        loc = paired["loc"].to_numpy(dtype=float)
        metric_values = paired["metric_value"].to_numpy(dtype=float)
        order = np.argsort(loc)
        loc = loc[order]
        metric_values = metric_values[order]
        versions = paired[VERSION_COL].to_numpy()[order]

        curve_x = np.linspace(loc.min(), loc.max(), 300)
        curve_y = model_prediction(
            row.best_model,
            curve_x,
            row.best_model_slope,
            row.best_model_intercept,
        )
        finite_curve = np.isfinite(curve_x) & np.isfinite(curve_y)
        if not finite_curve.any():
            continue

        fig, ax = plt.subplots(figsize=(8, 5))
        ax.scatter(loc, metric_values, color="#1f77b4", label="real data", zorder=3)
        ax.plot(
            curve_x[finite_curve],
            curve_y[finite_curve],
            color="#d62728",
            linewidth=2,
            label=f"{row.best_model} model",
        )
        for x_value, y_value, version in zip(loc, metric_values, versions, strict=False):
            ax.annotate(
                f"v{int(version)}",
                (x_value, y_value),
                xytext=(4, 4),
                textcoords="offset points",
                fontsize=8,
            )

        ax.set_title(
            f"{language}: {metric_key}\n"
            f"best fit from {LOC_METRIC_VERSION}, R^2={row.best_model_r2:.3f}"
        )
        ax.set_xlabel(LOC_METRIC_VERSION)
        ax.set_ylabel(metric_key)
        ax.grid(True, alpha=0.3)
        ax.legend()
        fig.tight_layout()

        output_path = output_dir / f"{safe_filename(language)}__{safe_filename(metric_key)}.png"
        fig.savefig(output_path, dpi=200, bbox_inches="tight")
        plt.close(fig)


def updated_degree_pairs(
    degree_metrics: pd.DataFrame,
    size_metrics: pd.DataFrame,
    level_name: str,
    loc_metric: str,
    metric_key: str,
) -> pd.DataFrame:
    keys = [
        "project",
        LANGUAGE_COL,
        "sample_number",
        "relative_file_path",
    ]
    if level_name == "version":
        keys = ["project", LANGUAGE_COL, "sample_number"]
    elif level_name == "function":
        keys.extend(["function_name", "function_start_line"])

    metric_rows = degree_metrics[
        (degree_metrics[METRIC_LEVEL_COL] == level_name)
        & (degree_metrics[METRIC_KEY] == metric_key)
    ][keys + [VALUE_COL]].rename(columns={VALUE_COL: "metric_value"})
    loc_rows = size_metrics[
        (size_metrics[METRIC_LEVEL_COL] == level_name)
        & (size_metrics[METRIC_KEY] == loc_metric)
    ][keys + [VALUE_COL]].rename(columns={VALUE_COL: "loc"})
    return metric_rows.merge(loc_rows, on=keys, how="inner").dropna(
        subset=["loc", "metric_value"]
    )


def updated_version_loc_metrics(size_metrics: pd.DataFrame) -> pd.DataFrame:
    file_loc = size_metrics[
        (size_metrics[METRIC_LEVEL_COL] == "file")
        & (size_metrics[METRIC_KEY] == LOC_METRIC_FILE)
    ]
    version_loc = (
        file_loc.groupby(
            ["project", LANGUAGE_COL, "sample_number"],
            as_index=False,
            dropna=False,
        )[VALUE_COL]
        .sum()
    )
    version_loc[METRIC_KEY] = LOC_METRIC_VERSION
    version_loc[METRIC_LEVEL_COL] = "version"
    return version_loc


def updated_degree_correlations(
    degree_metrics: pd.DataFrame,
    size_metrics: pd.DataFrame,
    level_name: str,
    loc_metric: str,
    metric_keys: list[str],
) -> pd.DataFrame:
    rows = []
    for language in sorted(degree_metrics[LANGUAGE_COL].dropna().unique()):
        for metric_key in metric_keys:
            pairs = updated_degree_pairs(
                degree_metrics[degree_metrics[LANGUAGE_COL] == language],
                size_metrics[size_metrics[LANGUAGE_COL] == language],
                level_name,
                loc_metric,
                metric_key,
            )
            rows.append(
                {
                    LANGUAGE_COL: language,
                    METRIC_LEVEL_COL: level_name,
                    METRIC_KEY: metric_key,
                    **safe_spearmanr_for_pairs(pairs),
                }
            )
    return pd.DataFrame(rows)


def write_updated_degree_correlations(
    results_dir: Path,
    metrics_dir: Path,
    output_dir: Path,
) -> None:
    degree_metrics = load_results(results_dir)
    size_metrics = load_size_metrics(metrics_dir)
    version_metrics = build_version_metrics(degree_metrics)
    version_loc = updated_version_loc_metrics(size_metrics)
    output_dir.mkdir(parents=True, exist_ok=True)

    levels = {
        "version": (
            version_metrics,
            version_loc,
            LOC_METRIC_VERSION,
            list(UPDATED_VERSION_METRICS.values()),
        ),
        "file": (
            degree_metrics,
            size_metrics,
            LOC_METRIC_FILE,
            UPDATED_DEGREE_METRICS_BY_LEVEL["file"],
        ),
        "function": (
            degree_metrics,
            size_metrics,
            LOC_METRIC_FUNCTION,
            UPDATED_DEGREE_METRICS_BY_LEVEL["function"],
        ),
    }
    combined = []
    for level_name, (level_degrees, level_sizes, loc_metric, metric_keys) in levels.items():
        result = updated_degree_correlations(
            level_degrees,
            level_sizes,
            level_name,
            loc_metric,
            metric_keys,
        )
        result = pd.concat(
            [result, result.apply(best_model_for_row, axis=1, result_type="expand")],
            axis=1,
        )
        result.to_csv(
            output_dir / f"updated_degree_correlation_{level_name}.csv", index=False
        )
        combined.append(result)

    pd.concat(combined, ignore_index=True).to_csv(
        output_dir / "updated_degree_correlation_all.csv", index=False
    )


def parse_args() -> argparse.Namespace:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--updated-degrees",
        action="store_true",
        help="calculate correlations only for refreshed degree metrics",
    )
    parser.add_argument("--results-dir", type=Path, default=root / "results")
    parser.add_argument("--metrics-dir", type=Path, default=root / "plots")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=root / "plots" / "callgraph_results",
    )
    return parser.parse_args()


def main():
    args = parse_args()
    if args.updated_degrees:
        write_updated_degree_correlations(
            args.results_dir, args.metrics_dir, args.output_dir
        )
        return

    raw_metrics = load_raw_metrics(raw_metrics_csv_paths())
    version_metrics = load_metrics_csv("./version-metrics.csv")

    # version

    statistics_version = [
        "total_lines_of_code",
        "total_effective_lines_of_code",
        "total_comment_lines_of_code",
        "total_cyclomatic",
        "total_four_property_maintainability_index",
        "total_three_property_maintainability_index",
        "total_visual_studio_maintainability_index",
        "total_halstead_operators",
        "total_halstead_unique_operators",
        "total_halstead_operands",
        "total_halstead_unique_operands",
        "total_halstead_length",
        "total_halstead_vocabulary",
        "total_halstead_volume",
        "total_halstead_bugs",
        "total_halstead_difficulty",
        "total_halstead_effort",
        "total_halstead_time_seconds",
    ]
    version_metrics_filtered = filter_metrics_keys(
        version_metrics, statistics_version
    )
    result = spearman_for_metric_language(
        version_metrics_filtered, LOC_METRIC_VERSION, "version"
    )
    result.to_csv("spearman_r_p_version.csv")
    write_metric_selection_tables(result, "version")
    plot_metric_language_grid(
        version_metrics_filtered, "version", LOC_METRIC_VERSION
    )
    plot_metric_splot(version_metrics_filtered, "version")
    plot_version_metric_models(version_metrics_filtered, result)

    # file
    statistics_file = [
        "comment_lines_of_code",
        "effective_lines_of_code",
        "lines_of_code",
        "maintainability_index_four_property",
        "maintainability_index_three_property",
        "maintainability_index_visual_studio",
        "mean_indegree_per_file",
        "mean_outdegree_per_file",
        "mean_unique_indegree_per_file",
        "mean_unique_outdegree_per_file",
        "total_cyclomatic",
        "cyclomatic_per_line",
        "mean_cyclomatic_per_function_per_file",
        "total_halstead_bugs",
        "total_halstead_difficulty",
        "total_halstead_effort",
        "total_halstead_length",
        "total_halstead_operands",
        "total_halstead_operators",
        "total_halstead_time_seconds",
        "total_halstead_unique_operands",
        "total_halstead_unique_operators",
        "total_halstead_vocabulary",
        "total_halstead_volume",
    ]
    metrics_file = raw_metrics[raw_metrics[METRIC_LEVEL_COL] == "file"].copy()
    metrics_file_filtered = filter_metrics_keys(
        metrics_file, statistics_file
    )
    result = spearman_for_metric_language(
        metrics_file_filtered, LOC_METRIC_FILE, "file"
    )
    result.to_csv("spearman_r_p_file.csv")
    write_metric_selection_tables(result, "file")
    plot_metric_language_grid(metrics_file_filtered, "file", LOC_METRIC_FILE)
    plot_metric_splot(metrics_file_filtered, "file")

    statistics_function = [
        "cyclomatic",
        "function_length",
        "halstead_bugs",
        "halstead_difficulty",
        "halstead_effort",
        "halstead_length",
        "halstead_operands",
        "halstead_operators",
        "halstead_time_seconds",
        "halstead_unique_operands",
        "halstead_unique_operators",
        "halstead_vocabulary",
        "halstead_volume",
        "maintainability_index_comment_percentage",
        "maintainability_index_four_property",
        "maintainability_index_three_property",
        "maintainability_index_visual_studio",
        "indegree",
        "outdegree",
        "unique_indegree",
        "unique_outdegree",
    ]
    metrics_function = raw_metrics[raw_metrics[METRIC_LEVEL_COL] == "function"].copy()
    metrics_function_filtered = filter_metrics_keys(
        metrics_function, statistics_function
    )
    result = spearman_for_metric_language(
        metrics_function_filtered, LOC_METRIC_FUNCTION, "function"
    )
    result.to_csv("spearman_r_p_function.csv")
    write_metric_selection_tables(result, "function")
    plot_metric_language_grid(
        metrics_function_filtered, "function", LOC_METRIC_FUNCTION
    )
    plot_metric_splot(metrics_function_filtered, "function")


if __name__ == "__main__":
    main()
