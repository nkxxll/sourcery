import argparse
from pathlib import Path

import pandas as pd

from helpers import Language, data


METRIC = "maintainability_index_four_property"
VERSION = 10
SAMPLES_PER_GROUP = 5
MEDIAN_TIE_SEED = 0
MEDIANS = {
    Language.GO.value: 103.6,
    Language.OCAML.value: 126.7,
}
DEFAULT_OUTPUT = Path("four_property_mi_links_version_10.typ")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Print GitHub links for high, median-range, and low four-property "
            "maintainability-index functions from version 10."
        )
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
    rows = metrics.sample(
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
    rows["value"] = pd.to_numeric(rows["value"], errors="coerce")
    rows = rows.dropna(subset=["value"]).sort_values(
        ["value", "codebase_id", "file_path", "function_start_line"]
    )

    low = rows.head(SAMPLES_PER_GROUP).copy()
    high = rows.tail(SAMPLES_PER_GROUP).sort_values("value", ascending=False).copy()
    remaining = rows.drop(index=low.index.union(high.index))
    median_candidates = remaining.assign(
        distance=(remaining["value"] - MEDIANS[language]).abs()
    )
    cutoff = median_candidates["distance"].nsmallest(SAMPLES_PER_GROUP).max()
    closest = median_candidates[median_candidates["distance"] < cutoff]
    tied = median_candidates[median_candidates["distance"] == cutoff]
    median = pd.concat(
        [
            closest,
            tied.sample(
                n=SAMPLES_PER_GROUP - len(closest),
                random_state=MEDIAN_TIE_SEED,
            ),
        ]
    ).sort_values("distance").copy()

    selected = pd.concat(
        [high.assign(category="high"), median.assign(category="around median"), low.assign(category="low")]
    )
    return metrics.with_github_urls(
        selected,
        resolve_refs=True,
        strict=True,
    )


def format_results(metrics) -> str:
    sections = []
    for language in [Language.GO.value, Language.OCAML.value]:
        rows = select_functions(metrics, language)
        lines = [f"== {language}", ""]
        for category in ["high", "around median", "low"]:
            lines.append(f"=== {category.title()}")
            for row in rows[rows["category"] == category].itertuples():
                lines.append(
                    f"- `{row.value:.2f}` `{row.function_name}` "
                    f"({row.file_path}) #link(\"{row.github_url}\")[GitHub]"
                )
            lines.append("")
        sections.append("\n".join(lines).rstrip())
    return "\n\n".join(sections) + "\n"


if __name__ == "__main__":
    args = parse_args()
    result = format_results(data(args.data_dir))
    args.output.write_text(result)
    print(result, end="")
