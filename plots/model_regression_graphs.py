import argparse
from dataclasses import dataclass
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
from matplotlib.gridspec import GridSpec
from matplotlib.lines import Line2D


DEFAULT_OUTPUT_PATH = Path("model_regression_examples.png")
BLUE = "#1f5bd8"
TEXT_SIZE = 11


@dataclass(frozen=True)
class ModelExample:
    name: str
    formula: str
    condition: str
    transformed_equation: str
    original_x_label: str
    original_y_label: str
    transformed_x_label: str
    transformed_y_label: str
    x: np.ndarray
    y: np.ndarray
    original_curve: tuple[np.ndarray, np.ndarray]
    transformed_x: np.ndarray
    transformed_y: np.ndarray


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Draw explanatory regression-model graphs."
    )
    parser.add_argument(
        "-o",
        "--outfile",
        type=Path,
        default=DEFAULT_OUTPUT_PATH,
        help=f"PNG output path (default: {DEFAULT_OUTPUT_PATH})",
    )
    return parser.parse_args()


def make_examples() -> list[ModelExample]:
    rng = np.random.default_rng(7)
    x = np.linspace(1, 10, 16)
    curve_x = np.linspace(0.7, 10.5, 300)

    linear_y = 0.7 + 0.75 * x + rng.normal(0, 0.45, len(x))
    power_y = 1.15 * x**0.62 * np.exp(rng.normal(0, 0.08, len(x)))
    logarithmic_y = 1.0 + 1.8 * np.log(x) + rng.normal(0, 0.25, len(x))
    exponential_y = 0.55 * np.exp(0.32 * x) * np.exp(rng.normal(0, 0.16, len(x)))

    return [
        ModelExample(
            name="Linear",
            formula=r"$y = a \cdot x$",
            condition="",
            transformed_equation=r"$y = \alpha + \beta \cdot x$",
            original_x_label=r"$x$",
            original_y_label=r"$y$",
            transformed_x_label=r"$x$",
            transformed_y_label=r"$y$",
            x=x,
            y=linear_y,
            original_curve=(curve_x, 0.7 + 0.75 * curve_x),
            transformed_x=x,
            transformed_y=linear_y,
        ),
        ModelExample(
            name="Power law",
            formula=r"$y = a \cdot x^b$",
            condition=r"$(b > 0)$",
            transformed_equation=r"$\log(y) = \alpha + \beta \cdot \log(x)$",
            original_x_label=r"$x$",
            original_y_label=r"$y$",
            transformed_x_label=r"$\log(x)$",
            transformed_y_label=r"$\log(y)$",
            x=x,
            y=power_y,
            original_curve=(curve_x, 1.15 * curve_x**0.62),
            transformed_x=np.log(x),
            transformed_y=np.log(power_y),
        ),
        ModelExample(
            name="Logarithmic growth",
            formula=r"$y = a + b \cdot \log(x)$",
            condition=r"$(b > 0)$",
            transformed_equation=r"$y = \alpha + \beta \cdot \log(x)$",
            original_x_label=r"$x$",
            original_y_label=r"$y$",
            transformed_x_label=r"$\log(x)$",
            transformed_y_label=r"$y$",
            x=x,
            y=logarithmic_y,
            original_curve=(curve_x, 1.0 + 1.8 * np.log(curve_x)),
            transformed_x=np.log(x),
            transformed_y=logarithmic_y,
        ),
        ModelExample(
            name="Exponential growth",
            formula=r"$y = a \cdot e^{b \cdot x}$",
            condition=r"$(b > 0)$",
            transformed_equation=r"$\log(y) = \alpha + \beta \cdot x$",
            original_x_label=r"$x$",
            original_y_label=r"$y$",
            transformed_x_label=r"$x$",
            transformed_y_label=r"$\log(y)$",
            x=x,
            y=exponential_y,
            original_curve=(curve_x, 0.55 * np.exp(0.32 * curve_x)),
            transformed_x=x,
            transformed_y=np.log(exponential_y),
        ),
    ]


def plot_examples(output_path: Path = DEFAULT_OUTPUT_PATH) -> None:
    examples = make_examples()
    plt.rcParams.update(
        {
            "font.family": "serif",
            "mathtext.fontset": "dejavuserif",
            "axes.linewidth": 0.8,
        }
    )

    fig = plt.figure(figsize=(13, 7.5), facecolor="white")
    grid = GridSpec(
        len(examples) + 1,
        3,
        figure=fig,
        height_ratios=[0.18, 1, 1, 1, 1],
        width_ratios=[1.2, 2.0, 2.7],
        hspace=0.35,
        wspace=0.28,
    )

    add_header(fig, grid)
    for row, example in enumerate(examples, start=1):
        add_model_text(fig.add_subplot(grid[row, 0]), example)
        add_original_plot(fig.add_subplot(grid[row, 1]), example)
        add_transformed_plot(fig.add_subplot(grid[row, 2]), example)

    add_table_lines(fig)
    fig.text(
        0.5,
        0.02,
        r"Figure: Common model forms and their linearized regression equations for predicting $y$ from $x$.",
        ha="center",
        fontsize=12,
    )
    fig.savefig(output_path, dpi=220, bbox_inches="tight")
    plt.close(fig)


