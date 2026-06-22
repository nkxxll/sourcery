from pathlib import Path

import pandas as pd
import matplotlib.pyplot as plt
from matplotlib.ticker import AutoMinorLocator, MaxNLocator, ScalarFormatter

from core import warn
from core import INPUT_INDEX_COL, LANGUAGE_COL, METRIC_COL, METRIC_LEVEL_COL, VALUE_COL

DEFAULT_OUTPUT_PATH = Path("metrics_over_versions.png")


def plot_metric_evolution_by_version(
    df: pd.DataFrame,
    metric_names: list[str],
    output_path: Path = DEFAULT_OUTPUT_PATH,
) -> None:
    metric_level = chart_metric_level(df)
    languages = sorted(df[LANGUAGE_COL].dropna().unique())
    if not languages:
        raise ValueError("No languages found for line chart")
    input_indices = sorted(df[INPUT_INDEX_COL].dropna().unique())
    x_positions = {input_index: pos for pos, input_index in enumerate(input_indices, 1)}
    x_ticks = list(x_positions.values())
    x_labels = [f"v{int(input_index)}" for input_index in input_indices]

    fig, axes = plt.subplots(
        len(metric_names),
        len(languages),
        figsize=(5 * len(languages), 4 * len(metric_names)),
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

            ax.plot(mean_x, means.values, marker="o", label="Mean")
            ax.plot(median_x, medians.values, marker="o", label="Median")
            ax.grid(True, axis="y", alpha=0.3)
            ax.legend()

    for ax in axes[-1]:
        ax.set_xlabel("Version")
    fig.suptitle(f"{metric_level.title()} Metric Evolution by Language")
    plt.tight_layout()
    fig.savefig(output_path, dpi=200, bbox_inches="tight")


def chart_metric_level(df: pd.DataFrame) -> str:
    if METRIC_LEVEL_COL not in df.columns:
        return "Metric"

    metric_levels = sorted(df[METRIC_LEVEL_COL].dropna().unique())
    if len(metric_levels) == 1:
        return str(metric_levels[0])

    return "Metric"
