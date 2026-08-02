import argparse
from pathlib import Path

import matplotlib.pyplot as plt

from helpers import data


DEFAULT_OUTPUT_PATH = Path("cyclomatic_complexity_vs_lines_of_code_go_ocaml.png")
DEFAULT_WIDE_OUTPUT_PATH = Path(
    "cyclomatic_complexity_vs_lines_of_code_go_ocaml_5000.png"
)
AXIS_LIMIT = 2000
WIDE_AXIS_LIMIT = 5000


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Plot file- and function-level cyclomatic complexity against "
            f"lines of code for Go and OCaml on a {AXIS_LIMIT} by "
            f"{AXIS_LIMIT} scale."
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
        "--wide-outfile",
        type=Path,
        default=DEFAULT_WIDE_OUTPUT_PATH,
        help=f"5000 by 5000 PNG output path (default: {DEFAULT_WIDE_OUTPUT_PATH})",
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
    wide_output_path: Path = DEFAULT_WIDE_OUTPUT_PATH,
) -> None:
    figure = data(data_dir).plot_go_ocaml_cyclomatic_to_loc(log=log)
    for ax in figure.axes:
        ax.set_xlim(1 if log else 0, AXIS_LIMIT)
        ax.set_ylim(1 if log else 0, AXIS_LIMIT)
    figure.savefig(output_path, dpi=200, bbox_inches="tight")

    for ax in figure.axes:
        ax.set_xlim(1 if log else 0, WIDE_AXIS_LIMIT)
        ax.set_ylim(1 if log else 0, WIDE_AXIS_LIMIT)
    figure.savefig(wide_output_path, dpi=200, bbox_inches="tight")
    plt.close(figure)


if __name__ == "__main__":
    args = parse_args()
    plot_cyclomatic_vs_loc(
        args.data_dir,
        args.outfile,
        log=args.log,
        wide_output_path=args.wide_outfile,
    )
    print(f"Wrote 2000x2000 cyclomatic/LOC scatterplot to {args.outfile}")
    print(f"Wrote 5000x5000 cyclomatic/LOC scatterplot to {args.wide_outfile}")
