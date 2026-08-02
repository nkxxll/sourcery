import argparse
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

from helpers import data
from plot_callgraph_results import load_results, load_size_metrics


REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT_PATH = Path("outdegree_vs_function_length_go_ocaml.png")
DEFAULT_FILE_OUTPUT_PATH = Path("outdegree_vs_file_length_go_ocaml.png")
FUNCTION_METRICS = ["outdegree", "unique_outdegree"]
FILE_METRICS = ["total_outdegree_per_file", "total_unique_outdegree_per_file"]
LANGUAGES = ["Golang", "Ocaml"]
AXIS_LIMIT = 1000


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Plot function outdegree and unique outdegree against function length "
            f"for Go and OCaml on a {AXIS_LIMIT} by {AXIS_LIMIT} scale."
        )
    )
    parser.add_argument(
        "-o",
        "--outfile",
        type=Path,
        default=DEFAULT_OUTPUT_PATH,
        help=f"PNG output path (default: {DEFAULT_OUTPUT_PATH})",
    )
    parser.add_argument(
        "--file-outfile",
        type=Path,
        default=DEFAULT_FILE_OUTPUT_PATH,
        help=f"File-level PNG output path (default: {DEFAULT_FILE_OUTPUT_PATH})",
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
        "--log",
        action="store_true",
        help="Use logarithmic axes, omitting non-positive values.",
    )
    return parser.parse_args()


def plot_file_outdegree(data_dir: Path, results_dir: Path, log: bool) -> plt.Figure:
    results = load_results(results_dir)
    size_metrics = load_size_metrics(data_dir)
    keys = [
        "project",
        "programming_language",
        "sample_number",
        "relative_file_path",
    ]
    loc = size_metrics[
        (size_metrics["metric_level"] == "file")
        & (size_metrics["metric_key"] == "lines_of_code")
    ][keys + ["value"]].rename(columns={"value": "loc"})
    fig, axes = plt.subplots(
        2, 2, figsize=(11, 8), sharex=True, sharey="row", squeeze=False
    )

    for row, metric in enumerate(FILE_METRICS):
        metric_rows = results[
            (results["metric_level"] == "file")
            & (results["metric_key"] == metric)
        ]
        for col, language in enumerate(LANGUAGES):
            paired = (
                metric_rows[metric_rows["programming_language"] == language][
                    keys + ["value"]
                ]
                .rename(columns={"value": "metric_value"})
                .merge(loc, on=keys, how="inner")
            )
            paired = paired[
                np.isfinite(paired["loc"])
                & np.isfinite(paired["metric_value"])
                & paired["loc"].between(0, AXIS_LIMIT)
                & paired["metric_value"].between(0, AXIS_LIMIT)
            ]
            if log:
                paired = paired[
                    (paired["loc"] > 0) & (paired["metric_value"] > 0)
                ]

            ax = axes[row, col]
            ax.scatter(paired["loc"], paired["metric_value"], alpha=0.25, s=10)
            ax.set_title(f"{language}: {metric}")
            ax.grid(True, alpha=0.25)
            if col == 0:
                ax.set_ylabel(metric)
            if row == len(FILE_METRICS) - 1:
                ax.set_xlabel("lines_of_code")
            if log:
                ax.set_xscale("log")
                ax.set_yscale("log")

    fig.suptitle("Total file outdegree vs lines of code", y=1.0)
    fig.tight_layout()
    return fig


def plot_outdegree(
    data_dir: Path = Path("."),
    output_path: Path = DEFAULT_OUTPUT_PATH,
    file_output_path: Path = DEFAULT_FILE_OUTPUT_PATH,
    results_dir: Path = REPOSITORY_ROOT / "results",
    log: bool = False,
) -> None:
    metrics_data = data(data_dir)
    function_fig = metrics_data.plot_go_ocaml_function_spread_to_loc(
        metrics=FUNCTION_METRICS,
        log=log,
    )
    file_fig = plot_file_outdegree(data_dir, results_dir, log)
    for ax in [*function_fig.axes, *file_fig.axes]:
        ax.set_xlim(1 if log else 0, AXIS_LIMIT)
        ax.set_ylim(1 if log else 0, AXIS_LIMIT)
    function_fig.savefig(output_path, dpi=200, bbox_inches="tight")
    file_fig.savefig(file_output_path, dpi=200, bbox_inches="tight")
    plt.close(function_fig)
    plt.close(file_fig)


if __name__ == "__main__":
    args = parse_args()
    plot_outdegree(
        args.data_dir,
        args.outfile,
        args.file_outfile,
        args.results_dir,
        log=args.log,
    )
    print(f"Wrote function-level outdegree scatterplot to {args.outfile}")
    print(f"Wrote file-level outdegree scatterplot to {args.file_outfile}")
