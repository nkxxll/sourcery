import argparse
import math
import subprocess
import warnings
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

from plot_callgraph_results import load_results


REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT_PATH = Path("callgraph_indegree_vs_loc_p99_go_ocaml.png")
LANGUAGES = ["Golang", "Ocaml"]
DEFAULT_PERCENTILE = 99.0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Plot file- and version-level callgraph indegree against lines of code "
            "read from each sampled Git commit."
        )
    )
    parser.add_argument(
        "--results-dir",
        type=Path,
        default=REPOSITORY_ROOT / "results",
        help="Directory containing raw callgraph result CSVs",
    )
    parser.add_argument(
        "--projects-dir",
        type=Path,
        default=REPOSITORY_ROOT / "toanalyze" / "listed",
        help="Directory containing the sampled repositories",
    )
    parser.add_argument(
        "-o",
        "--outfile",
        type=Path,
        default=DEFAULT_OUTPUT_PATH,
        help=f"PNG output path (default: {DEFAULT_OUTPUT_PATH})",
    )
    parser.add_argument(
        "--percentile",
        type=float,
        default=DEFAULT_PERCENTILE,
        help=f"Percentile used to crop each axis (default: {DEFAULT_PERCENTILE})",
    )
    args = parser.parse_args()
    if not 0 < args.percentile <= 100:
        parser.error("--percentile must be greater than 0 and at most 100")
    return args


def git_file_line_counts(
    repository: Path, objects: list[tuple[str, str]]
) -> dict[tuple[str, str], int]:
    process = subprocess.Popen(
        ["git", "cat-file", "--batch"],
        cwd=repository,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
    )
    assert process.stdin is not None
    assert process.stdout is not None

    counts = {}
    try:
        for commit, file_path in objects:
            process.stdin.write(f"{commit}:{file_path}\n".encode())
            process.stdin.flush()
            header = process.stdout.readline().decode(errors="replace").strip()
            if header.endswith(" missing"):
                continue

            fields = header.split()
            if len(fields) != 3 or fields[1] != "blob":
                raise ValueError(
                    f"unexpected git cat-file response for {commit}:{file_path}: {header}"
                )
            content = process.stdout.read(int(fields[2]))
            process.stdout.read(1)
            counts[(commit, file_path)] = content.count(b"\n") + bool(
                content and not content.endswith(b"\n")
            )
    finally:
        process.stdin.close()
        process.wait()

    if process.returncode:
        raise subprocess.CalledProcessError(process.returncode, process.args)
    return counts


def build_plot_data(
    results_dir: Path, projects_dir: Path
) -> dict[tuple[str, str], pd.DataFrame]:
    results = load_results(results_dir)
    files = results[
        (results["metric_level"] == "file")
        & (results["metric_key"] == "total_indegree_per_file")
    ].copy()
    files["sample_commit_hash"] = files["sample_commit_hash"].astype(str)
    line_counts = {}

    for (project, language), group in files.groupby(
        ["project", "programming_language"], sort=True
    ):
        language_dir = "go" if language == "Golang" else "ocaml"
        repository = projects_dir / language_dir / project
        if not repository.is_dir():
            warnings.warn(f"sampled repository not found: {repository}", stacklevel=2)
            continue
        objects = list(
            group[["sample_commit_hash", "relative_file_path"]]
            .drop_duplicates()
            .itertuples(index=False, name=None)
        )
        counts = git_file_line_counts(repository, objects)
        line_counts.update(
            ((project, *key), value) for key, value in counts.items()
        )

    files["loc"] = [
        line_counts.get((row.project, row.sample_commit_hash, row.relative_file_path))
        for row in files.itertuples()
    ]
    missing = int(files["loc"].isna().sum())
    if missing:
        warnings.warn(f"omitting {missing} files unavailable at their sampled commit")
    files = files.dropna(subset=["loc"]).rename(columns={"value": "metric_value"})
    files["loc"] = files["loc"].astype(int)

    version_keys = [
        "project",
        "programming_language",
        "sample_number",
        "sample_commit_hash",
    ]
    versions = files.groupby(version_keys, as_index=False)[
        ["loc", "metric_value"]
    ].sum()

    plot_data = {}
    for language in LANGUAGES:
        plot_data[("file", language)] = files[
            files["programming_language"] == language
        ]
        plot_data[("version", language)] = versions[
            versions["programming_language"] == language
        ]
    return plot_data


def nice_upper_limit(value: float) -> float:
    if value <= 0:
        return 1.0
    magnitude = 10 ** math.floor(math.log10(value))
    normalized = value / magnitude
    multiplier = next(step for step in (1, 2, 5, 10) if normalized <= step)
    return multiplier * magnitude


def axis_limit(values: list[pd.Series], percentile: float) -> float:
    combined = np.concatenate([value.to_numpy(dtype=float) for value in values])
    return nice_upper_limit(float(np.percentile(combined, percentile)))


def create_figure(
    plot_data: dict[tuple[str, str], pd.DataFrame], percentile: float
) -> plt.Figure:
    fig, axes = plt.subplots(2, 2, figsize=(12, 8), squeeze=False)
    for row, level in enumerate(("file", "version")):
        level_data = [plot_data[(level, language)] for language in LANGUAGES]
        x_limit = axis_limit([data["loc"] for data in level_data], percentile)
        y_limit = axis_limit([data["metric_value"] for data in level_data], percentile)
        for col, language in enumerate(LANGUAGES):
            data = plot_data[(level, language)]
            visible = data[
                (data["loc"] <= x_limit) & (data["metric_value"] <= y_limit)
            ]
            ax = axes[row, col]
            ax.scatter(visible["loc"], visible["metric_value"], alpha=0.25, s=10)
            ax.set_title(f"{language}: {level} (n={len(visible)})")
            ax.set_xlim(0, x_limit)
            ax.set_ylim(0, y_limit)
            ax.set_xlabel("lines_of_code" if level == "file" else "total_lines_of_code")
            if col == 0:
                ax.set_ylabel(
                    "total_indegree_per_file"
                    if level == "file"
                    else "total_indegree_per_version"
                )
            ax.grid(True, alpha=0.25)

    fig.suptitle(
        f"Callgraph indegree vs lines of code ({percentile:g}th-percentile crop)"
    )
    fig.tight_layout()
    return fig


def plot_callgraph_indegree_vs_loc(
    results_dir: Path,
    projects_dir: Path,
    output_path: Path,
    percentile: float = DEFAULT_PERCENTILE,
) -> None:
    plot_data = build_plot_data(results_dir, projects_dir)
    figure = create_figure(plot_data, percentile)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    figure.savefig(output_path, dpi=200, bbox_inches="tight")
    plt.close(figure)


if __name__ == "__main__":
    args = parse_args()
    plot_callgraph_indegree_vs_loc(
        args.results_dir, args.projects_dir, args.outfile, args.percentile
    )
    print(f"Wrote cropped scatterplot to {args.outfile}")
