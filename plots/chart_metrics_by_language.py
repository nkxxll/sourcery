from pathlib import Path

import pandas as pd
import matplotlib.pyplot as plt

from core import warn
from core import INPUT_INDEX_COL, LANGUAGE_COL, METRIC_COL, METRIC_LEVEL_COL, VALUE_COL

DEFAULT_OUTPUT_PATH = Path("metrics_by_language.png")


def plot_metrics_stacked_by_input(
    df: pd.DataFrame,
    metric_names: list[str],
    output_path: Path = DEFAULT_OUTPUT_PATH,
) -> None:
    input_indices = sorted(df[INPUT_INDEX_COL].unique())
    metric_level = chart_metric_level(df)

    fig, axes = plt.subplots(
        len(input_indices),
        len(metric_names),
        figsize=(6 * len(metric_names), 4 * len(input_indices)),
        sharey=False,
        squeeze=False,
    )

    for row, input_index in enumerate(input_indices):
        input_df = df[df[INPUT_INDEX_COL] == input_index]

        for col, metric_name in enumerate(metric_names):
            ax = axes[row][col]
            metric_df = input_df[input_df[METRIC_COL] == metric_name]

            ax.set_title(f"Version {input_index}: {metric_name}")
            ax.set_xlabel("Programming Language")
            ax.set_ylabel("Value")

            if metric_df.empty:
                warn(
                    f"Boxplot has no data for version {input_index}, "
                    f"metric {metric_name}"
                )
                ax.text(0.5, 0.5, "No data", ha="center", va="center")
                ax.set_xticks([])
                continue

            median_order = (
                metric_df.groupby(LANGUAGE_COL)[VALUE_COL]
                .median()
                .sort_values(ascending=False)
                .index
            )

            data = [
                metric_df.loc[metric_df[LANGUAGE_COL] == lang, VALUE_COL]
                for lang in median_order
            ]

            ax.boxplot(
                data,
                tick_labels=median_order,
                showfliers=False,
            )

            ax.tick_params(axis="x", rotation=45)

    fig.suptitle(f"{metric_level.title()} Metrics by Programming Language")
    plt.tight_layout()
    fig.savefig(output_path, dpi=200, bbox_inches="tight")


def chart_metric_level(df: pd.DataFrame) -> str:
    if METRIC_LEVEL_COL not in df.columns:
        return "Metric"

    metric_levels = sorted(df[METRIC_LEVEL_COL].dropna().unique())
    if len(metric_levels) == 1:
        return str(metric_levels[0])

    return "Metric"
