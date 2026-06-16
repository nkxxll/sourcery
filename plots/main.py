import pandas as pd
import matplotlib.pyplot as plt

CSV_PATH = "metrics.csv"
OUTPUT_PATH = "metrics_by_language.png"

# Choose one or many metrics here
METRIC_NAMES = [
    "lines_of_code",
    "total_cyclomatic",
]

LANGUAGE_COL = "programming_language"
METRIC_COL = "metric_key"
VALUE_COL = "value"


def load_metrics(csv_path: str, metric_names: list[str]) -> pd.DataFrame:
    df = pd.read_csv(csv_path)

    df = df[df[METRIC_COL].isin(metric_names)].copy()
    df = df.dropna(subset=[LANGUAGE_COL, METRIC_COL, VALUE_COL])
    df[VALUE_COL] = pd.to_numeric(df[VALUE_COL], errors="coerce")
    df = df.dropna(subset=[VALUE_COL])

    return df


def plot_metrics_side_by_side(df: pd.DataFrame, metric_names: list[str]) -> None:
    fig, axes = plt.subplots(
        1,
        len(metric_names),
        figsize=(6 * len(metric_names), 6),
        sharey=False,
    )

    if len(metric_names) == 1:
        axes = [axes]

    for ax, metric_name in zip(axes, metric_names):
        metric_df = df[df[METRIC_COL] == metric_name]

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

        ax.set_title(metric_name)
        ax.set_xlabel("Programming Language")
        ax.set_ylabel("Value")
        ax.tick_params(axis="x", rotation=45)

    fig.suptitle("Metrics per File by Programming Language")
    plt.tight_layout()
    fig.savefig(OUTPUT_PATH, dpi=200, bbox_inches="tight")
    plt.show()


def print_medians(df: pd.DataFrame, metric_names: list[str]) -> None:
    for metric_name in metric_names:
        metric_df = df[df[METRIC_COL] == metric_name]

        print(f"\nMedian values for {metric_name}:")
        print(
            metric_df.groupby(LANGUAGE_COL)[VALUE_COL]
            .median()
            .sort_values(ascending=False)
        )


if __name__ == "__main__":
    df = load_metrics(CSV_PATH, METRIC_NAMES)

    if df.empty:
        raise ValueError(f"No rows found for metrics: {METRIC_NAMES}")

    print_medians(df, METRIC_NAMES)
    plot_metrics_side_by_side(df, METRIC_NAMES)
