import argparse
import math
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

from plot_callgraph_results import load_results, load_size_metrics


REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_FULL_OUTPUT_PATH = Path("callgraph_outdegree_vs_loc_full_go_ocaml.png")
DEFAULT_CROPPED_OUTPUT_PATH = Path(
    "callgraph_outdegree_vs_loc_cropped_go_ocaml.png"
)
DEFAULT_PERCENTILE = 99.0
LANGUAGES = ["Golang", "Ocaml"]
LEVELS = [
    ("file", "lines_of_code", "total_outdegree_per_file"),
    ("function", "function_length", "outdegree"),
    ("version", "total_lines_of_code", "total_outdegree_per_version"),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Plot raw callgraph outdegree against lines of code at file, function, "
            "and version level for Go and OCaml."
        )
    )
    parser.add_argument(
        "--full-outfile",
        type=Path,
        default=DEFAULT_FULL_OUTPUT_PATH,
        help=f"Full-range PNG output path (default: {DEFAULT_FULL_OUTPUT_PATH})",
    )
    parser.add_argument(
        "--cropped-outfile",
        type=Path,
        default=DEFAULT_CROPPED_OUTPUT_PATH,
        help=f"Cropped PNG output path (default: {DEFAULT_CROPPED_OUTPUT_PATH})",
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        default=Path("."),
        help="Directory containing metrics CSV files (default: .)",
    )
    parser.add_argument(
        "--results-dir",
        type=Path,
        default=REPOSITORY_ROOT / "results",
        help="Directory containing raw callgraph result CSVs",
    )
    parser.add_argument(
        "--percentile",
        type=float,
        default=DEFAULT_PERCENTILE,
        help=f"Percentile used to crop outliers (default: {DEFAULT_PERCENTILE})",
    )
    args = parser.parse_args()
    if not 0 < args.percentile <= 100:
        parser.error("--percentile must be greater than 0 and at most 100")
    return args


def nice_upper_limit(value: float) -> float:
    if value <= 0:
        return 1.0
    magnitude = 10 ** math.floor(math.log10(value))
    normalized = value / magnitude
    if normalized <= 1:
        multiplier = 1
    elif normalized <= 2:
        multiplier = 2
    elif normalized <= 5:
        multiplier = 5
    else:
        multiplier = 10
    return multiplier * magnitude


def build_plot_data(
    data_dir: Path, results_dir: Path
) -> dict[tuple[str, str], pd.DataFrame]:
    results = load_results(results_dir)
    size_metrics = load_size_metrics(data_dir)
    base_keys = [
        "project",
        "programming_language",
        "sample_number",
        "relative_file_path",
    ]
    specifications = [
        (
            "file",
            "lines_of_code",
            "total_outdegree_per_file",
            base_keys,
        ),
        (
            "function",
            "function_length",
            "outdegree",
            base_keys + ["function_name", "function_start_line"],
        ),
    ]
    plot_data = {}

    for level, loc_metric, degree_metric, keys in specifications:
        degree_rows = results[
            (results["metric_level"] == level)
            & (results["metric_key"] == degree_metric)
        ]
        loc_rows = size_metrics[
            (size_metrics["metric_level"] == level)
            & (size_metrics["metric_key"] == loc_metric)
        ][keys + ["value"]].rename(columns={"value": "loc"})
        for language in LANGUAGES:
            paired = (
                degree_rows[degree_rows["programming_language"] == language][
                    keys + ["value"]
                ]
                .rename(columns={"value": "metric_value"})
                .merge(loc_rows, on=keys, how="inner")
            )
            plot_data[(level, language)] = finite_nonnegative(paired)

    version_keys = ["project", "programming_language", "sample_number"]
    version_degree = (
        results[
            (results["metric_level"] == "file")
            & (results["metric_key"] == "total_outdegree_per_file")
        ]
        .groupby(version_keys, as_index=False, dropna=False)["value"]
        .sum()
        .rename(columns={"value": "metric_value"})
    )
    version_loc = (
        size_metrics[
            (size_metrics["metric_level"] == "file")
            & (size_metrics["metric_key"] == "lines_of_code")
        ]
        .groupby(version_keys, as_index=False, dropna=False)["value"]
        .sum()
        .rename(columns={"value": "loc"})
    )
    for language in LANGUAGES:
        paired = version_degree[
            version_degree["programming_language"] == language
        ].merge(version_loc, on=version_keys, how="inner")
        plot_data[("version", language)] = finite_nonnegative(paired)

    return plot_data


