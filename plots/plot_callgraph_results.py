import argparse
import io
import warnings
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

from chart_metric_evolution import plot_metric_evolution_by_version
from chart_metrics_by_language import plot_metrics_stacked_by_input
from core import INPUT_INDEX_COL, METRIC_COL, METRIC_LEVEL_COL, VALUE_COL


HEADER_PREFIX = "metric_key,metric_label,"
LANGUAGES = ["Golang", "Ocaml"]
METRICS_BY_LEVEL = {
    "file": [
        "mean_indegree_per_file",
        "mean_outdegree_per_file",
        "mean_unique_indegree_per_file",
        "mean_unique_outdegree_per_file",
        "total_indegree_per_file",
        "total_outdegree_per_file",
        "total_unique_indegree_per_file",
        "total_unique_outdegree_per_file",
    ],
    "function": [
        "indegree",
        "outdegree",
        "unique_indegree",
        "unique_outdegree",
    ],
}
SCATTER_METRICS = {
    "indegree": ("mean_indegree_per_file", "indegree"),
    "outdegree": ("mean_outdegree_per_file", "outdegree"),
    "unique_indegree": ("mean_unique_indegree_per_file", "unique_indegree"),
    "unique_outdegree": ("mean_unique_outdegree_per_file", "unique_outdegree"),
}
TOTAL_FILE_SCATTER_METRICS = [
    "total_indegree_per_file",
    "total_outdegree_per_file",
    "total_unique_indegree_per_file",
    "total_unique_outdegree_per_file",
]
VERSION_METRICS = {
    "total_indegree_per_file": "total_indegree_per_version",
    "total_outdegree_per_file": "total_outdegree_per_version",
    "total_unique_indegree_per_file": "total_unique_indegree_per_version",
    "total_unique_outdegree_per_file": "total_unique_outdegree_per_version",
}


def parse_args() -> argparse.Namespace:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(
        description="Plot callgraph metrics from the per-project results CSVs."
    )
    parser.add_argument(
        "--metrics-dir",
        type=Path,
        default=root / "plots",
        help="Directory containing metrics1.csv through metrics10.csv",
    )
    parser.add_argument(
        "--results-dir",
        type=Path,
        default=root / "results",
        help="Directory containing result CSVs (default: repository results directory)",
    )
    parser.add_argument(
        "-o",
        "--output-dir",
        type=Path,
        default=root / "plots" / "callgraph_results",
        help="Directory for PNG and chart-data CSV outputs",
    )
    parser.add_argument(
        "--log",
        action="store_true",
        help="Use logarithmic axes for scatterplots, omitting non-positive values",
    )
    return parser.parse_args()


def read_result_csv(path: Path) -> pd.DataFrame:
    with path.open(encoding="utf-8", errors="replace") as result_file:
        for line in result_file:
            if line.startswith(HEADER_PREFIX):
                csv_text = line + result_file.read()
                break
        else:
            raise ValueError(f"no CSV header found in {path}")

    try:
        df = pd.read_csv(io.StringIO(csv_text))
    except Exception as exc:
        raise ValueError(f"failed to read CSV data from {path}: {exc}") from exc

    df["project"] = path.stem
    return df


def load_results(results_dir: Path) -> pd.DataFrame:
    paths = sorted(results_dir.glob("**/*.csv"))
    if not paths:
        raise ValueError(f"no CSV files found under {results_dir}")

    frames = []
    for path in paths:
        try:
            frames.append(read_result_csv(path))
        except ValueError as exc:
            warnings.warn(str(exc), RuntimeWarning, stacklevel=2)

    if not frames:
        raise ValueError(f"none of the files under {results_dir} contained CSV data")

    df = pd.concat(frames, ignore_index=True)
    required = {
        "programming_language",
        "sample_number",
        METRIC_LEVEL_COL,
        METRIC_COL,
        VALUE_COL,
    }
    missing = sorted(required - set(df.columns))
    if missing:
        raise ValueError("result CSVs are missing columns: " + ", ".join(missing))

    df[INPUT_INDEX_COL] = pd.to_numeric(df["sample_number"], errors="coerce")
    df[VALUE_COL] = pd.to_numeric(df[VALUE_COL], errors="coerce")
    df = df.dropna(subset=[INPUT_INDEX_COL, VALUE_COL]).copy()
    df[INPUT_INDEX_COL] = df[INPUT_INDEX_COL].astype(int)
    df["relative_file_path"] = df.apply(
        lambda row: relative_result_path(row["file_path"], row["project"]), axis=1
    )
    df["function_start_line"] = pd.to_numeric(
        df["function_start_line"], errors="coerce"
    )
    return df


def relative_result_path(file_path: object, project: str) -> str:
    path = str(file_path).replace("\\", "/")
    marker = f"/{project}/"
    if marker in path:
        return path.split(marker, 1)[1]
    return path.lstrip("./")


