open Lexing
open Parser
module Lexer = Ocamlhalstead.Lexer
module StringSet = Set.Make (String)

module Halstead = struct
  type func_def =
    { name : string
    ; start : int
    ; end_ : int
    }

  type t =
    { file : string
    ; functions : func_def list
    }

  type token =
    { type_ : Parser.token
    ; start : position
    ; end_ : position
    }

  type halstead =
    { unique_operators : StringSet.t
    ; unique_operands : StringSet.t
    ; operators : int
    ; operands : int
    }

  type function_metrics =
    { name : string
    ; metrics : halstead
    }

  type output =
    { totals : halstead
    ; functions : function_metrics list
    }

  let create_func_call name start end_ = { name; start; end_ }
  let parse_function func = Scanf.sscanf func "%[^:]:%d:%d" create_func_call

  let parse_input input =
    match input with
    | file :: functions ->
      let functions = List.map parse_function functions in
      { file; functions }
    | _ -> failwith "the input format is wrong breaking here"
  ;;

  let lex_file t =
    Lexer.init ();
    let lexbuf = Lexing.from_channel (open_in t.file) in
    let rec collect lexbuf acc =
      let start = lexbuf.lex_start_p in
      let token = Lexer.token lexbuf in
      let end_ = lexbuf.lex_curr_p in
      let tok = { type_ = token; start; end_ } in
      match token with
      | Parser.EOF -> List.rev (tok :: acc)
      | _ -> collect lexbuf (tok :: acc)
    in
    collect lexbuf []
  ;;

  let tokens_for_func_def (func_definition : func_def) token_list =
    let start_line = func_definition.start in
    let end_line = func_definition.end_ in
    List.filter
      (fun item ->
         let fstart_line = item.start.pos_lnum in
         let fend_line = item.end_.pos_lnum in
         fstart_line >= start_line && fend_line <= end_line)
      token_list
  ;;

  let halstead_init =
    { unique_operators = StringSet.empty
    ; unique_operands = StringSet.empty
    ; operators = 0
    ; operands = 0
    }
  ;;

  let token_name (token : Parser.token) =
    match token with
    | WITH -> "WITH"
    | WHILE -> "WHILE"
    | WHEN -> "WHEN"
    | VIRTUAL -> "VIRTUAL"
    | VAL -> "VAL"
    | UNDERSCORE -> "UNDERSCORE"
    | UIDENT _ -> "UIDENT"
    | TYPE -> "TYPE"
    | TRY -> "TRY"
    | TRUE -> "TRUE"
    | TO -> "TO"
    | TILDE -> "TILDE"
    | THEN -> "THEN"
    | STRUCT -> "STRUCT"
    | STRING _ -> "STRING"
    | STAR -> "STAR"
    | SIG -> "SIG"
    | SEMISEMI -> "SEMISEMI"
    | SEMI -> "SEMI"
    | RPAREN -> "RPAREN"
    | REC -> "REC"
    | RBRACKET -> "RBRACKET"
    | RBRACE -> "RBRACE"
    | QUOTED_STRING_ITEM _ -> "QUOTED_STRING_ITEM"
    | QUOTED_STRING_EXPR _ -> "QUOTED_STRING_EXPR"
    | QUOTE -> "QUOTE"
    | QUESTION -> "QUESTION"
    | PRIVATE -> "PRIVATE"
    | PREFIXOP _ -> "PREFIXOP"
    | PLUSEQ -> "PLUSEQ"
    | PLUSDOT -> "PLUSDOT"
    | PLUS -> "PLUS"
    | PERCENT -> "PERCENT"
    | OR -> "OR"
    | OPTLABEL _ -> "OPTLABEL"
    | OPEN -> "OPEN"
    | OF -> "OF"
    | OBJECT -> "OBJECT"
    | NONREC -> "NONREC"
    | NEW -> "NEW"
    | MUTABLE -> "MUTABLE"
    | MODULE -> "MODULE"
    | MINUSGREATER -> "MINUSGREATER"
    | MINUSDOT -> "MINUSDOT"
    | MINUS -> "MINUS"
    | METHOD -> "METHOD"
    | METAOCAML_ESCAPE -> "METAOCAML_ESCAPE"
    | METAOCAML_BRACKET_OPEN -> "METAOCAML_BRACKET_OPEN"
    | METAOCAML_BRACKET_CLOSE -> "METAOCAML_BRACKET_CLOSE"
    | MATCH -> "MATCH"
    | LPAREN -> "LPAREN"
    | LIDENT _ -> "LIDENT"
    | LETOP _ -> "LETOP"
    | LET -> "LET"
    | LESSMINUS -> "LESSMINUS"
    | LESS -> "LESS"
    | LBRACKETPERCENTPERCENT -> "LBRACKETPERCENTPERCENT"
    | LBRACKETPERCENT -> "LBRACKETPERCENT"
    | LBRACKETLESS -> "LBRACKETLESS"
    | LBRACKETGREATER -> "LBRACKETGREATER"
    | LBRACKETBAR -> "LBRACKETBAR"
    | LBRACKETATATAT -> "LBRACKETATATAT"
    | LBRACKETATAT -> "LBRACKETATAT"
    | LBRACKETAT -> "LBRACKETAT"
    | LBRACKET -> "LBRACKET"
    | LBRACELESS -> "LBRACELESS"
    | LBRACE -> "LBRACE"
    | LAZY -> "LAZY"
    | LABEL _ -> "LABEL"
    | INT _ -> "INT"
    | INITIALIZER -> "INITIALIZER"
    | INHERIT -> "INHERIT"
    | INFIXOP4 _ -> "INFIXOP4"
    | INFIXOP3 _ -> "INFIXOP3"
    | INFIXOP2 _ -> "INFIXOP2"
    | INFIXOP1 _ -> "INFIXOP1"
    | INFIXOP0 _ -> "INFIXOP0"
    | INCLUDE -> "INCLUDE"
    | IN -> "IN"
    | IF -> "IF"
    | HASHOP _ -> "HASHOP"
    | HASH -> "HASH"
    | GREATERRBRACKET -> "GREATERRBRACKET"
    | GREATERRBRACE -> "GREATERRBRACE"
    | GREATER -> "GREATER"
    | FUNCTOR -> "FUNCTOR"
    | FUNCTION -> "FUNCTION"
    | FUN -> "FUN"
    | FOR -> "FOR"
    | FLOAT _ -> "FLOAT"
    | FALSE -> "FALSE"
    | EXTERNAL -> "EXTERNAL"
    | EXCEPTION -> "EXCEPTION"
    | EQUAL -> "EQUAL"
    | EOL -> "EOL"
    | EOF -> "EOF"
    | END -> "END"
    | ELSE -> "ELSE"
    | EFFECT -> "EFFECT"
    | DOWNTO -> "DOWNTO"
    | DOTOP _ -> "DOTOP"
    | DOTDOT -> "DOTDOT"
    | DOT -> "DOT"
    | DONE -> "DONE"
    | DOCSTRING _ -> "DOCSTRING"
    | DO -> "DO"
    | CONSTRAINT -> "CONSTRAINT"
    | COMMENT _ -> "COMMENT"
    | COMMA -> "COMMA"
    | COLONGREATER -> "COLONGREATER"
    | COLONEQUAL -> "COLONEQUAL"
    | COLONCOLON -> "COLONCOLON"
    | COLON -> "COLON"
    | CLASS -> "CLASS"
    | CHAR _ -> "CHAR"
    | BEGIN -> "BEGIN"
    | BARRBRACKET -> "BARRBRACKET"
    | BARBAR -> "BARBAR"
    | BAR -> "BAR"
    | BANG -> "BANG"
    | BACKQUOTE -> "BACKQUOTE"
    | ASSERT -> "ASSERT"
    | AS -> "AS"
    | ANDOP _ -> "ANDOP"
    | AND -> "AND"
    | AMPERSAND -> "AMPERSAND"
    | AMPERAMPER -> "AMPERAMPER"
  ;;

  let token_to_string (token : Parser.token) =
    match token with
    | UIDENT name -> name
    | LIDENT name -> name
    | INT (value, _) -> value
    | FLOAT (value, _) -> value
    | STRING (value, _, _) -> value
    | CHAR value -> String.make 1 value
    | COMMENT (value, _) -> value
    | DOCSTRING doc -> Docstrings.docstring_body doc
    | QUOTED_STRING_ITEM (value, _, _, _, _) -> value
    | QUOTED_STRING_EXPR (value, _, _, _, _) -> value
    | PREFIXOP value -> value
    | INFIXOP4 value -> value
    | INFIXOP3 value -> value
    | INFIXOP2 value -> value
    | INFIXOP1 value -> value
    | INFIXOP0 value -> value
    | LETOP value -> value
    | ANDOP value -> value
    | LABEL value -> value
    | OPTLABEL value -> value
    | DOTOP value -> value
    | HASHOP value -> value
    | _ -> token_name token
  ;;

  let is_operand (token : Parser.token) =
    match token with
    | UIDENT _
    | LIDENT _
    | INT _
    | FLOAT _
    | STRING _
    | CHAR _
    | QUOTED_STRING_ITEM _
    | QUOTED_STRING_EXPR _
    | LABEL _
    | OPTLABEL _
    | TRUE
    | FALSE
    | UNDERSCORE -> true
    | _ -> false
  ;;

  let is_ignored (token : Parser.token) =
    match token with
    | COMMENT _
    | DOCSTRING _
    | EOL
    | EOF
    | LPAREN
    | RPAREN
    | LBRACKET
    | RBRACKET
    | LBRACE
    | RBRACE
    | LBRACELESS
    | LBRACKETLESS
    | LBRACKETGREATER
    | LBRACKETBAR
    | LBRACKETATATAT
    | LBRACKETATAT
    | LBRACKETAT
    | LBRACKETPERCENTPERCENT
    | LBRACKETPERCENT
    | GREATERRBRACKET
    | GREATERRBRACE
    | METAOCAML_BRACKET_OPEN
    | METAOCAML_BRACKET_CLOSE
    | BARRBRACKET
    | COMMA
    | SEMI
    | SEMISEMI
    | DOT
    | DOTDOT
    | COLON
    | QUESTION
    | QUOTE
    | BACKQUOTE
    | TILDE -> true
    | _ -> false
  ;;

  let builtin_keywords =
    StringSet.of_list
      [ "and"
      ; "as"
      ; "asr"
      ; "assert"
      ; "begin"
      ; "class"
      ; "constraint"
      ; "do"
      ; "done"
      ; "downto"
      ; "else"
      ; "end"
      ; "exception"
      ; "external"
      ; "false"
      ; "for"
      ; "fun"
      ; "function"
      ; "functor"
      ; "if"
      ; "in"
      ; "include"
      ; "inherit"
      ; "initializer"
      ; "lazy"
      ; "land"
      ; "let"
      ; "lor"
      ; "lsl"
      ; "lsr"
      ; "lxor"
      ; "match"
      ; "method"
      ; "mod"
      ; "module"
      ; "mutable"
      ; "new"
      ; "nonrec"
      ; "object"
      ; "of"
      ; "open"
      ; "or"
      ; "private"
      ; "rec"
      ; "sig"
      ; "struct"
      ; "then"
      ; "to"
      ; "true"
      ; "try"
      ; "type"
      ; "val"
      ; "virtual"
      ; "when"
      ; "while"
      ; "with"
      ]
  ;;

  let is_builtin_keyword (token : Parser.token) =
    let name = token_name token |> String.lowercase_ascii in
    StringSet.mem name builtin_keywords
  ;;

  let is_keyword (token : Parser.token) =
    let name = token_name token |> String.lowercase_ascii in
    StringSet.mem name builtin_keywords || Lexer.is_keyword name
  ;;

  let is_operator (token : Parser.token) =
    match token with
    | PREFIXOP _
    | INFIXOP4 _
    | INFIXOP3 _
    | INFIXOP2 _
    | INFIXOP1 _
    | INFIXOP0 _
    | LETOP _
    | ANDOP _
    | DOTOP _
    | HASHOP _
    | BAR
    | BARBAR
    | AMPERSAND
    | AMPERAMPER
    | PLUS
    | PLUSEQ
    | PLUSDOT
    | MINUS
    | MINUSDOT
    | MINUSGREATER
    | STAR
    | PERCENT
    | EQUAL
    | LESS
    | GREATER
    | COLONEQUAL
    | COLONCOLON
    | COLONGREATER
    | LESSMINUS
    | HASH
    | BANG -> true
    | _ -> false
  ;;

  let logging_enabled =
    match Sys.getenv_opt "HALSTEAD_LOG" with
    | Some value ->
      let value = String.lowercase_ascii value in
      value = "1" || value = "true" || value = "yes" || value = "on"
    | None -> false
  ;;

  let log_token ~scope ~kind (token : token) =
    if logging_enabled
    then (
      let tok = token.type_ in
      let pos = token.start in
      let col = pos.pos_cnum - pos.pos_bol in
      Printf.eprintf
        "[halstead] %s %s token=%s text=%s at %d:%d\n"
        scope
        kind
        (token_name tok)
        (String.escaped (token_to_string tok))
        pos.pos_lnum
        col)
  ;;

  let halstead_for_token_list ?(scope = "totals") token_list =
    let add_operator token hal =
      let name = token_to_string token.type_ in
      log_token ~scope ~kind:"operator" token;
      { hal with
        operators = hal.operators + 1
      ; unique_operators = StringSet.add name hal.unique_operators
      }
    in
    let add_operand token hal =
      let name = token_to_string token.type_ in
      log_token ~scope ~kind:"operand" token;
      { hal with
        operands = hal.operands + 1
      ; unique_operands = StringSet.add name hal.unique_operands
      }
    in
    List.fold_left
      (fun hal token ->
         let tok = token.type_ in
         if is_keyword tok || is_operator tok
         then add_operator token hal
         else if is_operand tok
         then add_operand token hal
         else if is_ignored tok
         then hal
         else (
           log_token ~scope ~kind:"undefined" token;
           hal))
      halstead_init
      token_list
  ;;

  let indent width = String.make width ' '

  let string_of_halstead_fields ~indent_width ~with_trailing_comma halstead_metrics =
    let fields =
      [ "unique_operators", StringSet.cardinal halstead_metrics.unique_operators
      ; "unique_operands", StringSet.cardinal halstead_metrics.unique_operands
      ; "operators", halstead_metrics.operators
      ; "operands", halstead_metrics.operands
      ]
    in
    let last_index = List.length fields - 1 in
    fields
    |> List.mapi (fun index (name, value) ->
      indent indent_width
      ^ "\""
      ^ name
      ^ "\": "
      ^ string_of_int value
      ^ if index = last_index && not with_trailing_comma then "\n" else ",\n")
    |> String.concat ""
  ;;

  let string_of_function_metrics ~is_last (func_metrics : function_metrics) =
    let line_end = if is_last then "\n" else ",\n" in
    indent 8
    ^ "{\n"
    ^ indent 12
    ^ "\"name\": \""
    ^ func_metrics.name
    ^ "\",\n"
    ^ indent 12
    ^ "\"metrics\": {\n"
    ^ string_of_halstead_fields
        ~indent_width:16
        ~with_trailing_comma:false
        func_metrics.metrics
    ^ indent 12
    ^ "}\n"
    ^ indent 8
    ^ "}"
    ^ line_end
  ;;

  let string_of_functions functions =
    match functions with
    | [] -> indent 4 ^ "\"functions\": []"
    | _ ->
      let last_index = List.length functions - 1 in
      indent 4
      ^ "\"functions\": [\n"
      ^ (functions
         |> List.mapi (fun index func_metrics ->
           string_of_function_metrics ~is_last:(index = last_index) func_metrics)
         |> String.concat "")
      ^ indent 4
      ^ "]"
  ;;

  let string_of_output output =
    "{\n"
    ^ indent 4
    ^ "\"totals\": {\n"
    ^ string_of_halstead_fields ~indent_width:8 ~with_trailing_comma:false output.totals
    ^ indent 4
    ^ "},\n"
    ^ string_of_functions output.functions
    ^ "\n}"
  ;;
end

let read_input () =
  let rec aux (acc : string list) =
    try
      let line = read_line () in
      aux (line :: acc)
    with
    | End_of_file -> List.rev acc
  in
  aux []
;;

let () =
  let input = read_input () in
  let halstead_info = Halstead.parse_input input in
  let tokens = Halstead.lex_file halstead_info in
  let totals = Halstead.halstead_for_token_list ~scope:"totals" tokens in
  let functions =
    List.map
      (fun func_def ->
         let func_tokens = Halstead.tokens_for_func_def func_def tokens in
         let metrics =
           Halstead.halstead_for_token_list
             ~scope:("function:" ^ func_def.name)
             func_tokens
         in
         { Halstead.name = func_def.name; metrics })
      halstead_info.functions
  in
  let output : Halstead.output = { totals; functions } in
  print_endline (Halstead.string_of_output output)
;;
