from pathlib import Path

import matplotlib.dates as mdates
import matplotlib.pyplot as plt
import pandas as pd

from core import LANGUAGE_COL, chart_data_output_path, read_csv_with_columns, warn

DEFAULT_OUTPUT_PATH = Path("project_time_spans.png")

CODEBASE_NAME_COL = "codebase_name"
FIRST_COMMIT_TIME_COL = "first_commit_time"
LAST_COMMIT_TIME_COL = "last_commit_time"


def load_commit_times(csv_paths: list[Path]) -> pd.DataFrame:
    df = pd.concat(
        [
            read_csv_with_columns(
                csv_path,
                {
                    CODEBASE_NAME_COL,
                    LANGUAGE_COL,
                    FIRST_COMMIT_TIME_COL,
                    LAST_COMMIT_TIME_COL,
                },
            )
            for csv_path in csv_paths
        ],
        ignore_index=True,
    )

    df[FIRST_COMMIT_TIME_COL] = pd.to_datetime(
        df[FIRST_COMMIT_TIME_COL], errors="coerce", utc=True
    )
    df[LAST_COMMIT_TIME_COL] = pd.to_datetime(
        df[LAST_COMMIT_TIME_COL], errors="coerce", utc=True
    )
    invalid_rows = df[df[[FIRST_COMMIT_TIME_COL, LAST_COMMIT_TIME_COL]].isna().any(axis=1)]
    if not invalid_rows.empty:
        warn(f"Dropped {len(invalid_rows)} row(s) with invalid commit times")

    df = df.dropna(subset=[FIRST_COMMIT_TIME_COL, LAST_COMMIT_TIME_COL]).copy()
    df["language"] = df[LANGUAGE_COL].map(display_language)
    df["duration_days"] = (
        df[LAST_COMMIT_TIME_COL] - df[FIRST_COMMIT_TIME_COL]
    ).dt.total_seconds() / 86400
    return df.sort_values(["language", FIRST_COMMIT_TIME_COL, CODEBASE_NAME_COL])


def plot_commit_time_spans(
    df: pd.DataFrame,
    output_path: Path = DEFAULT_OUTPUT_PATH,
) -> None:
    if df.empty:
        raise ValueError("No commit time rows to plot")

    colors = {"Go": "#0f3f88", "OCaml": "#9a5b00"}
    labels = df[CODEBASE_NAME_COL].astype(str).tolist()
    y_positions = range(len(df))

    fig_height = max(4, 0.38 * len(df) + 1.8)
    fig, ax = plt.subplots(figsize=(12, fig_height))

    for y_pos, (_, row) in zip(y_positions, df.iterrows(), strict=False):
        first_commit = row[FIRST_COMMIT_TIME_COL]
        last_commit = row[LAST_COMMIT_TIME_COL]
        language = row["language"]
        color = colors.get(language, "#6b6e73")

        ax.plot(
            [first_commit, last_commit],
            [y_pos, y_pos],
            color=color,
            linewidth=7,
            solid_capstyle="round",
            alpha=0.75,
        )
        ax.scatter(
            [first_commit, last_commit],
            [y_pos, y_pos],
            color=color,
            s=24,
            zorder=3,
        )

    ax.set_yticks(list(y_positions))
    ax.set_yticklabels(labels)
    ax.invert_yaxis()
    ax.set_xlabel("Commit time")
    ax.set_title("Project Time Span by Language")
    ax.grid(True, axis="x", alpha=0.25)
    ax.xaxis.set_major_locator(mdates.AutoDateLocator())
    ax.xaxis.set_major_formatter(mdates.ConciseDateFormatter(ax.xaxis.get_major_locator()))

    for language, color in colors.items():
        if language in set(df["language"]):
            ax.plot([], [], color=color, linewidth=7, label=language)
    ax.legend(loc="lower left")

    fig.tight_layout()
    fig.savefig(output_path, dpi=200, bbox_inches="tight")
    df[
        [
            CODEBASE_NAME_COL,
            "language",
            FIRST_COMMIT_TIME_COL,
            LAST_COMMIT_TIME_COL,
            "duration_days",
        ]
    ].to_csv(chart_data_output_path(output_path), index=False)


def display_language(language: str) -> str:
    normalized = str(language).lower()
    if normalized in {"go", "golang"}:
        return "Go"
    if normalized == "ocaml":
        return "OCaml"
    return str(language)
