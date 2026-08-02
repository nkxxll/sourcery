import argparse
from pathlib import Path

import pandas as pd

from helpers import Language, data


METRIC = "cyclomatic"
SAMPLES_PER_LANGUAGE = 10
CC_FIVE_SAMPLES_PER_LANGUAGE = 5
VERSION = 10
DEFAULT_OUTPUT = Path("cyclomatic_samples.typ")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Print project-spread median cyclomatic function samples as Typst."
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
    parser.add_argument(
        "--seed",
        type=int,
        default=0,
        help="Random seed (default: 0).",
    )
    return parser.parse_args()


def select_functions(metrics, language: str, seed: int) -> pd.DataFrame:
    rows = metrics.sample_median_by_language(
        count=SAMPLES_PER_LANGUAGE,
        metric=METRIC,
        level="function",
        input_index=VERSION,
        languages=[language],
        random=True,
        seed=seed,
        distribute_by_project=True,
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
    if len(rows) != SAMPLES_PER_LANGUAGE:
        raise ValueError(
            f"expected {SAMPLES_PER_LANGUAGE} {language} samples, got {len(rows)}"
        )
    return metrics.with_github_urls(rows, resolve_refs=True, strict=True)


def select_cc_five_functions(metrics, language: str, seed: int) -> pd.DataFrame:
    rows = metrics.sample(
        count=CC_FIVE_SAMPLES_PER_LANGUAGE,
        metric=METRIC,
        level="function",
        value=5,
        language=language,
        version=VERSION,
        random=True,
        seed=seed,
        distribute_by_project=True,
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
    if len(rows) != CC_FIVE_SAMPLES_PER_LANGUAGE:
        raise ValueError(
            f"expected {CC_FIVE_SAMPLES_PER_LANGUAGE} {language} CC 5 samples, "
            f"got {len(rows)}"
        )
    return metrics.with_github_urls(rows, resolve_refs=True, strict=True)


def format_results(metrics, seed: int = 0) -> str:
    sections = ["== Median cyclomatic-complexity sample functions", ""]
    for language in [Language.GO.value, Language.OCAML.value]:
        median_rows = select_functions(metrics, language, seed)
        cc_five_rows = select_cc_five_functions(metrics, language, seed)
        sections.append(
            f"=== {language} (median: {median_rows.iloc[0]['median']:.2f})"
        )
        sections.append("")
        for title, rows in [("Median", median_rows), ("CC 5", cc_five_rows)]:
            sections.append(f"==== {title}")
            for row in rows.itertuples():
                sections.append(
                    f"- `{row.value:.2f}` `{row.function_name}` "
                    f"({row.codebase_name}: {row.file_path}) "
                    f'#link("{row.github_url}")[GitHub]'
                )
            sections.append("")
        sections.append("")
    return "\n".join(sections).rstrip() + "\n"


if __name__ == "__main__":
    args = parse_args()
    result = format_results(data(args.data_dir), args.seed)
    args.output.write_text(result)
    print(result, end="")
