from pathlib import Path
import warnings

import pandas as pd

LANGUAGE_COL = "programming_language"
METRIC_LEVEL_COL = "metric_level"
METRIC_COL = "metric_key"
VALUE_COL = "value"
INPUT_INDEX_COL = "input_index"
VERSION_NUMBER_COL = "version_number"
LINES_OF_CODE_METRIC = "lines_of_code"
CYCLOMATIC_METRIC = "total_cyclomatic"
ADJUSTED_CYCLOMATIC_METRIC = "cyclomatic_per_line"


def chart_data_output_path(output_path: Path) -> Path:
    return output_path.with_suffix(".csv")


def warn(message: str) -> None:
    warnings.warn(message, RuntimeWarning, stacklevel=2)


def read_csv_with_columns(csv_path: Path, required_columns: set[str]) -> pd.DataFrame:
    try:
        df = pd.read_csv(csv_path)
    except Exception as exc:
        raise ValueError(f"Failed to read CSV {csv_path}: {exc}") from exc

    missing_columns = sorted(required_columns - set(df.columns))
    if missing_columns:
        raise ValueError(
            f"CSV {csv_path} is missing required columns: "
            + ", ".join(missing_columns)
        )

    return df


def warn_dropped_rows(reason: str, df: pd.DataFrame) -> None:
    if df.empty:
        return

    counts = df.groupby(INPUT_INDEX_COL).size().sort_index()
    details = counts.rename(lambda input_index: f"version {input_index}").to_string()
    warn(f"Dropped {len(df)} row(s) {reason} ({details})")


def load_metrics(csv_paths: list[Path], metric_names: list[str]) -> pd.DataFrame:
    df = read_versioned_csvs(
        csv_paths,
        {LANGUAGE_COL, METRIC_LEVEL_COL, METRIC_COL, VALUE_COL},
        use_version_number=True,
    )

    source_metric_names = set(metric_names)
    if ADJUSTED_CYCLOMATIC_METRIC in source_metric_names:
        source_metric_names.update([LINES_OF_CODE_METRIC, CYCLOMATIC_METRIC])

    df = df[df[METRIC_COL].isin(source_metric_names)].copy()

    missing_required = df[
        df[[LANGUAGE_COL, METRIC_LEVEL_COL, METRIC_COL, VALUE_COL]].isna().any(axis=1)
    ]
    warn_dropped_rows(
        f"with missing {LANGUAGE_COL}, {METRIC_LEVEL_COL}, {METRIC_COL}, or {VALUE_COL}",
        missing_required,
    )
    df = df.dropna(subset=[LANGUAGE_COL, METRIC_LEVEL_COL, METRIC_COL, VALUE_COL]).copy()

    numeric_values = pd.to_numeric(df[VALUE_COL], errors="coerce")
    invalid_values = df[numeric_values.isna()]
    warn_dropped_rows(f"with non-numeric {VALUE_COL}", invalid_values)
    df[VALUE_COL] = numeric_values
    df = df.dropna(subset=[VALUE_COL]).copy()

    if ADJUSTED_CYCLOMATIC_METRIC in metric_names:
        df = add_adjusted_cyclomatic_metric(df)

    df = df[df[METRIC_COL].isin(metric_names)].copy()

    require_metrics(df, metric_names, sorted(df[INPUT_INDEX_COL].dropna().unique()))

    return df


def load_extremes(csv_paths: list[Path], metric_names: list[str]) -> pd.DataFrame:
    df = read_versioned_csvs(csv_paths, {"extreme", "rank", METRIC_COL, VALUE_COL})
    df = df[df[METRIC_COL].isin(metric_names)].copy()

    missing_required = df[
        df[["extreme", "rank", METRIC_COL, VALUE_COL]].isna().any(axis=1)
    ]
    warn_dropped_rows(
        f"with missing extreme, rank, {METRIC_COL}, or {VALUE_COL}",
        missing_required,
    )
    df = df.dropna(subset=["extreme", "rank", METRIC_COL, VALUE_COL]).copy()

    numeric_ranks = pd.to_numeric(df["rank"], errors="coerce")
    invalid_ranks = df[numeric_ranks.isna()]
    warn_dropped_rows("with non-numeric rank", invalid_ranks)
    df["rank"] = numeric_ranks

    numeric_values = pd.to_numeric(df[VALUE_COL], errors="coerce")
    invalid_values = df[numeric_values.isna()]
    warn_dropped_rows(f"with non-numeric {VALUE_COL}", invalid_values)
    df[VALUE_COL] = numeric_values

    df = df.dropna(subset=["rank", VALUE_COL]).copy()
    df["rank"] = df["rank"].astype(int)

    require_metrics(df, metric_names, sorted(df[INPUT_INDEX_COL].dropna().unique()))

    return df


