import argparse
from pathlib import Path

import matplotlib.pyplot as plt

from helpers import data


DEFAULT_OUTPUT_PATH = Path("cyclomatic_complexity_vs_lines_of_code_go_ocaml.png")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Plot file- and function-level cyclomatic complexity against "
            "lines of code for Go and OCaml."
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


def plot_cyclomatic_vs_loc(
    data_dir: Path = Path("."),
    output_path: Path = DEFAULT_OUTPUT_PATH,
    log: bool = False,
) -> None:
    figure = data(data_dir).plot_go_ocaml_cyclomatic_to_loc(log=log)
    figure.savefig(output_path, dpi=200, bbox_inches="tight")
    plt.close(figure)


if __name__ == "__main__":
    args = parse_args()
    plot_cyclomatic_vs_loc(args.data_dir, args.outfile, log=args.log)
    print(f"Wrote cyclomatic/LOC scatterplot to {args.outfile}")
