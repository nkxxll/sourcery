from pathlib import Path

import pandas as pd
import matplotlib.pyplot as plt
from matplotlib.ticker import AutoMinorLocator, MaxNLocator, ScalarFormatter

from core import warn
from core import (
    CYCLOMATIC_METRIC,
    INPUT_INDEX_COL,
    LANGUAGE_COL,
    METRIC_COL,
    METRIC_LEVEL_COL,
    VALUE_COL,
    chart_data_output_path,
)

DEFAULT_OUTPUT_PATH = Path("metrics_over_versions.png")
COMPARISON_LANGUAGES = ["Golang", "Ocaml"]
CYCLOMATIC_METRICS = [
    CYCLOMATIC_METRIC,
    "mean_cyclomatic_per_function_per_file",
    "cyclomatic_per_line",
]
OCAML_ML_ONLY_SUFFIX = " (Ocaml .ml only)"


def plot_metric_evolution_by_version(
    df: pd.DataFrame,
    metric_names: list[str],
    output_path: Path = DEFAULT_OUTPUT_PATH,
) -> None:
    metric_level = chart_metric_level(df)
    df, metric_names = add_ocaml_ml_only_cyclomatic_rows(df, metric_names, metric_level)
    languages = sorted(df[LANGUAGE_COL].dropna().unique())
    if not languages:
        raise ValueError("No languages found for line chart")
    comparison_columns = ["Mean", "Median"]
    columns = languages + [f"{stat}: Golang vs Ocaml" for stat in comparison_columns]
    input_indices = sorted(df[INPUT_INDEX_COL].dropna().unique())
    x_positions = {input_index: pos for pos, input_index in enumerate(input_indices, 1)}
    x_ticks = list(x_positions.values())
    x_labels = [f"v{int(input_index)}" for input_index in input_indices]
    input_labels = dict(zip(input_indices, x_labels, strict=False))
    chart_rows = []

    fig, axes = plt.subplots(
        len(metric_names),
        len(columns),
        figsize=(5 * len(columns), 4 * len(metric_names)),
        sharex=True,
        squeeze=False,
    )

    for row, metric_name in enumerate(metric_names):
        metric_row_df = df[df[METRIC_COL] == metric_name]
        row_stats = metric_row_df.groupby([LANGUAGE_COL, INPUT_INDEX_COL])[VALUE_COL].agg(
            ["mean", "median"]
        )
        row_min = row_stats.min().min()
        row_max = row_stats.max().max()
        if pd.notna(row_min) and pd.notna(row_max):
            if row_min == row_max:
                padding = abs(row_min) * 0.05 or 1
            else:
                padding = (row_max - row_min) * 0.05
            row_ylim = (row_min - padding, row_max + padding)
        else:
            row_ylim = None

        for col, language in enumerate(languages):
            ax = axes[row][col]
            metric_df = df[
                (df[METRIC_COL] == metric_name) & (df[LANGUAGE_COL] == language)
            ]
            if row == 0:
                ax.set_title(language)

            if col == 0:
                ax.set_ylabel(f"{metric_name}\nValue")

            ax.set_xlim(0.5, len(input_indices) + 0.5)
            ax.set_xticks(x_ticks)
            ax.set_xticklabels(x_labels)
            ax.tick_params(axis="x", labelbottom=True)
            if row_ylim is not None:
                ax.set_ylim(row_ylim)
            ax.yaxis.set_major_locator(MaxNLocator(nbins=10))
            ax.yaxis.set_minor_locator(AutoMinorLocator(2))
            formatter = ScalarFormatter(useOffset=False)
            formatter.set_powerlimits((-4, 6))
            ax.yaxis.set_major_formatter(formatter)

            if metric_df.empty:
                warn(
                    f"Line chart has no data for metric {metric_name} "
                    f"and language {language}"
                )
                ax.text(0.5, 0.5, "No data", ha="center", va="center")
                continue

            grouped = metric_df.groupby(INPUT_INDEX_COL)[VALUE_COL]
            medians = grouped.median().sort_index()
            means = grouped.mean().sort_index()
            median_x = [x_positions[input_index] for input_index in medians.index]
            mean_x = [x_positions[input_index] for input_index in means.index]
            chart_rows.extend(
                line_chart_rows(metric_level, language, metric_name, "Mean", means, input_labels)
            )
            chart_rows.extend(
                line_chart_rows(
                    metric_level, language, metric_name, "Median", medians, input_labels
                )
            )

            ax.plot(mean_x, means.values, marker="o", label="Mean")
            ax.plot(median_x, medians.values, marker="o", label="Median")
            ax.grid(True, axis="y", alpha=0.3)
            ax.legend()

        for offset, stat in enumerate(comparison_columns, start=len(languages)):
            ax = axes[row][offset]
            if row == 0:
                ax.set_title(f"{stat}: Golang vs Ocaml")

            ax.set_xlim(0.5, len(input_indices) + 0.5)
            ax.set_xticks(x_ticks)
            ax.set_xticklabels(x_labels)
            ax.tick_params(axis="x", labelbottom=True)
            if row_ylim is not None:
                ax.set_ylim(row_ylim)
            ax.yaxis.set_major_locator(MaxNLocator(nbins=10))
            ax.yaxis.set_minor_locator(AutoMinorLocator(2))
            formatter = ScalarFormatter(useOffset=False)
            formatter.set_powerlimits((-4, 6))
            ax.yaxis.set_major_formatter(formatter)

            plotted = False
            for language in COMPARISON_LANGUAGES:
                metric_df = df[
                    (df[METRIC_COL] == metric_name) & (df[LANGUAGE_COL] == language)
                ]
                if metric_df.empty:
                    warn(
                        f"Line chart has no data for metric {metric_name} "
                        f"and language {language}"
                    )
                    continue

                grouped = metric_df.groupby(INPUT_INDEX_COL)[VALUE_COL]
                values = getattr(grouped, stat.lower())().sort_index()
                value_x = [x_positions[input_index] for input_index in values.index]
                chart_rows.extend(
                    line_chart_rows(
                        metric_level,
                        f"{stat}: Golang vs Ocaml",
                        metric_name,
                        stat,
                        values,
                        input_labels,
                        language=language,
                    )
                )
                ax.plot(value_x, values.values, marker="o", label=language)
                plotted = True

            if not plotted:
                ax.text(0.5, 0.5, "No data", ha="center", va="center")
                continue

            ax.grid(True, axis="y", alpha=0.3)
            ax.legend()

    for ax in axes[-1]:
        ax.set_xlabel("Version")
    fig.suptitle(f"{metric_level.title()} Metric Evolution by Language")
    plt.tight_layout(rect=[0, 0, 1, 0.95])
    fig.savefig(output_path, dpi=200, bbox_inches="tight")
    pd.DataFrame(chart_rows).to_csv(chart_data_output_path(output_path), index=False)