def read_versioned_csvs(
    csv_paths: list[Path],
    required_columns: set[str],
    use_version_number: bool = False,
) -> pd.DataFrame:
    df = (
        pd.concat(
            [read_csv_with_columns(csv_path, required_columns) for csv_path in csv_paths],
            keys=range(1, len(csv_paths) + 1),
            names=[INPUT_INDEX_COL],
        )
        .reset_index(level=INPUT_INDEX_COL)
        .reset_index(drop=True)
    )

    if use_version_number and VERSION_NUMBER_COL in df.columns:
        version_numbers = pd.to_numeric(df[VERSION_NUMBER_COL], errors="coerce")
        invalid_versions = df[version_numbers.isna()]
        warn_dropped_rows(f"with non-numeric {VERSION_NUMBER_COL}", invalid_versions)
        df[INPUT_INDEX_COL] = version_numbers
        df = df.dropna(subset=[INPUT_INDEX_COL]).copy()
        df[INPUT_INDEX_COL] = df[INPUT_INDEX_COL].astype(int)

    return df


def add_adjusted_cyclomatic_metric(df: pd.DataFrame) -> pd.DataFrame:
    key_cols = [
        col
        for col in [
            INPUT_INDEX_COL,
            LANGUAGE_COL,
            "codebase_id",
            "version_id",
            "sample_number",
            "file_path",
        ]
        if col in df.columns
    ]

    cyclomatic_rows = df[df[METRIC_COL] == CYCLOMATIC_METRIC].copy()
    loc_values = df[df[METRIC_COL] == LINES_OF_CODE_METRIC][
        key_cols + [VALUE_COL]
    ].rename(columns={VALUE_COL: "lines_of_code_value"})

    missing_loc_rows = (
        cyclomatic_rows.merge(
            loc_values[key_cols].drop_duplicates(),
            on=key_cols,
            how="left",
            indicator=True,
        )
        .query("_merge == 'left_only'")
        .drop(columns="_merge")
    )
    warn_dropped_rows(
        f"while computing {ADJUSTED_CYCLOMATIC_METRIC} because matching "
        f"{LINES_OF_CODE_METRIC} rows were missing",
        missing_loc_rows,
    )

    adjusted_rows = cyclomatic_rows.merge(loc_values, on=key_cols, how="inner")
    zero_loc_rows = adjusted_rows[adjusted_rows["lines_of_code_value"] == 0]
    warn_dropped_rows(
        f"while computing {ADJUSTED_CYCLOMATIC_METRIC} because "
        f"{LINES_OF_CODE_METRIC} was zero",
        zero_loc_rows,
    )
    adjusted_rows = adjusted_rows[adjusted_rows["lines_of_code_value"] != 0].copy()
    adjusted_rows[VALUE_COL] = (
        adjusted_rows[VALUE_COL] / adjusted_rows["lines_of_code_value"]
    )
    adjusted_rows[METRIC_COL] = ADJUSTED_CYCLOMATIC_METRIC

    if "metric_label" in adjusted_rows.columns:
        adjusted_rows["metric_label"] = "Cyclomatic Complexity/Line"

    adjusted_rows = adjusted_rows.drop(columns=["lines_of_code_value"])

    return pd.concat([df, adjusted_rows], ignore_index=True)


def require_metrics(
    df: pd.DataFrame,
    metric_names: list[str],
    input_indices,
) -> None:
    required = pd.MultiIndex.from_product(
        [input_indices, metric_names],
        names=[INPUT_INDEX_COL, METRIC_COL],
    )
    present = pd.MultiIndex.from_frame(
        df[[INPUT_INDEX_COL, METRIC_COL]].drop_duplicates()
    )
    missing = required.difference(present)

    if not missing.empty:
        missing_by_input = (
            missing.to_frame(index=False)
            .groupby(INPUT_INDEX_COL)[METRIC_COL]
            .agg(", ".join)
            .rename(lambda input_index: f"version {input_index}")
        )
        raise ValueError(
            "Missing required metric data after loading/filtering. "
            "Every input file must contain every requested metric. Missing: "
            + missing_by_input.to_string()
        )


def print_medians(df: pd.DataFrame, metric_names: list[str]) -> None:
    medians = (
        df[df[METRIC_COL].isin(metric_names)]
        .groupby([INPUT_INDEX_COL, METRIC_LEVEL_COL, METRIC_COL, LANGUAGE_COL])[VALUE_COL]
        .median()
        .sort_index()
    )
    print("\nMedian values by version, metric level, metric, and language:")
    print(medians)
