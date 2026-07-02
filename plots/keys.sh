#!/usr/bin/env bash
set -euo pipefail

{
  tail -n +2 metrics[0-9]*.csv | cut -d, -f1
  printf '%s\n' cyclomatic_per_line
} | sort -u
