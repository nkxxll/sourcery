#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s\n' "$0" >&2
  printf 'Analyzes toanalyze/listed/go and toanalyze/listed/ocaml, exports DB CSVs into plots/, then regenerates plots.\n' >&2
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
plots_dir="$repo_root/plots"
listed_dir="$repo_root/toanalyze/listed"
samples=10

if [[ $# -gt 0 ]]; then
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'error: unknown argument: %s\n' "$1" >&2
      usage
      exit 2
      ;;
  esac
fi

require_dir() {
  local dir="$1"
  if [[ ! -d "$dir" ]]; then
    printf 'error: required directory does not exist: %s\n' "$dir" >&2
    exit 1
  fi
}

project_names_file() {
  local output_path="$1"
  shift

  : > "$output_path"
  for projects_dir in "$@"; do
    shopt -s nullglob
    for project_path in "$projects_dir"/*; do
      if [[ -d "$project_path/.git" ]]; then
        printf '%s (sample analysis: %d samples)\n' "$(basename -- "$project_path")" "$samples" >> "$output_path"
      fi
    done
    shopt -u nullglob
  done

  if [[ ! -s "$output_path" ]]; then
    printf 'error: no git repositories found under listed project directories\n' >&2
    exit 1
  fi
}

collect_codebase_ids() {
  local names_path="$1"
  local codebases_path="$2"

  python3 - "$names_path" "$codebases_path" <<'PY'
import json
import sys

names_path, codebases_path = sys.argv[1:]
with open(names_path, encoding="utf-8") as handle:
    expected_names = [line.rstrip("\n") for line in handle if line.strip()]
with open(codebases_path, encoding="utf-8") as handle:
    codebases = json.load(handle)

ids_by_name = {codebase["name"]: codebase["id"] for codebase in codebases}
missing = [name for name in expected_names if name not in ids_by_name]
if missing:
    for name in missing:
        print(f"missing analyzed codebase: {name}", file=sys.stderr)
    sys.exit(1)

print(",".join(ids_by_name[name] for name in expected_names))
PY
}

require_dir "$listed_dir/go"
require_dir "$listed_dir/ocaml"
require_dir "$plots_dir"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

printf 'Analyzing Go listed repositories\n'
"$script_dir/sample_analyze_projects.sh" --dir "$listed_dir/go" --samples "$samples" --language golang --fail-fast

printf 'Analyzing OCaml listed repositories\n'
"$script_dir/sample_analyze_projects.sh" --dir "$listed_dir/ocaml" --samples "$samples" --language ocaml --fail-fast

project_names_file "$tmp_dir/project-names.txt" "$listed_dir/go" "$listed_dir/ocaml"
cargo run -p sourcery-db -- codebases > "$tmp_dir/codebases.json"
codebase_ids="$(collect_codebase_ids "$tmp_dir/project-names.txt" "$tmp_dir/codebases.json")"

if [[ -z "$codebase_ids" ]]; then
  printf 'error: no codebase ids resolved from analyzed projects\n' >&2
  exit 1
fi

printf 'Exporting metrics CSVs into %s\n' "$plots_dir"
for version in {1..10}; do
  cargo run -p sourcery-db -- analysis-metrics-csv \
    --codebase-ids "$codebase_ids" \
    --version "$version" \
    --outfile "$plots_dir/metrics${version}.csv"

  cargo run -p sourcery-db -- analysis-metric-extremes-csv \
    --codebase-ids "$codebase_ids" \
    --version "$version" \
    --outfile "$plots_dir/extremes${version}.csv"
done

cargo run -p sourcery-db -- analysis-version-metrics-csv \
  --codebase-ids "$codebase_ids" \
  --outfile "$plots_dir/version-metrics.csv"

printf 'Regenerating plots and proportionality reports\n'
"$plots_dir/run-all.sh"
