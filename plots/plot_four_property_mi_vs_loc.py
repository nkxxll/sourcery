import argparse
import math
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

import correlation
from helpers import Language, data


DEFAULT_FULL_OUTPUT_PATH = Path("four_property_mi_vs_loc_full_go_ocaml.png")
DEFAULT_CROPPED_OUTPUT_PATH = Path("four_property_mi_vs_loc_cropped_go_ocaml.png")
DEFAULT_PERCENTILE = 99.0
LEVELS = [
    ("file", correlation.LOC_METRIC_FILE, "maintainability_index_four_property"),
    (
        "function",
        correlation.LOC_METRIC_FUNCTION,
        "maintainability_index_four_property",
    ),
    (
        "version",
        correlation.LOC_METRIC_VERSION,
        "total_four_property_maintainability_index",
    ),
]
LANGUAGES = [Language.GO.value, Language.OCAML.value]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Plot four-property maintainability index against lines of code at "
            "file, function, and version level for Go and OCaml."
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
    """Round an upper limit to 1, 2, or 5 times a power of ten."""
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


def build_plot_data(data_dir: Path) -> dict[tuple[str, str], pd.DataFrame]:
    metrics_data = data(data_dir)
    plot_data = {}
    for level, loc_metric, mi_metric in LEVELS:
        metrics = (
            metrics_data.version_metrics
            if level == "version"
            else metrics_data.raw_metrics
        )
        metrics = metrics[metrics[correlation.METRIC_LEVEL_COL] == level]
        for language in LANGUAGES:
            language_metrics = metrics[
                metrics[correlation.LANGUAGE_COL] == language
            ]
            paired = correlation.build_metric_pairs(
                language_metrics, level, loc_metric, mi_metric
            )
            plot_data[(level, language)] = paired[
                np.isfinite(paired["loc"])
                & np.isfinite(paired["metric_value"])
                & (paired["loc"] >= 0)
            ].copy()
    return plot_data


def level_limits(
    plot_data: dict[tuple[str, str], pd.DataFrame],
    level: str,
    percentile: float,
) -> tuple[float, float]:
    pairs = [plot_data[(level, language)] for language in LANGUAGES]
    loc = np.concatenate([paired["loc"].to_numpy(dtype=float) for paired in pairs])
    mi = np.concatenate(
        [paired["metric_value"].to_numpy(dtype=float) for paired in pairs]
    )
    quantile = percentile / 100
    return (
        nice_upper_limit(float(np.quantile(loc, quantile))),
        nice_upper_limit(float(np.quantile(np.abs(mi), quantile))),
    )


def create_figure(
    plot_data: dict[tuple[str, str], pd.DataFrame],
    percentile: float,
    cropped: bool,
) -> plt.Figure:
    fig, axes = plt.subplots(3, 2, figsize=(12, 12), squeeze=False)

    for row, (level, loc_metric, mi_metric) in enumerate(LEVELS):
        limit_percentile = percentile if cropped else 100.0
        loc_limit, mi_limit = level_limits(plot_data, level, limit_percentile)
        for col, language in enumerate(LANGUAGES):
            paired = plot_data[(level, language)]
            if cropped:
                paired = paired[
                    (paired["loc"] <= loc_limit)
                    & (paired["metric_value"].abs() <= mi_limit)
                ]

            ax = axes[row, col]
            ax.scatter(paired["loc"], paired["metric_value"], alpha=0.25, s=10)
            ax.set_title(f"{language}: {level} (n={len(paired)})")
            ax.set_xlim(0, loc_limit)
            ax.set_ylim(-mi_limit, mi_limit)
            ax.set_xlabel(loc_metric)
            if col == 0:
                ax.set_ylabel(mi_metric)
            ax.grid(True, alpha=0.25)

    title = "Four-property maintainability index vs lines of code"
    if cropped:
        title += f" ({percentile:g}th-percentile crop)"
    else:
        title += " (full range)"
    fig.suptitle(title)
    fig.tight_layout()
    return fig


def plot_four_property_mi_vs_loc(
    data_dir: Path = Path("."),
    full_output_path: Path = DEFAULT_FULL_OUTPUT_PATH,
    cropped_output_path: Path = DEFAULT_CROPPED_OUTPUT_PATH,
    percentile: float = DEFAULT_PERCENTILE,
) -> None:
    plot_data = build_plot_data(data_dir)
    full_figure = create_figure(plot_data, percentile, cropped=False)
    cropped_figure = create_figure(plot_data, percentile, cropped=True)
    full_figure.savefig(full_output_path, dpi=200, bbox_inches="tight")
    cropped_figure.savefig(cropped_output_path, dpi=200, bbox_inches="tight")
    plt.close(full_figure)
    plt.close(cropped_figure)


if __name__ == "__main__":
    args = parse_args()
    plot_four_property_mi_vs_loc(
        args.data_dir,
        args.full_outfile,
        args.cropped_outfile,
        args.percentile,
    )
    print(f"Wrote full-range scatterplot to {args.full_outfile}")
    print(f"Wrote cropped scatterplot to {args.cropped_outfile}")
