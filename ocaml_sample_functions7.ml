(* https://github.com/comby-tools/comby/blob/3b6bdff7bc3b50361b621da9db030152772e7e6b/lib/kernel/matchers/regexp.ml#L96 *)
(* comby-tools/comby lib/kernel/matchers/regexp.ml:96-96 *)
    let make pattern = Re.Perl.(compile (re ~opts:compile_flags pattern))

(* https://github.com/fastpack/fastpack/blob/173e0a412474baefd1f0f22597274730a74f475d/FastpackTest/Test.ml#L34-L37 *)
(* fastpack/fastpack FastpackTest/Test.ml:34-37 *)
let print ~with_scope source =
  let program, _ = FastpackUtil.Parser.parse_source source in
  let result = FastpackUtil.Printer.print ~with_scope program in
  result

(* https://github.com/camlworks/dream/blob/2ce65e1010f2501f9319e8735eec8e1eeb676d4e/src/server/helpers.ml#L67-L68 *)
(* camlworks/dream src/server/helpers.ml:67-68 *)
let respond ?status ?code ?headers body =
  Lwt.return (response_with_body ?status ?code ?headers body)

(* https://github.com/batsh-dev-team/Batsh/blob/e38abb082deb38fc2942248a4fd855a79d2a35ca/src/bash.ml#L7-L10 *)
(* batsh-dev-team/Batsh src/bash.ml:7-10 *)
let compile (batsh : Parser.t) : t =
  let bash_ast = Bash_compile.compile batsh in
  let bash_ast_expanded = Bash_functions.expand bash_ast in
  {batsh; bash_ast; bash_ast_expanded}

(* https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/core/test/test_union_find.ml#L27-L29 *)
(* janestreet/core core/test/test_union_find.ml:27-29 *)
let set t x =
  set t x;
  assert (is_compressed t)

(* https://github.com/bcpierce00/unison/blob/273ec6298cd7f97f038ea8667701691c29b1b48f/src/remote.ml#L596-L597 *)
(* bcpierce00/unison src/remote.ml:596-597 *)
  let ofRootConncheck root =
    withConncheck (fun () -> findByRoot root)

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/lib/monads/monads_monad.ml#L204-L208 *)
(* BinaryAnalysisPlatform/bap lib/monads/monads_monad.ml:204-208 *)
        let count xs ~f =
          fold xs ~init:0 ~f:(fun n x ->
              f x >>| function
              | true -> n+1
              | false -> n)

(* https://github.com/airbus-seclab/bincat/blob/5d0ee3b56867059427eb0f4123c4d9de0b8059dd/ocaml/src/loaders/elf_core.ml#L373-L377 *)
(* airbus-seclab/bincat ocaml/src/loaders/elf_core.ml:373-377 *)
let to_p_type x =
  match (Z.to_int x) with
  | 0 -> PT_NULL  | 1 -> PT_LOAD   | 2 -> PT_DYNAMIC  | 3 -> PT_INTERP
  | 4 -> PT_NOTE  | 5 -> PT_SHLIB  | 6 -> PT_PHDR
  | _ -> PT_OTHER x

(* https://github.com/austral/austral/blob/0962d2a8a5d77f7daacd7f696819520733f4897d/lib/TypeClasses.ml#L32-L35 *)
(* austral/austral lib/TypeClasses.ml:32-35 *)
  let leftovers () =
    austral_raise DeclarationError [
      Text "The number of type parameters in the instance declaration must be the same as the number of type variables applied to the argument."
    ]

(* https://github.com/astrada/google-drive-ocamlfuse/blob/6076cd802709191528a227a9e5e0823f2c708ec8/src/drive.ml#L2170-L2174 *)
(* astrada/google-drive-ocamlfuse src/drive.ml:2170-2174 *)
  let remote_move ~custom_headers ~addParents ~fileId ~removeParents file =
    with_retry_default
      (FilesResource.update ~enforceSingleParent:true ~supportsAllDrives:true
         ~std_params:file_std_params ~custom_headers ~addParents ~fileId
         ~removeParents file)
