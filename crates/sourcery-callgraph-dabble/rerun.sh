#!/usr/bin/env bash

cargo run --bin printcallnames -- go ../../go_sample_functions3.go > go_callnames.txt
cargo run --bin printcallnames -- ocaml ../../ocaml_sample_functions3.ml > ocaml_callnames.txt
