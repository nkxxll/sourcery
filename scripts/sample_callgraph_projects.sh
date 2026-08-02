#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s --dir DIR --samples N [--output-dir DIR] [--language LANGUAGE] [--fail-fast]\n' "$0" >&2
  printf 'Runs sourcery-analyzer sample-callgraph on each git repository directly under DIR.\n' >&2
  printf 'Each repository is written to OUTPUT_DIR/REPOSITORY.csv; OUTPUT_DIR defaults to DIR.\n' >&2
  printf 'LANGUAGE is optional; valid values are the analyzer language args, e.g. golang, ocaml, python.\n' >&2
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

projects_dir=""
samples=""
output_dir=""
language=""
fail_fast=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir)
      if [[ $# -lt 2 ]]; then
        printf 'error: --dir requires a value\n' >&2
        usage
        exit 2
      fi
      projects_dir="$2"
      shift 2
      ;;
    --samples)
      if [[ $# -lt 2 ]]; then
        printf 'error: --samples requires a value\n' >&2
        usage
        exit 2
      fi
      samples="$2"
      shift 2
      ;;
    --output-dir)
      if [[ $# -lt 2 ]]; then
        printf 'error: --output-dir requires a value\n' >&2
        usage
        exit 2
      fi
      output_dir="$2"
      shift 2
      ;;
    --language)
      if [[ $# -lt 2 ]]; then
        printf 'error: --language requires a value\n' >&2
        usage
        exit 2
      fi
      language="$2"
      shift 2
      ;;
    --fail-fast)
      fail_fast=true
      shift
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

if [[ -z "$projects_dir" ]]; then
  printf 'error: --dir is required\n' >&2
  usage
  exit 2
fi

if [[ -z "$samples" ]]; then
  printf 'error: --samples is required\n' >&2
  usage
  exit 2
fi

if [[ ! "$samples" =~ ^[1-9][0-9]*$ ]]; then
  printf 'error: --samples must be a positive integer\n' >&2
  usage
  exit 2
fi

if [[ ! -d "$projects_dir" ]]; then
  printf 'error: directory does not exist: %s\n' "$projects_dir" >&2
  exit 2
fi

if [[ -z "$output_dir" ]]; then
  output_dir="$projects_dir"
fi

if [[ ! -d "$output_dir" ]]; then
  printf 'error: output directory does not exist: %s\n' "$output_dir" >&2
  exit 2
fi

analyze_project() {
  local project_path="$1"
  local project_name
  project_name="$(basename -- "$project_path")"
  local output_path="$output_dir/$project_name.csv"
  local cmd=(cargo run -p sourcery-analyzer -- sample-callgraph "$project_path" "$samples")

  if [[ -n "$language" ]]; then
    cmd+=("$language")
  fi

  printf 'Analyzing %s -> %s\n' "$project_path" "$output_path" >&2
  "${cmd[@]}" > "$output_path"
}

shopt -s nullglob
failures=()
analyzed=0
skipped=0

for project_path in "$projects_dir"/*; do
  if [[ ! -d "$project_path" ]]; then
    continue
  fi

  if [[ ! -d "$project_path/.git" ]]; then
    printf 'Skipping non-git directory: %s\n' "$project_path" >&2
    skipped=$((skipped + 1))
    continue
  fi

  if analyze_project "$project_path"; then
    analyzed=$((analyzed + 1))
  else
    failures+=("$project_path")
    printf 'error: analysis failed for %s\n' "$project_path" >&2
    if [[ "$fail_fast" == true ]]; then
      exit 1
    fi
  fi
done

if [[ $analyzed -eq 0 && ${#failures[@]} -eq 0 ]]; then
  printf 'No git repositories found directly under %s\n' "$projects_dir" >&2
  exit 1
fi

if [[ ${#failures[@]} -gt 0 ]]; then
  printf '\nFailed projects:\n' >&2
  for failure in "${failures[@]}"; do
    printf '%s\n' "- $failure" >&2
  done
  exit 1
fi

printf 'Completed callgraph CSV analysis for %d project(s); skipped %d non-git directories.\n' "$analyzed" "$skipped"
