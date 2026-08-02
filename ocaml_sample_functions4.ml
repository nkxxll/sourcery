(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/lib/monads/monads_monad.ml#L204-L208 *)
(* BinaryAnalysisPlatform/bap lib/monads/monads_monad.ml:204-208 *)
        let count xs ~f =
          fold xs ~init:0 ~f:(fun n x ->
              f x >>| function
              | true -> n+1
              | false -> n)

(* https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/core/test/test_union_find.ml#L27-L29 *)
(* janestreet/core core/test/test_union_find.ml:27-29 *)
let set t x =
  set t x;
  assert (is_compressed t)

(* https://github.com/airbus-seclab/bincat/blob/5d0ee3b56867059427eb0f4123c4d9de0b8059dd/ocaml/src/loaders/elf_core.ml#L373-L377 *)
(* airbus-seclab/bincat ocaml/src/loaders/elf_core.ml:373-377 *)
let to_p_type x =
  match (Z.to_int x) with
  | 0 -> PT_NULL  | 1 -> PT_LOAD   | 2 -> PT_DYNAMIC  | 3 -> PT_INTERP
  | 4 -> PT_NOTE  | 5 -> PT_SHLIB  | 6 -> PT_PHDR
  | _ -> PT_OTHER x

(* https://github.com/bcpierce00/unison/blob/273ec6298cd7f97f038ea8667701691c29b1b48f/src/remote.ml#L596-L597 *)
(* bcpierce00/unison src/remote.ml:596-597 *)
  let ofRootConncheck root =
    withConncheck (fun () -> findByRoot root)

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/lib/ogre/ogre.ml#L384-L385 *)
(* BinaryAnalysisPlatform/bap lib/ogre/ogre.ml:384-385 *)
    let pp_typ ppf typ =
      pp_print_string ppf (Type.string_of_typ typ)

(* https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/core/src/bigbuffer.ml#L137-L143 *)
(* janestreet/core core/src/bigbuffer.ml:137-143 *)
let add_buffer buf_dst buf_src =
  let len = buf_src.pos in
  let dst_pos = buf_dst.pos in
  let new_pos = dst_pos + len in
  if new_pos > buf_dst.len then resize buf_dst len;
  Bigstring.blito ~src:buf_src.bstr ~src_len:len ~dst:buf_dst.bstr ~dst_pos ();
  buf_dst.pos <- new_pos

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/plugins/api/api_main.ml#L219 *)
(* BinaryAnalysisPlatform/bap plugins/api/api_main.ml:219-219 *)
let rem_files descrs paths = List.iter ~f:(rem_api paths) descrs

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/lib/bap_image/bap_table.ml#L245-L246 *)
(* BinaryAnalysisPlatform/bap lib/bap_image/bap_table.ml:245-246 *)
let exists ?start ?until tab ~f =
  existsi ?start ?until tab ~f:(fun _ x -> f x)

(* https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/core/src/date_cache.ml#L322-L329 *)
(* janestreet/core core/src/date_cache.ml:322-329 *)
  let reset () =
    with_cache
      ~may_read_without_writing:(fun [@inline] _ -> false)
      ~read:(fun [@inline] _ -> ())
      ~write:reset_cache
      ~date_cache:(get_date_cache ())
      ~time:()
      ~zone:()

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/lib/bap_strings/bap_strings_detector.ml#L51-L52 *)
(* BinaryAnalysisPlatform/bap lib/bap_strings/bap_strings_detector.ml:51-52 *)
let hprop {p1; w0; w1; h={n;m}} =
  float m *. w0 +. float n *. w1 +. p1
