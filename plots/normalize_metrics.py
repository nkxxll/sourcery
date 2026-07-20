import pandas as pd

from core import (
    INPUT_INDEX_COL,
    LANGUAGE_COL,
    LINES_OF_CODE_METRIC,
    METRIC_COL,
    METRIC_LEVEL_COL,
    VALUE_COL,
    warn_dropped_rows,
)

LINES_OF_CODE_METRICS = {
    "file": LINES_OF_CODE_METRIC,
    "function": "function_length",
    "version": "total_lines_of_code",
}
IDENTITY_COLUMNS = [
    INPUT_INDEX_COL,
    LANGUAGE_COL,
    "codebase_id",
    "version_id",
    "sample_number",
    "file_path",
    "function_name",
    "function_start_line",
    "function_end_line",
]


def normalize_metrics_per_kloc(
    df: pd.DataFrame,
    metric_names: list[str],
) -> pd.DataFrame:
    normalized_parts = []

    for metric_level, lines_of_code_metric in LINES_OF_CODE_METRICS.items():
        level_df = df[df[METRIC_LEVEL_COL] == metric_level].copy()
        if level_df.empty:
            continue

        key_columns = [column for column in IDENTITY_COLUMNS if column in level_df]
        loc_values = (
            level_df[level_df[METRIC_COL] == lines_of_code_metric]
            .drop_duplicates(key_columns)
            .set_index(key_columns)[VALUE_COL]
            .rename("lines_of_code_value")
        )
        metric_rows = level_df[level_df[METRIC_COL].isin(metric_names)].copy()
        if metric_rows.empty:
            continue

        if loc_values.empty:
            metric_rows["lines_of_code_value"] = pd.NA
        else:
            metric_rows = metric_rows.merge(
                loc_values.reset_index(),
                on=key_columns,
                how="left",
            )
        missing_loc_rows = metric_rows[metric_rows["lines_of_code_value"].isna()]
        warn_dropped_rows(
            f"while normalizing metrics because matching {lines_of_code_metric} "
            "rows were missing",
            missing_loc_rows,
        )
        metric_rows = metric_rows.dropna(subset=["lines_of_code_value"]).copy()

        zero_loc_rows = metric_rows[metric_rows["lines_of_code_value"] == 0]
        warn_dropped_rows(
            f"while normalizing metrics because {lines_of_code_metric} was zero",
            zero_loc_rows,
        )
        metric_rows = metric_rows[metric_rows["lines_of_code_value"] != 0].copy()
        metric_rows[VALUE_COL] = (
            metric_rows[VALUE_COL] / (metric_rows["lines_of_code_value"] / 1000)
        )
        normalized_parts.append(metric_rows.drop(columns=["lines_of_code_value"]))

    if not normalized_parts:
        return df.iloc[0:0].copy()

    return pd.concat(normalized_parts, ignore_index=True)
