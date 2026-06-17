import argparse
import shutil
import subprocess
from pathlib import Path

import pandas as pd
import matplotlib.pyplot as plt

SCRIPT_DIR = Path(__file__).resolve().parent
OUTPUT_PATH = "metrics_by_language.png"
VERSIONS_OUTPUT_PATH = "metrics_over_versions.png"

# Choose one or many metrics here
METRIC_NAMES = [
    "lines_of_code",
    "total_cyclomatic",
    "cyclomatic_per_line",
]

LANGUAGE_COL = "programming_language"
METRIC_COL = "metric_key"
VALUE_COL = "value"
INPUT_INDEX_COL = "input_index"
LINES_OF_CODE_METRIC = "lines_of_code"
CYCLOMATIC_METRIC = "total_cyclomatic"
ADJUSTED_CYCLOMATIC_METRIC = "cyclomatic_per_line"


def load_metrics(csv_paths: list[Path], metric_names: list[str]) -> pd.DataFrame:
    dfs = []

    for input_index, csv_path in enumerate(csv_paths, start=1):
        df = pd.read_csv(csv_path)
        df[INPUT_INDEX_COL] = input_index
        dfs.append(df)

    df = pd.concat(dfs, ignore_index=True)

    source_metric_names = set(metric_names)
    if ADJUSTED_CYCLOMATIC_METRIC in source_metric_names:
        source_metric_names.update([LINES_OF_CODE_METRIC, CYCLOMATIC_METRIC])

    df = df[df[METRIC_COL].isin(source_metric_names)].copy()
    df = df.dropna(subset=[LANGUAGE_COL, METRIC_COL, VALUE_COL])
    df[VALUE_COL] = pd.to_numeric(df[VALUE_COL], errors="coerce")
    df = df.dropna(subset=[VALUE_COL])

    if ADJUSTED_CYCLOMATIC_METRIC in metric_names:
        df = add_adjusted_cyclomatic_metric(df)

    df = df[df[METRIC_COL].isin(metric_names)].copy()

    return df


def add_adjusted_cyclomatic_metric(df: pd.DataFrame) -> pd.DataFrame:
    key_cols = [
        col
        for col in [
            INPUT_INDEX_COL,
            LANGUAGE_COL,
            "codebase_id",
            "version_id",
            "sample_number",
            "file_path",
        ]
        if col in df.columns
    ]

    cyclomatic_rows = df[df[METRIC_COL] == CYCLOMATIC_METRIC].copy()
    loc_values = df[df[METRIC_COL] == LINES_OF_CODE_METRIC][
        key_cols + [VALUE_COL]
    ].rename(columns={VALUE_COL: "lines_of_code_value"})

    adjusted_rows = cyclomatic_rows.merge(loc_values, on=key_cols, how="inner")
    adjusted_rows = adjusted_rows[adjusted_rows["lines_of_code_value"] != 0].copy()
    adjusted_rows[VALUE_COL] = (
        adjusted_rows[VALUE_COL] / adjusted_rows["lines_of_code_value"]
    )
    adjusted_rows[METRIC_COL] = ADJUSTED_CYCLOMATIC_METRIC

    if "metric_label" in adjusted_rows.columns:
        adjusted_rows["metric_label"] = "Cyclomatic Complexity/Line"

    adjusted_rows = adjusted_rows.drop(columns=["lines_of_code_value"])

    return pd.concat([df, adjusted_rows], ignore_index=True)


def plot_metrics_stacked_by_input(df: pd.DataFrame, metric_names: list[str]) -> None:
    input_indices = sorted(df[INPUT_INDEX_COL].unique())

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

    fig.suptitle("Metrics per File by Programming Language")
    plt.tight_layout()
    fig.savefig(OUTPUT_PATH, dpi=200, bbox_inches="tight")


def plot_metric_evolution_by_version(df: pd.DataFrame, metric_names: list[str]) -> None:
    fig, axes = plt.subplots(
        len(metric_names),
        1,
        figsize=(9, 4 * len(metric_names)),
        sharex=True,
        squeeze=False,
    )

    for row, metric_name in enumerate(metric_names):
        ax = axes[row][0]
        metric_df = df[df[METRIC_COL] == metric_name]
        ax.set_title(metric_name)
        ax.set_ylabel("Median value")

        if metric_df.empty:
            ax.text(0.5, 0.5, "No data", ha="center", va="center")
            continue

        medians = metric_df.groupby(INPUT_INDEX_COL)[VALUE_COL].median().sort_index()

        ax.plot(medians.index, medians.values, marker="o")
        ax.grid(True, axis="y", alpha=0.3)
        ax.set_xticks(medians.index)

    axes[-1][0].set_xlabel("Version file number")
    fig.suptitle("Metric Evolution Across Version Files")
    plt.tight_layout()
    fig.savefig(VERSIONS_OUTPUT_PATH, dpi=200, bbox_inches="tight")


def print_medians(df: pd.DataFrame, metric_names: list[str]) -> None:
    for input_index in sorted(df[INPUT_INDEX_COL].unique()):
        input_df = df[df[INPUT_INDEX_COL] == input_index]

        for metric_name in metric_names:
            metric_df = input_df[input_df[METRIC_COL] == metric_name]

            print(f"\nMedian values for version {input_index}, {metric_name}:")
            print(
                metric_df.groupby(LANGUAGE_COL)[VALUE_COL]
                .median()
                .sort_values(ascending=False)
            )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "-i",
        "--interactive",
        action="store_true",
        help="choose metrics interactively with gum",
    )
    parser.add_argument(
        "csv_paths",
        nargs="+",
        type=Path,
        help="CSV files to plot, in version order",
    )
    return parser.parse_args()


def choose_metric_names_interactively() -> list[str]:
    if shutil.which("gum") is None:
        raise RuntimeError("gum is required when using -i/--interactive")

    keys = subprocess.run(
        [str(SCRIPT_DIR / "keys.sh")],
        cwd=SCRIPT_DIR,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()

    keys.append(ADJUSTED_CYCLOMATIC_METRIC)
    keys = sorted(set(keys))

    selected = subprocess.run(
        ["gum", "choose", "--no-limit", "--header", "Choose metrics to plot"],
        input="\n".join(keys) + "\n",
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.splitlines()

    if not selected:
        raise ValueError("No metrics selected")

    return selected


if __name__ == "__main__":
    args = parse_args()
    metric_names = (
        choose_metric_names_interactively() if args.interactive else METRIC_NAMES
    )
    df = load_metrics(args.csv_paths, metric_names)

    if df.empty:
        raise ValueError(f"No rows found for metrics: {metric_names}")

    print_medians(df, metric_names)
    plot_metrics_stacked_by_input(df, metric_names)
    plot_metric_evolution_by_version(df, metric_names)

    if "agg" not in plt.get_backend().lower():
        plt.show()
