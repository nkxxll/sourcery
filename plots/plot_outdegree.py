import argparse
from pathlib import Path

import matplotlib.pyplot as plt

from helpers import data


DEFAULT_OUTPUT_PATH = Path("outdegree_vs_function_length_go_ocaml.png")
METRICS = ["outdegree", "unique_outdegree"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Plot function outdegree and unique outdegree against function length "
            "for Go and OCaml."
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


def plot_outdegree(
    data_dir: Path = Path("."),
    output_path: Path = DEFAULT_OUTPUT_PATH,
    log: bool = False,
) -> None:
    fig = data(data_dir).plot_go_ocaml_function_spread_to_loc(
        metrics=METRICS,
        log=log,
    )
    fig.savefig(output_path, dpi=200, bbox_inches="tight")
    plt.close(fig)


if __name__ == "__main__":
    args = parse_args()
    plot_outdegree(args.data_dir, args.outfile, log=args.log)
    print(f"Wrote outdegree scatterplot to {args.outfile}")
