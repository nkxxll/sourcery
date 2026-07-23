import argparse
from pathlib import Path

import matplotlib.pyplot as plt

from helpers import data


DEFAULT_OUTPUT_DIR = Path(".")
LOC_OUTPUT_NAME = "file_change_count_vs_lines_of_code_go_ocaml.png"
CYCLOMATIC_OUTPUT_NAME = "file_change_count_vs_total_cyclomatic_go_ocaml.png"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Plot comparable Go and OCaml file change-count scatter plots."
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        default=Path("."),
        help="Directory containing metrics and file-change CSV files (default: .)",
    )
    parser.add_argument(
        "-o",
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help=f"Directory for PNG outputs (default: {DEFAULT_OUTPUT_DIR})",
    )
    parser.add_argument(
        "--version",
        default="latest",
        help="Version to plot, or 'all' (default: latest).",
    )
    parser.add_argument(
        "--log",
        action="store_true",
        help="Use logarithmic axes, omitting non-positive values.",
    )
    return parser.parse_args()


def plot_file_change_counts(
    data_dir: Path = Path("."),
    output_dir: Path = DEFAULT_OUTPUT_DIR,
    version: int | str = "latest",
    log: bool = False,
) -> list[Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    metrics = data(data_dir)
    try:
        version_value = int(version)
    except ValueError:
        version_value = version

    plots = [
        (
            metrics.plot_go_ocaml_file_loc_change_counts(
                version=version_value, log=log
            ),
            output_dir / LOC_OUTPUT_NAME,
        ),
        (
            metrics.plot_go_ocaml_file_cyclomatic_change_counts(
                version=version_value, log=log
            ),
            output_dir / CYCLOMATIC_OUTPUT_NAME,
        ),
    ]
    output_paths = []
    for figure, output_path in plots:
        figure.savefig(output_path, dpi=200, bbox_inches="tight")
        plt.close(figure)
        output_paths.append(output_path)
    return output_paths


if __name__ == "__main__":
    args = parse_args()
    output_paths = plot_file_change_counts(
        args.data_dir,
        args.output_dir,
        version=args.version,
        log=args.log,
    )
    for output_path in output_paths:
        print(f"Wrote file change-count scatterplot to {output_path}")
