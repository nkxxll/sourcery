(* https://github.com/comby-tools/comby/blob/3b6bdff7bc3b50361b621da9db030152772e7e6b/lib/app/vendored/camlzip/zip.ml#L618-L645 *)
(* comby-tools/comby lib/app/vendored/camlzip/zip.ml:618-645 *)
let add_entry data ofile ?(extra = "") ?(comment = "")
    ?(level = 6) ?(mtime = Unix.time()) name =
  let e = add_entry_header ofile extra comment level mtime name in
  let crc = Zlib.update_crc_string Int32.zero data 0 (String.length data) in
  let compr_size =
    match level with
      0 ->
      output_substring ofile.of_channel data 0 (String.length data);
      String.length data
    | _ ->
      let in_pos = ref 0 in
      let out_pos = ref 0 in
      try
        Zlib.compress ~level ~header:false
          (fun buf ->
             let n = min (String.length data - !in_pos)
                 (Bytes.length buf) in
             String.blit data !in_pos buf 0 n;
             in_pos := !in_pos + n;
             n)
          (fun buf n ->
             output ofile.of_channel buf 0 n;
             out_pos := !out_pos + n);
        !out_pos
      with Zlib.Error(_, _) ->
        raise (Error("", "", "compression error")) in
  let e' = add_data_descriptor ofile crc compr_size (String.length data) e in
  ofile.of_entries <- e' :: ofile.of_entries

(* https://github.com/fastpack/fastpack/blob/173e0a412474baefd1f0f22597274730a74f475d/FastpackTest/Resolver.ml#L9-L34 *)
(* fastpack/fastpack FastpackTest/Resolver.ml:9-34 *)
let show resolved =
  let resolved =
    let (location, dependencies) = resolved in
    let location =
      match location with
      | Fastpack.Module.File { filename; preprocessors } ->
        let filename =
          match filename with
          | None -> None
          | Some filename -> Some (Test.cleanup_project_path filename)
        in
        let preprocessors =
          preprocessors
          |> List.map (
            fun (p, opt) ->
              Test.(cleanup_project_path p, cleanup_project_path opt)
          )
        in
        Fastpack.Module.File { filename; preprocessors }
      | _ -> location
    in
    (location, List.map Test.cleanup_project_path dependencies)
  in
  Format.(pp_set_margin str_formatter 60);
  pp_resolved Format.str_formatter resolved;
  Format.flush_str_formatter ()

(* https://github.com/camlworks/dream/blob/2ce65e1010f2501f9319e8735eec8e1eeb676d4e/src/server/log.ml#L315-L329 *)
(* camlworks/dream src/server/log.ml:315-329 *)
  let forward ~(destination_log : _ Logs.log) user's_k =
    let `Initialized = initialized () in

    destination_log (fun log ->
      user's_k (fun ?request format_and_arguments ->
        let tags =
          match request with
          | None -> Logs.Tag.empty
          | Some request ->
            match get_request_id ~request () with
            | None -> Logs.Tag.empty
            | Some request_id ->
              Logs.Tag.add logs_lib_tag request_id Logs.Tag.empty
        in
        log ~tags format_and_arguments))

(* https://github.com/batsh-dev-team/Batsh/blob/e38abb082deb38fc2942248a4fd855a79d2a35ca/src/semantic_checker.ml#L17-L26 *)
(* batsh-dev-team/Batsh src/semantic_checker.ml:17-26 *)
let check_toplevel (topl : toplevel) =
  match topl with
  | Statement (Global _) ->
    raise (Error "qualifier 'global' must be used in a function")
  | Statement (Return _) ->
    raise (Error "statement 'return' must be used in a function")
  | Statement _ ->
    ()
  | Function func ->
    check_function func

(* https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/command/src/command.ml#L272-L284 *)
(* janestreet/core command/src/command.ml:272-284 *)
    let complete univ_map ~part:prefix =
      match auto_complete with
      | Some complete -> complete univ_map ~part:prefix
      | None ->
        List.filter_map (Map.to_alist map) ~f:(fun (name, _) ->
          match S.is_prefix name ~prefix with
          | false -> None
          | true ->
            (* Bash completion will not accept [Foo] as a completion for [f]. So we need
               to match the capitalization given. *)
            let suffix = String.subo name ~pos:(String.length prefix) in
            let name = prefix ^ suffix in
            Some name)
