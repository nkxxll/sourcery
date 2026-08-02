"""Print the highest-indegree Go and OCaml functions as GitHub links."""

from __future__ import annotations

import argparse
import random
from pathlib import Path

import pandas as pd

from helpers import Language, SOURCERY_DB, data


DEFAULT_COUNT = 10
DEFAULT_VERSION = 10
METRIC = "indegree"
LANGUAGES = [Language.GO.value, Language.OCAML.value]
DEFAULT_OUTPUT = Path(__file__).resolve().parent / "top_indegree_functions.typ"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Print the highest-indegree Go and OCaml functions."
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        default=Path(__file__).resolve().parent,
        help="Directory containing metrics CSV files.",
    )
    parser.add_argument(
        "--version",
        type=int,
        default=DEFAULT_VERSION,
        help=f"Version number to inspect (default: {DEFAULT_VERSION}).",
    )
    parser.add_argument(
        "--count",
        type=int,
        default=DEFAULT_COUNT,
        help=f"Functions to print per language (default: {DEFAULT_COUNT}).",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=None,
        help="Seed used to choose randomly among equal-indegree functions.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help=f"Typst output file (default: {DEFAULT_OUTPUT}).",
    )
    parser.add_argument(
        "--sourcery-db",
        type=Path,
        default=SOURCERY_DB,
        help=f"sourcery-db executable (default: {SOURCERY_DB}).",
    )
    return parser.parse_args()


def select_top_functions(
    metrics, language: str, count: int, version: int, rng: random.Random
) -> pd.DataFrame:
    """Select top rows without sorting; ties are resolved randomly."""
    if count < 1:
        raise ValueError("count must be positive")

    rows = metrics.sample(
        count=len(metrics.raw_metrics),
        metric=METRIC,
        level="function",
        language=language,
        version=version,
        columns=[
            "value",
            "codebase_id",
            "codebase_name",
            "programming_language",
            "version_id",
            "version_number",
            "sample_number",
            "file_path",
            "function_name",
            "function_start_line",
            "function_end_line",
        ],
    )
    rows["value"] = pd.to_numeric(rows["value"], errors="coerce")
    rows = rows.dropna(subset=["value"]).reset_index(drop=True)

    selected: list[pd.Series] = []
    remaining = rows.copy()
    for _ in range(min(count, len(remaining))):
        highest = remaining["value"].max()
        tied = remaining.index[remaining["value"] == highest].tolist()
        chosen_index = rng.choice(tied)
        selected.append(remaining.loc[chosen_index])
        remaining = remaining.drop(index=chosen_index)

    if not selected:
        return rows.iloc[0:0].copy()
    return pd.DataFrame(selected).reset_index(drop=True)


def format_results(metrics, count: int, version: int, seed: int | None, sourcery_db: Path) -> str:
    rng = random.Random(seed)
    sections = [f"== Top indegree functions (version {version})", ""]
    for language in LANGUAGES:
        rows = select_top_functions(metrics, language, count, version, rng)
        if rows.empty:
            raise ValueError(f"no {METRIC} functions found for {language}")
        rows = metrics.with_github_urls(
            rows, resolve_refs=True, strict=True, sourcery_db=sourcery_db
        )
        sections.extend([f"=== {language}", ""])
        for row in rows.itertuples():
            sections.append(
                f'- `{row.value:.2f}` `{row.function_name}` '
                f'({row.file_path}) #link("{row.github_url}")[GitHub]'
            )
        sections.append("")
    return "\n".join(sections).rstrip() + "\n"


def main() -> None:
    args = parse_args()
    result = format_results(
        data(args.data_dir), args.count, args.version, args.seed, args.sourcery_db
    )
    args.output.write_text(result)
    print(result, end="")


if __name__ == "__main__":
    main()
