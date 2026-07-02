#!/usr/bin/env bash

cargo run -p sourcery-change-count toanalyze/listed/go -l Go -o file-change-counts-go.csv
cargo run -p sourcery-change-count toanalyze/listed/ocaml -l OCaml -o file-change-counts-ocaml.csv
