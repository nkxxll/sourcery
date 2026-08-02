import argparse
from pathlib import Path

import matplotlib.pyplot as plt

from helpers import data


DEFAULT_OUTPUT_NAMES = {
    False: "indegree_vs_lines_of_code_go_ocaml.png",
    True: "unique_indegree_vs_lines_of_code_go_ocaml.png",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Plot file- and function-level indegree against lines of code "
            "for Go and OCaml."
        )
    )
    parser.add_argument(
        "-o",
        "--output-dir",
        type=Path,
        default=Path("."),
        help="Directory for PNG outputs (default: .)",
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        default=Path("."),
        help="Directory containing metrics CSV files (default: .)",
    )
    parser.add_argument(
        "--log",
        action="store_true",
        help="Use logarithmic axes, omitting non-positive values.",
    )
    return parser.parse_args()


def plot_indegree_vs_loc(
    data_dir: Path = Path("."),
    output_dir: Path = Path("."),
    log: bool = False,
) -> list[Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    metrics = data(data_dir)
    output_paths = []
    for unique, output_name in DEFAULT_OUTPUT_NAMES.items():
        figure = metrics.plot_go_ocaml_indegree_to_loc(unique=unique, log=log)
        output_path = output_dir / output_name
        figure.savefig(output_path, dpi=200, bbox_inches="tight")
        plt.close(figure)
        output_paths.append(output_path)
    return output_paths


if __name__ == "__main__":
    args = parse_args()
    for output_path in plot_indegree_vs_loc(
        args.data_dir, args.output_dir, log=args.log
    ):
        print(f"Wrote indegree/LOC scatterplot to {output_path}")
