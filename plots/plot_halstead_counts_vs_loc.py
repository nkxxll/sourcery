import argparse
import math
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

import correlation
from helpers import Language, data


DEFAULT_PERCENTILE = 99.0
LANGUAGES = [Language.GO.value, Language.OCAML.value]
HALSTEAD_METRICS = [
    "operators",
    "operands",
    "unique_operators",
    "unique_operands",
]
LEVELS = [
    ("file", correlation.LOC_METRIC_FILE),
    ("function", correlation.LOC_METRIC_FUNCTION),
    ("version", correlation.LOC_METRIC_VERSION),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Plot Halstead operator and operand counts against lines of code at "
            "file, function, and version level for Go and OCaml."
        )
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("."),
        help="Directory for generated PNG files (default: .)",
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


def metric_key(level: str, metric: str) -> str:
    prefix = "" if level == "function" else "total_"
    return f"{prefix}halstead_{metric}"


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
    data_dir: Path,
) -> dict[tuple[str, str, str], pd.DataFrame]:
    metrics_data = data(data_dir)
    plot_data = {}

    for metric in HALSTEAD_METRICS:
        for level, loc_metric in LEVELS:
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
                    language_metrics,
                    level,
                    loc_metric,
                    metric_key(level, metric),
                )
                plot_data[(metric, level, language)] = paired[
                    np.isfinite(paired["loc"])
                    & np.isfinite(paired["metric_value"])
                    & (paired["loc"] >= 0)
                    & (paired["metric_value"] >= 0)
                ].copy()

    return plot_data


def symmetric_limit(
    plot_data: dict[tuple[str, str, str], pd.DataFrame],
    metric: str,
    level: str,
    percentile: float,
) -> float:
    pairs = [plot_data[(metric, level, language)] for language in LANGUAGES]
    loc = np.concatenate([paired["loc"].to_numpy(dtype=float) for paired in pairs])
    values = np.concatenate(
        [paired["metric_value"].to_numpy(dtype=float) for paired in pairs]
    )
    quantile = percentile / 100
    return nice_upper_limit(
        max(
            float(np.quantile(loc, quantile)),
            float(np.quantile(values, quantile)),
        )
    )


def create_figure(
    plot_data: dict[tuple[str, str, str], pd.DataFrame],
    metric: str,
    percentile: float,
    cropped: bool,
) -> plt.Figure:
    fig, axes = plt.subplots(3, 2, figsize=(12, 12), squeeze=False)

    for row, (level, loc_metric) in enumerate(LEVELS):
        limit = symmetric_limit(
            plot_data,
            metric,
            level,
            percentile if cropped else 100.0,
        )
        level_metric = metric_key(level, metric)
        for col, language in enumerate(LANGUAGES):
            paired = plot_data[(metric, level, language)]
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
                ax.set_ylabel(level_metric)
            ax.grid(True, alpha=0.25)

    readable_metric = metric.replace("_", " ")
    title = f"Halstead {readable_metric} vs lines of code"
    if cropped:
        title += f" ({percentile:g}th-percentile crop)"
    else:
        title += " (full range)"
    fig.suptitle(title)
    fig.tight_layout()
    return fig


def plot_halstead_counts_vs_loc(
    data_dir: Path = Path("."),
    output_dir: Path = Path("."),
    percentile: float = DEFAULT_PERCENTILE,
) -> list[Path]:
    plot_data = build_plot_data(data_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    output_paths = []

    for metric in HALSTEAD_METRICS:
        for cropped in [False, True]:
            range_name = "cropped" if cropped else "full"
            output_path = (
                output_dir / f"halstead_{metric}_vs_loc_{range_name}_go_ocaml.png"
            )
            figure = create_figure(plot_data, metric, percentile, cropped)
            figure.savefig(output_path, dpi=200, bbox_inches="tight")
            plt.close(figure)
            output_paths.append(output_path)

    return output_paths


if __name__ == "__main__":
    args = parse_args()
    for output_path in plot_halstead_counts_vs_loc(
        args.data_dir, args.output_dir, args.percentile
    ):
        print(f"Wrote {output_path}")