def add_header(fig: plt.Figure, grid: GridSpec) -> None:
    headers = [
        "Model",
        "Regression model (original scale)",
        "Linearized regression (transformed scale)",
    ]
    for col, header in enumerate(headers):
        ax = fig.add_subplot(grid[0, col])
        ax.axis("off")
        ax.text(0, 0.5, header, fontsize=13, fontweight="bold", va="center")


def add_model_text(ax: plt.Axes, example: ModelExample) -> None:
    ax.axis("off")
    ax.text(0.02, 0.82, example.name, fontsize=TEXT_SIZE + 1, va="top")
    ax.text(0.02, 0.50, example.formula, fontsize=TEXT_SIZE + 1, va="top")
    if example.condition:
        ax.text(0.02, 0.25, example.condition, fontsize=TEXT_SIZE + 1, va="top")


def add_original_plot(ax: plt.Axes, example: ModelExample) -> None:
    curve_x, curve_y = example.original_curve
    style_axis(ax, example.original_x_label, example.original_y_label)
    ax.scatter(example.x, example.y, s=18, color=BLUE, alpha=0.9)
    ax.plot(curve_x, curve_y, color=BLUE, linewidth=1.7)
    set_limits(ax, np.concatenate([example.x, curve_x]), np.concatenate([example.y, curve_y]))


def add_transformed_plot(ax: plt.Axes, example: ModelExample) -> None:
    style_axis(ax, example.transformed_x_label, example.transformed_y_label)
    ax.scatter(example.transformed_x, example.transformed_y, s=18, color=BLUE, alpha=0.9)
    slope, intercept = np.polyfit(example.transformed_x, example.transformed_y, 1)
    line_x = np.linspace(example.transformed_x.min(), example.transformed_x.max(), 100)
    ax.plot(line_x, intercept + slope * line_x, color=BLUE, linewidth=1.7)
    ax.text(
        -0.32,
        0.65,
        example.transformed_equation,
        transform=ax.transAxes,
        fontsize=TEXT_SIZE + 1,
        va="center",
    )
    set_limits(ax, example.transformed_x, example.transformed_y)


def style_axis(ax: plt.Axes, x_label: str, y_label: str) -> None:
    for spine in ax.spines.values():
        spine.set_visible(False)
    ax.set_xticks([])
    ax.set_yticks([])
    ax.set_facecolor("white")
    ax.annotate(
        "",
        xy=(1.02, 0),
        xytext=(0, 0),
        xycoords="axes fraction",
        arrowprops={"arrowstyle": "-|>", "linewidth": 0.9, "color": "black"},
    )
    ax.annotate(
        "",
        xy=(0, 1.02),
        xytext=(0, 0),
        xycoords="axes fraction",
        arrowprops={"arrowstyle": "-|>", "linewidth": 0.9, "color": "black"},
    )
    ax.text(1.06, -0.03, x_label, transform=ax.transAxes, fontsize=TEXT_SIZE + 1)
    ax.text(-0.04, 1.06, y_label, transform=ax.transAxes, fontsize=TEXT_SIZE + 1)


def set_limits(ax: plt.Axes, x_values: np.ndarray, y_values: np.ndarray) -> None:
    x_padding = (x_values.max() - x_values.min()) * 0.08
    y_padding = (y_values.max() - y_values.min()) * 0.12
    ax.set_xlim(x_values.min() - x_padding, x_values.max() + x_padding)
    ax.set_ylim(y_values.min() - y_padding, y_values.max() + y_padding)


def add_table_lines(fig: plt.Figure) -> None:
    axes = fig.axes
    positions = [ax.get_position(fig).frozen() for ax in axes]

    left = min(pos.x0 for pos in positions)
    right = max(pos.x1 for pos in positions)
    top = max(pos.y1 for pos in positions)
    bottom = min(pos.y0 for pos in positions)

    col_boundaries = [axes[col].get_position(fig).x0 for col in range(1, 3)]
    row_boundaries = [axes[row * 3].get_position(fig).y1 for row in range(1, 5)]

    for x_pos in col_boundaries:
        fig.add_artist(Line2D([x_pos, x_pos], [bottom, top], color="0.45", linewidth=0.6))
    for y_pos in [top, *row_boundaries, bottom]:
        fig.add_artist(Line2D([left, right], [y_pos, y_pos], color="0.45", linewidth=0.6))


if __name__ == "__main__":
    args = parse_args()
    plot_examples(args.outfile)
    print(f"Wrote regression model examples to {args.outfile}")
