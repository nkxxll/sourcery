#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

metrics_csvs=()
extremes_csvs=()
version_metrics_csv="version-metrics.csv"
required_analysis_metrics=(
  mean_outdegree_per_file
  mean_unique_outdegree_per_file
  mean_indegree_per_file
  mean_unique_indegree_per_file
  indegree
  unique_indegree
  outdegree
  unique_outdegree
)

require_metrics() {
  uv run python - "$@" <<'PY'
import csv
import sys
from pathlib import Path

metric_keys = set(sys.argv[1].split(","))
csv_paths = [Path(path) for path in sys.argv[2:]]

for csv_path in csv_paths:
    with csv_path.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        if reader.fieldnames is None or "metric_key" not in reader.fieldnames:
            print(f"{csv_path} is missing metric_key column", file=sys.stderr)
            sys.exit(1)
        present = {row["metric_key"] for row in reader}

    missing = sorted(metric_keys - present)
    if missing:
        print(
            f"{csv_path} is missing required metric keys: {', '.join(missing)}",
            file=sys.stderr,
        )
        sys.exit(1)
PY
}

if [[ ! -f "$version_metrics_csv" ]]; then
  printf 'Missing %s\n' "$version_metrics_csv" >&2
  exit 1
fi

for version in {1..10}; do
  metrics_path="metrics${version}.csv"
  extremes_path="extremes${version}.csv"

  if [[ -f "$metrics_path" ]]; then
    metrics_csvs+=("$metrics_path")
  else
    printf 'Missing %s\n' "$metrics_path" >&2
    exit 1
  fi

  if [[ -f "$extremes_path" ]]; then
    extremes_csvs+=("$extremes_path")
  else
    printf 'Missing %s\n' "$extremes_path" >&2
    exit 1
  fi
done

required_analysis_metrics_csv=$(IFS=,; printf '%s' "${required_analysis_metrics[*]}")
require_metrics "$required_analysis_metrics_csv" "${metrics_csvs[@]}" "${extremes_csvs[@]}"

uv run main.py boxplot "${metrics_csvs[@]}"
uv run main.py linechart "${metrics_csvs[@]}"

uv run main.py boxplot -o metrics_by_language.png "$version_metrics_csv"
uv run main.py linechart -o metrics_over_versions.png "$version_metrics_csv"

uv run main.py extremes "${extremes_csvs[@]}"

uv run proportionality.py \
  -o proportionality_report.md \
  --summary-csv proportionality_results.csv \
  "${metrics_csvs[@]}" \
  "$version_metrics_csv"

uv run proportionality.py \
  --over-version-aggregates \
  -o proportionality_over_versions_report.md \
  --summary-csv proportionality_over_versions_results.csv

uv run proportionality.py \
  --over-version-aggregates \
  --statistics Mean \
  -o proportionality_over_versions_mean_report.md \
  --summary-csv proportionality_over_versions_mean_results.csv
