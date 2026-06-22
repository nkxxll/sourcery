import argparse
import shutil
from pathlib import Path

import matplotlib.pyplot as plt

from chart_metric_evolution import (
    DEFAULT_OUTPUT_PATH as LINECHART_OUTPUT_PATH,
    plot_metric_evolution_by_version,
)
from chart_metrics_by_language import (
    DEFAULT_OUTPUT_PATH as BOXPLOT_OUTPUT_PATH,
    plot_metrics_stacked_by_input,
)
from core import (
    ADJUSTED_CYCLOMATIC_METRIC,
    METRIC_COL,
    METRIC_LEVEL_COL,
    read_csv_with_columns,
    load_extremes,
    load_metrics,
    print_medians,
)
from extremes_report import DEFAULT_OUTPUT_PATH, write_extremes_report

SCRIPT_DIR = Path(__file__).resolve().parent


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="chart", required=True)

    boxplot_parser = subparsers.add_parser(
        "boxplot",
        help="plot metric distributions by programming language",
    )
    add_chart_args(boxplot_parser, BOXPLOT_OUTPUT_PATH)

    linechart_parser = subparsers.add_parser(
        "linechart",
        help="plot metric mean and median over version files",
    )
    add_chart_args(linechart_parser, LINECHART_OUTPUT_PATH)

    extremes_parser = subparsers.add_parser(
        "extremes",
        help="write a Markdown report for ranked metric extremes",
    )
    add_extremes_args(extremes_parser)

    return parser.parse_args()


def add_chart_args(
    parser: argparse.ArgumentParser,
    default_output_path: Path,
) -> None:
    add_metric_args(parser)
    parser.add_argument(
        "-o",
        "--outfile",
        type=Path,
        default=default_output_path,
        help=f"PNG output path (default: {default_output_path})",
    )
    parser.add_argument(
        "csv_paths",
        nargs="+",
        type=Path,
        help="CSV files to plot, in version order",
    )


def add_extremes_args(parser: argparse.ArgumentParser) -> None:
    add_metric_args(parser)
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT_PATH,
        help=f"Markdown report path (default: {DEFAULT_OUTPUT_PATH})",
    )
    parser.add_argument(
        "csv_paths",
        nargs="+",
        type=Path,
        help="extremes CSV files to report, in version order",
    )


def add_metric_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "-i",
        "--interactive",
        action="store_true",
        help="choose metrics interactively with gum",
    )


def load_metric_names(csv_paths: list[Path]) -> list[str]:
    metric_names = sorted(
        set(
            metric_name
            for csv_path in csv_paths
            for metric_name in read_csv_with_columns(csv_path, {METRIC_COL})[
                METRIC_COL
            ].dropna()
        )
    )
    if not metric_names:
        raise ValueError("input CSVs contain no metric names")

    if {"lines_of_code", "total_cyclomatic"}.issubset(metric_names):
        metric_names.append(ADJUSTED_CYCLOMATIC_METRIC)

    return metric_names


def choose_metric_names_interactively(metric_names: list[str]) -> list[str]:
    if shutil.which("gum") is None:
        raise RuntimeError("gum is required when using -i/--interactive")

    selected = subprocess.run(
        ["gum", "choose", "--no-limit", "--header", "Choose metrics to plot"],
        input="\n".join(metric_names) + "\n",
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.splitlines()

    if not selected:
        raise ValueError("No metrics selected")

    return selected


def choose_metric_names(interactive: bool, csv_paths: list[Path]) -> list[str]:
    metric_names = load_metric_names(csv_paths)

    if interactive:
        return choose_metric_names_interactively(metric_names)

    return metric_names


def output_path_with_metric_level(output_path: Path, metric_level: str) -> Path:
    return output_path.with_name(
        f"{output_path.stem}_{metric_level}{output_path.suffix}"
    )


def metric_names_for_level(df, metric_names: list[str], metric_level: str) -> list[str]:
    level_metric_names = set(
        df.loc[df[METRIC_LEVEL_COL] == metric_level, METRIC_COL].dropna().unique()
    )
    return [metric_name for metric_name in metric_names if metric_name in level_metric_names]


if __name__ == "__main__":
    args = parse_args()
    metric_names = choose_metric_names(args.interactive, args.csv_paths)

    if args.chart == "extremes":
        extremes_metric_names = [
            metric_name
            for metric_name in metric_names
            if metric_name != ADJUSTED_CYCLOMATIC_METRIC
        ]
        df = load_extremes(args.csv_paths, extremes_metric_names)

        if df.empty:
            raise ValueError(f"No rows found for metrics: {extremes_metric_names}")

        write_extremes_report(df, extremes_metric_names, args.output)
        print(f"Wrote extremes report to {args.output}")
        raise SystemExit

    df = load_metrics(args.csv_paths, metric_names)

    if df.empty:
        raise ValueError(f"No rows found for metrics: {metric_names}")

    print_medians(df, metric_names)

    for metric_level in ["file", "function", "version"]:
        level_df = df[df[METRIC_LEVEL_COL] == metric_level]
        if level_df.empty:
            continue

        level_metric_names = metric_names_for_level(df, metric_names, metric_level)
        level_output_path = output_path_with_metric_level(args.outfile, metric_level)

        if args.chart == "boxplot":
            plot_metrics_stacked_by_input(
                level_df,
                level_metric_names,
                level_output_path,
            )
        elif args.chart == "linechart":
            plot_metric_evolution_by_version(
                level_df,
                level_metric_names,
                level_output_path,
            )

        print(f"Wrote {metric_level} {args.chart} to {level_output_path}")

    if "agg" not in plt.get_backend().lower():
        plt.show()
