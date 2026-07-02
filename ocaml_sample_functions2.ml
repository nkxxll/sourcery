(* https://github.com/comby-tools/comby/blob/3b6bdff7bc3b50361b621da9db030152772e7e6b/lib/kernel/matchers/evaluate.ml#L42-L43 *)
(* comby-tools/comby lib/kernel/matchers/evaluate.ml:42-43 *)
  let rewrite_substitute template env =
    Rewrite.substitute ~metasyntax ~external_handler ?filepath template env

(* https://github.com/fastpack/fastpack/blob/173e0a412474baefd1f0f22597274730a74f475d/FastpackTest/Watch.ml#L116-L121 *)
(* fastpack/fastpack FastpackTest/Watch.ml:116-121 *)
              let change_and_rebuild ~actions r =
                let%lwt filesChanged = change_files actions in
                let%lwt () = Lwt_io.flush_all () in
                let%lwt () = Lwt_unix.sleep pause in
                Builder.rebuild ~filesChanged ~prevResult:r builder
                >>= saveResult (Some r)

(* https://github.com/camlworks/dream/blob/2ce65e1010f2501f9319e8735eec8e1eeb676d4e/src/mirage/mirage.ml#L19 *)
(* camlworks/dream src/mirage/mirage.ml:19-19 *)
let to_dream_method meth = H1.Method.to_string meth |> Method.string_to_method

(* https://github.com/batsh-dev-team/Batsh/blob/e38abb082deb38fc2942248a4fd855a79d2a35ca/src/formatutil.ml#L3-L4 *)
(* batsh-dev-team/Batsh src/formatutil.ml:3-4 *)
let print_indent (buf : Buffer.t) (indent : int) =
  Buffer.add_string buf (String.make indent ' ')

(* https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/core/src/span_ns.ml#L176 *)
(* janestreet/core core/src/span_ns.ml:176-176 *)
let[@zero_alloc] scale t f = round_nearest_ns (float t *. f)

(* https://github.com/bcpierce00/unison/blob/273ec6298cd7f97f038ea8667701691c29b1b48f/src/remote.ml#L2154 *)
(* bcpierce00/unison src/remote.ml:2154-2154 *)
  let reply msg = sendHandshakeMsg conn msg in

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/plugins/powerpc/powerpc_compare.ml#L19-L28 *)
(* BinaryAnalysisPlatform/bap plugins/powerpc/powerpc_compare.ml:19-28 *)
let cmpdi cpu ops =
  let bf = unsigned cpu.reg ops.(0) in
  let ra = signed cpu.reg ops.(1) in
  let si = signed imm ops.(2) in
  RTL.[
    nth bit bf 0 := ra < si;
    nth bit bf 1 := ra > si;
    nth bit bf 2 := ra = si;
    nth bit bf 3 := cpu.so;
  ]

(* https://github.com/airbus-seclab/bincat/blob/5d0ee3b56867059427eb0f4123c4d9de0b8059dd/ocaml/src/disassembly/x86Imports.ml#L141-L143 *)
(* airbus-seclab/bincat ocaml/src/disassembly/x86Imports.ml:141-143 *)
  let init () =
    Stubs.init ();
    init_imports ()

(* https://github.com/austral/austral/blob/0962d2a8a5d77f7daacd7f696819520733f4897d/lib/CodeGen.ml#L657-L659 *)
(* austral/austral lib/CodeGen.ml:657-659 *)
and gen_method_decl mn (MConcreteMethod (id, _, params, rt, _)) =
  let d = Desc "Method forward declaration" in
  CFunctionDeclaration (d, gen_ins_meth_id id, gen_params mn params, gen_type rt, LinkageInternal)

(* https://github.com/astrada/google-drive-ocamlfuse/blob/6076cd802709191528a227a9e5e0823f2c708ec8/src/dbCache.ml#L151-L153 *)
(* astrada/google-drive-ocamlfuse src/dbCache.ml:151-153 *)
  let prepare_update_state_stmt db =
    let sql = "UPDATE resource SET state = :state WHERE id = :id;" in
    Sqlite3.prepare db sql

(* https://github.com/comby-tools/comby/blob/3b6bdff7bc3b50361b621da9db030152772e7e6b/lib/app/vendored/patdiff/kernel/src/format.ml#L216-L217 *)
(* comby-tools/comby lib/app/vendored/patdiff/kernel/src/format.ml:216-217 *)
  let omake_style_error_message_start ~file ~line =
    sprintf "File \"%s\", line %d, characters 0-1:" file line

(* https://github.com/fastpack/fastpack/blob/173e0a412474baefd1f0f22597274730a74f475d/FastpackTest/Test.ml#L34-L37 *)
(* fastpack/fastpack FastpackTest/Test.ml:34-37 *)
let print ~with_scope source =
  let program, _ = FastpackUtil.Parser.parse_source source in
  let result = FastpackUtil.Printer.print ~with_scope program in
  result

(* https://github.com/camlworks/dream/blob/2ce65e1010f2501f9319e8735eec8e1eeb676d4e/test/expect/pure/formats/query/query.ml#L66-L71 *)
(* camlworks/dream test/expect/pure/formats/query/query.ml:66-71 *)
let all_queries string =
  Dream.request ~target:("/?" ^ string) ""
  |> Dream.all_queries
  |> List.map (fun (name, value) -> Printf.sprintf "%S=%S" name value)
  |> String.concat " "
  |> Printf.printf "[%s]\n"

(* https://github.com/batsh-dev-team/Batsh/blob/e38abb082deb38fc2942248a4fd855a79d2a35ca/src/bash_functions.ml#L84-L86 *)
(* batsh-dev-team/Batsh src/bash_functions.ml:84-86 *)
let expand_function ((name : identifier), (stmts : statements))
  : (identifier * statements) =
  (name, expand_statements stmts)

(* https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/core/test/test_blang.ml#L228 *)
(* janestreet/core core/test/test_blang.ml:228-228 *)
    let tg () = t (), g ()

(* https://github.com/bcpierce00/unison/blob/273ec6298cd7f97f038ea8667701691c29b1b48f/src/fspath.ml#L38 *)
(* bcpierce00/unison src/fspath.ml:38-38 *)
let toString (Fspath f) = f

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/lib/bap_c/bap_c_type.ml#L222-L227 *)
(* BinaryAnalysisPlatform/bap lib/bap_c/bap_c_type.ml:222-227 *)
let pointer ?(attrs=[]) ?const ?volatile ?(restrict=false) t : t =
  `Pointer {
    t;
    attrs;
    qualifier = qualifier ?const ?volatile restrict;
  }

(* https://github.com/airbus-seclab/bincat/blob/5d0ee3b56867059427eb0f4123c4d9de0b8059dd/ocaml/src/npk/newspeak/npkcontext.ml#L297 *)
(* airbus-seclab/bincat ocaml/src/npk/newspeak/npkcontext.ml:297-297 *)
let forget_loc () = cur_loc := Newspeak.unknown_loc

(* https://github.com/austral/austral/blob/0962d2a8a5d77f7daacd7f696819520733f4897d/lib/TypeErrors.ml#L303-L310 *)
(* austral/austral lib/TypeErrors.ml:303-310 *)
let path_not_public ~type_name ~slot_name =
  austral_raise TypeError [
    Text "The slot ";
    Code (ident_string slot_name);
    Text " is not publically visible in the type ";
    Code (ident_string type_name);
    Text " and so cannot be read."
  ]

(* https://github.com/astrada/google-drive-ocamlfuse/blob/6076cd802709191528a227a9e5e0823f2c708ec8/src/dbCache.ml#L462-L469 *)
(* astrada/google-drive-ocamlfuse src/dbCache.ml:462-469 *)
  let update_resource_state cache state id =
    with_transaction cache (fun db ->
        let stmt = ResourceStmts.prepare_update_state_stmt db in
        bind_text stmt ":state"
          (Some (CacheData.Resource.State.to_string state));
        bind_int stmt ":id" (Some id);
        final_step stmt;
        finalize_stmt stmt)
