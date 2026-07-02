(* https://github.com/comby-tools/comby/blob/3b6bdff7bc3b50361b621da9db030152772e7e6b/test/common/test_integration.ml#L9-L12 *)
(* comby-tools/comby test/common/test_integration.ml:9-12 *)
let rewrite_all template source rewrite_template =
  all template source
  |> (fun matches -> Option.value_exn (Rewrite.all ~source ~rewrite_template matches))
  |> fun { rewritten_source; _ } -> rewritten_source

(* https://github.com/fastpack/fastpack/blob/173e0a412474baefd1f0f22597274730a74f475d/FastpackTest/Test.ml#L34-L37 *)
(* fastpack/fastpack FastpackTest/Test.ml:34-37 *)
let print ~with_scope source =
  let program, _ = FastpackUtil.Parser.parse_source source in
  let result = FastpackUtil.Printer.print ~with_scope program in
  result

(* https://github.com/camlworks/dream/blob/2ce65e1010f2501f9319e8735eec8e1eeb676d4e/src/graphql/graphql.ml#L81-L84 *)
(* camlworks/dream src/graphql/graphql.ml:81-84 *)
let close_and_clean ?code subscriptions websocket =
  let%lwt () = Message.close_websocket ?code websocket in
  Hashtbl.iter (fun _ close -> close ()) subscriptions;
  Lwt.return_unit

(* https://github.com/batsh-dev-team/Batsh/blob/e38abb082deb38fc2942248a4fd855a79d2a35ca/src/bash.ml#L12-L15 *)
(* batsh-dev-team/Batsh src/bash.ml:12-15 *)
let print (bash : t) : string =
  let buf = Buffer.create 1024 in
  Bash_format.print buf bash.bash_ast_expanded;
  Buffer.contents buf

(* https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/core/src/float.ml#L118-L121 *)
(* janestreet/core core/src/float.ml:118-121 *)
  let validate_negative t =
    (Validate.first_failure [@mode p])
      (validate_not_nan t)
      ((ZZ.validate_negative [@mode p]) t)

(* https://github.com/bcpierce00/unison/blob/273ec6298cd7f97f038ea8667701691c29b1b48f/src/sortri.ml#L94-L97 *)
(* bcpierce00/unison src/sortri.ml:94-97 *)
let sortBySize () =
  saveSortingPrefs();
  zeroSortingPrefs();
  Prefs.set bysize true

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/plugins/disassemble/disassemble_main.ml#L436-L439 *)
(* BinaryAnalysisPlatform/bap plugins/disassemble/disassemble_main.ml:436-439 *)
  let parse_fmt fmt =
    match String.split ~on:'-' fmt with
    | [fmt;ver] -> fmt, Some ver
    | _ -> fmt,None

(* https://github.com/airbus-seclab/bincat/blob/5d0ee3b56867059427eb0f4123c4d9de0b8059dd/ocaml/src/utils/dump.ml#L16-L19 *)
(* airbus-seclab/bincat ocaml/src/utils/dump.ml:16-19 *)
let string_of_src src =
  match src with
  | R r -> "r-"^(Register.name r)
  | M (a, sz) -> "M("^(Data.Address.to_string a)^","^(string_of_int sz)^")"

(* https://github.com/austral/austral/blob/0962d2a8a5d77f7daacd7f696819520733f4897d/lib/TypeClasses.ml#L37-L40 *)
(* austral/austral lib/TypeClasses.ml:37-40 *)
  let lone_tyvar () =
    austral_raise DeclarationError [
      Text "Typeclass arguments cannot be lone type parameters."
    ]

(* https://github.com/astrada/google-drive-ocamlfuse/blob/6076cd802709191528a227a9e5e0823f2c708ec8/bin/gdfuseFuse.ml#L70-L73 *)
(* astrada/google-drive-ocamlfuse bin/gdfuseFuse.ml:70-73 *)
let utime path atime mtime =
  Utils.log_with_header "utime %s %f %f\n%!" path atime mtime;
  with_drive_op ~label:"utime" ~param:path (fun () ->
      Drive.utime path atime mtime)

(* https://github.com/comby-tools/comby/blob/3b6bdff7bc3b50361b621da9db030152772e7e6b/lib/app/vendored/patdiff/lib/src/configuration.ml#L367-L370 *)
(* comby-tools/comby lib/app/vendored/patdiff/lib/src/configuration.ml:367-370 *)
  let create_header h_opt prefix =
    On_disk.Header.to_internal
      (Option.value ~default:On_disk.Rule.blank h_opt)
      ~default:prefix

(* https://github.com/camlworks/dream/blob/2ce65e1010f2501f9319e8735eec8e1eeb676d4e/src/server/router.ml#L100-L103 *)
(* camlworks/dream src/server/router.ml:100-103 *)
let method_matches method_set method_ =
  match method_set with
  | #Method.method_ as method' -> Method.methods_equal method' method_
  | `Any -> true

(* https://github.com/batsh-dev-team/Batsh/blob/e38abb082deb38fc2942248a4fd855a79d2a35ca/src/bash_compile.ml#L241-L244 *)
(* batsh-dev-team/Batsh src/bash_compile.ml:241-244 *)
let compile (batsh : Parser.t) : t =
  let symtable = Parser.symtable batsh in
  let program = Bash_transform.split (Parser.ast batsh) ~symtable in
  List.map program ~f: (compile_toplevel ~symtable)

(* https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/core/src/float_with_finite_only_serialization.ml#L29-L32 *)
(* janestreet/core core/src/float_with_finite_only_serialization.ml:29-32 *)
          let%template to_binable t =
            verify t;
            t
          [@@mode m = (global, local)]

(* https://github.com/bcpierce00/unison/blob/273ec6298cd7f97f038ea8667701691c29b1b48f/src/uigtk3.ml#L976-L979 *)
(* bcpierce00/unison src/uigtk3.ml:976-979 *)
let getPassword passwordDialog rootName msg response =
  match !passwordDialog with
  | Some { labelAppend; _ } -> labelAppend msg
  | None -> createPasswordDialog passwordDialog rootName msg response

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/lib/knowledge/bap_knowledge.ml#L2594-L2597 *)
(* BinaryAnalysisPlatform/bap lib/knowledge/bap_knowledge.ml:2594-2597 *)
      let is_public {package} obj {Env.pubs} =
        match Map.find pubs package with
        | None -> false
        | Some pubs -> Set.mem pubs obj in

(* https://github.com/airbus-seclab/bincat/blob/5d0ee3b56867059427eb0f4123c4d9de0b8059dd/ocaml/src/npk/newspeak/lowspeak.ml#L566-L569 *)
(* airbus-seclab/bincat ocaml/src/npk/newspeak/lowspeak.ml:566-569 *)
let visit visitor prog =
  Hashtbl.iter (visit_glb visitor) prog.globals;
  visit_blk visitor prog.init;
  Hashtbl.iter (visit_fun visitor) prog.fundecs

(* https://github.com/austral/austral/blob/0962d2a8a5d77f7daacd7f696819520733f4897d/lib/Env.ml#L629-L632 *)
(* austral/austral lib/Env.ml:629-632 *)
let add_exported_function (env: env) (id: decl_id) (export_name: string): env =
  let (Env { files; mods; methods; decls; monos; exports }) = env in
  let env = Env { files; mods; methods; decls; monos; exports = (id, export_name) :: exports } in
  env

(* https://github.com/astrada/google-drive-ocamlfuse/blob/6076cd802709191528a227a9e5e0823f2c708ec8/src/cache.ml#L58-L61 *)
(* astrada/google-drive-ocamlfuse src/cache.ml:58-61 *)
  let invalidate_path cache path =
    if cache.CacheData.in_memory then
      MemoryCache.Resource.invalidate_path cache path
    else DbCache.Resource.invalidate_path cache path

(* https://github.com/comby-tools/comby/blob/3b6bdff7bc3b50361b621da9db030152772e7e6b/lib/app/vendored/patdiff/kernel/src/output.ml#L11-L14 *)
(* comby-tools/comby lib/app/vendored/patdiff/kernel/src/output.ml:11-14 *)
let implies_unrefined t =
  match t with
  | Ansi | Html -> false
  | Ascii -> true