def add_ocaml_ml_only_cyclomatic_rows(
    df: pd.DataFrame,
    metric_names: list[str],
    metric_level: str,
) -> tuple[pd.DataFrame, list[str]]:
    if metric_level != "file" or "file_path" not in df.columns:
        return df, metric_names

    extra_rows = []
    expanded_metric_names = []
    for metric_name in metric_names:
        expanded_metric_names.append(metric_name)
        if metric_name not in CYCLOMATIC_METRICS:
            continue

        ml_only_rows = ocaml_ml_only_cyclomatic_df(df, metric_name).copy()
        golang_rows = df[
            (df[LANGUAGE_COL] == "Golang") & (df[METRIC_COL] == metric_name)
        ].copy()
        if ml_only_rows.empty or golang_rows.empty:
            continue

        ml_only_metric_name = f"{metric_name}{OCAML_ML_ONLY_SUFFIX}"
        ml_only_rows[METRIC_COL] = ml_only_metric_name
        golang_rows[METRIC_COL] = ml_only_metric_name
        extra_rows.extend([golang_rows, ml_only_rows])
        expanded_metric_names.append(ml_only_metric_name)

    if not extra_rows:
        return df, metric_names

    return pd.concat([df, *extra_rows], ignore_index=True), expanded_metric_names


def ocaml_ml_only_cyclomatic_df(
    df: pd.DataFrame,
    metric_name: str,
) -> pd.DataFrame:
    if "file_path" not in df.columns:
        return df.iloc[0:0].copy()

    return df[
        (df[LANGUAGE_COL] == "Ocaml")
        & (df[METRIC_COL] == metric_name)
        & df["file_path"].fillna("").astype(str).str.endswith(".ml")
    ].copy()


def line_chart_rows(
    metric_level: str,
    panel: str,
    metric_name: str,
    statistic: str,
    values: pd.Series,
    input_labels: dict[int, str],
    language: str | None = None,
) -> list[dict[str, object]]:
    return [
        {
            "metric_level": metric_level,
            "panel": panel,
            METRIC_COL: metric_name,
            LANGUAGE_COL: language if language is not None else panel,
            "statistic": statistic,
            INPUT_INDEX_COL: input_index,
            "x_label": input_labels[input_index],
            VALUE_COL: value,
        }
        for input_index, value in values.items()
    ]


def chart_metric_level(df: pd.DataFrame) -> str:
    if METRIC_LEVEL_COL not in df.columns:
        return "Metric"

    metric_levels = sorted(df[METRIC_LEVEL_COL].dropna().unique())
    if len(metric_levels) == 1:
        return str(metric_levels[0])

    return "Metric"