def load_size_metrics(metrics_dir: Path) -> pd.DataFrame:
    paths = [metrics_dir / f"metrics{sample}.csv" for sample in range(1, 11)]
    missing = [path for path in paths if not path.is_file()]
    if missing:
        raise ValueError("missing metrics CSVs: " + ", ".join(map(str, missing)))

    columns = [
        METRIC_COL,
        "codebase_name",
        "programming_language",
        "sample_number",
        METRIC_LEVEL_COL,
        "file_path",
        "function_name",
        "function_start_line",
        VALUE_COL,
    ]
    frames = []
    for path in paths:
        metrics = pd.read_csv(path, usecols=columns)
        frames.append(
            metrics[metrics[METRIC_COL].isin(["lines_of_code", "function_length"])]
        )

    df = pd.concat(frames, ignore_index=True)
    df["project"] = df["codebase_name"].str.replace(
        r" \(sample analysis: \d+ samples\)$", "", regex=True
    )
    df["relative_file_path"] = df["file_path"].astype(str).str.replace(
        "\\", "/", regex=False
    )
    df["function_name"] = df["function_name"].str.replace(
        r":\d+:\d+$", "", regex=True
    )
    df["function_start_line"] = pd.to_numeric(
        df["function_start_line"], errors="coerce"
    )
    df["sample_number"] = pd.to_numeric(df["sample_number"], errors="coerce")
    df[VALUE_COL] = pd.to_numeric(df[VALUE_COL], errors="coerce")
    return df.dropna(subset=["sample_number", VALUE_COL]).copy()


def plot_line_and_box_charts(df: pd.DataFrame, output_dir: Path) -> list[Path]:
    output_paths = []
    for level, metric_names in METRICS_BY_LEVEL.items():
        level_df = df[
            (df[METRIC_LEVEL_COL] == level) & df[METRIC_COL].isin(metric_names)
        ].copy()
        missing = sorted(set(metric_names) - set(level_df[METRIC_COL].unique()))
        if missing:
            warnings.warn(
                f"skipping {level} line and box charts; missing metrics: {', '.join(missing)}",
                RuntimeWarning,
                stacklevel=2,
            )
            continue

        line_path = output_dir / f"callgraph_linechart_{level}.png"
        box_path = output_dir / f"callgraph_boxplot_{level}.png"
        plot_metric_evolution_by_version(level_df, metric_names, line_path)
        plot_metrics_stacked_by_input(level_df, metric_names, box_path)
        output_paths.extend([line_path, box_path])
    return output_paths


def build_version_metrics(df: pd.DataFrame) -> pd.DataFrame:
    file_totals = df[
        (df[METRIC_LEVEL_COL] == "file")
        & df[METRIC_COL].isin(VERSION_METRICS)
    ].copy()
    version_metrics = (
        file_totals.groupby(
            [
                "project",
                "codebase_name",
                "programming_language",
                "sample_number",
                INPUT_INDEX_COL,
                METRIC_COL,
            ],
            as_index=False,
            dropna=False,
        )[VALUE_COL]
        .sum()
    )
    version_metrics[METRIC_COL] = version_metrics[METRIC_COL].map(VERSION_METRICS)
    version_metrics[METRIC_LEVEL_COL] = "version"
    return version_metrics


def plot_version_charts(df: pd.DataFrame, output_dir: Path) -> list[Path]:
    metric_names = list(VERSION_METRICS.values())
    line_path = output_dir / "callgraph_linechart_version.png"
    box_path = output_dir / "callgraph_boxplot_version.png"
    plot_metric_evolution_by_version(df, metric_names, line_path)
    plot_metrics_stacked_by_input(df, metric_names, box_path)
    return [line_path, box_path]


