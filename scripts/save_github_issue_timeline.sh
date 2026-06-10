#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s OWNER/REPO [github_issue_timeline.py args...]\n' "$0" >&2
  printf 'Requires DATABASE_URL, gh, and psql.\n' >&2
}

if [[ $# -lt 1 ]]; then
  usage
  exit 2
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
  printf 'error: DATABASE_URL is required\n' >&2
  exit 2
fi

repo="$1"
shift

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
migration="$repo_root/migrations/20260609120000_add_github_issue_timeline_events.sql"

psql "$DATABASE_URL" --quiet --set ON_ERROR_STOP=1 --file "$migration"

"$script_dir/github_issue_timeline.py" "$repo" --save --no-output "$@"

printf 'Saved GitHub issue timeline for %s\n' "$repo"
