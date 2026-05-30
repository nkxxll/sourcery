#!/usr/bin/env bash

set -o pipefail

build=false
while getopts "b" opt; do
  case "$opt" in
    b) build=true ;;
    *) echo "Usage: $0 [-b] <commit-hash>" >&2; exit 2 ;;
  esac
done
shift $((OPTIND-1))

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

if [ "$build" = true ]; then
    codebase=$(exec "$script_dir/get_first_codebase.sh" -b)
else
    codebase=$(exec "$script_dir/get_first_codebase.sh")
fi

if [ "$build" = true ]; then
    version=$(exec "$script_dir/get_last_verion_of_codebase.sh" -b "$codebase")
else
    version=$(exec "$script_dir/get_last_verion_of_codebase.sh" "$codebase")
fi

files="$("$bin_path" current-files "$codebase" "$version" 2>/dev/null || true)"
printf '%s\n' "$files"