def finite_nonnegative(paired: pd.DataFrame) -> pd.DataFrame:
    return paired[
        np.isfinite(paired["loc"])
        & np.isfinite(paired["metric_value"])
        & (paired["loc"] >= 0)
        & (paired["metric_value"] >= 0)
    ].copy()


def symmetric_limit(
    plot_data: dict[tuple[str, str], pd.DataFrame],
    level: str,
    percentile: float,
) -> float:
    pairs = [plot_data[(level, language)] for language in LANGUAGES]
    loc = np.concatenate([paired["loc"].to_numpy(dtype=float) for paired in pairs])
    outdegree = np.concatenate(
        [paired["metric_value"].to_numpy(dtype=float) for paired in pairs]
    )
    quantile = percentile / 100
    return nice_upper_limit(
        max(
            float(np.quantile(loc, quantile)),
            float(np.quantile(outdegree, quantile)),
        )
    )


def create_figure(
    plot_data: dict[tuple[str, str], pd.DataFrame],
    percentile: float,
    cropped: bool,
) -> plt.Figure:
    fig, axes = plt.subplots(3, 2, figsize=(12, 12), squeeze=False)

    for row, (level, loc_metric, degree_metric) in enumerate(LEVELS):
        limit = symmetric_limit(
            plot_data, level, percentile if cropped else 100.0
        )
        for col, language in enumerate(LANGUAGES):
            paired = plot_data[(level, language)]
            if cropped:
                paired = paired[
                    (paired["loc"] <= limit) & (paired["metric_value"] <= limit)
                ]

            ax = axes[row, col]
            ax.scatter(paired["loc"], paired["metric_value"], alpha=0.25, s=10)
            ax.set_title(f"{language}: {level} (n={len(paired)})")
            ax.set_xlim(0, limit)
            ax.set_ylim(0, limit)
            ax.set_xlabel(loc_metric)
            if col == 0:
                ax.set_ylabel(degree_metric)
            ax.grid(True, alpha=0.25)

    title = "Callgraph outdegree vs lines of code"
    if cropped:
        title += f" ({percentile:g}th-percentile crop)"
    else:
        title += " (full range)"
    fig.suptitle(title)
    fig.tight_layout()
    return fig


def plot_callgraph_outdegree_vs_loc(
    data_dir: Path = Path("."),
    results_dir: Path = REPOSITORY_ROOT / "results",
    full_output_path: Path = DEFAULT_FULL_OUTPUT_PATH,
    cropped_output_path: Path = DEFAULT_CROPPED_OUTPUT_PATH,
    percentile: float = DEFAULT_PERCENTILE,
) -> None:
    plot_data = build_plot_data(data_dir, results_dir)
    full_figure = create_figure(plot_data, percentile, cropped=False)
    cropped_figure = create_figure(plot_data, percentile, cropped=True)
    full_figure.savefig(full_output_path, dpi=200, bbox_inches="tight")
    cropped_figure.savefig(cropped_output_path, dpi=200, bbox_inches="tight")
    plt.close(full_figure)
    plt.close(cropped_figure)


if __name__ == "__main__":
    args = parse_args()
    plot_callgraph_outdegree_vs_loc(
        args.data_dir,
        args.results_dir,
        args.full_outfile,
        args.cropped_outfile,
        args.percentile,
    )
    print(f"Wrote full-range scatterplot to {args.full_outfile}")
    print(f"Wrote cropped scatterplot to {args.cropped_outfile}")
