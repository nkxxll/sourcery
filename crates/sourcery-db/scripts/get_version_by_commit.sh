#!/usr/bin/env bash

set -euo pipefail

# Usage: get_version_by_commit.sh [-b] <commit-hash>
# -b: rebuild the sourcery-db binary before running

build=false
while getopts "b" opt; do
  case "$opt" in
    b) build=true ;;
    *) echo "Usage: $0 [-b] <commit-hash>" >&2; exit 2 ;;
  esac
done
shift $((OPTIND-1))

if [ $# -ne 1 ]; then
  echo "Usage: $0 [-b] <commit-hash>" >&2
  exit 2
fi

commit_hash="$1"

err() { echo "$@" >&2; }

if ! command -v jq >/dev/null 2>&1; then
  err "jq is required but not installed"
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  err "cargo is required but not installed"
  exit 1
fi

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

if [ ! -x "$bin_path" ]; then
  echo "Binary not found at $bin_path, building..."
  (cd "$repo_root" && cargo build -p sourcery-db)
fi

: "${DATABASE_URL:?DATABASE_URL must be set}"

codebases="$("$bin_path" codebases 2>/dev/null || true)"
if [ -z "${codebases}" ]; then
  err "Failed to fetch codebases"
  exit 1
fi

codebase_id="$(printf '%s' "$codebases" | jq -r '.[0].id // empty')"
if [ -z "${codebase_id}" ]; then
  err "No Golang codebase found in codebases output"
  exit 1
fi

version="$("$bin_path" version-by-commit "$codebase_id" "$commit_hash" 2>/dev/null || true)"
if [ -z "${version}" ]; then
  err "Failed to fetch version for codebase ${codebase_id} and commit ${commit_hash}"
  exit 1
fi

version_id="$(printf '%s' "$version" | jq -r 'if . == null then empty elif type == "object" then .id // empty else . end')"
if [ -z "${version_id}" ]; then
  err "No version found for codebase ${codebase_id} and commit ${commit_hash}"
  exit 1
fi

printf '%s\n' "$version_id"
