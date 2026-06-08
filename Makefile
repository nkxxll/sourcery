# Makefile for building gohalstead (Go) and ocamlhalstead (OCaml/dune)

BIN_DIR := $(CURDIR)/bin
GO_DIR := gohalstead
OCAML_DIR := ocamlhalstead

.PHONY: all gohalstead ocamlhalstead clean test print-path

all: gohalstead ocamlhalstead

gohalstead:
	cd $(GO_DIR) && go build -o $(BIN_DIR)/gohalstead .

ocamlhalstead:
	cd $(OCAML_DIR) && dune build -p ocamlhalstead -j auto @install
	@if [ -f _build/default/bin/ocamlhalstead.exe ]; then \
		cp _build/default/bin/ocamlhalstead.exe $(BIN_DIR)/ocamlhalstead; \
	elif [ -f _build/default/bin/main.exe ]; then \
		cp _build/default/bin/main.exe $(BIN_DIR)/ocamlhalstead; \
	elif [ -f $(OCAML_DIR)/_build/default/bin/main.exe ]; then \
			cp ocamlhalstead/_build/default/bin/main.exe $(BIN_DIR)/ocamlhalstead; \
	else \
		echo "OCaml binary not found in _build/default/bin"; exit 1; \
	fi

test:
	cd $(GO_DIR) && go test ./...
	cd $(OCAML_DIR) && dune runtest

clean:
	rm -rf $(BIN_DIR)
	cd $(OCAML_DIR) && dune clean || true

print-path:
	@echo "PATH for make: $(PATH)"
	@echo "Binaries in $(BIN_DIR):" && ls -la $(BIN_DIR) || true
