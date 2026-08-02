#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s [--samples N]\n' "$0" >&2
  printf 'Runs sample-callgraph for repositories in toanalyze/listed/go and toanalyze/listed/ocaml.\n' >&2
  printf 'Results are written under results/go and results/ocaml.\n' >&2
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
projects_root="$repo_root/toanalyze/listed"
results_root="$repo_root/results"
samples=10

while [[ $# -gt 0 ]]; do
  case "$1" in
    --samples)
      if [[ $# -lt 2 ]]; then
        printf 'error: --samples requires a value\n' >&2
        usage
        exit 2
      fi
      samples="$2"
      shift 2
      ;;
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
done

if [[ ! "$samples" =~ ^[1-9][0-9]*$ ]]; then
  printf 'error: --samples must be a positive integer\n' >&2
  usage
  exit 2
fi

for language in go ocaml; do
  projects_dir="$projects_root/$language"
  if [[ ! -d "$projects_dir" ]]; then
    printf 'error: project directory does not exist: %s\n' "$projects_dir" >&2
    exit 1
  fi
done

mkdir -p "$results_root/go" "$results_root/ocaml"

printf 'Analyzing Go repositories\n'
"$script_dir/sample_callgraph_projects.sh" \
  --dir "$projects_root/go" \
  --samples "$samples" \
  --language golang \
  --output-dir "$results_root/go" \
  --fail-fast

printf 'Analyzing OCaml repositories\n'
"$script_dir/sample_callgraph_projects.sh" \
  --dir "$projects_root/ocaml" \
  --samples "$samples" \
  --language ocaml \
  --output-dir "$results_root/ocaml" \
  --fail-fast

printf 'Completed callgraph CSV analysis; results are in %s\n' "$results_root"
