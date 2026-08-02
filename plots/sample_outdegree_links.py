import random
import argparse
from pathlib import Path

import pandas as pd

from helpers import Language, data


METRIC = "outdegree"
MAX_SAMPLES = 5
MEDIAN_SAMPLES = 10
VERSION = 10
DEFAULT_OUTPUT = Path("outdegree_samples.typ")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Print high and median outdegree function samples as Typst."
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        default=Path("."),
        help="Directory containing metrics CSV files (default: .)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help=f"Typst output file (default: {DEFAULT_OUTPUT}).",
    )
    return parser.parse_args()


def select_functions(metrics, language: str) -> pd.DataFrame:
    max_rows = metrics.sample(
        count=len(metrics.raw_metrics),
        metric=METRIC,
        level="function",
        language=language,
        version=VERSION,
        columns=[
            "value",
            "codebase_id",
            "codebase_name",
            "sample_number",
            "version_number",
            "file_path",
            "function_name",
            "function_start_line",
            "function_end_line",
        ],
    )
    max_rows["value"] = pd.to_numeric(max_rows["value"], errors="coerce")
    max_rows = max_rows.dropna(subset=["value"]).sort_values(
        ["value"],
        ascending=[False],
    )
    max_rows = max_rows.head(MAX_SAMPLES).assign(category="maximum")

    median_rows = metrics.sample_median_by_language(
        count=MEDIAN_SAMPLES,
        metric=METRIC,
        level="function",
        input_index=VERSION,
        languages=[language],
        columns=[
            "value",
            "codebase_id",
            "codebase_name",
            "sample_number",
            "version_number",
            "file_path",
            "function_name",
            "function_start_line",
            "function_end_line",
        ],
        random=True,
        distribute_by_project=True,
    ).assign(category="median")

    selected = pd.concat([max_rows, median_rows], ignore_index=True)
    return metrics.with_github_urls(selected, resolve_refs=True, strict=True)


def format_results(metrics) -> str:
    sections = ["== outdegree sample functions", ""]
    for language in [Language.GO.value, Language.OCAML.value]:
        rows = select_functions(metrics, language)
        sections.append(f"=== {language}")
        sections.append("")
        for category in ["maximum", "median"]:
            sections.append(f"==== {category.title()}")
            for row in rows[rows["category"] == category].itertuples():
                sections.append(
                    f"- `{row.value:.2f}` `{row.function_name}` "
                    f'({row.file_path}) #link("{row.github_url}")[GitHub]'
                )
            sections.append("")
    return "\n".join(sections).rstrip() + "\n"


if __name__ == "__main__":
    args = parse_args()
    result = format_results(data(args.data_dir))
    args.output.write_text(result)
    print(result, end="")
