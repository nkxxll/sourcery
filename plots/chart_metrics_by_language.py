from pathlib import Path

import pandas as pd
import matplotlib.pyplot as plt

from core import warn
from core import (
    INPUT_INDEX_COL,
    LANGUAGE_COL,
    METRIC_COL,
    METRIC_LEVEL_COL,
    VALUE_COL,
    chart_data_output_path,
)

DEFAULT_OUTPUT_PATH = Path("metrics_by_language.png")
LANGUAGE_ORDER = ["Golang", "Ocaml"]


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
    chart_rows = []

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

            present_languages = set(metric_df[LANGUAGE_COL].dropna().unique())
            language_order = [
                language for language in LANGUAGE_ORDER if language in present_languages
            ]
            language_order.extend(
                sorted(present_languages - set(language_order))
            )

            data = [
                metric_df.loc[metric_df[LANGUAGE_COL] == lang, VALUE_COL]
                for lang in language_order
            ]
            for language, values in zip(language_order, data, strict=False):
                chart_rows.append(
                    {
                        "metric_level": metric_level,
                        INPUT_INDEX_COL: input_index,
                        METRIC_COL: metric_name,
                        LANGUAGE_COL: language,
                        **boxplot_summary(values),
                    }
                )

            ax.boxplot(
                data,
                tick_labels=language_order,
                showfliers=False,
            )

            ax.tick_params(axis="x", rotation=45)

    fig.suptitle(f"{metric_level.title()} Metrics by Programming Language")
    plt.tight_layout(rect=[0, 0, 1, 0.95])
    fig.savefig(output_path, dpi=200, bbox_inches="tight")
    pd.DataFrame(chart_rows).to_csv(chart_data_output_path(output_path), index=False)


def boxplot_summary(values: pd.Series) -> dict[str, float]:
    q1 = values.quantile(0.25)
    median = values.median()
    q3 = values.quantile(0.75)
    iqr = q3 - q1
    lower_bound = q1 - 1.5 * iqr
    upper_bound = q3 + 1.5 * iqr
    whisker_values = values[(values >= lower_bound) & (values <= upper_bound)]
    return {
        "whisker_low": whisker_values.min(),
        "q1": q1,
        "median": median,
        "q3": q3,
        "whisker_high": whisker_values.max(),
    }


def chart_metric_level(df: pd.DataFrame) -> str:
    if METRIC_LEVEL_COL not in df.columns:
        return "Metric"

    metric_levels = sorted(df[METRIC_LEVEL_COL].dropna().unique())
    if len(metric_levels) == 1:
        return str(metric_levels[0])

    return "Metric"
