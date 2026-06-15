#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s [--clone-dir DIR]\n' "$0" >&2
  printf 'Clones repos from gh_query/go_list.txt and gh_query/ocaml_list.txt.\n' >&2
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
clone_dir="$repo_root/toanalyze/listed"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --clone-dir)
      if [[ $# -lt 2 ]]; then
        printf 'error: --clone-dir requires a value\n' >&2
        usage
        exit 2
      fi
      clone_dir="$2"
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

clone_list() {
  local language="$1"
  local list_path="$2"

  if [[ ! -f "$list_path" ]]; then
    printf 'error: list file does not exist: %s\n' "$list_path" >&2
    exit 2
  fi

  mkdir -p "$clone_dir/$language"

  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line%%#*}"
    line="${line//[$' \t\r\n\"']/}"

    if [[ -z "$line" ]]; then
      continue
    fi

    local destination="$clone_dir/$language/${line//\//__}"

    if [[ -d "$destination/.git" ]]; then
      printf 'Fetching %s into %s\n' "$line" "$destination"
      git -C "$destination" fetch --all --tags --prune
    elif [[ -e "$destination" ]]; then
      printf 'error: destination exists but is not a git repo: %s\n' "$destination" >&2
      exit 1
    else
      printf 'Cloning %s into %s\n' "$line" "$destination"
      git clone "https://github.com/$line.git" "$destination"
    fi
  done < "$list_path"
}

clone_list go "$repo_root/gh_query/go_list.txt"
clone_list ocaml "$repo_root/gh_query/ocaml_list.txt"

printf 'Completed cloning listed repos into %s\n' "$clone_dir"
