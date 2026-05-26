package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"go/scanner"
	"go/token"
	"os"
	"strconv"
	"strings"
)

type Halstead struct {
	UniqueOperators int `json:"unique_operators"`
	UniqueOperands  int `json:"unique_operands"`
	Operators       int `json:"operators"`
	Operands        int `json:"operands"`
}

type FunctionMetrics struct {
	Name    string   `json:"name"`
	Metrics Halstead `json:"metrics"`
}

type Output struct {
	Totals    Halstead          `json:"totals"`
	Functions []FunctionMetrics `json:"functions"`
}

type HalsteadBuilder struct {
	uniqueOperators map[string]struct{}
	uniqueOperands  map[string]struct{}
	operators       int
	operands        int
}

type FunctionDefinition struct {
	name  string
	start int
	end   int
}

type Input struct {
	file                string
	functionDefinitions []FunctionDefinition
}

func readStdin() Input {
	scanner := bufio.NewScanner(os.Stdin)
	var functionDefinition []FunctionDefinition
	var file string
	if scanner.Scan() {
		file = scanner.Text()
	} else {
		panic("there is not file provided for halstead metric analysis")
	}

	for scanner.Scan() {
		line := scanner.Text()
		parts := strings.SplitN(line, ":", 3)
		if len(parts) != 3 {
			panic("there is an error with the reading of the line")
		}
		name := parts[0]
		start, err := strconv.Atoi(parts[1])
		if err != nil {
			panic(fmt.Sprintf("the input is malformatted. error: %v\n", err))
		}
		end, err := strconv.Atoi(parts[2])
		if err != nil {
			panic(fmt.Sprintf("the input is malformatted. error: %v\n", err))
		}
		functionDefinition = append(functionDefinition, FunctionDefinition{
			name,
			start,
			end,
		})
	}

	if err := scanner.Err(); err != nil {
		fmt.Fprintln(os.Stderr, "error reading stdin:", err)
		os.Exit(1)
	}
	return Input{
		file,
		functionDefinition,
	}
}

func (i Input) ToString() string {
	var b strings.Builder
	fmt.Fprintf(&b, "File: %s\n", i.file)
	for _, funcDef := range i.functionDefinitions {
		fmt.Fprintf(&b, "FuncDef:\n\tname: %s\n\tstart: %d\n\tend: %d\n", funcDef.name, funcDef.start, funcDef.end)
	}
	return b.String()
}

type tokenInfo struct {
	tok token.Token
	lit string
	pos token.Position
}

var ignoredTokens = map[token.Token]struct{}{
	token.COMMENT:   {},
	token.SEMICOLON: {},
	token.COMMA:     {},
	token.LPAREN:    {},
	token.RPAREN:    {},
	token.LBRACK:    {},
	token.RBRACK:    {},
	token.LBRACE:    {},
	token.RBRACE:    {},
	token.COLON:     {},
	token.PERIOD:    {},
	token.ELLIPSIS:  {},
}

func NewHalsteadBuilder() *HalsteadBuilder {
	return &HalsteadBuilder{
		uniqueOperators: map[string]struct{}{},
		uniqueOperands:  map[string]struct{}{},
	}
}

func (b *HalsteadBuilder) AddOperator(name string) {
	b.operators++
	b.uniqueOperators[name] = struct{}{}
}

func (b *HalsteadBuilder) AddOperand(name string) {
	b.operands++
	b.uniqueOperands[name] = struct{}{}
}

func (b *HalsteadBuilder) Build() Halstead {
	return Halstead{
		UniqueOperators: len(b.uniqueOperators),
		UniqueOperands:  len(b.uniqueOperands),
		Operators:       b.operators,
		Operands:        b.operands,
	}
}

func tokenName(tok token.Token, lit string) string {
	if lit != "" {
		return lit
	}
	return tok.String()
}

func (b *HalsteadBuilder) AddToken(tok token.Token, lit string) {
	if tok == token.EOF {
		return
	}
	if _, ignored := ignoredTokens[tok]; ignored {
		return
	}
	if tok == token.IDENT || tok.IsLiteral() {
		b.AddOperand(tokenName(tok, lit))
		return
	}
	if tok.IsKeyword() || tok.IsOperator() {
		b.AddOperator(tokenName(tok, lit))
	}
}

func lexFile(filePath string) ([]tokenInfo, error) {
	content, err := os.ReadFile(filePath)
	if err != nil {
		return nil, fmt.Errorf("the file could not be read. error: %v", err)
	}

	var s scanner.Scanner
	fset := token.NewFileSet()
	file := fset.AddFile(filePath, fset.Base(), len(content))
	s.Init(file, content, nil, scanner.ScanComments)

	tokens := make([]tokenInfo, 0, 256)
	for {
		pos, tok, lit := s.Scan()
		position := fset.Position(pos)
		tokens = append(tokens, tokenInfo{tok: tok, lit: lit, pos: position})
		if tok == token.EOF {
			break
		}
	}
	return tokens, nil
}

func tokensForFuncDefinition(funcDefinition FunctionDefinition, tokenList []tokenInfo) []tokenInfo {
	startLine := funcDefinition.start
	endLine := funcDefinition.end
	filtered := make([]tokenInfo, 0, len(tokenList))
	for _, item := range tokenList {
		if item.pos.Line >= startLine && item.pos.Line <= endLine {
			filtered = append(filtered, item)
		}
	}
	return filtered
}

func halsteadForTokenList(tokenList []tokenInfo) Halstead {
	builder := NewHalsteadBuilder()
	for _, tokenInfo := range tokenList {
		builder.AddToken(tokenInfo.tok, tokenInfo.lit)
	}
	return builder.Build()
}

func (i Input) SourceFileToHalstead() (Output, error) {
	tokens, err := lexFile(i.file)
	if err != nil {
		return Output{}, err
	}

	totals := halsteadForTokenList(tokens)
	functions := make([]FunctionMetrics, 0, len(i.functionDefinitions))
	for _, funcDef := range i.functionDefinitions {
		funcTokens := tokensForFuncDefinition(funcDef, tokens)
		metrics := halsteadForTokenList(funcTokens)
		functions = append(functions, FunctionMetrics{
			Name:    funcDef.name,
			Metrics: metrics,
		})
	}

	return Output{
		Totals:    totals,
		Functions: functions,
	}, nil
}

func main() {
	input := readStdin()
	output, err := input.SourceFileToHalstead()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	data, err := json.MarshalIndent(output, "", "  ")
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	fmt.Println(string(data))
}
