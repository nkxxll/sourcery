#!/usr/bin/env bash

set -euo pipefail

# Usage: get_latest_version.sh [-b]
# -b: rebuild the sourcery-db binary before running

build=false
while getopts "b" opt; do
  case "$opt" in
    b) build=true ;;
    *) echo "Usage: $0 [-b]" >&2; exit 2 ;;
  esac
done
shift $((OPTIND-1))

err() { echo "$@" >&2; }

# prerequisites
if ! command -v jq >/dev/null 2>&1; then
  err "jq is required but not installed"
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  err "cargo is required but not installed"
  exit 1
fi

# locate repository root (fallback to relative path from script)
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
repo_root=""
if git -C "$script_dir" rev-parse --show-toplevel >/dev/null 2>&1; then
  repo_root="$(git -C "$script_dir" rev-parse --show-toplevel)"
else
  repo_root="$(cd "$script_dir/../.." >/dev/null 2>&1 && pwd)"
fi

bin_path="$repo_root/target/debug/sourcery-db"

if [ "$build" = true ]; then
  echo "Rebuilding sourcery-db..."
  (cd "$repo_root" && cargo build -p sourcery-db --quiet)
fi

# ensure binary exists (build once more if needed)
if [ ! -x "$bin_path" ]; then
  echo "Binary not found at $bin_path, building..."
  (cd "$repo_root" && cargo build -p sourcery-db)
fi

: "${DATABASE_URL:?DATABASE_URL must be set}"

# get codebases JSON
codebases="$("$bin_path" codebases 2>/dev/null || true)"
if [ -z "${codebases}" ]; then
  err "Failed to fetch codebases"
  exit 1
fi

# extract first Golang codebase id
codebase_id="$(printf '%s' "$codebases" | jq -r '.[0].id // empty')"
if [ -z "${codebase_id}" ]; then
  err "No Golang codebase found in codebases output"
  exit 1
fi

# fetch versions for the codebase
versions="$("$bin_path" codebase-metrics "$codebase_id" 2>/dev/null || true)"
if [ -z "${versions}" ]; then
  err "Failed to fetch versions for codebase ${codebase_id}"
  exit 1
fi

# get id of latest version (last element)
latest_id="$(printf '%s' "$versions" | jq -r '.[-1].id // empty')"
if [ -z "${latest_id}" ]; then
  err "No versions found for codebase ${codebase_id}"
  exit 1
fi

printf '%s\n' "$latest_id"
