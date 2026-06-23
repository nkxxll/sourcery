#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

metrics_csvs=()
extremes_csvs=()
version_metrics_csv="version-metrics.csv"

if [[ ! -f "$version_metrics_csv" ]]; then
  echo "Missing $version_metrics_csv" >&2
  exit 1
fi

for version in {1..10}; do
  metrics_path="metrics${version}.csv"
  extremes_path="extremes${version}.csv"

  if [[ -f "$metrics_path" ]]; then
    metrics_csvs+=("$metrics_path")
  else
    echo "Missing $metrics_path" >&2
    exit 1
  fi

  if [[ -f "$extremes_path" ]]; then
    extremes_csvs+=("$extremes_path")
  else
    echo "Missing $extremes_path" >&2
    exit 1
  fi
done

uv run main.py boxplot "${metrics_csvs[@]}"
uv run main.py linechart "${metrics_csvs[@]}"

uv run main.py boxplot -o metrics_by_language.png "$version_metrics_csv"
uv run main.py linechart -o metrics_over_versions.png "$version_metrics_csv"

uv run main.py extremes "${extremes_csvs[@]}"
