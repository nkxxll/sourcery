"""Add exact versioned GitHub links to Vim-style function locations."""

from __future__ import annotations

import argparse
import csv
import re
import sys
from pathlib import Path
from urllib.parse import quote

from helpers import SOURCERY_DB, version_commit_hash


VERSION = 10
METRIC = "outdegree"
DEFAULT_INPUT = Path(__file__).resolve().parent / "function_samples_gf.txt"
DEFAULT_MAPPING = Path(__file__).resolve().parent / "metrics10.csv"
DEFAULT_OUTPUT = Path(__file__).resolve().parent / "function_samples_links.typ"
LOCATION_RE = re.compile(r"^(?P<location>.+):(?P<line>[1-9][0-9]*)$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Add versioned GitHub links to Vim-style function locations."
    )
    parser.add_argument(
        "--input",
        type=Path,
        default=DEFAULT_INPUT,
        help=f"Input locations file (default: {DEFAULT_INPUT}).",
    )
    parser.add_argument(
        "--mapping",
        type=Path,
        default=DEFAULT_MAPPING,
        help=f"Metrics CSV containing codebase IDs (default: {DEFAULT_MAPPING}).",
    )
    parser.add_argument(
        "--sourcery-db",
        type=Path,
        default=SOURCERY_DB,
        help=f"sourcery-db executable (default: {SOURCERY_DB}).",
    )
    parser.add_argument(
        "--version",
        type=int,
        default=VERSION,
        help=f"Version/sample number to link (default: {VERSION}).",
    )
    parser.add_argument(
        "--metric",
        default=METRIC,
        help=f"Metric value to include before each function (default: {METRIC}).",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help=f"Typst output file (default: {DEFAULT_OUTPUT}).",
    )
    return parser.parse_args()


def mapping_data(
    mapping_path: Path, metric: str
) -> tuple[dict[str, str], dict[tuple[str, str, int], tuple[str, float]]]:
    """Read repository IDs and function names without loading metric values."""
    codebase_ids: dict[str, str] = {}
    function_data: dict[tuple[str, str, int], tuple[str, float]] = {}
    with mapping_path.open(newline="") as mapping_file:
        for row in csv.DictReader(mapping_file):
            name = row["codebase_name"].split(" (", 1)[0]
            codebase_id = codebase_ids.setdefault(name, row["codebase_id"])
            if codebase_id != row["codebase_id"]:
                raise ValueError(
                    f"codebase name {name!r} maps to multiple IDs: "
                    f"{codebase_id!r} and {row['codebase_id']!r}"
                )

            if row.get("metric_key", "").strip() != metric:
                continue
            function_name = row.get("function_name", "").strip()
            start_line = row.get("function_start_line", "").strip()
            file_path = row.get("file_path", "").strip()
            value = row.get("value", "").strip()
            if function_name and file_path and start_line and value:
                key = (name, file_path, int(float(start_line)))
                # Some OCaml declarations share a source line; retain the first
                # CSV ordering, which is also the ordering used by the metrics.
                function_data.setdefault(key, (function_name, float(value)))
    return codebase_ids, function_data


def parse_location(value: str) -> tuple[str, str, int]:
    match = LOCATION_RE.fullmatch(value)
    if match is None:
        raise ValueError(f"invalid Vim location: {value!r}")

    location = match["location"]
    parts = Path(location).parts
    language_index = next(
        (index for index, part in enumerate(parts) if part in {"go", "ocaml"}),
        None,
    )
    if language_index is None or language_index + 2 >= len(parts):
        raise ValueError(
            f"cannot determine repository and file path from location: {value!r}"
        )

    repository = parts[language_index + 1]
    file_path = "/".join(parts[language_index + 2 :])
    return repository, file_path, int(match["line"])


def github_url(repository: str, file_path: str, commit: str, line: int) -> str:
    if "__" not in repository:
        raise ValueError(f"repository is not in owner__name form: {repository!r}")
    owner, name = repository.split("__", 1)
    return (
        f"https://github.com/{quote(owner, safe='')}/{quote(name, safe='')}"
        f"/blob/{quote(commit, safe='')}/{quote(file_path, safe='/')}#L{line}"
    )


def format_locations(
    input_path: Path,
    mapping_path: Path,
    sourcery_db: Path,
    version: int,
    metric: str,
) -> str:
    ids, function_data = mapping_data(mapping_path, metric)
    output: list[str] = []
    for input_line_number, raw_line in enumerate(
        input_path.read_text().splitlines(), start=1
    ):
        if not raw_line.strip():
            output.append("")
            continue
        try:
            repository, file_path, line = parse_location(raw_line.strip())
            codebase_id = ids[repository]
            function_name, value = function_data[(repository, file_path, line)]
            commit = version_commit_hash(codebase_id, version, sourcery_db)
            if commit is None:
                raise ValueError(
                    f"no version {version} commit found for {repository!r}"
                )
            url = github_url(repository, file_path, commit, line)
        except (KeyError, ValueError) as error:
            raise ValueError(f"{input_path}:{input_line_number}: {error}") from error
        output.append(
            f'- `{value:.2f}` `{function_name}` '
            f'({raw_line.strip()}) #link("{url}")[GitHub]'
        )
    return "\n".join(output) + "\n"


def main() -> None:
    args = parse_args()
    try:
        result = format_locations(
            args.input, args.mapping, args.sourcery_db, args.version, args.metric
        )
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
    args.output.write_text(result)
    print(result, end="")


if __name__ == "__main__":
    main()
