#!/usr/bin/env bash

{
  tail -n +2 metrics.csv | cut -d, -f1
  printf '%s\n' cyclomatic_per_line
} | sort -u
