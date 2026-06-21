from pathlib import Path

import pandas as pd

from core import warn
from core import INPUT_INDEX_COL, METRIC_COL, VALUE_COL

DEFAULT_OUTPUT_PATH = Path("extremes_report.md")


def write_extremes_report(
    df: pd.DataFrame,
    metric_names: list[str],
    output_path: Path = DEFAULT_OUTPUT_PATH,
) -> None:
    output_path.write_text(render_extremes_report(df, metric_names), encoding="utf-8")


def render_extremes_report(df: pd.DataFrame, metric_names: list[str]) -> str:
    lines = ["# Extremes Report", ""]
    warn_missing_optional_report_columns(df)

    for metric_name in metric_names:
        metric_df = df[df[METRIC_COL] == metric_name]
        if metric_df.empty:
            warn(f"Extremes report has no rows for metric {metric_name}; skipping section")
            continue

        metric_label = first_present(metric_df, "metric_label", metric_name)
        lines.extend([f"## {escape_markdown(metric_label)}", ""])

        for input_index in sorted(metric_df[INPUT_INDEX_COL].unique()):
            version_df = metric_df[metric_df[INPUT_INDEX_COL] == input_index]
            version_number = first_present(version_df, "version_number", input_index)
            lines.extend([f"### Version {escape_markdown(version_number)}", ""])

            for extreme in ["max", "min"]:
                extreme_df = version_df[version_df["extreme"] == extreme].sort_values("rank")
                if extreme_df.empty:
                    warn(
                        f"Extremes report has no {extreme} rows for "
                        f"metric {metric_name}, version {input_index}; skipping subsection"
                    )
                    continue

                lines.extend([f"#### {extreme.title()}", ""])
                lines.append(
                    "| rank | value | language | codebase | level | location |"
                )
                lines.append("| ---: | ---: | --- | --- | --- | --- |")

                for _, row in extreme_df.iterrows():
                    lines.append(
                        "| "
                        + " | ".join(
                            [
                                escape_markdown(row["rank"]),
                                escape_markdown(format_value(row[VALUE_COL])),
                                escape_markdown(row.get("programming_language", "")),
                                escape_markdown(clean_codebase_name(row.get("codebase_name", ""))),
                                escape_markdown(row.get("metric_level", "")),
                                escape_markdown(format_location(row)),
                            ]
                        )
                        + " |"
                    )

                lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def warn_missing_optional_report_columns(df: pd.DataFrame) -> None:
    optional_columns = [
        "metric_label",
        "version_number",
        "programming_language",
        "codebase_name",
        "metric_level",
        "file_path",
        "function_name",
        "function_start_line",
    ]
    missing_columns = [column for column in optional_columns if column not in df.columns]
    if missing_columns:
        warn(
            "Extremes report missing optional column(s); blank or fallback values "
            "will be used: "
            + ", ".join(missing_columns)
        )


def first_present(df: pd.DataFrame, column: str, fallback: object) -> object:
    if column not in df.columns:
        return fallback

    values = df[column].dropna()
    if values.empty:
        return fallback

    return values.iloc[0]


def clean_codebase_name(value: object) -> str:
    if pd.isna(value):
        return ""

    return str(value).replace(" (sample analysis: 10 samples)", "")


def format_location(row: pd.Series) -> str:
    file_path = row.get("file_path", "")
    function_name = row.get("function_name", "")
    start_line = row.get("function_start_line", "")

    if pd.isna(file_path):
        file_path = ""
    if pd.isna(function_name) or function_name == "":
        return str(file_path)

    if pd.isna(start_line) or start_line == "":
        return f"{file_path}::{function_name}"

    return f"{file_path}:{format_value(start_line)}::{function_name}"


def format_value(value: object) -> str:
    if pd.isna(value):
        return ""

    if isinstance(value, float) and value.is_integer():
        return str(int(value))

    return str(value)


def escape_markdown(value: object) -> str:
    return format_value(value).replace("|", "\\|").replace("\n", " ")
