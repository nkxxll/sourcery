import argparse
import sys
from pathlib import Path

from PIL import Image

Image.MAX_IMAGE_PIXELS = None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Crop one plot out of a larger PNG grid."
    )
    parser.add_argument("input", type=Path, help="source image path")
    parser.add_argument("output", type=Path, help="cropped image output path")
    parser.add_argument(
        "--box",
        type=int,
        nargs=4,
        metavar=("LEFT", "TOP", "RIGHT", "BOTTOM"),
        help="crop using pixel coordinates instead of a grid position",
    )
    parser.add_argument("--rows", type=int, help="number of rows in the plot grid")
    parser.add_argument("--cols", type=int, help="number of columns in the plot grid")
    parser.add_argument("--row", type=int, help="1-based row of the plot to crop")
    parser.add_argument("--col", type=int, help="1-based column of the plot to crop")
    parser.add_argument(
        "--padding",
        type=int,
        default=0,
        help="extra pixels to include around the crop (default: 0)",
    )
    return parser.parse_args()


def validate_positive(name: str, value: int | None) -> int:
    if value is None:
        raise ValueError(f"{name} is required unless --box is used")
    if value < 1:
        raise ValueError(f"{name} must be at least 1")
    return value


def clamp(value: int, lower: int, upper: int) -> int:
    return max(lower, min(value, upper))


def grid_box(
    width: int,
    height: int,
    rows: int,
    cols: int,
    row: int,
    col: int,
    padding: int,
) -> tuple[int, int, int, int]:
    if row > rows:
        raise ValueError(f"row {row} is outside a {rows}-row grid")
    if col > cols:
        raise ValueError(f"col {col} is outside a {cols}-column grid")

    cell_width = width / cols
    cell_height = height / rows
    left = round((col - 1) * cell_width) - padding
    top = round((row - 1) * cell_height) - padding
    right = round(col * cell_width) + padding
    bottom = round(row * cell_height) + padding

    return (
        clamp(left, 0, width),
        clamp(top, 0, height),
        clamp(right, 0, width),
        clamp(bottom, 0, height),
    )


def crop_image(args: argparse.Namespace) -> None:
    if args.padding < 0:
        raise ValueError("padding must be at least 0")

    with Image.open(args.input) as image:
        width, height = image.size

        if args.box is not None:
            left, top, right, bottom = args.box
            box = (
                clamp(left - args.padding, 0, width),
                clamp(top - args.padding, 0, height),
                clamp(right + args.padding, 0, width),
                clamp(bottom + args.padding, 0, height),
            )
        else:
            rows = validate_positive("--rows", args.rows)
            cols = validate_positive("--cols", args.cols)
            row = validate_positive("--row", args.row)
            col = validate_positive("--col", args.col)
            box = grid_box(width, height, rows, cols, row, col, args.padding)

        left, top, right, bottom = box
        if left >= right or top >= bottom:
            raise ValueError(f"invalid crop box: {box}")

        args.output.parent.mkdir(parents=True, exist_ok=True)
        image.crop(box).save(args.output)

    print(f"Wrote {args.output} from crop box {box}")


def main() -> int:
    try:
        crop_image(parse_args())
    except (OSError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
