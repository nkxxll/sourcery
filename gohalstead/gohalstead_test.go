package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"
	"testing"
)

func parseFixture(path string) (Input, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return Input{}, err
	}
	lines := strings.Split(strings.TrimSpace(string(data)), "\n")
	if len(lines) == 0 || strings.TrimSpace(lines[0]) == "" {
		return Input{}, fmt.Errorf("fixture is empty")
	}

	file := lines[0]
	functionDefinitions := make([]FunctionDefinition, 0, len(lines)-1)
	for _, line := range lines[1:] {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		parts := strings.SplitN(line, ":", 3)
		if len(parts) != 3 {
			return Input{}, fmt.Errorf("invalid fixture line: %q", line)
		}
		start, err := strconv.Atoi(parts[1])
		if err != nil {
			return Input{}, fmt.Errorf("invalid start line in fixture: %w", err)
		}
		end, err := strconv.Atoi(parts[2])
		if err != nil {
			return Input{}, fmt.Errorf("invalid end line in fixture: %w", err)
		}
		functionDefinitions = append(functionDefinitions, FunctionDefinition{
			name:  parts[0],
			start: start,
			end:   end,
		})
	}

	return Input{
		file:                file,
		functionDefinitions: functionDefinitions,
	}, nil
}

func TestFixtureOutput(t *testing.T) {
	input, err := parseFixture("fixture.txt")
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}

	output, err := input.SourceFileToHalstead()
	if err != nil {
		t.Fatalf("compute halstead: %v", err)
	}

	actual, err := json.MarshalIndent(output, "", "  ")
	if err != nil {
		t.Fatalf("marshal output: %v", err)
	}

	expected, err := os.ReadFile("expected_output.json")
	if err != nil {
		t.Fatalf("read expected output: %v", err)
	}

	if strings.TrimSpace(string(actual)) != strings.TrimSpace(string(expected)) {
		t.Fatalf("output mismatch\nexpected:\n%s\nactual:\n%s", strings.TrimSpace(string(expected)), strings.TrimSpace(string(actual)))
	}
}
