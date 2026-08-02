"""Print randomized median function samples as Vim gF locations."""

from __future__ import annotations

import argparse
import os
from pathlib import Path

import pandas as pd

from helpers import Language, data


VERSION = 10
SAMPLES_PER_LANGUAGE = 10
DEFAULT_METRIC = "outdegree"
DEFAULT_OUTPUT = Path(__file__).resolve().parent / "function_samples_gf.txt"
LANGUAGES = [Language.GO.value, Language.OCAML.value]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Print randomized version-10 median function samples as "
            "relative-path:start-line locations for Vim gF."
        )
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        default=Path(__file__).resolve().parent,
        help="Directory containing metrics CSV files.",
    )
    parser.add_argument(
        "--metric",
        default=DEFAULT_METRIC,
        help=f"Function metric used to select the median (default: {DEFAULT_METRIC}).",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=0,
        help="Random seed (default: 0).",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help=f"Write locations to this file (default: {DEFAULT_OUTPUT}).",
    )
    return parser.parse_args()


def sample_functions(metrics, metric: str, seed: int) -> pd.DataFrame:
    samples = []
    for language in LANGUAGES:
        rows = metrics.sample_median_by_language(
            count=SAMPLES_PER_LANGUAGE,
            metric=metric,
            level="function",
            input_index=VERSION,
            languages=[language],
            random=True,
            seed=seed,
            columns=[
                "codebase_name",
                "programming_language",
                "file_path",
                "function_name",
                "function_start_line",
                "value",
            ],
        )
        if len(rows) != SAMPLES_PER_LANGUAGE:
            raise ValueError(
                f"expected {SAMPLES_PER_LANGUAGE} {language} samples, "
                f"got {len(rows)}"
            )
        samples.append(rows)

    return pd.concat(samples, ignore_index=True)


def repository_path(codebase_name: object, language: object) -> Path:
    repo_name = str(codebase_name).split(" (", 1)[0]
    language_directory = {
        Language.GO.value: "go",
        Language.OCAML.value: "ocaml",
    }.get(str(language))
    if language_directory is None:
        raise ValueError(f"unsupported programming language: {language!r}")

    workspace_root = Path(__file__).resolve().parents[1]
    toanalyze = workspace_root / "toanalyze"
    matches = sorted(toanalyze.glob(f"*/{language_directory}/{repo_name}"))
    if len(matches) != 1:
        raise ValueError(
            f"could not find exactly one local repository for {repo_name!r} "
            f"({language_directory}); found {len(matches)}"
        )
    return matches[0]


def location(row: pd.Series, cwd: Path) -> str:
    file_path = row.get("file_path")
    start_line = row.get("function_start_line")
    if pd.isna(file_path) or pd.isna(start_line):
        raise ValueError("sample is missing file_path or function_start_line")

    absolute_path = repository_path(
        row["codebase_name"], row["programming_language"]
    ) / str(file_path).lstrip("/")
    return f"{os.path.relpath(absolute_path, cwd)}:{int(float(start_line))}"


def format_locations(rows: pd.DataFrame, cwd: Path | None = None) -> str:
    cwd = cwd or Path.cwd()
    return "\n".join(location(row, cwd) for _, row in rows.iterrows()) + "\n"


if __name__ == "__main__":
    args = parse_args()
    result = format_locations(sample_functions(data(args.data_dir), args.metric, args.seed))
    args.output.write_text(result)
    print(result, end="")