def plot_scatter(
    df: pd.DataFrame,
    size_metrics: pd.DataFrame,
    metric_name: str,
    file_metric: str,
    function_metric: str,
    output_path: Path,
    log: bool,
) -> bool:
    specifications = [
        ("file", "lines_of_code", file_metric, []),
        (
            "function",
            "function_length",
            function_metric,
            ["function_name", "function_start_line"],
        ),
    ]
    pairs = {}
    all_x = []

    base_keys = [
        "project",
        "programming_language",
        "sample_number",
        "relative_file_path",
    ]
    for level, loc_metric, degree_metric, extra_keys in specifications:
        keys = base_keys + extra_keys
        level_df = df[
            (df[METRIC_LEVEL_COL] == level) & (df[METRIC_COL] == degree_metric)
        ]
        loc_df = size_metrics[
            (size_metrics[METRIC_LEVEL_COL] == level)
            & (size_metrics[METRIC_COL] == loc_metric)
        ][keys + [VALUE_COL]].rename(columns={VALUE_COL: "loc"})
        for language in LANGUAGES:
            language_df = level_df[level_df["programming_language"] == language]
            paired = language_df[keys + [VALUE_COL]].rename(
                columns={VALUE_COL: "metric_value"}
            ).merge(loc_df, on=keys, how="inner")
            paired = paired[
                np.isfinite(paired["loc"])
                & np.isfinite(paired["metric_value"])
            ]
            if log:
                paired = paired[
                    (paired["loc"] > 0) & (paired["metric_value"] > 0)
                ]
            pairs[(level, language)] = paired
            all_x.extend(paired["loc"])

    if not all_x:
        warnings.warn(
            f"skipping {metric_name} scatterplot; results need lines_of_code and "
            "function_length rows with matching project/sample/entity keys",
            RuntimeWarning,
            stacklevel=2,
        )
        return False

    fig, axes = plt.subplots(2, 2, figsize=(11, 8), squeeze=False)
    for row, (level, loc_metric, degree_metric, _) in enumerate(specifications):
        for col, language in enumerate(LANGUAGES):
            ax = axes[row, col]
            paired = pairs[(level, language)]
            ax.scatter(paired["loc"], paired["metric_value"], alpha=0.25, s=10)
            ax.set_title(language)
            ax.set_xlabel(loc_metric)
            ax.set_ylabel(f"{degree_metric} ({level})")
            ax.grid(True, alpha=0.25)
            if log:
                ax.set_xscale("log")
                ax.set_yscale("log")

    fig.suptitle(f"{metric_name.replace('_', ' ').title()} vs lines of code")
    fig.tight_layout()
    fig.savefig(output_path, dpi=200, bbox_inches="tight")
    plt.close(fig)
    return True


def plot_file_scatter(
    df: pd.DataFrame,
    size_metrics: pd.DataFrame,
    metric_name: str,
    output_path: Path,
    log: bool,
) -> bool:
    keys = [
        "project",
        "programming_language",
        "sample_number",
        "relative_file_path",
    ]
    metric_df = df[
        (df[METRIC_LEVEL_COL] == "file") & (df[METRIC_COL] == metric_name)
    ]
    loc_df = size_metrics[
        (size_metrics[METRIC_LEVEL_COL] == "file")
        & (size_metrics[METRIC_COL] == "lines_of_code")
    ][keys + [VALUE_COL]].rename(columns={VALUE_COL: "loc"})
    pairs = {}

    for language in LANGUAGES:
        paired = (
            metric_df[metric_df["programming_language"] == language][
                keys + [VALUE_COL]
            ]
            .rename(columns={VALUE_COL: "metric_value"})
            .merge(loc_df, on=keys, how="inner")
        )
        paired = paired[
            np.isfinite(paired["loc"]) & np.isfinite(paired["metric_value"])
        ]
        if log:
            paired = paired[(paired["loc"] > 0) & (paired["metric_value"] > 0)]
        pairs[language] = paired

    if not any(not paired.empty for paired in pairs.values()):
        warnings.warn(
            f"skipping {metric_name} scatterplot; no matching LOC rows",
            RuntimeWarning,
            stacklevel=2,
        )
        return False

    fig, axes = plt.subplots(1, 2, figsize=(11, 4), squeeze=False)
    for col, language in enumerate(LANGUAGES):
        ax = axes[0, col]
        paired = pairs[language]
        ax.scatter(paired["loc"], paired["metric_value"], alpha=0.25, s=10)
        ax.set_title(language)
        ax.set_xlabel("lines_of_code")
        ax.set_ylabel(metric_name)
        ax.grid(True, alpha=0.25)
        if log:
            ax.set_xscale("log")
            ax.set_yscale("log")

    fig.suptitle(f"{metric_name.replace('_', ' ').title()} vs lines of code")
    fig.tight_layout()
    fig.savefig(output_path, dpi=200, bbox_inches="tight")
    plt.close(fig)
    return True


def main() -> None:
    args = parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    df = load_results(args.results_dir)
    size_metrics = load_size_metrics(args.metrics_dir)
    output_paths = plot_line_and_box_charts(df, args.output_dir)
    output_paths.extend(
        plot_version_charts(build_version_metrics(df), args.output_dir)
    )

    for metric_name, (file_metric, function_metric) in SCATTER_METRICS.items():
        output_path = args.output_dir / f"{metric_name}_vs_loc.png"
        if plot_scatter(
            df,
            size_metrics,
            metric_name,
            file_metric,
            function_metric,
            output_path,
            args.log,
        ):
            output_paths.append(output_path)

    for metric_name in TOTAL_FILE_SCATTER_METRICS:
        output_path = args.output_dir / f"{metric_name}_vs_loc.png"
        if plot_file_scatter(
            df, size_metrics, metric_name, output_path, args.log
        ):
            output_paths.append(output_path)

    for output_path in output_paths:
        print(f"Wrote {output_path}")


if __name__ == "__main__":
    main()
