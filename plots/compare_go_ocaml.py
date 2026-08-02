from pathlib import Path

import pandas as pd

ROOT = Path(__file__).parent
LEVELS = ("file", "function", "version")
LANGUAGES = ("Golang", "Ocaml")


def load_values() -> pd.DataFrame:
    frames = []
    for level in LEVELS:
        metrics = pd.read_csv(ROOT / f"metrics_over_versions_{level}.csv")
        callgraph = pd.read_csv(
            ROOT / "callgraph_results" / f"callgraph_linechart_{level}.csv"
        )

        # Each CSV also contains duplicate rows for its combined comparison panel.
        metrics = metrics[metrics["panel"] == metrics["programming_language"]]
        callgraph = callgraph[
            callgraph["panel"] == callgraph["programming_language"]
        ]

        # Dedicated callgraph output is authoritative for overlapping metrics.
        metrics = metrics[~metrics["metric_key"].isin(callgraph["metric_key"])]
        frames.extend((metrics, callgraph))

    return pd.concat(frames, ignore_index=True)


def version_ranges(versions: pd.Series) -> str:
    numbers = sorted(versions.astype(str).str.removeprefix("v").astype(int))
    if not numbers:
        return "none"

    ranges: list[str] = []
    start = previous = numbers[0]
    for number in numbers[1:] + [None]:
        if number is not None and number == previous + 1:
            previous = number
            continue
        ranges.append(f"v{start}" if start == previous else f"v{start}-v{previous}")
        if number is not None:
            start = previous = number
    return ", ".join(ranges)


def typst_cell(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace("#", "\\#")
    escaped = escaped.replace("[", "\\[").replace("]", "\\]")
    escaped = escaped.replace("*", "\\*").replace("_", "\\_")
    return f"[{escaped}]"


def build_table(values: pd.DataFrame) -> str:
    keys = ["metric_level", "metric_key", "statistic", "input_index", "x_label"]
    language_keys = keys + ["programming_language"]
    if values.duplicated(language_keys).any():
        raise ValueError("duplicate language values found")

    comparison = values.pivot(
        index=keys, columns="programming_language", values="value"
    ).reset_index()
    if comparison[list(LANGUAGES)].isna().any().any():
        raise ValueError("a Go or OCaml value is missing")

    comparison["lower"] = "Equal"
    comparison.loc[comparison["Ocaml"] < comparison["Golang"], "lower"] = "Ocaml"
    comparison.loc[comparison["Golang"] < comparison["Ocaml"], "lower"] = "Golang"

    summary_rows = []
    summary_groups = [("Overall", comparison)] + [
        (level.title(), comparison[comparison["metric_level"] == level])
        for level in LEVELS
    ]
    for label, group in summary_groups:
        total = len(group)
        counts = group["lower"].value_counts()
        cells = [label]
        for result in ("Ocaml", "Golang", "Equal"):
            count = counts.get(result, 0)
            cells.append(f"{count:,} / {total:,} ({count / total:.1%})")
        summary_rows.append("  " + ", ".join(map(typst_cell, cells)) + ",")

    non_ties = comparison[comparison["lower"] != "Equal"]
    ocaml_non_ties = (non_ties["lower"] == "Ocaml").sum()

    rows = []
    group_keys = ["metric_level", "metric_key", "statistic"]
    for (level, metric, statistic), group in comparison.groupby(group_keys):
        versions = {
            result: version_ranges(group.loc[group["lower"] == result, "x_label"])
            for result in ("Ocaml", "Golang", "Equal")
        }
        label = metric.replace("_", " ")
        cells = (
            level.title(),
            label,
            statistic,
            versions["Ocaml"],
            versions["Golang"],
            versions["Equal"],
        )
        rows.append("  " + ", ".join(map(typst_cell, cells)) + ",")

    return "\n".join(
        [
            "#set page(width: 297mm, height: 210mm, margin: 10mm)",
            "#set text(size: 7pt)",
            "",
            "= Go vs OCaml metric values by version",
            "",
            (
                "A listed version means that language has the strictly lower value. "
                "Callgraph values come from "
                "`callgraph_results/callgraph_linechart_*.csv`; all other values "
                "come from `metrics_over_versions_*.csv`."
            ),
            "",
            "== Summary",
            "",
            "#table(",
            "  columns: (18mm, 1fr, 1fr, 1fr),",
            "  inset: 4pt,",
            "  align: (left, center, center, center),",
            "  table.header(",
            "    [*Level*], [*OCaml lower*], [*Go lower*], [*Equal*],",
            "  ),",
            *summary_rows,
            ")",
            "",
            (
                f"Excluding ties, OCaml is lower in {ocaml_non_ties:,} / "
                f"{len(non_ties):,} comparisons ({ocaml_non_ties / len(non_ties):.1%})."
            ),
            "",
            "== Detailed comparisons",
            "",
            "#table(",
            "  columns: (13mm, 1fr, 14mm, 34mm, 34mm, 28mm),",
            "  inset: 3pt,",
            "  align: (left, left, center, left, left, left),",
            "  table.header(",
            "    [*Level*], [*Metric*], [*Statistic*], [*OCaml lower*],",
            "    [*Go lower*], [*Equal*],",
            "  ),",
            *rows,
            ")",
            "",
        ]
    )


if __name__ == "__main__":
    output = ROOT / "go_vs_ocaml_metrics.typ"
    output.write_text(build_table(load_values()))
    print(f"Wrote {output}")
