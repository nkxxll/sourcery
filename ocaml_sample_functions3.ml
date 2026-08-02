(* https://github.com/bcpierce00/unison/blob/273ec6298cd7f97f038ea8667701691c29b1b48f/src/test.ml#L250-L912 *)
(* bcpierce00/unison src/test.ml:250-912 *)
let test() =
  Util.warnPrinter := None;
  Prefs.set Trace.logging false;
  Prefs.set Trace.terse true;
  Trace.sendLogMsgsToStderr := false;

  let origPrefs = Prefs.dump 1 in

  let runtest name prefs f =
    Util.msg "%s...\n" name;
    Util.convertUnixErrorsToFatal "Test.test" (fun() ->
      currentTest := name;
      Prefs.load origPrefs 1;
      loadPrefs prefs;
      debug (fun() -> Util.msg "Emptying backup directory\n");
      Lwt_unix.run (Globals.allRootsIter (fun r -> makeBackupEmpty r ()));
      debug (fun() -> Util.msg "Running test\n");
      f();
    ) in

  Util.msg "Running internal tests...\n";

  (* Paranoid checks, to make sure we do not delete anybody's filesystem! *)
  if not (Safelist.for_all
            (fun r -> Util.findsubstring "test" r <> None)
            (Globals.rawRoots())) then
    raise (Util.Fatal
      "Self-tests can only be run if both roots include the string 'test'");
  if Util.findsubstring "test" (Fspath.toPrintString (Stasher.backupDirectory())) = None then
    raise (Util.Fatal
        ("Self-tests can only be run if the 'backupdir' preference (or wherever the backup "
       ^ "directory name is coming from, e.g. the UNISONBACKUPDIR environment variable) "
       ^ "includes the string 'test'"));

  Lwt_unix.run (Globals.allRootsIter (fun r -> makeRootEmpty r ()));

  let (r2,r1) = Globals.roots () in
  let (rr2, rr1) = Globals.rawRootPair () in
  (* Util.msg "r1 = %s  r2 = %s...\n" (Common.root2string r1) (Common.root2string r2); *)
  let bothRootsLocal =
    match (r1,r2) with
      (Common.Local,_),(Common.Local,_) -> true
    | _ -> false in

  let put c fs =
    Lwt_unix.run
      (match c with
        R1 -> putfs r1 fs | R2 -> putfs r2 fs | BACKUP1 | BACKUP2 -> assert false) in

  let failures = ref 0 in

  let check name c fs =
    debug (fun() -> Util.msg "Checking %s / %s\n" (!currentTest) name);
    let actual =
        Lwt_unix.run
          ((match c with
            R1 -> getfs r1 | R2 -> getfs r2 | BACKUP1 -> getbackup r1 | BACKUP2 -> getbackup r2) ())  in
    let fail () =
      Util.msg
        "Test %s / %s: \nExpected %s = \n  %s\nbut found\n  %s\n"
        (!currentTest) name (checkable2string c) (fs2string fs) (fsopt2string actual);
      failures := !failures+1;
      raise (Util.Fatal (Printf.sprintf "Self-test %s / %s failed!" (!currentTest) name)) in
    match actual with
        Some(a) -> if not (equal a fs) then fail()
      | None -> fail() in

  let checkmissing name c =
    debug (fun() -> Util.msg "Checking nonexistence %s / %s\n" (!currentTest) name);
    let actual =
      Lwt_unix.run
        ((match c with
          R1 -> getfs r1 | R2 -> getfs r2 | BACKUP1 -> getbackup r1 | BACKUP2 -> getbackup r2) ()) in
    if  actual <> None then begin
      Util.msg
        "Test %s / %s: \nExpected %s MISSING\nbut found\n  %s\n"
        (!currentTest) name (checkable2string c) (fsopt2string actual);
      failures := !failures+1;
      raise (Util.Fatal (Printf.sprintf "Self-test %s / %s failed!" (!currentTest) name))
    end in

  let check_assert name thunk =
    debug (fun () -> Util.msg "Checking %s / %s\n" (!currentTest) name);
    let fail s =
      Util.msg "Test %s / %s: \n%s\n" (!currentTest) name s;
      incr failures;
      raise (Util.Fatal (Printf.sprintf "Self-test %s / %s failed!" (!currentTest) name)) in
    match thunk () with
    | None -> ()
    | Some err -> fail err in

  let assert_n_ris n err () =
    match Recon.reconcileAll (Update.findUpdates None) with
    | (ris, _, _) when List.compare_length_with ris n = 0 -> None
    | (ris, _, _) -> debug (fun () -> displayRis ris); Some err
  in

  let assert_eq a b err () = if a = b then None else Some err in

  (* N.b.: When making up tests, it's important to choose file contents of different
     lengths.  The reason for this is that, on some Unix systems, it is possible for
     the inode number of a just-deleted file to be reassigned to the very next file
     created -- i.e., to the updated version of the file that the test script has
     just written.  If the length of the contents is also the same and the test is
     running fast enough that the whole thing happens within a second, then the
     update will be missed! *)

  (* Test that update propagation transport works *)
  let maxth = [| "0"; "1"; "5"; "6"; "7" |] in
    (* Number of threads: default (0); 1 (corner case);
       one less, equal to, and one more than number of updates *)
  for i = 1 to Array.length maxth do
    runtest ("propagation 1." ^ string_of_int i) ["maxthreads = " ^ maxth.(i - 1)] (fun () ->
      put R1 (Dir []); put R2 (Dir []); sync ();
      let r1 = ["a", File "a"; "b", File "b"; "d1", Dir ["a", File "a1"; "b", File "b1"]]
      and r2 = ["x", File "x"; "y", File "y"; "d2", Dir ["x", File "x2"; "y", File "y2"]] in
      let expect = Dir (r1 @ r2) in
      put R1 (Dir r1); put R2 (Dir r2); sync ();
      check "1" R1 expect;
      check "2" R2 expect;
      (* File->file update, file->dir update, subdir update, dir->file update, delete *)
      let r1 = ["a", File "au"; "b", Dir []; "d1", Dir ["a", File "a1u"; "b", File "b1"]; "d2", File "du"] in
      let expect = Dir r1 in
      put R1 (Dir r1); sync ();
      check "3" R1 expect;
      check "4" R2 expect;
      let r2 = ["a", File "au2"; "b", File "bu2"; "d1", Dir ["a", File "a1u2"]; "d2", Dir ["z", File "z"]] in
      let expect = Dir r2 in
      put R2 (Dir r2); sync ();
      check "5" R1 expect;
      check "6" R2 expect
    )
  done;

  (* Test that .git is treated atomically. *)
  runtest "Atomicity of certain directories 1" ["atomic = Name .git";
                                                "force = newer"] (fun() ->
      let orig = (Dir ["foo", Dir [".git", Dir ["a", File "foo";
                                                "b", File "bar";
                                                "c", File "baz"]]]) in
      put R1 orig;
      Unix.sleep 2; (* in case time granularity is coarse on this FS *)
      put R2 orig; sync();
      let expected = (Dir ["foo", Dir [".git", Dir ["a", File "modified on R1";
                                                    "b", File "bar";
                                                    "c", File "modified on R1"]]]) in
      put R2 (Dir ["foo", Dir [".git",
                               Dir ["a", File "foo";
                                    "b", File "modified on R2";
                                    "c", File "modified on R2"]]]);
      Unix.sleep 2; 
      put R1 expected;
      sync ();
      check "1" R2 expected;
      check "2" R1 expected
    );

  runtest "Atomicity of certain directories 2" ["atomic = Name .git"] (fun() ->
      let a = (Dir ["foo", Dir [".git", Dir ["a", File "foo";
                                             "b", File "bar";
                                             "c", File "baz";
                                             "d", File "quux"]]]) in
      let b = (Dir ["foo", Dir [".git", Dir ["a", File "foo";
                                             "b", File "bar";
                                             "c", File "baz";
                                             "e", File "quux"]]]) in
      put R1 a; put R2 b; sync();
      check "1" R1 a;
      check "2" R2 b
    );

  (* Check for the bug reported by Ralf Lehmann *)
  if not bothRootsLocal then
    runtest "backups 1 (remote)" ["backup = Name *"] (fun() ->
      put R1 (Dir []); put R2 (Dir []); sync();
      debug (fun () -> Util.msg "First check\n");
      checkmissing "1" BACKUP1;
      checkmissing "2" BACKUP2;
      (* Create a file *)
      put R1 (Dir ["test.txt", File "1"]); sync();
      checkmissing "3" BACKUP1;
      checkmissing "4" BACKUP2;
      (* Change it and check that the old version got backed up on the target host *)
      put R1 (Dir ["test.txt", File "2"]); sync();
      checkmissing "5" BACKUP1;
      check "6" BACKUP2 (Dir [("test.txt", File "1")]);
    );

  if bothRootsLocal then
    runtest "fastercheckUNSAFE 1" ["fastercheckUNSAFE = true"] (fun() ->
      put R1 (Dir []); put R2 (Dir []); sync();
      (* Create a file on both sides with different contents *)
      put R1 (Dir ["x", File "foo"]);
      put R2 (Dir ["x", File "bar"]); sync();
      check "1a" R1 (Dir ["x", File "foo"]);
      check "1b" R2 (Dir ["x", File "bar"]);
      (* Change contents on one side and see that we do NOT get a conflict (!) *)
      put R1 (Dir ["x", File "newcontents"]); sync();
      check "2a" R1 (Dir ["x", File "newcontents"]);
      check "2b" R2 (Dir ["x", File "newcontents"]);

      (* Start again *)
      put R1 (Dir []); put R2 (Dir []); sync();
      (* Create a file on both sides with different contents *)
      put R1 (Dir ["x", File "foo"]);
      put R2 (Dir ["x", File "bar"]); sync();
      (* Change contents without changing size and check that change is propagated *)
      put R1 (Dir ["x", File "f00"]); sync();

      check "3a" R1 (Dir ["x", File "f00"]);
      check "3b" R2 (Dir ["x", File "f00"]);

      (* Start again *)
      put R1 (Dir []); put R2 (Dir []); sync();
      (* Create a new file on one side only *)
      put R1 (Dir ["x", File "foo"]); sync();
      (* Check that change is propagated *)
      check "4" R2 (Dir ["x", File "foo"]);
    );

  if bothRootsLocal then
    runtest "backups 1 (local)" ["backup = Name *"] (fun() ->
      put R1 (Dir []); put R2 (Dir []); sync();
      (* Create a file and a directory *)
      put R1 (Dir ["x", File "foo"; "d", Dir ["a", File "barr"]]); sync();
      (* Delete them *)
      put R1 (Dir []); sync();
      check "1" BACKUP1 (Dir ["x", File "foo"; "d", Dir ["a", File "barr"]]);
      (* Put them back and delete them once more *)
      put R1 (Dir ["x", File "FOO"; "d", Dir ["a", File "BARR"]]); sync();
      put R1 (Dir []); sync();
      check "2" BACKUP1 (Dir [("x", File "FOO"); ("d", Dir [("a", File "BARR")]);
                              (".bak.1.x", File "foo"); (".bak.1.d", Dir [("a", File "barr")])])
    );

  runtest "backups 2" ["backup = Name *"; "backuplocation = local"] (fun() ->
    put R1 (Dir []); put R2 (Dir []); sync();
    (* Create a file and a directory *)
    put R1 (Dir ["x", File "foo"; "d", Dir ["a", File "barr"]]); sync();
    (* Delete them *)
    put R1 (Dir []); sync();
    (* Check that they have been backed up correctly on the other side *)
    check "1" R2 (Dir [(".bak.0.x", File "foo"); (".bak.0.d", Dir [("a", File "barr")])]);
  );

  runtest "backups 2a" ["backup = Name *"; "backuplocation = local"] (fun() ->
    put R1 (Dir []); put R2 (Dir []); sync();
    (* Create a file and a directory *)
    put R1 (Dir ["foo", File "1"]); sync();
    check "1" R1 (Dir [("foo", File "1")]);
    check "2" R2 (Dir [("foo", File "1")]);
    put R1 (Dir ["foo", File "2"]); sync();
    check "3" R1 (Dir [("foo", File "2")]);
    check "4" R2 (Dir [("foo", File "2"); (".bak.0.foo", File "1")]);
  );

  runtest "backups 3" ["backup = Name *"; "backuplocation = local"; "backupcurrent = Name *"] (fun() ->
    put R1 (Dir []); put R2 (Dir []); sync();
    put R1 (Dir ["x", File "foo"]); sync ();
    check "1a" R1 (Dir [("x", File "foo"); (".bak.0.x", File "foo")]);
    check "1b" R2 (Dir [("x", File "foo"); (".bak.0.x", File "foo")]);
    put R2 (Dir ["x", File "barr"; (".bak.0.x", File "foo")]); sync ();
    check "2a" R1 (Dir [("x", File "barr"); (".bak.1.x", File "foo"); (".bak.0.x", File "barr")]);
    check "2b" R2 (Dir [("x", File "barr"); (".bak.1.x", File "foo"); (".bak.0.x", File "barr")]);
  );

  runtest "backups 4" ["backup = Name *"; "backupcurrent = Name *"; "maxbackups = 7"] (fun() ->
    put R1 (Dir []); put R2 (Dir []); sync();
    put R1 (Dir ["x", File "foo"]); sync();
    check "1a" BACKUP1 (Dir [("x", File "foo")]);
    put R1 (Dir ["x", File "barr"]); sync();
    check "1b" BACKUP1 (Dir [("x", File "barr"); (".bak.1.x", File "foo")]);
    put R2 (Dir ["x", File "bazzz"]); sync();
    check "1c" BACKUP1 (Dir [("x", File "bazzz"); (".bak.2.x", File "foo"); (".bak.1.x", File "barr")]);
  );

  runtest "backups 5 (directories)" ["backup = Name *"; "backupcurrent = Name *"; "maxbackups = 7"] (fun() ->
    put R1 (Dir []); put R2 (Dir []); sync();
    (* Create a directory x containing files a and l; check that the current version gets backed up *)
    put R1 (Dir ["x", Dir ["a", File "foo"; "l", File "./foo"]]); sync();
    check "1" BACKUP1 (Dir [("x", Dir [("l", File "./foo"); ("a", File "foo")])]);
    (* On replica 2, delete file a, create file b, and edit file l *)
    put R2 (Dir ["x", Dir ["b", File "barr"; "l", File "./barr"]]); sync();
    check "2" BACKUP1 (Dir [("x", Dir [("l", File "./barr"); ("b", File "barr"); ("a", File "foo"); (".bak.1.l", File "./foo")])]);
    (* On replica 1, replace the whole directory by a file; when we check the result, we need to know
       whether we're running the test locally or remotely; in the former case, we should see *both* the
       old and the new version as backups *)
    put R1 (Dir ["x", File "bazzz"]); sync();
    if bothRootsLocal then
      check "3" BACKUP1 (Dir [("x", File "bazzz"); (".bak.2.x", Dir [("l", File "./barr"); ("b", File "barr"); ("a", File "foo"); (".bak.1.l", File "./foo")]); (".bak.1.x", Dir [("l", File "./barr"); ("b", File "barr")])])
    else
      check "3" BACKUP1 (Dir [("x", File "bazzz"); (".bak.1.x", Dir [("l", File "./barr"); ("b", File "barr"); ("a", File "foo"); (".bak.1.l", File "./foo")])]);
  );

  runtest "backups 6 (backup prefix/suffix)" ["backup = Name *";
               "backuplocation = local";
               "backupprefix = back/$VERSION-";
               "backupsuffix = .backup";
               "backupcurrent = Name *"] (fun() ->
    put R1 (Dir []); put R2 (Dir []); sync();
    put R1 (Dir ["x", File "foo"]); sync();
    check "1" R1 (Dir [("x", File "foo"); ("back", Dir [("0-x.backup", File "foo")])]);
  );

  if not (Prefs.read Globals.someHostIsRunningWindows) then begin
    runtest "links 1 (directories and links)" ["backup = Name *"; "backupcurrent = Name *"; "maxbackups = 7"] (fun() ->
      put R1 (Dir []); put R2 (Dir []); sync();
      put R1 (Dir ["x", Dir ["a", File "foo"; "l", Link "./foo"]]); sync();
      check "1" BACKUP1 (Dir [("x", Dir [("l", Link "./foo"); ("a", File "foo")])]);
      put R2 (Dir ["x", Dir ["b", File "barr"; "l", Link "./barr"]]); sync();
      check "2" BACKUP1 (Dir [("x", Dir [("l", Link "./barr"); ("b", File "barr"); ("a", File "foo"); (".bak.1.l", Link "./foo")])]);
      put R1 (Dir ["x", File "bazzz"]); sync();
      if bothRootsLocal then
        check "3" BACKUP1
          (Dir [("x", File "bazzz");
                (".bak.2.x", Dir [("l", Link "./barr"); ("b", File "barr"); ("a", File "foo");
                                  (".bak.1.l", Link "./foo")]);
                (".bak.1.x", Dir [("l", Link "./barr"); ("b", File "barr")])])
      else
        check "3" BACKUP1
          (Dir [("x", File "bazzz");
                (".bak.1.x", Dir [("l", Link "./barr"); ("b", File "barr");
                                  ("a", File "foo"); (".bak.1.l", Link "./foo")])]);
    );

    (* Test that we correctly fail when we try to 'follow' a symlink that does not
       point to anything *)
    runtest "links 2 (symlink to nowhere)" ["follow = Name y"] (fun() ->
      let orig = (Dir []) in
      put R1 orig; put R2 orig; sync();
      put R1 (Dir ["y", Link "x"]); sync();
      check "1" R2 orig;
    );

    (* Check for the bug reported by Sebastian Elsner (Jan 2018) *)
    (* NOT POSSIBLE because the test API does not enable one to play with file
       owners, but I put the test here anyway. *)
    (*
    runtest "owner of path directories" ["owner"; "path = a/b"] (fun() ->
      put R1 (Dir ["a", Dir ["b", Dir["foo", File "Foo";
                                      "bar", File "Bar";
                                      "baz", File "Baz";]]]]);
      setOwner R1 "a/b" "testuser";  (* does not exist *)
      put R2 (Dir []);
      sync();
      checkOwner "1" R2 "a/b" "testuser";  (* does not exist *)
    );
    *)
  end;

  if Moves.enabled () then begin
    let sync_count_moves () =
      let moved = ref 0 in
      let count_moves _ _ dbg =
        (* A very crude way of counting propagated moves *)
        if dbg = "mov" then incr moved
      in
      Uutil.setProgressPrinter count_moves;
      sync ~verbose:false ();
      Uutil.setProgressPrinter (fun _ _ _ -> ());
      !moved
    in

    let assert_n_mov n err () = assert_eq (sync_count_moves ()) n err () in

    let testmoves n name (expect_move : [`NoMove | `Move | `RevertMove | `RevertNoMove]) ?total expect_n_mov =
      let expect_n_ris = match total with Some x -> x | None -> expect_n_mov in
      let err = name ^ " was not detected" in
      check_assert (n ^ "a") (assert_n_ris expect_n_ris err);
      let expect_n_moved = match expect_move with
        | `NoMove | `RevertNoMove -> 0
        | `Move | `RevertMove -> expect_n_mov
      in
      let err = name ^ match expect_move with
        | `NoMove -> " was propagated as a move"
        | `Move -> " was not propagated as a move"
        | `RevertMove -> " was not reverted as a move"
        | `RevertNoMove -> " was reverted as a move"
      in
      check_assert (n ^ "b") (assert_n_mov expect_n_moved err)
    in

    runtest "moves/renames 1 (plain)" ["moves-experimental = true"] (fun () ->
      put R1 (Dir []); put R2 (Dir []); sync ();
      (* Create a file and a directory *)
      put R1 (Dir ["x", File "foo"; "d", Dir ["a", File "barr"]]); sync ();
      (* Rename a file *)
      let newfs = Dir ["y", File "foo"; "d", Dir ["a", File "barr"]] in
      put R2 newfs;
      testmoves "1" "File rename" `Move 1;
      check "2" R1 newfs;
      (* Rename a directory *)
      let newfs = Dir ["y", File "foo"; "e", Dir ["a", File "barr"]] in
      put R2 newfs;
      testmoves "3" "Directory rename" `Move 1;
      check "4" R1 newfs;
      (* Move a file *)
      let newfs = Dir ["e", Dir ["a", File "barr"; "y", File "foo"]] in
      put R2 newfs;
      testmoves "5" "File move" `Move 1;
      check "6" R1 newfs;
      (* Move a directory *)
      put R1 (Dir ["d", Dir ["a", File "barr"]; "q", Dir []]); sync ();
      let newfs = Dir ["q", Dir ["d", Dir ["a", File "barr"]]] in
      put R2 newfs;
      testmoves "7" "Directory move" `Move 1;
      check "8" R1 newfs;
      (* Multiple moves and renames *)
      put R1 (Dir []); put R2 (Dir []); sync ();
      put R1 (Dir ["x", File "foo"; "y", File "bar"; "z", File "bah";
        "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]]);
      sync ();
      let newfs = Dir ["y", File "bar"; "o", File "bah";
        "c", Dir ["b", File "quu"; "x", File "foo"; "z", File "bar"];
        "e", Dir ["a", File "barr"]] in
      put R1 newfs;
      testmoves "9" "Multi-rename" `Move ~total:4 3;
      check "10" R2 newfs;
      (* Non-rename a file *)
      put R1 (Dir []); put R2 (Dir []); sync ();
      put R1 (Dir ["y", File "bar"]); sync ();
      put R1 (Dir ["x", File "bah"]);
      testmoves "11" "Non-rename" `NoMove ~total:2 0;
      check "12" R2 (Dir ["x", File "bah"]);
    );

    runtest "moves/renames 2 (reverting)" ["moves-experimental = true"; "force = " ^ rr1] (fun () ->
      put R1 (Dir []); put R2 (Dir []); sync ();
      (* Create a file and a directory *)
      let origfs = Dir ["x", File "foo"; "d", Dir ["a", File "barr"]] in
      put R1 origfs; sync ();
      (* Rename a file *)
      let newfs = Dir ["y", File "foo"; "d", Dir ["a", File "barr"]] in
      put R2 newfs;
      testmoves "1" "File rename" `RevertMove 1;
      check "2" R2 origfs;
      (* Rename a directory *)
      let newfs = Dir ["x", File "foo"; "e", Dir ["a", File "barr"]] in
      put R2 newfs;
      testmoves "3" "Directory rename" `RevertMove 1;
      check "4" R2 origfs;
      (* Move a file *)
      let newfs = Dir ["d", Dir ["a", File "barr"; "x", File "foo"]] in
      put R2 newfs;
      testmoves "5" "File move" `RevertMove 1;
      check "6" R1 origfs;
      (* Move a directory *)
      let origfs = Dir ["d", Dir ["a", File "barr"]; "q", Dir []] in
      put R1 origfs; put R2 origfs; sync ();
      let newfs = Dir ["q", Dir ["d", Dir ["a", File "barr"]]] in
      put R2 newfs;
      testmoves "7" "Directory move" `RevertMove 1;
      check "8" R1 origfs;
      (* Multiple moves and renames *)
      put R1 (Dir []); put R2 (Dir []); sync ();
      let origfs = Dir ["x", File "foo"; "y", File "bar"; "z", File "bah";
        "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 origfs; put R2 origfs; sync ();
      let newfs = Dir ["y", File "bar"; "o", File "bah";
        "c", Dir ["b", File "quu"; "x", File "foo"; "z", File "bar"];
        "e", Dir ["a", File "barr"]] in
      put R2 newfs;
      testmoves "9" "Multi-rename" `Move ~total:4 3;
      check "10" R1 origfs;
    );

    runtest "moves/renames 3 (overwriting)" ["moves-experimental = true"] (fun () ->
      put R1 (Dir []); put R2 (Dir []); sync ();
      (* Create files and directories *)
      let origfs = Dir ["x", File "foo"; "y", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 origfs; put R2 origfs; sync ();
      (* Rename a file over another *)
        (* Currently not supported by move detection *)
      (* Rename a file over a directory *)
      let newfs = Dir ["c", File "foo"; "y", File "bar"; "d", Dir ["a", File "barr"]] in
      put R1 newfs;
      testmoves "1" "File rename" `Move 1;
      check "2" R2 newfs;
      (* Rename a directory over another *)
        (* Currently not supported by move detection *)
      (* Rename a directory over a file *)
      put R1 origfs; put R2 origfs; sync ();
      let newfs = Dir ["x", File "foo"; "c", Dir ["b", File "quu"]; "y", Dir ["a", File "barr"]] in
      put R1 newfs;
      testmoves "3" "Directory rename" `Move 1;
      check "4" R2 newfs;
    );

    runtest "moves/renames 4 (reverting overwrites)" ["moves-experimental = true"; "force = " ^ rr1] (fun () ->
      put R1 (Dir []); put R2 (Dir []); sync ();
      (* Create files and directories *)
      let origfs = Dir ["x", File "foo"; "y", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 origfs; sync ();
      (* Rename a file over a directory *)
      let newfs = Dir ["c", File "foo"; "y", File "bar"; "d", Dir ["a", File "barr"]] in
      put R2 newfs;
      testmoves "1" "File rename" `RevertNoMove 1;
      check "2" R2 origfs;
      (* Rename a directory over a file *)
      put R1 origfs; put R2 origfs; sync ();
      let newfs = Dir ["x", File "foo"; "c", Dir ["b", File "quu"]; "y", Dir ["a", File "barr"]] in
      put R2 newfs;
      testmoves "3" "Directory rename" `RevertNoMove 1;
      check "4" R2 origfs;
    );

    runtest "moves/renames 5 (conflicts)" ["moves-experimental = true"; "force = " ^ rr1] (fun () ->
      put R1 (Dir []); put R2 (Dir []); sync ();
      (* Create files and directories *)
      let origfs = Dir ["x", File "foo"; "y", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 origfs; sync ();
      (* Rename a file with delete conflict *)
      let newfs1 = Dir ["c", File "foo"; "y", File "bar"; "d", Dir ["a", File "barr"]]
      and newfs2 = Dir ["x", File "foo"; "y", File "bar"; "d", Dir ["a", File "barr"]] in
      put R1 newfs1;
      put R2 newfs2;
      testmoves "1" "Conflicting file rename" `Move 1;
      check "2" R2 newfs1;
      (* Rename directory with delete conflict *)
      put R1 origfs; put R2 origfs; sync ();
      let newfs1 = Dir ["x", File "foo"; "y", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]]
      and newfs2 = Dir ["x", File "foo"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 newfs1;
      put R2 newfs2;
      testmoves "3" "Conflicting directory rename" `Move 1;
      check "4" R2 newfs1;
      (* Rename a file with change conflict *)
      put R1 origfs; put R2 origfs; sync ();
      let newfs1 = Dir ["z", File "foo"; "y", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]]
      and newfs2 = Dir ["x", File "fooz"; "y", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 newfs1;
      put R2 newfs2;
      testmoves "5" "Conflicting file rename" `NoMove 1;
      check "6" R2 newfs1;
      (* Rename directory with change conflict *)
      let newfs = Dir ["x", File "foo"; "e", Dir ["a", File "barr"]] in
      put R1 newfs;
      put R2 (Dir ["x", File "foo"; "d", Dir ["b", File "barr"]]);
      testmoves "7" "Conflicting directory rename" `NoMove 1;
      check "8" R2 newfs;
      (* Rename a file with file create conflict *)
      put R1 origfs; put R2 origfs; sync ();
      let newfs1 = Dir ["z", File "foo"; "y", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]]
      and newfs2 = Dir ["x", File "foo"; "y", File "bar"; "z", File "fooz"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 newfs1;
      put R2 newfs2;
      testmoves "9" "Conflicting file rename" `Move 1;
      check "10" R2 newfs1;
      (* Rename a file with dir create conflict *)
      put R1 origfs; put R2 origfs; sync ();
      let newfs1 = Dir ["z", File "foo"; "y", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]]
      and newfs2 = Dir ["x", File "foo"; "y", File "bar"; "z", Dir []; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 newfs1;
      put R2 newfs2;
      testmoves "11" "Conflicting file rename" `Move 1;
      check "12" R2 newfs1;
      (* Rename a dir with file create conflict *)
      put R1 origfs; put R2 origfs; sync ();
      let newfs1 = Dir ["x", File "foo"; "y", File "bar"; "z", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]]
      and newfs2 = Dir ["x", File "foo"; "y", File "bar"; "z", File "fooz"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 newfs1;
      put R2 newfs2;
      testmoves "13" "Conflicting directory rename" `Move 1;
      check "14" R2 newfs1;
      (* Rename a dir with dir create conflict *)
        (* Currently not supported by reconciliation + move detection *)
    );

    runtest "moves/renames 6 (unsupported)" ["moves-experimental = true"] (fun () ->
      put R1 (Dir []); put R2 (Dir []); sync ();
      (* Create files and directories *)
      let origfs = Dir ["x", File "foo"; "y", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 origfs; sync ();
      (* Rename two files with a target conflict *)
      let newfs1 = Dir ["z", File "foo"; "y", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]]
      and newfs2 = Dir ["x", File "foo"; "z", File "bar"; "c", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]] in
      put R1 newfs1;
      put R2 newfs2;
      testmoves "1" "Unsupported conflicting file rename target" `NoMove ~total:2 0;
      (* Rename two directories with a target conflict *)
      let newfs1 = Dir ["x", File "foo"; "y", File "bar"; "z", Dir ["b", File "quu"]; "d", Dir ["a", File "barr"]]
      and newfs2 = Dir ["x", File "foo"; "y", File "bar"; "c", Dir ["b", File "quu"]; "z", Dir ["a", File "barr"]] in
      put R1 newfs1;
      put R2 newfs2;
      testmoves "2" "Unsupported conflicting dir rename target" `NoMove ~total:4 0;
    );
  end;

  if not bothRootsLocal then
  begin
    let localR, remoteR, localRaw =
      match r1 with
      | Common.Local, _ -> R1, R2, r1
      | _ -> R2, R1, r2
    in

    (* Test RPC function "fingerprintSubfile" *)
    runtest "RPC: transfer append" [] (fun () ->
      let prefixLen = 1024 * 1024 + 1 in
      let len = prefixLen + 31 in
      let contents = String.make len '.' in
      let fileName = "bigfile" in
      let prefixPath = Path.fromString fileName in
      let (workingDir, _) = Fspath.findWorkingDir (snd localRaw) prefixPath in
      let prefixName = Path.toString (Os.tempPath ~fresh:false workingDir prefixPath) in
      put remoteR (Dir [(fileName, File contents)]);
      put localR (Dir [(prefixName, File (String.sub contents 0 prefixLen))]);
      sync ();
      check "1" localR (Dir [(fileName, File contents)]);
    );

    (* Test RPC function "updateProps" *)
    runtest "RPC: update props" ["times = true"] (fun () ->
      let state = [("a", File "x")] in
      put remoteR (Dir state);
      put localR (Dir []);
      sync ();
      (* Having to sleep here is an unfortunate side-effect of the current
         Windows limitations-inspired time comparison algorithm which is
         designed to work on FAT filesystems (2-second granularity). *)
      Unix.sleep 2;
      put remoteR (Dir state);
      sync ();
      check "1" localR (Dir state);
    );

    (* Test RPC function "replaceArchive" *)
    runtest "RPC: replaceArchive" [] (fun () ->
      put localR (Dir [("n", File "to delete")]);
      put remoteR (Dir []);
      sync ();
      put remoteR (Dir []);
      sync ();
      check "1" localR (Dir []);
    );

    (* Test RPC functions "mkdir" and "setDirProp" *)
    runtest "RPC: mkdir, setDirProp" [] (fun () ->
      let state = [("subd", Dir [])] in
      put localR (Dir state);
      put remoteR (Dir []);
      sync ();
      check "1" remoteR (Dir state);
    );

    (* Test RPC function "setupTargetPaths" *)
    runtest "RPC: merge" ["merge = Name ma -> echo x> NEW"; "backupcurr = Name ma"] (fun () ->
      let result = match Sys.os_type with
        | "Win32" -> ("ma", File "x\r\n")
        | _ -> ("ma", File "x\n")
      in
      put localR (Dir [("ma", File "a")]);
      put remoteR (Dir [("ma", File "b")]);
      sync ();
      check "1" localR (Dir [result]);
      check "2" remoteR (Dir [result]);
    );
  end;

  if !failures = 0 then
    Util.msg "Success :-)\n"
  else
    raise (Util.Fatal "Self-tests failed\n")

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/plugins/x86/x86_lifter.ml#L365-L1679 *)
(* BinaryAnalysisPlatform/bap plugins/x86/x86_lifter.ml:365-1679 *)
  let rec to_ir mode addr next ss pref has_rex has_vex =
    let module R = (val (vars_of_mode mode)) in
    let open R in
    let load = load_s mode ss in (* Need to change this if we want seg_ds <> None *)
    let op2e = op2e_s mode ss has_rex in
    let op2e_keep_width = op2e_s_keep_width mode ss has_rex in
    let op2e_dbl = op2e_dbl_s mode ss has_rex in
    let store = store_s mode ss in
    let assn = assn_s mode ss has_rex has_vex in
    let assn_dbl = assn_dbl_s mode ss has_rex has_vex in
    let mi = int_of_mode mode in
    let mt = type_of_mode mode in
    let rbp_e = Bil.var rbp in
    let rsp_e = Bil.var rsp in
    let rsi_e = Bil.var rsi in
    let rdi_e = Bil.var rdi in
    let rax_e = Bil.var rax in
    let rdx_e = Bil.var rdx in
    let ah_e = ah_e mode in
    let disfailwith = disfailwith mode in
    let unimplemented = unimplemented mode in
    let is_small_imm op t = match op with
      | Oimm imm -> Word.bitwidth imm < bitwidth_of_type t
      | _ -> false in
    let sign_extend_imm op t =
      let op_typ = match op with
        | Oimm imm -> Type.imm (Word.bitwidth imm)
        | _ -> disfailwith "imm operand expected" in
      Bil.(cast signed (bitwidth_of_type t) (op2e op_typ op)) in
    let is_repz = function
      | [x] -> x = repz
      | _ -> false in
    let is_repnz = function
      | [x] -> x = repnz
      | _ -> false in
    function
    | Nop -> []
    | Bswap(t, op) ->
      let e = match t with
        | Type.Imm 32 | Type.Imm 64 -> let (op', t') = op2e_keep_width t op in
          reverse_bytes op' t'
        | _ -> disfailwith "bswap: Expected 32 or 64 bit type"
      in
      [assn t op e]
    | Retn (op, far_ret) when List.is_empty pref || is_repz pref || is_repnz pref ->
      let temp = tmp mt in
      let load_stmt = if far_ret
        then (* TODO Mess with segment selectors here *)
          unimplemented "long retn not supported"
        else Bil.Move (temp, load_s mode seg_ss (size_of_typ mt) rsp_e)
      in
      let rsp_stmts =
        Bil.Move (rsp, Bil.(rsp_e + (Int (mi (bytes_of_width mt)))))::
        (match op with
         | None -> []
         | Some(t, src) ->
           [Bil.Move (rsp, Bil.(rsp_e + (op2e t src)))]
        ) in
      load_stmt::
      rsp_stmts@
      [Bil.Jmp (Bil.Var temp)]
    | Mov(t, dst, src, condition) ->
      let c_src = (match condition with
          | None -> op2e t src
          | Some c -> Bil.Ite (c, op2e t src, op2e t dst))
      in
      (* Find base by looking at LDT or GDT *)
      let base_e e =
        (* 0 = GDT, 1 = LDT *)
        let ti = Bil.Extract (3, 3, e) in
        let base = Bil.Ite (ti, Bil.var ldt, Bil.var gdt) in
        (* Extract index into table *)
        let entry_size, entry_shift = match mode with
          | X86 -> reg64_t, 6  (* "1<<6 = 64" *)
          | X8664 -> reg128_t, 7 (* "1<<7 = 128" *)
        in
        let index = Bil.(Cast (UNSIGNED, !!mt, (Extract (15, 4, e)) lsl (Int (mi entry_shift)))) in
        (* Load the table entry *)
        let table_entry = load_s mode None (size_of_typ entry_size) (Bil.(base + index)) in
        (* Extract the base *)
        concat_explist
          ((match mode with
              | X86 -> []
              | X8664 -> (Bil.Extract (95, 64, table_entry)) :: [])
           @  (Bil.Extract (63, 56, table_entry))
              :: (Bil.Extract (39, 32, table_entry))
              :: (Bil.Extract (31, 16, table_entry))
              :: [])
      in
      let bs =
        let dst_e = op2e t dst in
        if [%compare.equal: operand] dst o_fs && !compute_segment_bases
        then [Bil.Move (fs_base, base_e dst_e)]
        else if [%compare.equal: operand] dst o_gs && !compute_segment_bases
        then [Bil.Move (gs_base, base_e dst_e)]
        else []
      in
      assn t dst c_src :: bs
    | Movs(Type.Imm _bits as t) ->
      let stmts =
        store_s mode seg_es t rdi_e (load_s mode seg_es (size_of_typ t) rsi_e)
        :: string_incr mode t rsi
        :: string_incr mode t rdi
        :: []
      in
      if List.is_empty pref then
        stmts
      else if ints_mem pref repz || ints_mem pref repnz then
        (* movs has only rep instruction others just considered to be rep *)
        rep_wrap ~mode ~addr ~next stmts
      else
        unimplemented "unsupported prefix for movs"
    | Movzx(t, dst, ts, src) ->
      [assn t dst Bil.(Cast (UNSIGNED, !!t, op2e ts src))]
    | Movsx(t, dst, ts, src) ->
      [assn t dst Bil.(Cast (SIGNED, !!t, op2e ts src))]
    | Movdq(ts, s, td, d, align) ->
      let (s, al) = match s with
        | Ovec _ | Oreg _-> op2e ts s, []
        | Oaddr a -> op2e ts s, [a]
        | Oimm _ | Oseg _ -> disfailwith "invalid source operand for movdq"
      in
      let (d, al) = match d with
        (* Behavior is to clear the xmm bits *)
        | Ovec _ -> assn td d Bil.(Cast (UNSIGNED, !!td, s)), al
        | Oreg _ -> assn td d s, al
        | Oaddr a -> assn td d s, a::al
        | Oimm _ | Oseg _ -> disfailwith "invalid dest operand for movdq"
      in
      (* sources tell me that movdqa raises a general protection exception
       * if its operands aren't aligned on a 16-byte boundary *)
      let im i = Bil.Int (int_of_mode mode i) in
      let al =
        if align then
          List.map ~f:(fun a -> Bil.If (Bil.((a land im 15) = im 0), [], [Cpu_exceptions.general_protection])) al
        else []
      in
      d::al
    | Movoffset((tdst, dst), offsets) ->
      (* If a vex prefix is present, then extra space is filled with 0.
         Otherwise, the bits are preserved. *)
      let padding hi lo =
        if hi < lo then []
        else if has_vex then [int_exp 0 (hi - lo + 1)]
        else [Bil.Extract (hi, lo, op2e tdst dst)]
      in
      let offsets = List.sort ~compare:(fun {offdstoffset=o1; _} {offdstoffset=o2; _} -> Int.compare o1 o2) offsets in
      let add_exp (elist,nextbit) {offlen; offtyp; offop; offsrcoffset; offdstoffset} =
        Bil.Extract ((!!offlen + offsrcoffset - 1), offsrcoffset, (op2e offtyp offop))
        :: padding (offdstoffset - 1) nextbit
        @ elist, offdstoffset + !!offlen
      in
      let elist, nextbit = List.fold_left ~f:add_exp ~init:([],0) offsets in
      let elist = padding (!!tdst - 1) nextbit @ elist in
      [assn tdst dst (concat_explist elist)]
    | Punpck(t, et, o, d, s, vs) ->
      let nelem = match t, et with
        | Type.Imm n, Type.Imm n' -> n / n'
        | _ -> disfailwith "invalid"
      in
      assert (nelem mod 2 = 0);
      let nelem_per_src = nelem / 2 in
      let halft = Type.Imm (!!t / 2) in
      let castf = match o with
        | High -> fun e -> Bil.(Cast (HIGH, !!halft, e))
        | Low -> fun e -> Bil.(Cast (LOW, !!halft, e))
      in
      let se, de = castf (op2e t s), castf (op2e t d) in
      let st, dt = tmp halft, tmp halft in
      let et' = !!et in
      let mape i =
        [extract_element et' (Bil.Var st) i; extract_element et' (Bil.Var dt) i]
      in
      let e = concat_explist (List.concat (List.map ~f:mape (List.range ~stride:(-1) ~stop:`inclusive (nelem_per_src-1) 0))) in
      let dest = match vs with
        | None -> d
        | Some vdst -> vdst
      in
      [Bil.Move (st, se);
       Bil.Move (dt, de);
       assn t dest e]
    | Ppackedbinop(t, et, fbop, _, d, s, vs) ->
      let t_width = bitwidth_of_type t  in
      let e_width = bitwidth_of_type et in
      let nops = t_width / e_width in
      let byte = 8 in
      let tmp_dst = tmp t in
      let vdst = Option.value ~default:d vs in
      let iv  = tmp (Type.imm byte) in
      let elt = tmp et in
      let zero = Word.zero t_width in
      let bits = Word.of_int ~width:byte e_width in

      let foreach_size f = List.concat @@ List.init nops ~f in
      let getelement o i =
        (* assumption: immediate operands are repeated for all vector
           elements *)
        match o with
        | Oimm _ -> op2e et o
        | _ -> extract_element !!et (op2e t o) i in

      List.concat Bil.[
          [tmp_dst := int zero];
          foreach_size (fun i -> [
                elt := fbop (getelement d i) (getelement s i);
                iv := int @@ Word.of_int ~width:byte i;
                tmp_dst :=
                  var tmp_dst lor
                  ((cast unsigned t_width (var elt)) lsl (var iv * int bits))
              ]
            );
          [assn t vdst (var tmp_dst)]
        ]
    | Pbinop(t, fbop, _s, o1, o2, vop) ->
      (match vop with
       | None -> [assn t o1 (fbop (op2e t o1) (op2e t o2))]
       | Some vop -> [assn t o1 (fbop (op2e t vop) (op2e t o2))])
    | Pcmp (t,elet,bop,_,dst,src,vsrc) ->
      let elebits = bitwidth_of_type elet in
      let t_width = bitwidth_of_type t in
      let ncmps = t_width / elebits in
      let src = match src with
        | Ovec _ -> op2e t src
        | Oaddr a -> load (size_of_typ t) a
        | Oreg _ | Oimm _ | Oseg _ -> disfailwith "invalid" in
      let vsrc = Option.value ~default:dst vsrc in
      let byte = 8 in
      let tmp_dst = tmp t in
      let iv  = tmp (Type.imm byte) in
      let elt = tmp elet in
      let elt_1 = tmp elet in
      let elt_2 = tmp elet in
      let _one = Word.of_int ~width:elebits (-1) in
      let zero = Word.zero elebits in
      let bits = Word.of_int ~width:byte elebits in
      let zero_long = Word.zero t_width in
      let foreach_size f =
        let sizes = List.init ncmps ~f:(fun i ->
            let left_bit = i * elebits in
            let right_bit = (i + 1) * elebits - 1 in
            let i = Word.of_int ~width:byte i in
            i, left_bit, right_bit) in
        List.concat @@ List.map sizes ~f in

      List.concat Bil.[
          [tmp_dst := int zero_long];
          foreach_size (fun (i, left_bit, right_bit) -> [
                elt_1 := extract right_bit left_bit src;
                elt_2 := extract right_bit left_bit (op2e t vsrc);
                iv := int i;
                if_ (binop bop (var elt_1) (var elt_2)) [
                  elt := int _one;
                ] [
                  elt := int zero;
                ];
                tmp_dst :=
                  var tmp_dst lor
                  ((cast unsigned t_width (var elt)) lsl (var iv * int bits))
              ]);
          [assn t dst (var tmp_dst)] ]
    | Pmov (t, dstet, srcet, dst, src, ext, _) ->
      let nelem = match t, dstet with
        | Type.Imm n, Type.Imm n' -> n / n'
        | _ -> disfailwith "invalid"
      in
      let getelt op i = extract_element !!srcet (op2e t op) i in
      let extcast =
        let open Exp in
        match ext with
        | UNSIGNED | SIGNED -> fun e -> Bil.Cast (ext, !!dstet, e)
        | _ -> disfailwith "invalid"
      in
      let extend i = extcast (getelt src i) in
      let e = concat_explist (List.map ~f:extend (List.range ~stride:(-1) ~stop:`inclusive (nelem-1) 0)) in
      [assn t dst e]
    | Pmovmskb (t,dst,src) ->
      let nbytes = bytes_of_width t in
      let src = match src with
        | Ovec _ -> op2e t src
        | _ -> disfailwith "invalid operand"
      in
      let get_bit i = Bil.Extract(i*8-1, i*8-1, src) in
      let byte_indices = List.init ~f:(fun i -> i + 1) nbytes in (* list 1-nbytes *)
      let all_bits = List.rev (List.map ~f:get_bit byte_indices) in
      (* could also be done with shifts *)
      let padt = Type.Imm(32 - nbytes) in
      let or_together_bits = List.fold_left ~f:(fun acc i -> Bil.Concat(acc,i)) ~init:(int_exp 0 (!!padt)) all_bits in
      [assn reg32_t dst or_together_bits]
    | Palignr (t,dst,src,vsrc,imm) ->
      let (dst_e, t_concat) = op2e_keep_width t dst in
      let (src_e, t_concat') = op2e_keep_width t src in
      (* previously, this code called Typecheck.infer_ast.
       * We're now just preserving the widths, so here we assert
       * that our 2 "preserved widths" are the same. *)
      assert (!!t_concat = !!t_concat');
      let imm = op2e t imm in
      let conct = Bil.(dst_e ^ src_e) in
      let shift = Bil.(conct lsr (Cast (UNSIGNED, !!t_concat, imm lsl (int_exp 3 !!t)))) in
      let high, low = match t with
        | Type.Imm 256 -> 255, 0
        | Type.Imm 128 -> 127, 0
        | Type.Imm 64 -> 63, 0
        | _ -> disfailwith "impossible: used non 64/128/256-bit operand in palignr"
      in
      let result = Bil.Extract (high, low, shift) in
      let im i = Bil.Int (int_of_mode mode i) in
      let addresses = List.fold
          ~f:(fun acc -> function Oaddr a -> a::acc | _ -> acc) ~init:[] [src;dst] in
      (* Palignr seems to cause a CPU general protection exception if this fails.
       * previously this code used the ast.ml Assert statement, which is gone,
       * so it's been replaced with Bil's CpuExn *)
      List.map ~f:(fun addr -> Bil.If (Bil.((addr land im 15) = im 0),
                                       [], [Cpu_exceptions.general_protection])) addresses
      @ (match vsrc with
          | None -> [assn t dst result]
          | Some vdst -> [assn t vdst result])
    | Pcmpstr(t,xmm1,xmm2m128,_imm,imm8cb,pcmpinfo) ->
      let open Pcmpstr in
      let concat elt_width exps = match exps with
        | [] -> disfailwith "trying concat on empty list"
        | first :: exps' ->
          let len = List.length exps * elt_width in
          let exp = List.fold exps' ~init:first ~f:(fun e e' -> Bil.(e ^ e')) in
          let tmp = tmp (Type.imm len) in
          tmp, Bil.[tmp := exp] in

      (* All bytes and bits are numbered with zero being the least
         significant. This includes strings! *)
      (* NOTE: Strings are backwards, at least when they are in
         registers.  This doesn't seem to be documented in the Intel
         manual.  This means that the NULL byte comes before the
         string. *)
      let xmm1_e = op2e t xmm1 in
      let xmm2m128_e = op2e t xmm2m128 in
      let regm = type_of_mode mode in

      let nelem, _nbits, elemt = match imm8cb.ssize with
        | Bytes -> 16, 8, Type.imm 8
        | Words -> 8, 16, Type.imm 16 in

      (* Get element index in e *)
      let get_elem = extract_element !!elemt in
      let get_xmm1 = get_elem xmm1_e in
      let get_xmm2 = get_elem xmm2m128_e in

      let nelem_range =
        List.range ~stride:(-1) ~stop:`inclusive (nelem-1) 0 in

      (* Build expressions that assigns the correct values to the
         is_valid variables using implicit (NULL-based) string length. *)
      let implicit_check is_valid_xmm_i get_xmm_i =
        let f acc i =
          let previous_valid =
            if i = 0 then exp_true
            else Bil.var (is_valid_xmm_i (i - 1)) in
          let current_valid =
            Bil.(get_xmm_i i <> (int_exp 0 !!elemt)) in
          let x = is_valid_xmm_i i in
          Bil.(x := previous_valid land current_valid) :: acc in
        List.fold ~f:f ~init:[] nelem_range in

      (* Build expressions that assigns the correct values to the
         is_valid variables using explicit string length. *)
      let explicit_check is_valid_xmm_i sizee =
        (* Max size is nelem *)
        let nelem_e = Word.of_int ~width:(bitwidth_of_type regm) nelem in
        let sizev = tmp ~name:"sz" regm in
        let init = Bil.[
            if_ (int nelem_e < sizee) [
              sizev := int nelem_e;
            ] [
              sizev := sizee;
            ]
          ] in
        let f acc i =
          (* Current element is valid *)
          let current_valid = Bil.(int_exp i !!regm < var sizev) in
          let x = is_valid_xmm_i i in
          Bil.(x := current_valid) :: acc in
        List.fold_left ~f ~init nelem_range in

      (* Get var name indicating whether index in xmm num is a valid
         byte (before NULL byte). *)
      (* XXX more hashtable code *)
      let is_valid =
        let vh = Hashtbl.Poly.create () ~size:(2*nelem) in
        (fun xmmnum index -> match Hashtbl.find vh (xmmnum,index) with
           | Some v -> v
           | None ->
             let name = sprintf "is_valid_xmm%d_ele%d" xmmnum index in
             let v = tmp ~name bool_t in
             Hashtbl.add_exn vh ~key:(xmmnum,index) ~data:v;
             v) in

      let is_valid_xmm1 index = is_valid 1 index in
      let is_valid_xmm2 index = is_valid 2 index in
      let is_valid_xmm1_e index = Bil.Var(is_valid_xmm1 index) in
      let is_valid_xmm2_e index = Bil.Var(is_valid_xmm2 index) in

      let xmm1_checks, xmm2_checks =
        match pcmpinfo.len with
        | Implicit ->
          implicit_check is_valid_xmm1 get_xmm1,
          implicit_check is_valid_xmm2 get_xmm2
        | Explicit ->
          explicit_check is_valid_xmm1 rax_e,
          explicit_check is_valid_xmm2 rdx_e in

      let get_bit index =
        match imm8cb.agg with
        | EqualAny ->
          (* Is xmm2[index] at xmm1[j]? *)
          let check_char acc j =
            let is_eql = Bil.(get_xmm2 index = get_xmm1 j) in
            let is_valid = is_valid_xmm1_e j in
            Bil.((is_eql land is_valid) lor acc)
          in
          (* Is xmm2[index] included in xmm1[j] for any j? *)
          Bil.(is_valid_xmm2_e index land
               (List.fold ~f:check_char ~init:exp_false nelem_range))
        | Ranges ->
          (* Is there an even j such that xmm1[j] <= xmm2[index] <=
             xmm1[j+1]? *)
          let check_char acc j =
            (* XXX: Should this be AND? *)
            let ind0 = 2 * j in
            let ind1 = ind0 + 1 in
            let rangevalid = Bil.(is_valid_xmm1_e ind0 land is_valid_xmm1_e ind1) in
            let (<=) = match imm8cb.ssign with
              | Unsigned -> Bil.(<=)
              | Signed -> Bil.(<=$) in
            let (land) = Bil.(land) in
            let inrange =
              (get_xmm1 ind0 <= get_xmm2 index) land (get_xmm2 index <= get_xmm1 ind1) in
            Bil.(rangevalid land (inrange lor acc)) in
          Bil.(is_valid_xmm2_e index land
               List.fold_left ~f:check_char ~init:exp_false (List.range ~stride:(-1) ~stop:`inclusive Stdlib.(nelem/2-1) 0))
        | EqualEach ->
          (* Does xmm1[index] = xmm2[index]? *)
          let xmm1_invalid = Bil.(UnOp (NOT, (is_valid_xmm1_e index))) in
          let xmm2_invalid = Bil.(UnOp (NOT, (is_valid_xmm2_e index))) in
          let bothinvalid = Bil.(xmm1_invalid land xmm2_invalid) in
          let eitherinvalid = Bil.(xmm1_invalid lor xmm2_invalid) in
          let equal = Bil.(get_xmm1 index = get_xmm2 index) in
          (* both invalid -> true
             one invalid -> false
             both valid -> check same byte *)
          Bil.(bothinvalid lor (UnOp (NOT, eitherinvalid) land equal))
        | EqualOrdered ->
          (* Does the substring xmm1 occur at xmm2[index]? *)
          let check_char acc j =
            let equal = Bil.(get_xmm1 j = get_xmm2 Stdlib.(index+j)) in
            let substrended = Bil.(UnOp (NOT, (is_valid_xmm1_e j))) in
            let bigstrended = Bil.UnOp (NOT, (is_valid_xmm2_e (index+j))) in
            (* substrended => true
               bigstrended => false
               byte diff => false
               byte same => keep going  *)
            Bil.(substrended lor
                 (UnOp (NOT,bigstrended) land (equal land acc))) in
          (* Is xmm1[j] equal to xmm2[index+j]? *)
          List.fold_left ~f:check_char ~init:exp_true (List.range
                                                         ~stride:(-1) ~stop:`inclusive (nelem-index-1) 0)
      in
      let bits = List.map ~f:get_bit nelem_range in
      let res, res_bil = concat 1 bits in
      let int_res_1 = tmp ~name:"IntRes1" reg16_t in
      let int_res_2 = tmp ~name:"IntRes2" reg16_t in

      let contains_null e =
        let elts = List.init nelem ~f:(fun i -> Bil.(get_elem e i = int_exp 0 !!elemt)) in
        List.fold elts ~init:exp_false ~f:(fun acc elt -> Bil.(acc lor elt)) in

      (* For pcmpistri/pcmpestri *)
      let sb exp =
        List.fold_left ~f:(fun acc i ->
            Bil.Ite (Bil.(exp_true = Extract (i, i, exp)),
                     (int_exp i !!regm), acc))
          ~init:(int_exp nelem !!regm)
          (match imm8cb.outselectsig with
           | LSB -> List.range ~stride:(-1) ~start:`exclusive ~stop:`inclusive nelem 0
           | MSB -> List.init ~f:(fun x -> x) nelem)
      in

      (* For pcmpistrm/pcmpestrm *)
      let mask e =
        match imm8cb.outselectmask with
        | Bitmask -> Bil.(cast unsigned 128 e)
        | Bytemask ->
          let get_element i =
            Bil.(cast unsigned !!elemt (extract i i e)) in
          let range = List.range ~stride:(-1)
              ~start:`exclusive ~stop:`inclusive nelem 0 in
          concat_explist (List.map ~f:get_element range) in

      let res_of_cb = match imm8cb with
        | {negintres1=false; _} -> Bil.(int_res_2 := var int_res_1)
        | {negintres1=true; maskintres1=false; _} -> (* int_res_1 is bitwise-notted *)
          Bil.(int_res_2 := unop not (var int_res_1))
        | {negintres1=true; maskintres1=true; _} ->
          (* only the valid elements in xmm2 are bitwise-notted *)
          (* XXX: Right now we duplicate the valid element computations
             when negating the valid elements.  They are also used by the
             aggregation functions.  A better way to implement this might
             be to write the valid element information out as a temporary
             bitvector.  The aggregation functions and this code would
             then extract the relevant bit to see if an element is
             valid. *)
          let validvector =
            let range = List.range ~stride:(-1)
                ~start:`exclusive ~stop:`inclusive nelem 0 in
            let bits = List.map ~f:is_valid_xmm2_e range  in
            Bil.(cast unsigned 16 (concat_explist bits)) in
          Bil.(int_res_2 := validvector lxor var int_res_1) in

      let res_of_pcmpinfo = match pcmpinfo.out with
        | Index -> Bil.(rcx := sb (var int_res_2))
        (* FIXME: ymms should be used instead of xmms here *)
        | Mask -> Bil.(ymms.(0) := mask (var int_res_2)) in

      List.concat [
        xmm1_checks;
        xmm2_checks;
        res_bil;
        Bil.[
          int_res_1 := cast unsigned 16 (var res);
          res_of_cb;
          res_of_pcmpinfo;
          cf := var int_res_2 <> int_exp 0 16;
          zf := contains_null xmm2m128_e;
          sf := contains_null xmm1_e;
          oF := extract 0 0 (var int_res_2);
          af := int_exp 0 1;
          pf := int_exp 0 1;
        ]
      ]

    | Pshufd (t, dst, src, vsrc, imm) ->
      let src_e = op2e t src in
      let imm_e = op2e t imm in
      (* XXX: This would be more straight-forward if implemented using
         map, instead of fold *)
      let get_dword ndword =
        let high_b = 2 * (ndword mod 4) + 1 in
        let low_b = 2 * (ndword mod 4) in
        let index = Bil.(Cast (UNSIGNED, !!t, Extract (high_b, low_b, imm_e))) in
        let t' = !!t in
        (* Use the same pattern for the top half of a ymm register *)
        (* had to stop using extract_element_symbolic, since that calls
         * Typecheck.infer_ast. I believe this captures the same
         * "width logic", but this is a good place to check if things start
         * going wrong later. *)
        let (index, index_width) = if t' = 256 && ndword > 3 then
            (Bil.(index + (Int (BV.of_int ~width:(!!t) 4))), 256)
          else (index, t') in
        extract_element_symbolic_with_width (Type.imm 32) src_e index index_width
      in
      let topdword = match t with Type.Imm 128 -> 3 | _ -> 7 in
      let dwords = concat_explist (List.map ~f:get_dword (List.range ~stride:(-1) ~stop:`inclusive topdword 0)) in
      (match vsrc with
       | None -> [assn t dst dwords]
       | Some vdst -> [assn t vdst dwords])
    | Pshufb (exp_type, dst_op, src_op, vsrc) ->
      let op_size = bitwidth_of_type exp_type in
      let index_bits = match op_size with
        | 64 -> 3
        | 128 | 256 -> 4
        | _ -> disfailwith "invalid size for pshufb" in
      let foreach_byte f = List.concat @@ List.init (op_size / 8) ~f in

      let src = op2e exp_type src_op in
      let dst = op2e exp_type dst_op in
      let dst_op = Option.value ~default:dst_op vsrc in
      let zero = Bil.int (Word.zero op_size) in
      let msb_one = Bil.int (Word.of_int ~width:8 0x80) in

      let byte_t = Type.imm 8 in
      let byte = int_exp 8 8 in
      let iv = tmp ~name:"i" byte_t in
      let mask_byte_i = tmp byte_t in
      let tmp_byte = tmp exp_type in
      let tmp_dst = tmp exp_type in
      let ind = tmp byte_t in

      let check_mem_alignment = match src_op with
        | Oaddr addr when (op_size = 128) ->
          let word_size = width_of_mode mode in
          let zero = Bil.int (Word.zero word_size) in
          let oxf = Bil.int (Word.of_int ~width:word_size 0xf) in
          [ Bil.(if_ (oxf land addr <> zero) [cpuexn 13] []) ]
        | _ -> [] in

      List.concat [
        check_mem_alignment;
        [Bil.move tmp_dst zero];
        foreach_byte (fun i ->
            Bil.[
              iv := int (Word.of_int ~width:8 i);
              mask_byte_i := extract 7 0 (src lsr (var iv * byte));
              if_ (msb_one land var mask_byte_i = msb_one) [
                tmp_byte := zero;
              ] (* else *) [
                ind := cast unsigned 8 (extract index_bits 0 (var mask_byte_i));
                tmp_byte :=
                  cast unsigned op_size (extract 7 0 (dst lsr (var ind * byte)))
              ];
              tmp_dst := Bil.(var tmp_dst lor (var tmp_byte lsl (var iv * byte)));
            ]);
        [assn exp_type dst_op (Bil.var tmp_dst)]
      ]

    | Lea(t, r, a) when List.is_empty pref ->
      (* See Table 3-64 *)
      (* previously, it checked whether addrbits > opbits before the cast_low.
       * The conclusion we came to was that
         - if they were equal, the cast is basically a nop
         - if it was the other way round, you want to extend it anyway.
       * (I may be remembering things wrongly) *)
      [assn t r Bil.(Cast (LOW, !!t, a))]
    | Call(o1, ra)  ->
      (* If o1 is an immediate, we should syntactically have Jump(imm)
         so that the CFG algorithm knows where the jump goes.  Otherwise
         it will point to BB_Indirect.

         Otherwise, we should evaluate the operand before decrementing esp.
         (This really only matters when esp is the base register of a memory
         lookup. *)
      let target = op2e mt o1 in
      (match o1 with
       | Oimm _ ->
         [Bil.Move (rsp, Bil.(rsp_e - (Int (mi (bytes_of_width mt)))));
          store_s mode None mt rsp_e (Bil.Int ra);
          Bil.Jmp target]
       | _ ->
         let t = tmp mt in
         [Bil.Move (t, target);
          Bil.Move (rsp, Bil.(rsp_e - (Int (mi (bytes_of_width mt)))));
          store_s mode None mt rsp_e (Bil.Int ra);
          Bil.Jmp (Bil.Var t)])
    | Jump(o) ->
      [Bil.Jmp (jump_target mode ss has_rex o)]
    | Jcc(o, c) ->
      [Bil.If (c, [Bil.Jmp (jump_target mode ss has_rex o)], [])]
    | Setcc(t, o1, c) ->
      [assn t o1 Bil.(Cast (UNSIGNED, !!t, c))]
    | Shift(st, s, dst, shift) ->
      let old = tmp ~name:"tmp" s in
      let s' = !!s in
      let size = int_exp s' s' in
      let s_f = Bil.(match st with
          | LSHIFT -> (lsl)
          | RSHIFT -> (lsr)
          | ARSHIFT -> (asr)
          | _ -> disfailwith "invalid shift type") in
      let dste = op2e s dst in
      let count_mask = Bil.(size - int_exp 1 s') in
      let count = Bil.(op2e s shift land count_mask) in
      let new_of = match st with
        | LSHIFT -> Bil.((Cast (HIGH, !!bool_t, dste)) lxor cf_e)
        | RSHIFT -> Bil.(Cast (HIGH, !!bool_t, var old))
        | ARSHIFT -> exp_false
        | _ -> disfailwith "impossible"
      in
      let new_cf =
        (* undefined for SHL and SHR instructions where the count is greater than
           or equal to the size (in bits) of the destination operand *)
        match st with
        | LSHIFT -> Bil.(Cast (LOW, !!bool_t, var old lsr (size - count)))
        | RSHIFT | ARSHIFT ->
          Bil.(Cast (HIGH, !!bool_t, var old lsl (size - count)))
        | _ -> failwith "impossible"
      in
      Bil.[
        old := dste;
        assn s dst (s_f dste count);
        if_ (count <> int_exp 0 s') [
          cf := new_cf;
          sf := compute_sf dste;
          zf := compute_zf s' dste;
          pf := compute_pf s dste;
          af := unknown "after-shift" bool_t;
          if_ (count = int_exp 1 s') [
            oF := new_of;
          ] [
            oF := unknown "after-shift" bool_t;
          ]
        ] [];
      ]
    | Shiftd(st, s, dst, fill, count) ->
      let was = tmp ~name:"tmp" s in
      let e_dst = op2e s dst in
      let e_fill = op2e s fill in
      let s' = !!s in
      (* Check for 64-bit operand *)
      let size = int_exp s' s' in
      let count_mask = Bil.(size - int_exp 1 s') in
      let e_count = Bil.(op2e s count land count_mask) in
      let new_cf =  match st with
        | LSHIFT -> Bil.(Cast (LOW, !!bool_t, var was lsr (size - e_count)))
        | RSHIFT -> Bil.(Cast (HIGH, !!bool_t, var was lsl (size - e_count)))
        | _ -> disfailwith "impossible" in
      let new_of = Bil.(Cast (HIGH, !!bool_t, (var was lxor e_dst))) in
      let unk_of =
        Bil.Unknown ("OF undefined after shiftd of more then 1 bit", bool_t) in
      let ret1 = match st with
        | LSHIFT -> Bil.(e_fill lsr (size - e_count))
        | RSHIFT -> Bil.(e_fill lsl (size - e_count))
        | _ -> disfailwith "impossible" in
      let ret2 = match st with
        | LSHIFT -> Bil.(e_dst lsl e_count)
        | RSHIFT -> Bil.(e_dst lsr e_count)
        | _ -> disfailwith "impossible" in
      let result = Bil.(ret1 lor ret2) in
      (* SWXXX If shift is greater than the operand size, dst and
         flags are undefined *)
      Bil.[
        was := e_dst;
        assn s dst result;
        if_ (e_count <> int_exp 0 s') [
          cf := new_cf;
          sf := compute_sf e_dst;
          zf := compute_zf s' e_dst;
          pf := compute_pf s e_dst;
          (* For a 1-bit shift, the OF flag is set if a sign change occurred;
             otherwise, it is cleared. For shifts greater than 1 bit, the OF flag
             is undefined. *)
          if_ (e_count = int_exp 1 s') [
            oF := new_of;
          ] [
            oF := unk_of;
          ]
        ] []
      ]
    | Rotate(shift_type, exp_type, dst_op, shift_op, use_cf) ->
      if use_cf then unimplemented "rotate use_vf";
      let word_size = bitwidth_of_type exp_type in
      let mask_size = word_size - 1 in
      let count_var = tmp exp_type in
      let count = Bil.var count_var in
      let zero = int_exp 0 word_size in
      let one  = int_exp 1 word_size in
      let dst  = op2e exp_type dst_op in
      let shift_mask = int_exp mask_size word_size in
      let size = int_exp word_size word_size in
      let shift = op2e exp_type shift_op in

      if [%compare.equal: binop] shift_type LSHIFT then
        Bil.[
          count_var := (shift land shift_mask) mod size;
          assn exp_type dst_op ((dst lsl count) lor (dst lsr (size - count)));
          if_ (count = zero) [
            cf := cast low 1 dst;
          ] (* else *) [
            if_ (count = one) [
              oF := cf_e lxor (cast high 1 dst);
            ]  (* else  *) [
              oF := unknown "OF undefined after rotate of more then 1 bit" bool_t;
            ]
          ]
        ]
      else
        Bil.[
          count_var := (shift land shift_mask) mod size;
          assn exp_type dst_op ((dst lsr count) lor (dst lsl (size - count)));
          if_ (count = zero) [
            cf := cast high 1 dst;
          ] (* else *) [
            if_ (count = one) [
              oF := cast high 1 dst lxor (cast high 1 (dst lsl int_exp 1 word_size));
            ]  (* else  *) [
              oF := unknown "OF undefined after rotate of more then 1 bit" bool_t;
            ]
          ]
        ]
    | Bt(t, bitoffset, bitbase) ->
      let t' = !!t in
      let offset = op2e t bitoffset in
      let value, shift = match bitbase with
        | Oreg _ ->
          let reg = op2e t bitbase in
          let shift = Bil.(offset land int_exp Stdlib.(t' - 1) t') in
          reg, shift
        | Oaddr a ->
          let offset = Bil.(cast unsigned (width_of_mode mode) offset) in
          let byte = load (size_of_typ reg8_t) Bil.(a + offset lsr int_exp 3 t') in
          let shift = Bil.(Cast (LOW, !!reg8_t, offset) land int_exp 7 8) in
          byte, shift
        | Ovec _ | Oseg _ | Oimm _ -> disfailwith "Invalid bt operand"
      in
      [
        Bil.Move (cf, Bil.(Cast (LOW, !!bool_t, value lsr shift)));
        Bil.Move (oF, Bil.Unknown ("OF undefined after bt", bool_t));
        Bil.Move (sf, Bil.Unknown ("SF undefined after bt", bool_t));
        Bil.Move (af, Bil.Unknown ("AF undefined after bt", bool_t));
        Bil.Move (pf, Bil.Unknown ("PF undefined after bt", bool_t))
      ]
    | Bs(t, dst, src, dir) ->
      let is_zero_count = List.rev pref |> List.exists ~f:Int.((=) 0xf3) in
      let width = !!t in
      let src_e = op2e t src in
      let is_fwd = match dir with
        | Backward -> false
        | Forward -> true in
      let res = tmp t in
      let assn_res = Bil.(res := src_e) in
      let scan_bil =
        if is_fwd then match width with
          | 16 -> ctz16 res
          | 32 -> ctz32 res
          | 64 -> ctz64 res
          | _ -> disfailwith "Invalid bitscan width"
        else match width with
          | 16 -> clz16 res
          | 32 -> clz32 res
          | 64 -> clz64 res
          | _ -> disfailwith "Invalid bitscan width" in
      let assn_result =
        let n1 = width - 1 in
        assn t dst @@ if is_zero_count || is_fwd then Bil.var res
        else Bil.(var res lxor int Word.(of_int n1 ~width)) in
      let bil = assn_res :: scan_bil in
      let bil =
        if is_zero_count then bil @ Bil.[
            assn_result;
            cf := var res = int Word.(of_int ~width width);
            zf := var res = int Word.(zero width);
          ]
        else Bil.[
            if_ (src_e = int (Word.zero width)) [
              zf := int Word.b1;
              assn t dst @@ unknown "bits" t;
            ] (bil @ Bil.[assn_result; zf := int Word.b0]);
          ] in
      bil @ bitscan_flags is_zero_count
    | Hlt -> [] (* x86 Hlt is essentially a NOP *)
    | Rdtsc ->
      let undef reg = assn reg32_t reg (Bil.Unknown ("rdtsc", reg32_t)) in
      List.map ~f:undef [o_rax; o_rdx]
    | Cpuid ->
      let undef reg = assn reg32_t reg (Bil.Unknown ("cpuid", reg32_t)) in
      List.map ~f:undef [o_rax; o_rbx; o_rcx; o_rdx]
    | Xgetbv ->
      let undef reg = assn reg32_t reg (Bil.Unknown ("xgetbv", reg32_t)) in
      List.map ~f:undef [o_rax; o_rdx]
    | Stmxcsr (dst) ->
      let dst = match dst with
        | Oaddr addr -> addr
        | _ -> disfailwith "stmxcsr argument cannot be non-memory"
      in
      [store reg32_t dst (Bil.Var mxcsr);(*(Unknown ("stmxcsr", reg32_t));*) ]
    | Ldmxcsr (src) ->
      let src = match src with
        | Oaddr addr -> addr
        | _ -> disfailwith "ldmxcsr argument cannot be non-memory"
      in
      [ Bil.Move (mxcsr, load (size_of_typ reg32_t) src); ]
    | Fnstcw (dst) ->
      let dst = match dst with
        | Oaddr addr -> addr
        | _ -> disfailwith "fnstcw argument cannot be non-memory"
      in
      [store reg16_t dst (Bil.Var fpu_ctrl); ]
    | Fldcw (src) ->
      let src = match src with
        | Oaddr addr -> addr
        | _ -> disfailwith "fldcw argument cannot be non-memory"
      in
      [ Bil.Move (fpu_ctrl, load (size_of_typ reg16_t) src); ]
    | Fld _src ->
      unimplemented "unsupported FPU register stack"
    | Fst (_dst,_pop) ->
      unimplemented "unsupported FPU flags"
    | Cmps(Type.Imm _bits as t) ->
      let t' = !!t in
      let src1   = tmp t in
      let src2   = tmp t in
      let tmpres = tmp t in
      let stmts =
        Bil.Move (src1, op2e t (Oaddr rsi_e))
        :: Bil.Move (src2, op2e_s mode seg_es has_rex t (Oaddr rdi_e))
        :: Bil.Move (tmpres, Bil.(Var src1 - Var src2))
        :: string_incr mode t rsi
        :: string_incr mode t rdi
        :: set_flags_sub t' (Bil.Var src1) (Bil.Var src2) (Bil.Var tmpres)
      in
      begin match pref with
        | [] -> stmts
        | [single] when single = repz || single = repnz ->
          rep_wrap ~mode ~check_zf:single ~addr ~next stmts
        | _ -> unimplemented "unsupported flags in cmps" end
    | Scas(Type.Imm _bits as t) ->
      let t' = !!t in
      let src1   = tmp t in
      let src2   = tmp t in
      let tmpres = tmp t in
      let stmts =
        let open Stmt in
        Move (src1, Bil.(Cast (LOW, !!t, Var rax)))
        :: Move (src2, op2e_s mode seg_es has_rex t (Oaddr rdi_e))
        :: Move (tmpres, Bil.(Var src1 - Var src2))
        :: string_incr mode t rdi
        :: set_flags_sub t' (Bil.Var src1) (Bil.Var src2) (Bil.Var tmpres)
      in
      begin match pref with
        | [] -> stmts
        | [single] when single = repz || single = repnz ->
          rep_wrap ~mode ~check_zf:single ~addr ~next stmts
        | _ -> unimplemented "unsupported flags in scas" end
    | Stos(Type.Imm _bits as t) ->
      let stmts = [store_s mode seg_es t rdi_e (op2e t o_rax);
                   string_incr mode t rdi]
      in
      begin match pref with
        | [] -> stmts
        | [single] when single = repz -> rep_wrap ~mode ~addr ~next stmts
        | _ -> unimplemented "unsupported prefix for stos" end
    | Push(t, o) ->
      let o = if is_small_imm o t then sign_extend_imm o t
        else op2e t o in
      let tmp = tmp t in (* only really needed when o involves esp *)
      Bil.Move (tmp, o)
      :: Bil.Move (rsp, Bil.(rsp_e - Int (mi (bytes_of_width t))))
      :: store_s mode seg_ss t rsp_e (Bil.var tmp) (* FIXME: can ss be overridden? *)
      :: []
    | Pop(t, o) ->
      (* From the manual:

         "The POP ESP instruction increments the stack pointer (ESP)
         before data at the old top of stack is written into the
         destination"

         So, effectively there is no incrementation.
      *)
      assn t o (load_s mode seg_ss (size_of_typ t) rsp_e)
      :: if [%compare.equal: operand] o o_rsp then []
      else [Bil.Move (rsp, Bil.(rsp_e + Int (mi (bytes_of_width t))))]
    | Pushf(t) ->
      (* Note that we currently treat these fields as unknowns, but the
         manual says: When copying the entire EFLAGS register to the
         stack, the VM and RF flags (bits 16 and 17) are not copied;
         instead, the values for these flags are cleared in the EFLAGS
         image stored on the stack. *)
      let flags_e = match t with
        | Type.Imm 16 -> flags_e
        | Type.Imm 32 -> eflags_e
        | Type.Imm 64 -> rflags_e
        | _ -> failwith "impossible"
      in
      Bil.Move (rsp, Bil.(rsp_e - Int (mi (bytes_of_width t))))
      :: store_s mode seg_ss t rsp_e flags_e
      :: []
    | Popf t ->
      let assnsf = match t with
        | Type.Imm 16 -> assns_flags_to_bap
        | Type.Imm 32 -> assns_eflags_to_bap
        | Type.Imm 64 -> assns_rflags_to_bap
        | _ -> failwith "impossible"
      in
      let tmp = tmp t in
      let extractlist =
        List.map
          ~f:(fun i ->
              Bil.(Extract (i, i, Var tmp)))
          (List.range ~stride:(-1) ~start:`exclusive ~stop:`inclusive !!t 0)
      in
      Bil.Move (tmp, load_s mode seg_ss (size_of_typ t) rsp_e)
      :: Bil.Move (rsp, Bil.(rsp_e + Int (mi (bytes_of_width t))))
      :: List.concat (List.map2_exn ~f:(fun f e -> f e) assnsf
                        extractlist)
    | Popcnt(t, s, d) ->
      let width = !!t in
      let bits = op2e t s in
      let res = tmp t in
      let assn_src = Bil.(res := bits) in
      let cnt = match width with
        | 16 -> popcnt16 res
        | 32 -> popcnt32 res
        | 64 -> popcnt64 res
        | _ -> disfailwith "Invalid popcnt width" in
      let bil = (assn_src :: cnt) @ Bil.[assn t d (var res)] in
      let flags = List.map ~f:(fun r ->
          Bil.Move (r, int_exp 0 1)) [cf; oF; sf; af; pf] in
      set_zf width bits :: (bil @ flags)
    | Sahf ->
      let assnsf = assns_lflags_to_bap in
      let tah = tmp ~name:"AH" reg8_t in
      let extractlist =
        List.map
          ~f:(fun i ->
              Bil.(Extract (i, i, Var tah)))
          (List.range ~stride:(-1) ~stop:`inclusive 7 0)
      in
      Bil.Move (tah, ah_e)
      :: List.concat (List.map2_exn ~f:(fun f e -> f e) assnsf extractlist)
    | Lahf ->
      let o_ah = Oreg 4 in
      [assn reg8_t o_ah lflags_e]
    | Add(t, o1, o2) ->
      let tmp  = tmp t and tmp2 = tmp t in
      Bil.Move (tmp, op2e t o1)
      :: Bil.Move (tmp2, op2e t o2)
      :: assn t o1 Bil.(op2e t o1 + Var tmp2)
      :: let s1 = Bil.Var tmp in let s2 = Bil.Var tmp2 in let r = op2e t o1 in
      set_flags_add !!t s1 s2 r
    | Adc(t, o1, o2) ->
      let orig1 = tmp t in
      let orig2 = tmp t in
      let bits = !!t in
      let t' = Type.Imm (bits + 1) in
      let c e = Bil.(Cast (UNSIGNED, !!t', e)) in
      (* Literally compute the addition with an extra bit and see
         what the value is for CF *)
      let s1 = Bil.Var orig1 in let s2 = Bil.Var orig2 in let r = op2e t o1 in
      let bige = Bil.(c s1 + c s2 + c (Cast (UNSIGNED, !!t, cf_e))) in
      Bil.Move (orig1, op2e t o1)
      :: Bil.Move (orig2, op2e t o2)
      :: assn t o1 Bil.(s1 + s2 + Cast (UNSIGNED, !!t, cf_e))
      :: Bil.Move (cf, Bil.Extract (bits, bits, bige))
      :: set_aopszf_add !!t s1 s2 r
    | Inc(t, o) (* o = o + 1 *) ->
      let t' = !!t in
      let tmp = tmp t in
      Bil.Move (tmp, op2e t o)
      :: assn t o Bil.(op2e t o + int_exp 1 t')
      :: set_aopszf_add t' (Bil.Var tmp) (int_exp 1 t') (op2e t o)
    | Dec(t, o) (* o = o - 1 *) ->
      let t' = !!t in
      let tmp = tmp t in
      Bil.Move (tmp, op2e t o)
      :: assn t o Bil.(op2e t o - int_exp 1 t')
      :: set_aopszf_sub t' (Bil.Var tmp) (int_exp 1 t') (op2e t o) (* CF is maintained *)
    | Sub(t, o1, o2) (* o1 = o1 - o2 *) ->
      let oldo1 = tmp t in
      let oldo2 = tmp t in
      let op1 = op2e t o1 in
      let op2 =
        if is_small_imm o2 t then sign_extend_imm o2 t
        else op2e t o2 in
      Bil.([
          oldo1 := op1;
          oldo2 := op2;
          assn t o1 (op1 - op2);
        ] @ set_flags_sub (bitwidth_of_type t) (var oldo1) (var oldo2) op1)
    | Sbb(t, o1, o2) ->
      let tmp_s = tmp t in
      let tmp_d = tmp t in
      let orig_s = Bil.Var tmp_s in
      let orig_d = Bil.Var tmp_d in
      let sube = Bil.(orig_s + Cast (UNSIGNED, !!t, cf_e)) in
      let d = op2e t o1 in
      let s1 =
        if is_small_imm o2 t then sign_extend_imm o2 t
        else op2e t o2 in
      Bil.Move (tmp_s, s1)
      :: Bil.Move (tmp_d, d)
      :: assn t o1 Bil.(orig_d - sube)
      :: Bil.Move (oF, Bil.(Cast (HIGH, !!bool_t, (orig_s lxor orig_d) land (orig_d lxor d))))
      (* When src = 0xffffffff and cf=1, the processor sets CF=1.

         Note that we compute dest = dest - (0xffffffff + 1) = 0, so the
         subtraction does not overflow.

         So, I am guessing that CF is set if the subtraction overflows
         or the addition overflows.

         Maybe we should implement this by doing the actual computation,
         like we do for adc.
      *)
      (* sub overflow | add overflow *)
      :: Bil.Move (cf, Bil.((sube > orig_d) lor (sube < orig_s)))
      :: set_apszf !!t orig_s orig_d d
    | Cmp(t, o1, o2) ->
      let tmp = tmp t in
      Bil.Move (tmp, Bil.(op2e t o1 - op2e t o2))
      :: set_flags_sub !!t (op2e t o1) (op2e t o2) (Bil.Var tmp)
    | Cmpxchg(t, src, dst) ->
      let t' = !!t in
      let eax_e = op2e t o_rax in
      let dst_e = op2e t dst in
      let src_e = op2e t src in
      let tmp = tmp t in
      Bil.Move (tmp, Bil.(eax_e - dst_e))
      :: set_flags_sub t' eax_e dst_e (Bil.Var tmp)
      @ assn t dst (Bil.Ite (zf_e, src_e, dst_e))
        :: assn t o_rax (Bil.Ite (zf_e, eax_e, dst_e))
        :: []
    | Cmpxchg8b o -> (* only 32bit case *)
      let accumulator = Bil.Concat((op2e reg32_t o_rdx),(op2e reg32_t o_rax)) in
      let dst_e = op2e reg64_t o in
      let src_e = Bil.Concat((op2e reg32_t o_rcx),(op2e reg32_t o_rbx)) in
      let dst_low_e = Bil.Extract(63, 32, dst_e) in
      let dst_hi_e = Bil.Extract(31, 0, dst_e) in
      let eax_e = op2e reg32_t o_rax in
      let edx_e = op2e reg32_t o_rdx in
      let equal = tmp bool_t in
      let equal_v = Bil.Var equal in
      [
        Bil.Move (equal, Bil.(accumulator = dst_e));
        Bil.Move (zf, equal_v);
        assn reg64_t o (Bil.Ite (equal_v, src_e, dst_e));
        assn reg32_t o_rax (Bil.Ite (equal_v, eax_e, dst_low_e));
        assn reg32_t o_rdx (Bil.Ite (equal_v, edx_e, dst_hi_e))
      ]
    | Xadd(t, dst_op, src_op) ->
      let tmp_res = tmp t in
      let tmp_dst = tmp t in
      let tmp_src = tmp t in
      let dst = op2e t dst_op in
      let src = op2e t src_op in
      Bil.[
        tmp_src := src;
        tmp_dst := dst;
        tmp_res := src + dst;
        assn t src_op dst;
        assn t dst_op (var tmp_res);
      ] @ set_flags_add !!t (Bil.var tmp_dst) (Bil.var tmp_src) dst;
    | Xchg(t, src, dst) ->
      let tmp = tmp t in
      [ Bil.Move (tmp, op2e t src);
        assn t src (op2e t dst);
        assn t dst (Bil.Var tmp); ]
    | And(t, o1, o2) ->
      assn t o1 Bil.(op2e t o1 land op2e t o2)
      :: Bil.Move (oF, exp_false)
      :: Bil.Move (cf, exp_false)
      :: Bil.Move (af, Bil.Unknown ("AF is undefined after and", bool_t))
      :: set_pszf t (op2e t o1)
    | Or(t, o1, o2) ->
      let o2 =
        if is_small_imm o2 t then sign_extend_imm o2 t
        else op2e t o2 in
      assn t o1 Bil.(op2e t o1 lor o2)
      :: Bil.Move (oF, exp_false)
      :: Bil.Move (cf, exp_false)
      :: Bil.Move (af, Bil.Unknown ("AF is undefined after or", bool_t))
      :: set_pszf t (op2e t o1)
    | Xor(t, o1, o2) when [%compare.equal: operand] o1 o2 ->
      assn t o1 Bil.(Int (BV.of_int ~width:(!!t) 0))
      :: Bil.Move (af, Bil.Unknown ("AF is undefined after xor", bool_t))
      :: List.map ~f:(fun v -> Bil.Move (v, exp_true)) [zf; pf]
      @  List.map ~f:(fun v -> Bil.Move (v, exp_false)) [oF; cf; sf]
    | Xor(t, o1, o2) ->
      assn t o1 Bil.(op2e t o1 lxor op2e t o2)
      :: Bil.Move (oF, exp_false)
      :: Bil.Move (cf, exp_false)
      :: Bil.Move (af, Bil.Unknown ("AF is undefined after xor", bool_t))
      :: set_pszf t (op2e t o1)
    | Test(t, o1, o2) ->
      let o2 =
        if is_small_imm o2 t then sign_extend_imm o2 t
        else op2e t o2 in
      let tmp = tmp t in
      Bil.Move (tmp, Bil.(op2e t o1 land o2))
      :: Bil.Move (oF, exp_false)
      :: Bil.Move (cf, exp_false)
      :: Bil.Move (af, Bil.Unknown ("AF is undefined after and", bool_t))
      :: set_pszf t (Bil.Var tmp)
    | Ptest(t, o1, o2) ->
      let open Stmt in
      let t' = !!t in
      let tmp1 = tmp t and tmp2 = tmp t in
      Move (tmp1, Bil.(op2e t o2 land op2e t o1))
      :: Move (tmp2, Bil.(op2e t o2 land (exp_not (op2e t o1))))
      :: Move (af, exp_false)
      :: Move (oF, exp_false)
      :: Move (pf, exp_false)
      :: Move (sf, exp_false)
      :: Move (zf, Bil.(Var tmp1 = Int (BV.of_int ~width:t' 0)))
      :: [Move (cf, Bil.(Var tmp2 = Int (BV.of_int ~width:t' 0)))]
    | Not(t, o) ->
      [assn t o (exp_not (op2e t o))]
    | Neg(t, o) ->
      let t' = !!t in
      let tmp = tmp t in
      let min_int =
        Bil.BinOp (LSHIFT, int_exp 1 t', int_exp (t'-1) t')
      in
      Bil.Move (tmp, op2e t o)
      ::assn t o Bil.(int_exp 0 t' - op2e t o)
      ::Bil.Move (cf, Bil.(Ite (Var tmp = int_exp 0 t', int_exp 0 1, int_exp 1 1)))
      ::Bil.Move (oF, Bil.(Ite (Var tmp = min_int, int_exp 1 1, int_exp 0 1)))
      ::set_apszf_sub t' (Bil.Var tmp) (int_exp 0 t') (op2e t o)
    | Mul (t, src) ->
      (* Mul always multiplies EAX by src and stores the result in EDX:EAX
         starting from the "right hand side" based on the type t of src *)

      (* The OF and CF flags are set to 0 if the upper half of the result is 0;
         otherwise, they are set to 1 *)
      let new_t = Type.Imm (!!t * 2) in
      let assnstmts, assne = Bil.(assn_dbl t ((Cast (UNSIGNED, !!new_t, op2e t o_rax)) * (Cast (UNSIGNED, !!new_t, op2e t src))))
      in
      let flag =
        let highbit = !!new_t - 1 in
        let lowbit = !!new_t / 2 in
        Bil.((Extract (highbit, lowbit, assne)) <> int_exp 0 !!t)
      in
      assnstmts
      @
      [
        Bil.Move (oF, flag);
        Bil.Move (cf, flag);
        Bil.Move (sf, Bil.Unknown ("SF is undefined after Mul", bool_t));
        Bil.Move (zf, Bil.Unknown ("ZF is undefined after Mul", bool_t));
        Bil.Move (af, Bil.Unknown ("AF is undefined after Mul", bool_t));
        Bil.Move (pf, Bil.Unknown ("PF is undefined after Mul", bool_t))
      ]
    | Imul (t, (oneopform, dst), src1, src2) ->
      let new_t = Type.Imm (!!t * 2) in
      let mul_stmts =
        (match oneopform with
         | true ->
           (* For one operand form, use assn_double *)
           let assnstmts, assne =
             assn_dbl t Bil.((Cast (SIGNED, !!new_t, op2e t src1)) * (Cast (SIGNED, !!new_t, op2e t src2))) in
           let flag =
             (* Intel checks if EAX == EDX:EAX.  Instead of doing this, we are just
                going to check if the upper bits are != 0 *)
             let highbit = !!new_t - 1 in
             let lowbit = !!new_t / 2 in
             Bil.((Extract (highbit, lowbit, assne)) <> int_exp 0 !!t)
           in
           assnstmts @
           [Bil.Move (oF, flag);
            Bil.Move (cf, flag)]
         | false ->
           (* Two and three operand forms *)
           let tmp = tmp new_t in
           (* Flag is set when the result is truncated *)
           let flag = Bil.(Var tmp <> Cast (SIGNED, !!new_t, op2e t dst)) in
           [(Bil.Move (tmp, Bil.((Cast (SIGNED, !!new_t, op2e t src1)) * (Cast (SIGNED, !!new_t, op2e t src2)))));
            (assn t dst Bil.(Cast (LOW, !!t, Var tmp)));
            Bil.Move (oF, flag);
            Bil.Move (cf, flag)] )
      in
      mul_stmts@[
        Bil.Move (pf, Bil.Unknown ("PF is undefined after imul", bool_t));
        Bil.Move (sf, Bil.Unknown ("SF is undefined after imul", bool_t));
        Bil.Move (zf, Bil.Unknown ("ZF is undefined after imul", bool_t));
        Bil.Move (af, Bil.Unknown ("AF is undefined after imul", bool_t));]
    | Div(t, src) ->
      let dt' = !!t * 2 in
      let dt = Type.Imm dt' in
      let zero = int_exp 0 dt' in
      let dividend = tmp ~name:"dividend" dt in
      let divisor = tmp ~name:"divisor" dt in
      let tdiv = tmp ~name:"div" dt in
      let trem = tmp ~name:"rem" dt in
      let result = Bil.(cast low!!t (var trem) ^ cast low !!t (var tdiv)) in
      let apply_result = fst (assn_dbl t result) in
      Bil.[
        divisor := cast unsigned !!dt (op2e t src);
        dividend := op2e_dbl t;
        if_ (var divisor = zero) [
          Cpu_exceptions.divide_by_zero
        ](* else *) [
          tdiv := var dividend / var divisor;
          trem := var dividend mod var divisor;
          if_ (cast high !!t (var tdiv) = int_exp 0 !!t)
            apply_result (* else *)
            [Cpu_exceptions.divide_by_zero]
        ]
      ] @ undefine [cf; oF; sf; zf; af; pf]
    | Idiv(t, src) ->
      let dt' = !!t * 2 in
      let dt = Type.Imm dt' in
      let zero = int_exp 0 dt' in
      let dividend = tmp ~name:"dividend" dt in
      let divisor = tmp ~name:"divisor" dt in
      let tdiv = tmp ~name:"div" dt in
      let trem = tmp ~name:"rem" dt in
      let result = Bil.(cast low !!t (var trem) ^ cast low !!t (var tdiv)) in
      let apply_result = fst @@ assn_dbl t result in
      let lbound =
        let dtm1 = dt' - 1 in
        Word.(one dt' lsl of_int ~width:dt' dtm1) in
      let ubound = Word.(lbound - one dt') in
      Bil.[
        divisor := cast signed !!dt (op2e t src);
        dividend := op2e_dbl t;
        if_ (var divisor = zero) [
          Cpu_exceptions.divide_by_zero
        ] (* else *) [
          tdiv := var dividend /$ var divisor;
          trem := var dividend %$ var divisor;
          if_ ((var tdiv >$ int ubound) lor (var tdiv <$ int lbound)) [
            Cpu_exceptions.divide_by_zero;
          ] (* else *)
            apply_result;
        ];
      ] @ undefine [cf; oF; sf; zf; af; pf]
    | Cld ->
      [Bil.Move (df, exp_false)]
    | Leave t when List.is_empty pref -> (* #UD if Lock prefix is used *)
      Bil.Move (rsp, rbp_e)
      ::to_ir mode addr next ss pref has_rex has_vex (Pop(t, o_rbp))
    | Interrupt3 -> [Bil.CpuExn 3]
    | Interrupt(Oimm i) -> [
        match Addr.to_int i with
        | Ok i -> Bil.cpuexn i
        | Error _ ->
          let dst = asprintf "interrupt@%s" (Addr.string_of_value i) in
          Bil.(encode call dst)
      ]
    | Sysenter | Syscall -> [
        Bil.(encode call "syscall")
      ]
    (* Match everything exhaustively *)
    | Leave _ ->  unimplemented "to_ir: Leave"
    | Lea _ ->  unimplemented "to_ir: Lea"
    | Movs _ ->  unimplemented "to_ir: Movs"
    | Cmps _ ->  unimplemented "to_ir: Cmps"
    | Scas _ ->  unimplemented "to_ir: Scas"
    | Stos _ ->  unimplemented "to_ir: Stos"
    | Retn _ ->  unimplemented "to_ir: Retn"
    | Interrupt _ ->  unimplemented "to_ir: Interrupt"

(* https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/plugins/x86/x86_disasm.ml#L420-L1185 *)
(* BinaryAnalysisPlatform/bap plugins/x86/x86_disasm.ml:420-1185 *)
  let get_opcode _pref ({rex; vex; rm_extend; addrsize; _} as prefix) a =
    let parse_disp_addr, parse_modrm_addr, parse_modrmseg_addr,
        parse_modrmext_addr =
      let open Type in
      match addrsize with
      | Imm 16 -> parse_disp16, parse_modrm16 rex, parse_modrm16seg rex, parse_modrm16ext rex
      | Imm 32 -> parse_disp32, parse_modrm3264 rex vex addrsize a, parse_modrm3264seg rex vex addrsize a, parse_modrm3264ext rex vex addrsize a
      | Imm 64 -> parse_disp64, parse_modrm3264 rex vex addrsize a, parse_modrm3264seg rex vex addrsize a, parse_modrm3264ext rex vex addrsize a
      | _ -> failwith "Bad address type"
    in
    let parse_modrm_vec = parse_modrm3264_vec rex vex addrsize a in
    let mi = int_of_mode mode in
    let _mi64 = int64_of_mode mode in
    let mbi = big_int_of_mode mode in
    let mt = type_of_mode mode in
    (* A VEX prefix always implies the first byte of 0x0f *)
    let b1, na = if Option.is_some vex then 0x0f, a else Char.to_int (g a), s a in
    match b1 with (* Table A-2 *)
    (*** 00 to 3d are near the end ***)
    | 0x40 | 0x41 | 0x42 | 0x43 | 0x44 | 0x45 | 0x46 | 0x47 ->
      (Inc(prefix.opsize, Oreg(rm_extend lor (b1 land 7))), na)
    | 0x48 | 0x49 | 0x4a | 0x4b | 0x4c | 0x4d | 0x4e | 0x4f ->
      (Dec(prefix.opsize, Oreg(rm_extend lor (b1 land 7))), na)
    | 0x50 | 0x51 | 0x52 | 0x53 | 0x54 | 0x55 | 0x56 | 0x57 ->
      (Push(prefix.bopsize, Oreg(rm_extend lor (b1 land 7))), na)
    | 0x58 | 0x59 | 0x5a | 0x5b | 0x5c | 0x5d | 0x5e | 0x5f ->
      (Pop(prefix.bopsize, Oreg(rm_extend lor (b1 land 7))), na)
    | 0x63 when [%compare.equal: mode] mode X8664 ->
      let (r, rm, na) = parse_modrm_addr None na in
      (Movsx(prefix.opsize, r, reg32_t, rm), na)
    | 0x68 | 0x6a  ->
      let (o, na) =
        (* SWXXX Sign extend these? *)
        if b1=0x68 then parse_immz prefix.opsize na else parse_immb na
      in
      let size = match mode with
        | X86 -> prefix.opsize
        | X8664 -> reg64_t
      in
      (Push(size, o), na)
    | 0x69 | 0x6b ->
      let it =
        if b1 = 0x6b
        then reg8_t
        else if Type.equal prefix.opsize reg16_t then reg16_t
        else reg32_t
      in
      let (r, rm, na) = parse_modrm_addr (Some it) na in
      let (o, na) = parse_simm it na in
      (Imul(prefix.opsize, (false,r), rm, (oimm_resize o prefix.opsize)), na)
    | 0x70 | 0x71 | 0x72 | 0x73 | 0x74 | 0x75 | 0x76 | 0x77 | 0x78 | 0x79
    | 0x7a | 0x7b | 0x7c | 0x7d | 0x7e | 0x7f ->
      let (i,na) = parse_disp8 na in
      (Jcc(Jabs(Oimm(add_to_addr na i)), cc_to_exp b1), na)
    | 0x80 | 0x81 | 0x82 | 0x83 ->
      let it = match b1 with
        | 0x81 -> if Type.equal prefix.opsize reg64_t then reg32_t else prefix.opsize
        | _ -> reg8_t
      in
      let (r, rm, na) = parse_modrmext_addr (Some it) na in
      let (o, na) = parse_immz it na in
      let (o2, na) = ((oimm_resize o prefix.opsize), na) in
      let opsize = if b1 land 1 = 0 then reg8_t else prefix.opsize in
      (match r with (* Grp 1 *)
       | 0 -> (Add(opsize, rm, o2), na)
       | 1 -> (Or(opsize, rm, o2), na)
       | 2 -> (Adc(opsize, rm, o2), na)
       | 3 -> (Sbb(opsize, rm, o2), na)
       | 4 -> (And(opsize, rm, o2), na)
       | 5 -> (Sub(opsize, rm, o2), na)
       | 6 -> (Xor(opsize, rm, o2), na)
       | 7 -> (Cmp(opsize, rm, o2), na)
       | _ -> disfailwith
                (Printf.sprintf "impossible Grp 1 opcode: %02x/%d" b1 r)
      )
    | 0x84
    | 0x85 -> let (r, rm, na) = parse_modrm_addr None na in
      let o = if b1 = 0x84 then reg8_t else prefix.opsize in
      (Test(o, rm, r), na)
    | 0x87 -> let (r, rm, na) = parse_modrm_addr None na in
      (Xchg(prefix.opsize, r, rm), na)
    | 0x88 -> let (r, rm, na) = parse_modrm_addr None na in
      (Mov(reg8_t, rm, r, None), na)
    | 0x89 -> let (r, rm, na) = parse_modrm_addr None na in
      (Mov(prefix.opsize, rm, r, None), na)
    | 0x8a -> let (r, rm, na) = parse_modrm_addr None na in
      (Mov(reg8_t, r, rm, None), na)
    | 0x8b -> let (r, rm, na) = parse_modrm_addr None na in
      (Mov(prefix.opsize, r, rm, None), na)
    | 0x8c -> let (r, rm, na) = parse_modrmseg_addr None na in
      let extend = if Type.equal prefix.opsize reg64_t then reg64_t else reg16_t in
      (Mov(extend, rm, r, None), na)
    | 0x8d -> let (r, rm, na) = parse_modrm_addr None na in
      (match rm with
       | Oaddr a -> (Lea(prefix.opsize, r, a), na)
       | _ -> disfailwith "invalid lea (must be address)")
    | 0x8e -> let (r, rm, na) = parse_modrmseg_addr None na in
      (Mov(reg16_t, r, rm, None), na)
    | 0x90 -> (Nop, na)
    | 0x91 | 0x92 | 0x93 | 0x94 | 0x95 | 0x96 | 0x97 ->
      let reg = Oreg (rm_extend lor (b1 land 7)) in
      (Xchg(prefix.opsize, o_rax, reg), na)
    | 0x98 -> let srct =
                let open Type in
                match prefix.opsize with
                | Imm 16 -> reg8_t
                | Imm 32 -> reg16_t
                | Imm 64 -> reg32_t
                | _ -> disfailwith "invalid opsize for CBW/CWDE/CWQE"
      in
      (Movsx(prefix.opsize, o_rax, srct, o_rax), na)
    | 0x9c -> (Pushf(prefix.bopsize), na)
    (* Intel says that popfq needs to have a REX.W prefix, but gas
       and clang both insist that no prefix is needed! *)
    | 0x9d -> (Popf(prefix.bopsize), na)
    | 0x9e -> (Sahf, na)
    | 0x9f -> (Lahf, na)
    | 0xa0 | 0xa1 ->
      let t = if b1 = 0xa0 then Type.imm 8 else prefix.opsize in
      let (addr, na) = parse_disp_addr na in
      (Mov(t, o_rax, Oaddr (mbi addr |> Bil.int), None), na)
    | 0xa2 | 0xa3 ->
      let t = if b1 = 0xa2 then reg8_t else prefix.opsize in
      let (addr, na) = parse_disp_addr na in
      (Mov(t, Oaddr (mbi addr |> Bil.int), o_rax, None), na)
    | 0xa4 -> (Movs reg8_t, na)
    | 0xa5 -> (Movs prefix.opsize, na)
    | 0xa6 -> (Cmps reg8_t, na)
    | 0xa7 -> (Cmps prefix.opsize, na)
    | 0xae -> (Scas reg8_t, na)
    | 0xaf -> (Scas prefix.opsize, na)
    | 0xa8 -> let (i, na) = parse_imm8 na in
      (Test(reg8_t, o_rax, i), na)
    | 0xa9 -> let it = if Type.equal prefix.opsize reg64_t then reg32_t else prefix.opsize in
      let (i,na) = parse_immz it na in
      (Test(prefix.opsize, o_rax, oimm_resize i prefix.opsize), na)
    | 0xaa -> (Stos reg8_t, na)
    | 0xab -> (Stos prefix.opsize, na)
    | 0xb0 | 0xb1 | 0xb2 | 0xb3 | 0xb4 | 0xb5 | 0xb6
    | 0xb7 -> let (i, na) = parse_imm8 na in
      (Mov(reg8_t, Oreg(rm_extend lor (b1 land 7)), i, None), na)
    | 0xb8 | 0xb9 | 0xba | 0xbb | 0xbc | 0xbd | 0xbe
    | 0xbf -> let (i, na) = parse_immv prefix.opsize na in
      (Mov(prefix.opsize, Oreg(rm_extend lor (b1 land 7)), i, None), na)
    | 0xc2 | 0xc3 (* Near ret *)
    | 0xca | 0xcb (* Far ret *) ->
      let far_ret = if (b1 = 0xc2 || b1 = 0xc3) then false else true in
      if (b1 = 0xc3 || b1 = 0xcb) then (Retn(None, far_ret), na)
      else let (imm,na) = parse_immw na in
        (Retn(Some(mt, imm), far_ret), na)
    | 0xc6
    | 0xc7 -> let t = if b1 = 0xc6 then reg8_t else prefix.opsize in
      let it = match b1 with
        | 0xc6 -> reg8_t
        | 0xc7 when prefix.opsize_override -> reg16_t
        | 0xc7 -> reg32_t
        | _ -> failwith "impossible"
      in
      let (e, rm, na) = parse_modrmext_addr (Some it) na in
      let (i,na) = parse_immz it na in
      (match e with (* Grp 11 *)
       | 0 -> (Mov(t, rm, oimm_resize i t, None), na)
       | _ -> disfailwith (Printf.sprintf "Invalid opcode: %02x/%d" b1 e)
      )
    | 0xc9 -> (Leave (type_of_mode mode), na)
    | 0xcc -> (Interrupt3, na)
    | 0xcd -> let (i,na) = parse_imm8 na in
      (Interrupt(i), na)

    (* 0xd8-0xdf can be followed by a secondary opcode, OR a modrm
       byte. But the secondary opcode is only used when the modrm
       byte does not specify a memory address. *)
    | 0xd8 | 0xd9 | 0xda | 0xdb | 0xdc | 0xdd | 0xde | 0xdf ->
      let b2, _ = parse_int8 na in
      let (r, rm, na) = parse_modrmext_addr None na in
      (match r, rm with
       | 2, Oaddr _ ->
         (match b1 with
          | 0xd9 | 0xdd -> (Fst(rm, false), na)
          | _ ->
            unimplemented (Printf.sprintf "unsupported opcode: %02x/%d" b1 r)
         )
       | 3, Oaddr _ ->
         (match b1 with
          | 0xd9 | 0xdd -> (Fst(rm, true), na)
          | _ ->
            unimplemented (Printf.sprintf "unsupported opcode: %02x/%d" b1 r)
         )
       | 5, Oaddr _ ->
         (match b1 with
          | 0xd9 -> (Fldcw rm, na)
          | 0xdb -> (Fld rm, na)
          | _ ->
            unimplemented (Printf.sprintf "unsupported opcode: %02x/%d" b1 r)
         )
       | 7, Oaddr _ ->
         (match b1 with
          | 0xd9 -> (Fnstcw rm, na)
          | 0xdb -> (Fst(rm, true), na)
          | _ ->
            unimplemented (Printf.sprintf "unsupported opcode: %02x/%d" b1 r)
         )
       | _, Oaddr _ ->
         unimplemented (Printf.sprintf "unsupported opcode: %02x/%d" b1 r)
       | _, _ ->
         unimplemented (Printf.sprintf "unsupported opcode: %02x %s" b1 (Word.string_of_value b2))
      )

    | 0xe8 -> let t = expanded_jump_type prefix.opsize in
      let (i,na) = parse_disp t na in
      (* I suppose the width of the return address should be addrsize *)
      (Call (Oimm (add_to_addr na i), resize_word na !!addrsize), na)
    | 0xe9 -> let t = expanded_jump_type prefix.opsize in
      let (i,na) = parse_disp t na in
      (Jump (Jabs (Oimm (add_to_addr na i))), na)
    | 0xeb -> let (i,na) = parse_disp8 na in
      (Jump (Jabs (Oimm (add_to_addr na i))), na)
    | 0xc0 | 0xc1
    | 0xd0 | 0xd1 | 0xd2
    | 0xd3 -> let immoff = if (b1 land 0xfe) = 0xc0 then Some reg8_t else None in
      let (r, rm, na) = parse_modrmext_addr immoff na in
      let opsize = if (b1 land 1) = 0 then reg8_t else prefix.opsize in
      let (amt, na) = match b1 land 0xfe with
        | 0xc0 -> parse_imm8 na
        | 0xd0 -> (Oimm Addr.b1, na)
        | 0xd2 -> (o_rcx, na)
        | _ ->
          disfailwith (Printf.sprintf "impossible opcode: %02x/%d" b1 r)
      in
      let open Bil in
      (match r with (* Grp 2 *)
       | 0 -> (Rotate(LSHIFT, opsize, rm, amt, false),na)
       | 1 -> (Rotate(RSHIFT, opsize, rm, amt, false),na)
       (* SWXXX Implement these *)
       | 2 -> unimplemented
                (* (Rotate(LSHIFT, opsize, rm, amt, true),na) *)
                (Printf.sprintf "unsupported opcode: %02x/%d" b1 r)
       | 3 -> unimplemented
                (* (Rotate(RSHIFT, opsize, rm, amt, true),na) *)
                (Printf.sprintf "unsupported opcode: %02x/%d" b1 r)
       | 4 -> (Shift(LSHIFT, opsize, rm, amt), na)
       | 5 -> (Shift(RSHIFT, opsize, rm, amt), na)
       | 7 -> (Shift(ARSHIFT, opsize, rm, amt), na)
       | _ -> disfailwith
                (Printf.sprintf "impossible opcode: %02x/%d" b1 r)
      )
    | 0xe3 ->
      let t = !!addrsize in
      let rcx_e = Bil.var R.rcx in
      let (i,na) = parse_disp8 na in
      (* (Jcc (Jrel (BV.litz na t, BV.litz i t), Bop.(rcx_e = Int (mi 0))), na) *)
      (Jcc (Jrel (resize_word na t, resize_word i t), Bil.(rcx_e = (mi 0 |> int))), na)
    | 0xf4 -> (Hlt, na)
    | 0xf6
    | 0xf7 -> let t = if b1 = 0xf6 then reg8_t else prefix.opsize in
      let it = if Type.equal t reg64_t then reg32_t else t in
      let (r, rm, na) = parse_modrmext_addr (Some it) na in
      (match r with (* Grp 3 *)
       | 0 ->
         let (imm, na) = parse_immz it na in
         (Test(t, rm, oimm_resize imm t), na)
       | 2 -> (Not(t, rm), na)
       | 3 -> (Neg(t, rm), na)
       | 4 ->
         (match b1 with
          | 0xf6 -> (Mul(t, rm), na)
          | 0xf7 -> (Mul(t, rm), na)
          | _ -> disfailwith
                   (Printf.sprintf "impossible opcode: %02x/%d" b1 r)
         )
       | 5 ->
         (match b1 with
          | 0xf6 -> (Imul(t, (true,o_rax), o_rax, rm), na)
          | 0xf7 -> (Imul(t, (true,o_rdx), o_rax, rm), na)
          | _ -> disfailwith
                   (Printf.sprintf "impossible opcode: %02x/%d" b1 r)
         )
       | 6 ->
         (match b1 with
          | 0xf6 -> (Div(reg8_t, rm) , na)
          | 0xf7 -> (Div(t, rm), na)
          | _ -> disfailwith
                   (Printf.sprintf "impossible opcode: %02x/%d" b1 r)
         )
       | 7 ->
         (match b1 with
          | 0xf6 -> (Idiv(reg8_t, rm) , na)
          | 0xf7 -> (Idiv(t, rm), na)
          | _ -> disfailwith
                   (Printf.sprintf "impossible opcode: %02x/%d" b1 r)
         )
       | _ ->
         disfailwith (Printf.sprintf "impossible opcode: %02x/%d" b1 r)
      )
    | 0xfc -> (Cld, na)
    | 0xfe -> let (r, rm, na) = parse_modrmext_addr None na in
      (match r with (* Grp 4 *)
       | 0 -> (Inc (reg8_t, rm), na)
       | 1 -> (Dec (reg8_t, rm), na)
       | _ -> disfailwith
                (Printf.sprintf "impossible opcode: %02x/%d" b1 r)
      )
    | 0xff -> let (r, rm, na) = parse_modrmext_addr None na in
      let t = !!addrsize in
      (match r with (* Grp 5 *)
       | 0 -> (Inc (prefix.opsize, rm), na)
       | 1 -> (Dec (prefix.opsize, rm), na)
       | 2 -> (Call (rm, resize_word na t), na)
       | 3 -> unimplemented (* callf *)
                (Printf.sprintf "unsupported opcode: %02x/%d" b1 r)
       | 4 -> (Jump (Jabs rm), na)
       | 5 -> unimplemented (* jmpf *)
                (Printf.sprintf "unsupported opcode: %02x/%d" b1 r)
       | 6 -> let size = match mode with
           | X86 -> prefix.opsize
           | X8664 -> reg64_t
         in
         (Push(size, rm), na)
       | _ -> disfailwith
                (Printf.sprintf "impossible opcode: %02x/%d" b1 r)
      )
    (*** 00 to 3e ***)
    | b1 when b1 < 0x3e && (b1 land 7) < 6 ->
      (
        let ins a = match b1 lsr 3 with
          | 0 -> Add a
          | 1 -> Or a
          | 2 -> Adc a
          | 3 -> Sbb a
          | 4 -> And a
          | 5 -> Sub a
          | 6 -> Xor a
          | 7 -> Cmp a
          | _ -> disfailwith (Printf.sprintf "impossible opcode: %02x" b1)
        in
        let t = if (b1 land 1) = 0  then reg8_t else prefix.opsize in
        (* handle sign extended immediate cases *)
        let it = if Type.equal t reg64_t then reg32_t else t in
        let (o1, o2, na) = match b1 land 7 with
          | 0 | 1 -> let r, rm, na = parse_modrm_addr None na in
            (rm, r, na)
          | 2 | 3 -> let r, rm, na = parse_modrm_addr None na in
            (r, rm, na)
          | 4 -> let i, na = parse_immb na in
            (o_rax, i, na)
          | 5 -> let i, na = parse_immz it na in
            (o_rax, oimm_resize i t, na)
          | _ -> disfailwith (Printf.sprintf "impossible opcode: %02x" b1)
        in
        (ins(t, o1, o2), na)
      )
    (* Two byte opcodes *)
    | 0x0f -> (
        (* Add in the second implied vex prefix *)
        let b2, na = match vex with
          | Some {vex_map_select=2; _} -> 0x38, na
          | Some {vex_map_select=3; _} -> 0x3a, na
          | Some {vex_map_select=1; _} | None -> Char.to_int (g na), s na
          | Some {vex_map_select; _} -> disfailwith (Printf.sprintf "reserved mmmmmm vex value: %d" vex_map_select)
        in
        match b2 with (* Table A-3 *)
        | 0x01 ->
          let b3, nna = Char.to_int (g na), (s na) in
          (match b3 with
           | 0xd0 -> (Xgetbv, nna)
           | _ -> disfailwith (Printf.sprintf "unsupported opcode %02x %02x %02x" b1 b2 b3))
        | 0x05 when [%compare.equal: mode] mode X8664 -> (Syscall, na)
        | 0x1f ->
          (* Even though we don't use the operand to nop, we need to
             parse it to get the next address *)
          let _, _, na = parse_modrm_addr None na in
          (Nop, na)
        | 0x10 | 0x11 when (prefix.repeat || prefix.nrepeat) -> (* MOVSS, MOVSD *)
          let r, rm, rv, na = parse_modrm_vec None na in
          let t = if prefix.repeat then reg32_t else reg64_t in
          let d, s, td = if b2 = 0x10 then r, rm, reg128_t else rm, r, t in
          (match rm, rv with
           | Ovec _, Some rv ->
             let nt = !!t in
             (Movoffset((reg128_t, d),
                        {offlen=Type.imm (128 - nt); offtyp=reg128_t; offop=rv; offsrcoffset=nt; offdstoffset=nt}
                        :: {offlen=t; offtyp=reg128_t; offop=s; offsrcoffset=0; offdstoffset=0} :: []), na)
           | Ovec _, None ->
             (Movdq(t, s, t, d, false), na)
           | Oaddr _, _ ->
             (Movdq(t, s, td, d, false), na)
           | _ -> disfailwith "impossible")
        | 0x12 | 0x13 | 0x16 | 0x17 -> (* MOVLPS, MOVLPD, MOVHPS, MOVHPD, MOVHLPS, MOVHLPD, MOVLHPS, MOVLHPD *)
          let r, rm, rv, na = parse_modrm_vec None na in
          let tdst, dst, telt, tsrc1, src1, off_src1, off_dst1, src2 =
            match b2 with
            | (0x12 | 0x16) when Option.is_some rv ->
              let offs1, offs2, offd1, offd2 = match b2, rm with
                | 0x12, Ovec _ -> 64, 64, 64, 0
                | 0x12, _ -> 64, 0, 64, 0
                | 0x16, _ -> 0, 0, 0, 64
                | _ -> disfailwith "impossible"
              in
              let rv = match rv with
                | Some r -> r
                | None -> disfailwith "impossible"
              in
              let src2 = [{offlen=reg64_t; offtyp=reg128_t; offop=rm; offsrcoffset=offs2; offdstoffset=offd2}] in
              reg128_t, r, reg64_t, reg128_t, rv, offs1, offd1, src2
            | 0x12 | 0x13 | 0x16 | 0x17 ->
              let offset = match b2 with
                | 0x12 | 0x13 -> 0
                | 0x16 | 0x17 -> 64
                | _ -> disfailwith "impossible"
              in
              let s, d, offs, offd = match b2, rm with
                | 0x12, Ovec _ -> rm, r, 64, 0
                | (0x12 | 0x16), _ -> rm, r, 0, offset
                | (0x13 | 0x17), _ -> r, rm, offset, 0
                | _ -> disfailwith "impossible"
              in
              let ts = match s with Ovec _ -> reg128_t | _ -> reg64_t in
              reg128_t, d, reg64_t, ts, s, offs, offd, []
            | _ -> disfailwith "impossible"
          in
          (Movoffset((tdst, dst),
                     [{offlen=telt; offtyp=tsrc1; offop=src1; offsrcoffset=off_src1; offdstoffset=off_dst1}]@src2), na)
        | 0x10 | 0x11 | 0x28 | 0x29 | 0x6e | 0x7e | 0x6f | 0x7f | 0xd6 ->
          (* REGULAR MOVDQ *)
          let r, rm, _, na = parse_modrm_vec None na in
          let src, dst, tsrc, tdst, align = match b2 with
            | 0x10 | 0x11 | 0x28 | 0x29 -> (* MOVUPS, MOVUPD, MOVAPS, MOVAPD *)
              let s, d = match b2 with
                | 0x10 | 0x28 -> rm, r
                | 0x11 | 0x29 -> r, rm
                | _ -> disfailwith "impossible"
              in
              let align = match b2 with
                | 0x10 | 0x11 -> false
                | 0x28 | 0x29 -> true
                | _ -> disfailwith "impossible"
              in
              let t = if Type.equal prefix.mopsize reg256_t then reg256_t else reg128_t in
              s, d, t, t, align
            | 0x6e | 0x7e -> (* MOVD, MOVQ *)
              let t = if Type.equal prefix.opsize reg64_t then reg64_t else reg32_t in
              let s, d, ts, td = match b2 with
                | 0x6e -> toreg rm, r, t, reg128_t
                | 0x7e when prefix.repeat -> rm, r, reg64_t, reg128_t
                | 0x7e -> r, toreg rm, t, t
                | _ -> disfailwith "impossible"
              in
              s, d, ts, td, false
            | 0x6f | 0x7f -> (* MOVQ, MOVDQA, MOVDQU *)
              let s, d = match b2 with
                | 0x6f -> rm, r
                | 0x7f -> r, rm
                | _ -> disfailwith "impossible"
              in
              let size = if prefix.repeat && Option.is_none prefix.vex then reg128_t else prefix.mopsize in
              let align = if prefix.opsize_override then true else false in
              s, d, size, size, align
            | 0xd6 -> (* MOVQ *)
              r, rm, reg64_t, reg64_t, false
            | _ -> unimplemented
                     (Printf.sprintf "mov opcode case missing: %02x" b2)
          in
          (Movdq(tsrc, src, tdst, dst, align), na)
        | 0x31 -> (Rdtsc, na)
        | 0x34 -> (Sysenter, na)
        | 0x38 ->
          (* Three byte opcodes *)
          let b3 = Char.to_int (g na) and na = s na in
          (match b3 with
           | 0x00 ->
             let d, s, rv, na = parse_modrm_vec None na in
             (Pshufb(prefix.mopsize, d, s, rv), na)
           | 0x17 when prefix.opsize_override ->
             let d, s, _, na = parse_modrm_vec None na in
             (Ptest(prefix.mopsize, d, s), na)
           | 0x29 when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             (Pcmp(prefix.mopsize, Type.imm 64, Bil.EQ, "pcmpeq", r, rm, rv), na)
           | 0x20 | 0x21 | 0x22 | 0x23 | 0x24 | 0x25
           | 0x30 | 0x31 | 0x32 | 0x33 | 0x34 | 0x35 when prefix.opsize_override ->
             (* pmovsx and pmovzx *)
             let r, rm, _, na = parse_modrm_vec None na in
             (* determine sign/zero extension *)
             let ext, name = match (b3 land 0xf0) with
               | 0x20 -> Bil.signed, "pmovsx"
               | 0x30 -> Bil.unsigned, "pmovzx"
               | _ -> disfailwith "impossible"
             in
             (* determine dest/src element size *)
             let dstet, srcet, fullname = match (b3 land 0x0f) with
               | 0x00 -> reg16_t, reg8_t, name ^ "bw"
               | 0x01 -> reg32_t, reg8_t, name ^ "bd"
               | 0x02 -> reg64_t, reg8_t, name ^ "bq"
               | 0x03 -> reg32_t, reg16_t, name ^ "wd"
               | 0x04 -> reg64_t, reg16_t, name ^ "wq"
               | 0x05 -> reg64_t, reg32_t, name ^ "dq"
               | _ -> disfailwith "impossible"
             in
             (Pmov(prefix.mopsize, dstet, srcet, r, rm, ext, fullname), na)
           | 0x37 when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             (Pcmp(prefix.mopsize, Type.imm 64, Bil.SLT, "pcmpgt", r, rm, rv), na)
           | 0x38 when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             Ppackedbinop(prefix.mopsize, Type.imm 8, min_symbolic ~is_signed:true, "pminsb", r, rm, rv), na
           | 0x39 when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             Ppackedbinop(prefix.mopsize, Type.imm 32, min_symbolic ~is_signed:true, "pminsd", r, rm, rv), na
           | 0x3a when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             Ppackedbinop(prefix.mopsize, Type.imm 16, min_symbolic ~is_signed:false, "pminuw", r, rm, rv), na
           | 0x3b when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             Ppackedbinop(prefix.mopsize, Type.imm 32, min_symbolic ~is_signed:false, "pminud", r, rm, rv), na
           | 0x3c when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             Ppackedbinop(prefix.mopsize, Type.imm 8, max_symbolic ~is_signed:true, "pmaxsb", r, rm, rv), na
           | 0x3d when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             Ppackedbinop(prefix.mopsize, Type.imm 32, max_symbolic ~is_signed:true, "pmaxsd", r, rm, rv), na
           | 0x3e when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             Ppackedbinop(prefix.mopsize, Type.imm 16, max_symbolic ~is_signed:false, "pmaxuw", r, rm, rv), na
           | 0x3f when prefix.opsize_override ->
             let r, rm, rv, na = parse_modrm_vec None na in
             Ppackedbinop(prefix.mopsize, Type.imm 32, max_symbolic ~is_signed:false, "pmaxud", r, rm, rv), na
           | _ -> disfailwith (Printf.sprintf "opcode unsupported: 0f 38 %02x" b3))
        | 0x3a ->
          let b3 = Char.to_int (g na) and na = s na in
          (match b3 with
           | 0x0f ->
             let r, rm, rv, na = parse_modrm_vec (Some reg8_t) na in
             let i, na = parse_imm8 na in
             (Palignr(prefix.mopsize, r, rm, rv, i), na)
           | 0x60 | 0x61 | 0x62 | 0x63 ->
             let r, rm, _, na = parse_modrm_vec (Some reg8_t) na in
             let i, na = parse_imm8 na in
             (match i with
              | Oimm imm ->
                let open Pcmpstr in
                let imm8cb = parse_imm8cb imm in
                let pcmp = {out=if b3 land 0x1 = 0x1 then Index else Mask;
                            len=if b3 land 0x2 = 0x2 then Implicit else Explicit} in
                (Pcmpstr(prefix.mopsize, r, rm, i, imm8cb, pcmp), na)
              | _ ->  unimplemented "unsupported non-imm op for pcmpistri")
           | _ ->  unimplemented
                     (Printf.sprintf "unsupported opcode %02x %02x %02x" b1 b2 b3)
          )
        (* conditional moves *)
        | 0x40 | 0x41 | 0x42 | 0x43 | 0x44 | 0x45 | 0x46 | 0x47 | 0x48 | 0x49
        | 0x4a | 0x4b | 0x4c | 0x4d | 0x4e | 0x4f ->
          let (r, rm, na) = parse_modrm_addr None na in
          (Mov(prefix.opsize, r, rm, Some(cc_to_exp b2)), na)
        | 0x57 -> unimplemented "now it is handled by lisp loader"
        | 0x60 | 0x61 | 0x62 | 0x68 | 0x69 | 0x6a ->
          let order = match b2 with
            | 0x60 | 0x61 | 0x62 -> Low
            | 0x68 | 0x69 | 0x6a -> High
            | _ -> disfailwith "impossible"
          in
          let elemt = match b2 with
            | 0x60 | 0x68 -> Type.imm 8
            | 0x61 | 0x69 -> Type.imm 16
            | 0x62 | 0x6a -> Type.imm 32
            | _ -> disfailwith "impossible"
          in
          let r, rm, rv, na = parse_modrm_vec None na in
          (Punpck(prefix.mopsize, elemt, order, r, rm, rv), na)
        | 0x6c | 0x6d when prefix.opsize_override ->
          let order = match b2 with
            | 0x6c -> Low
            | 0x6d -> High
            | _ -> disfailwith "impossible"
          in
          let elemt = Type.imm 64 in
          let r, rm, rv, na = parse_modrm_vec None na in
          (Punpck(prefix.mopsize, elemt, order, r, rm, rv), na)
        | 0x64 | 0x65 | 0x66 | 0x74 | 0x75 | 0x76  as o ->
          let r, rm, rv, na = parse_modrm_vec None na in
          let elet = match o land 0x0F with | 0x4 -> reg8_t | 0x5 -> reg16_t | 0x6 -> reg32_t | _ ->
            disfailwith "impossible" in
          let bop, bstr = match o land 0xF0 with
            | 0x70 -> Bil.EQ, "pcmpeq"
            | 0x60 -> Bil.SLT, "pcmpgt"
            | _ -> disfailwith "impossible" in
          (Pcmp(prefix.mopsize, elet, bop, bstr, r, rm, rv), na)
        | 0x70 when Type.equal prefix.opsize reg16_t ->
          let r, rm, rv, na = parse_modrm_vec (Some reg8_t) na in
          let i, na = parse_imm8 na in
          (Pshufd(prefix.mopsize, r, rm, rv, i), na)
        | 0x71 | 0x72 | 0x73 ->
          let t = prefix.mopsize in
          let r, rm, rv, na = parse_modrm_vec (Some reg8_t) na in
          let i, na = parse_imm8 na in
          let bi8 = Word.of_int ~width:8 0x8 in
          let fbop, str, et, i =
            let open Bil in
            match b2, r, i with
            | _, Ovec 2, _ -> Bil.binop RSHIFT, "psrl", lowbits2elemt b2, i
            | _, Ovec 6, _ -> Bil.binop LSHIFT, "psll", lowbits2elemt b2, i
            | _, Ovec 4, _ -> Bil.binop ARSHIFT, "psra", lowbits2elemt b2, i
            (* The shift amount of next two elements are multiplied by eight *)
            | 0x73, Ovec 3, Oimm i when prefix.opsize_override -> Bil.binop RSHIFT, "psrldq", t, Oimm (Addr.(i * bi8))
            | 0x73, Ovec 7, Oimm i when prefix.opsize_override -> Bil.binop LSHIFT, "pslldq", t, Oimm (Addr.(i * bi8))
            | _, Oreg i, _ -> disfailwith (Printf.sprintf "invalid psrl/psll encoding b2=%#x r=%#x" b2 i)
            | _ -> disfailwith "impossible"
          in
          (Ppackedbinop(t, et, fbop, str, rm, i, rv), na)
        | 0x80 | 0x81 | 0x82 | 0x83 | 0x84 | 0x85 | 0x86 | 0x87 | 0x88 | 0x89
        | 0x8a | 0x8b | 0x8c | 0x8d | 0x8e | 0x8f ->
          let t = expanded_jump_type prefix.opsize in
          let (i,na) = parse_disp t na in
          (Jcc(Jabs(Oimm(add_to_addr na i)), cc_to_exp b2), na)
        (* add other opcodes for setcc here *)
        | 0x90 | 0x91 | 0x92 | 0x93 | 0x94 | 0x95 | 0x96 | 0x97 | 0x98 | 0x99
        | 0x9a | 0x9b | 0x9c | 0x9d | 0x9e | 0x9f ->
          let _r, rm, na = parse_modrm_addr None na in
          (* unclear what happens otherwise *)
          assert (Type.equal prefix.opsize reg32_t);
          (Setcc(reg8_t, rm, cc_to_exp b2), na)
        | 0xa2 -> (Cpuid, na)
        | 0xa3 | 0xba ->
          let it = if b2 = 0xba then Some reg8_t else None in
          let (r, rm, na) = parse_modrm_addr it na in
          let r, na = if b2 = 0xba then parse_imm8 na else r, na in
          (Bt(prefix.opsize, r, rm), na)
        | 0xa4 ->
          (* shld *)
          let (r, rm, na) = parse_modrm_addr (Some reg8_t) na in
          let (i, na) = parse_imm8 na in
          (Shiftd(Bil.LSHIFT, prefix.opsize, rm, r, i), na)
        | 0xa5 ->
          (* shld *)
          let (r, rm, na) = parse_modrm_addr None na in
          (Shiftd(Bil.LSHIFT, prefix.opsize, rm, r, o_rcx), na)
        | 0xac ->
          (* shrd *)
          let (r, rm, na) = parse_modrm_addr (Some reg8_t) na in
          let (i, na) = parse_imm8 na in
          (Shiftd(Bil.RSHIFT, prefix.opsize, rm, r, i), na)
        | 0xad ->
          (* shrd *)
          let (r, rm, na) = parse_modrm_addr None na in
          (Shiftd(Bil.RSHIFT, prefix.opsize, rm, r, o_rcx), na)
        | 0xae ->
          let (r, rm, na) = parse_modrmext_addr None na in
          (match r with
           | 2 -> (Ldmxcsr rm, na) (* ldmxcsr *)
           | 3 -> (Stmxcsr rm, na) (* stmxcsr *)
           | _ -> unimplemented
                    (Printf.sprintf "unsupported opcode: %02x %02x/%d" b1 b2 r)
          )
        | 0xaf ->
          let (r, rm, na) = parse_modrm_addr None na in
          (Imul(prefix.opsize, (false,r), r, rm), na)
        | 0xb1 ->
          let r, rm, na = parse_modrm_addr None na in
          (Cmpxchg (prefix.opsize, r, rm), na)
        | 0xb6
        | 0xb7 -> let st = if b2 = 0xb6 then reg8_t else reg16_t in
          let r, rm, na = parse_modrm_addr None na in
          (Movzx(prefix.opsize, r, st, rm), na)
        | 0xb8 when prefix.repeat ->
          let r, rm, na = parse_modrm_addr None na in
          (Popcnt (prefix.opsize, rm, r), na)
        | 0xbc | 0xbd ->
          let dir = match b2 with | 0xbc -> Forward | 0xbd -> Backward | _ -> failwith "impossible" in
          let r, rm, na = parse_modrm_addr None na in
          (Bs (prefix.opsize, r, rm, dir), na)
        | 0xbe
        | 0xbf -> let st = if b2 = 0xbe then reg8_t else reg16_t in
          let r, rm, na = parse_modrm_addr None na in
          (Movsx(prefix.opsize, r, st, rm), na)
        | 0xc1 ->
          let r, rm, na = parse_modrm_addr None na in
          (Xadd(prefix.opsize, r, rm), na)
        | 0xc7 ->
          let r, rm, na = parse_modrmext_addr None na in
          (match r with
           | 1 -> (Cmpxchg8b(rm), na)
           | _ -> unimplemented
                    (Printf.sprintf "unsupported opcode: %02x %02x/%d" b1 b2 r)
          )
        | 0xc8 | 0xc9 | 0xca | 0xcb | 0xcc | 0xcd | 0xce | 0xcf ->
          (Bswap(prefix.opsize, Oreg(rm_extend lor (b2 land 7))), na)
        | 0xd1 | 0xd2 | 0xd3 | 0xe1 | 0xe2 | 0xf1 | 0xf2 | 0xf3 ->
          let t = prefix.mopsize in
          let r, rm, rv, na = parse_modrm_vec None na in
          let et = lowbits2elemt b2 in
          let fbop, str = match b2 land 0xf0 with
            | 0xd0 -> Bil.(lsr), "psrl"
            | 0xe0 -> Bil.(asr), "psra"
            | 0xf0 -> Bil.(lsl), "psll"
            | _ -> disfailwith "invalid"
          in
          (Ppackedbinop(t, et, fbop, str, r, rm, rv), na)
        | 0xda ->
          let r, rm, rv, na = parse_modrm_vec None na in
          (Ppackedbinop(prefix.mopsize, Type.imm 8, min_symbolic ~is_signed:false, "pminub", r, rm, rv), na)
        | 0xdb ->
          let r, rm, rv, na = parse_modrm_vec None na in
          (Pbinop(prefix.mopsize, Bil.(land), "pand", r, rm, rv), na)
        | 0xd7 ->
          let r, rm, na = parse_modrm_addr None na in
          let r, rm = r, tovec rm in
          (Pmovmskb(prefix.mopsize, r, rm), na)
        | 0xde ->
          let r, rm, rv, na = parse_modrm_vec None na in
          (Ppackedbinop(prefix.mopsize, Type.imm 8, max_symbolic ~is_signed:false, "pmaxub", r, rm, rv), na)
        | 0xdf ->
          let r, rm, rv, na = parse_modrm_vec None na in
          let andn x y = Bil.(lnot x land y) in
          (Pbinop(prefix.mopsize, andn, "pandn", r, rm, rv), na)
        | 0xe0 | 0xe3 ->
          (* pavg *)
          let r, rm, rv, na = parse_modrm_vec None na in
          (* determine whether we're using bytes or words *)
          let et = match b2 land 0x0f with
            | 0x00 -> reg8_t
            | 0x03 -> reg16_t
            | _ -> disfailwith "invalid"
          in
          let one = int_exp 1 !!et in
          let average x y = Bil.(((x + y) + one) lsr one) in
          (Ppackedbinop(prefix.mopsize, et, average, "pavg", r, rm, rv), na)
        | 0xea ->
          let r, rm, rv, na = parse_modrm_vec None na in
          (Ppackedbinop(prefix.mopsize, Type.imm 16, min_symbolic ~is_signed:true, "pminsw", r, rm, rv), na)
        | 0xeb ->
          let r, rm, rv, na = parse_modrm_vec None na in
          (Pbinop(prefix.mopsize, Bil.(lor), "por", r, rm, rv), na)
        | 0xee ->
          let r, rm, rv, na = parse_modrm_vec None na in
          (Ppackedbinop(prefix.mopsize, Type.imm 16, max_symbolic ~is_signed:true, "pmaxsw", r, rm, rv), na)
        | 0xef ->
          let r, rm, rv, na = parse_modrm_vec None na in
          (Pbinop(prefix.mopsize, Bil.(lxor), "pxor", r, rm, rv), na)
        | 0xf0 ->
          let r, rm, _, na = parse_modrm_vec None na in
          let t = if Type.equal prefix.mopsize reg256_t then reg256_t else reg128_t in
          (Movdq(t, rm, t, r, false), na)
        | 0xf8 | 0xf9 | 0xfa | 0xfb ->
          let r, rm, rv, na = parse_modrm_vec None na in
          let eltsize = match b2 land 7 with
            | 0 -> reg8_t
            | 1 -> reg16_t
            | 2 -> reg32_t
            | 3 -> reg64_t
            | _ -> disfailwith "impossible"
          in
          (* XXX I should just have put in a binop *)
          (Ppackedbinop(prefix.mopsize, eltsize, Bil.(-), "psub", r, rm, rv), na)
        | 0xfc | 0xfd | 0xfe | 0xd4 ->
          let r, rm, rv, na = parse_modrm_vec None na in
          let eltsize = match b2 with
            | 0xfc -> reg8_t
            | 0xfd -> reg16_t
            | 0xfe -> reg32_t
            | 0xd4 -> reg64_t
            | _ -> disfailwith "impossible"
          in
          (Ppackedbinop(prefix.mopsize, eltsize, Bil.(+), "padd", r, rm, rv), na)
        | _ -> unimplemented
                 (Printf.sprintf "unuspported opcode: %02x %02x" b1 b2)
      )
    | n -> unimplemented (Printf.sprintf "unsupported single opcode: %02x" n)

(* https://github.com/airbus-seclab/bincat/blob/5d0ee3b56867059427eb0f4123c4d9de0b8059dd/ocaml/src/disassembly/core_x86.ml#L2650-L2931 *)
(* airbus-seclab/bincat ocaml/src/disassembly/core_x86.ml:2650-2931 *)
    let rec decode s =
      match check_context s (getchar s) with
      | '\x00' -> (* ADD *) add_sub_mrm s Add false 8 0
      | '\x01' -> (* ADD *) add_sub_mrm s Add false s.operand_sz 0
      | '\x02' -> (* ADD *) add_sub_mrm s Add false 8 1
      | '\x03' -> (* ADD *) add_sub_mrm s Add false s.operand_sz 1
      | '\x04' -> (* ADD AL with immediate operand *) add_sub_immediate s Add false eax 8 8
      | '\x05' -> (* ADD eAX with immediate operand *) add_sub_immediate s Add false eax s.operand_sz s.imm_sz
      | '\x06' -> (* PUSH es *) let es' = to_reg es 16 in push s [V es', 16]
      | '\x07' -> (* POP es *) let es' = to_reg es s.operand_sz in pop s [V es', s.operand_sz]
      | '\x08' -> (* OR *) or_xor_and_mrm s Or 8 0
      | '\x09' -> (* OR *) or_xor_and_mrm s Or s.operand_sz 0
      | '\x0A' -> (* OR *) or_xor_and_mrm s Or 8 1
      | '\x0B' -> (* OR *) or_xor_and_mrm s Or s.operand_sz 1
      | '\x0C' -> (* OR imm8 *) or_xor_and_eax s Or 8 8
      | '\x0D' -> (* OR imm *) or_xor_and_eax s Or s.imm_sz s.operand_sz


      | '\x0E' -> (* PUSH cs *) let cs' = to_reg cs 16 in push s [V cs', 16]
      | '\x0F' -> (* 2-byte escape *) decode_snd_opcode s

      | '\x10' -> (* ADC *) add_sub_mrm s Add true 8 0
      | '\x11' -> (* ADC *) add_sub_mrm s Add true s.operand_sz 0
      | '\x12' -> (* ADC *) add_sub_mrm s Add true 8 1
      | '\x13' -> (* ADC *) add_sub_mrm s Add true s.operand_sz 1

      | '\x14' -> (* ADC AL with immediate *) add_sub_immediate s Add true eax 8 8
      | '\x15' -> (* ADC eAX with immediate *) add_sub_immediate s Add true eax s.operand_sz s.imm_sz
      | '\x16' -> (* PUSH ss *) let ss' = to_reg ss 16 in push s [V ss', 16]
      | '\x17' -> (* POP ss *) let ss' = to_reg ss s.operand_sz in pop s [V ss', s.operand_sz]

      | '\x18' -> (* SBB *) add_sub_mrm s Sub true 8 0
      | '\x19' -> (* SBB *) add_sub_mrm s Sub true s.operand_sz 0
      | '\x1A' -> (* SBB *) add_sub_mrm s Sub true 8 1
      | '\x1B' -> (* SBB *) add_sub_mrm s Sub true s.operand_sz 1
      | '\x1C' -> (* SBB AL with immediate *) add_sub_immediate s Sub true eax 8 8
      | '\x1D' -> (* SBB eAX with immediate *) add_sub_immediate s Sub true eax s.operand_sz s.imm_sz
      | '\x1E' -> (* PUSH ds *) let ds' = to_reg ds 16 in push s [V ds', 16]
      | '\x1F' -> (* POP ds *) let ds' = to_reg ds s.operand_sz in pop s [V ds', s.operand_sz]

      | '\x20' -> (* AND *) or_xor_and_mrm s And 8 0
      | '\x21' -> (* AND *) or_xor_and_mrm s And s.operand_sz 0
      | '\x22' -> (* AND *) or_xor_and_mrm s And 8 1
      | '\x23' -> (* AND *) or_xor_and_mrm s And s.operand_sz 1
      | '\x24' -> (* AND imm8 *) or_xor_and_eax s And 8 8
      | '\x25' -> (* AND imm *) or_xor_and_eax s And s.imm_sz s.operand_sz

      | '\x26' -> (* data segment = es *) s.segments.data <- es; decode s
      | '\x27' -> (* DAA *) daa s
      | '\x28' -> (* SUB *) add_sub_mrm s Sub false 8 0
      | '\x29' -> (* SUB *) add_sub_mrm s Sub false s.operand_sz 0
      | '\x2A' -> (* SUB *) add_sub_mrm s Sub false 8 1
      | '\x2B' -> (* SUB *) add_sub_mrm s Sub false s.operand_sz 1
      | '\x2C' -> (* SUB AL with immediate *) add_sub_immediate s Sub false eax 8 8
      | '\x2D' -> (* SUB eAX with immediate *) add_sub_immediate s Sub false eax s.operand_sz s.imm_sz
      | '\x2E' -> (* data segment = cs *) s.segments.data <- cs; (* will be set back to default value if the instruction is a jcc *) decode s
      | '\x2F' -> (* DAS *) das s

      | '\x30' -> (* XOR *) or_xor_and_mrm s Xor 8 0
      | '\x31' -> (* XOR *) or_xor_and_mrm s Xor s.operand_sz 0
      | '\x32' -> (* XOR *) or_xor_and_mrm s Xor 8 1
      | '\x33' -> (* XOR *) or_xor_and_mrm s Xor s.operand_sz 1
      | '\x34' -> (* XOR imm8 *) or_xor_and_eax s Xor 8 8
      | '\x35' -> (* XOR imm *) or_xor_and_eax s Xor s.imm_sz s.operand_sz

      | '\x36' -> (* data segment = ss *) s.segments.data <- ss; decode s
      | '\x37' -> (* AAA *) aaa s
      | '\x38' -> (* CMP *) cmp_mrm s 8 0
      | '\x39' -> (* CMP *) cmp_mrm s s.operand_sz 0
      | '\x3A' -> (* CMP *) cmp_mrm s 8 1
      | '\x3B' -> (* CMP *) cmp_mrm s s.operand_sz 1
      | '\x3C' -> (* CMP AL with immediate *)
         let imm = get_imm s 8 8 false in
         return s (cmp_stmts (Lval (V (P (eax, 0, 7)))) imm 8)
      | '\x3D' -> (* CMP eAX with immediate *)
         let i = get_imm s s.imm_sz s.operand_sz false in
         return s (cmp_stmts (Lval (V (P (eax, 0, s.operand_sz-1)))) i s.operand_sz)
      | '\x3E' -> (* data segment = ds *) s.segments.data <- ds (* will be set back to default value if the instruction is a jcc *); decode s
      | '\x3F' -> (* AAS *) aas s

      | c when '\x40' <= c && c <= '\x47' -> (* INC or REX *)
         begin
           try
             let rex = decode_from_0x40_to_0x4F c s.operand_sz in
             L.debug2 (fun p -> p "got rex prefix: w:%d r:%d x:%d b:%d" rex.w rex.r rex.x rex.b_);
             s.rex <- rex; if s.rex.w = 1 then s.operand_sz <- 64; decode s
           with Exit -> (* INC *)
             let r = find_reg ((Char.code c) - 0x40) s.operand_sz in inc_dec (V r) Add s s.operand_sz
         end

      | c when '\x48' <= c && c <= '\x4F' -> (* DEC or REX *)
         begin
           try
             let rex = decode_from_0x40_to_0x4F c s.operand_sz in
             L.debug2 (fun p -> p "got rex prefix: w:%d r:%d x:%d b:%d" rex.w rex.r rex.x rex.b_);
             if rex.w = 1 && s.rex.op_switch then
               (* previous operand_switch is ignored *)
               switch_sizes s;
             s.rex <- rex; if s.rex.w = 1 then s.operand_sz <- 64; decode s
           with Exit ->
             let r = find_reg ((Char.code c) - 0x48) s.operand_sz in
             inc_dec (V r) Sub s s.operand_sz
         end

      | c when '\x50' <= c && c <= '\x57' -> (* PUSH general register *)
         if not s.rex.op_switch then
           begin
             try
               if s.operand_sz <> 16 then
                 let n = Arch.get_operand_sz_for_stack () in
                 s.operand_sz <- n
             with Exit -> ()
           end;
         let n = (Char.code c) - 0x50 in
         let n'= s.rex.b_ lsl 3 + n in
         let r = find_reg n' s.operand_sz in
         push s [V r, s.operand_sz]

      | c when '\x58' <= c && c <= '\x5F' -> (* POP into general register *)
         if not s.rex.op_switch then
           begin
             try
               if s.operand_sz <> 16 then
               let n = Arch.get_operand_sz_for_stack () in
                 s.operand_sz <- n
             with Exit -> ()
           end;
         let n = (Char.code c) - 0x58 in
         let n' = s.rex.b_ lsl 3 + n in
         let r = find_reg n' s.operand_sz in
         pop s [V r, s.operand_sz]

      | '\x60' -> (* PUSHA *) let l = List.map (fun v -> find_reg_v v s.operand_sz, s.operand_sz) [0 ; 1 ; 2 ; 3 ; 5 ; 6 ; 7] in push s l
      | '\x61' -> (* POPA *) let l = List.map (fun v -> find_reg_v v s.operand_sz, s.operand_sz) [7 ; 6 ; 3 ; 2 ; 1 ; 0] in pop s l

      | '\x63' ->
         if !Config.address_sz = 32 then (* ARPL *) arpl s
         else
           (*MOVSXD *)
           let reg, rm = operands_from_mod_reg_rm s 32 ~dst_sz:64 1 in
           return s [ Set (reg, UnOp(SignExt 64, rm)) ]

      | '\x64' -> (* segment data = fs *) s.segments.data <- fs; decode s
      | '\x65' -> (* segment data = gs *) s.segments.data <- gs; decode s
      | '\x66' -> (* operand size switch *) switch_sizes s; s.rex.op_switch <- true; (* for x86: 66H ignored if REX.W = 1, see Vol 2A 2.2.1.2, this condition will be used in the rex prefix decoding *) decode s
      | '\x67' -> (* address size switch *) s.addr_sz <- if s.addr_sz = 16 then 32 else if s.addr_sz = 32 then 16 else 32; decode s
      | '\x68' -> (* PUSH immediate *) push_immediate s s.imm_sz
      | '\x69' -> (* IMUL immediate *) let dst, src = operands_from_mod_reg_rm s s.operand_sz 1 in let imm = get_imm s s.operand_sz s.operand_sz true in imul_stmts s dst src imm
      | '\x6a' -> (* PUSH byte *) push_immediate s 8
      | '\x6b' -> (* IMUL imm8 *) let dst, src = operands_from_mod_reg_rm s s.operand_sz 1 in let imm = get_imm s 8 s.operand_sz true in imul_stmts s dst src imm

      | '\x6c' -> (* INSB *) ins s 8
      | '\x6d' -> (* INSW/D *) ins s s.addr_sz
      | '\x6e' -> (* OUTSB *) outs s 8
      | '\x6f' -> (* OUTSW/D *) outs s s.addr_sz

      | c when '\x70' <= c && c <= '\x7F' -> (* JCC: short displacement jump on condition *) let v = (Char.code c) - 0x70 in jcc s v 8

      | '\x80' -> (* grp1 opcode table *) grp1 s 8 8
      | '\x81' -> (* grp1 opcode table *) grp1 s s.operand_sz s.imm_sz
      | '\x82' -> error s.a ("Undefined opcode 0x82")
      | '\x83' -> (* grp1 opcode table *) grp1 s s.operand_sz 8
      | '\x84' -> (* TEST /r8 *) let dst, src = (operands_from_mod_reg_rm s 8 0) in return s (test_stmts dst src 8)
      | '\x85' -> (* TEST /r *) let dst, src = operands_from_mod_reg_rm s s.operand_sz 0 in return s (test_stmts dst src s.operand_sz)
      | '\x86' -> (* XCHG byte registers *)  xchg_mrm s 8
      | '\x87' -> (* XCHG word or double-word registers *) xchg_mrm s s.operand_sz
      | '\x88' -> (* MOV *) mov_mrm s 8 0
      | '\x89' -> (* MOV *) mov_mrm s s.operand_sz 0
      | '\x8A' -> (* MOV *) mov_mrm s 8 1
      | '\x8B' -> (* MOV *) mov_mrm s s.operand_sz 1


      | '\x8c' -> (* MOV with segment as src *)
         let _mod, reg, rm = mod_nnn_rm (Char.code (getchar s)) in
         let dst = find_reg_v rm 16 in
         let src = V (T (to_segment_reg s.a reg)) in
         return s [ Set (dst, Lval src) ]

      | '\x8d' -> (* LEA *) lea s
      | '\x8e' -> (* MOV with segment as dst *)
         let _mod, reg, rm = mod_nnn_rm (Char.code (getchar s)) in
         let dst = V ( T (to_segment_reg s.a reg)) in
         let src = find_reg_v rm 16 in
         return s [ Set (dst, Lval src) ]
      | '\x8f' -> (* POP of word or double word *) let dst, _src = operands_from_mod_reg_rm s s.operand_sz 0 in pop s [dst, s.operand_sz]

      | '\x90'                  -> (* NOP *) return s [Nop]
      | c when '\x91' <= c && c <= '\x97' -> (* XCHG word or double-word with eAX *) xchg_with_eax s ((Char.code c) - 0x90)
      | '\x98' -> (* CBW *) let dst = V (to_reg eax s.operand_sz) in return s [Set (dst, UnOp (SignExt s.operand_sz, Lval (V (to_reg eax (s.operand_sz / 2)))))]
      | '\x99' -> (* CWD / CDQ *) cwd_cdq s
      | '\x9a' -> (* CALL *)
         let off = int_of_bytes s (s.operand_sz / 8) in
         let cs' = get_base_address s cs in
         let a = Data.Address.add_offset (Data.Address.of_int Data.Address.Global cs' s.addr_sz) off in
         return s (call s (A a))
      | '\x9b' -> (* WAIT *) error s.a "WAIT decoder. Interpreter halts"
      | '\x9c' -> (* PUSHF *) pushf s s.addr_sz
      | '\x9d' -> (* POPF *) popf s s.addr_sz
      | '\xa0' -> (* MOV EAX *) mov_with_eax s 8 false
      | '\xa1' -> (* MOV EAX *) mov_with_eax s s.operand_sz false (* yes! it is 32 for x64 also, see Vol 2A 2.2.1.4 *)
      | '\xa2' -> (* MOV EAX *) mov_with_eax s 8 true
      | '\xa3' -> (* MOV EAX *) mov_with_eax s s.operand_sz true (* yes! it is 32 for x64 also, see Vol 2A 2.2.1.4 *)
      | '\xa4' -> (* MOVSB *) movs s 8
      | '\xa5' -> (* MOVSW *) movs s s.addr_sz
      | '\xa6' -> (* CMPSB *) cmps s 8
      | '\xa7' -> (* CMPSW *) cmps s s.addr_sz
      | '\xa8' -> (* TEST AL, imm8 *) return s (test_stmts (find_reg_v 0 8) (get_imm s 8 8 false) 8)
      | '\xa9' -> (* TEST xAX, imm *) return s (test_stmts (find_reg_v 0 s.operand_sz) (get_imm s s.imm_sz s.operand_sz false) s.operand_sz )
      | '\xaa' -> (* STOS on byte *) stos s 8
      | '\xab' -> (* STOS *) stos s s.addr_sz
      | '\xac' -> (* LODS on byte *) lods s 8
      | '\xad' -> (* LODS *) lods s s.addr_sz
      | '\xae' -> (* SCAS on byte *) scas s 8
      | '\xaf' -> (* SCAS *) scas s s.addr_sz

      | c when '\xb0' <= c && c <= '\xb3' -> (* MOV immediate byte into byte register *) let r = (find_reg_v ((Char.code c) - 0xb0) 8) in return s [Set (r, Const (Word.of_int (int_of_byte s) 8))]
      | c when '\xb4' <= c && c <= '\xb7' -> (* MOV immediate byte into byte register (higher part) *)
         let n = (Char.code c) - 0xb4  in
         let r = V (P (Hashtbl.find register_tbl n, 8, 15)) in
         return s [Set (r, Const (Word.of_int (int_of_byte s) 8))]
         
      | c when '\xb8' <= c && c <= '\xbf' -> mov_imm_direct s c
      | '\xc0' -> (* shift grp2 with byte size*) grp2 s 8 None
      | '\xc1' -> (* shift grp2 with word or double-word size *) grp2 s s.operand_sz None
      | '\xc2' -> (* RET NEAR and pop word *) let pop_sz = get_imm s 16 s.operand_sz false in return s [ Return; Set(V(T esp), BinOp(Add, Lval (V (T esp)), BinOp(Add, const (s.addr_sz/8) s.operand_sz, pop_sz))); ]
      | '\xc3' -> (* RET NEAR *) return s [ Return; set_esp Add (T esp) s.addr_sz; ]
      | '\xc4' -> (* LES *) load_far_ptr s es
      | '\xc5' -> (* LDS *) load_far_ptr s ds
      | '\xc6' -> (* MOV with byte *) mov_immediate s 8
      | '\xc7' -> (* MOV with word or double *) mov_immediate s s.imm_sz

      | '\xc9' -> (* LEAVE *)
         let sp = V (to_reg esp !Config.stack_width) in
         let bp = V (to_reg ebp !Config.stack_width) in
         return s ( (Set (sp, Lval bp))::(pop_stmts false s [bp, !Config.stack_width]))

      | '\xca' -> (* RET FAR and pop a word *)
         return s ([Return ; set_esp Add (T esp) s.addr_sz ; ] @ (pop_stmts false s [V (T cs), 16] @ (* pop imm16 *) [set_esp Add (T esp) 16]))
                                             
      | '\xcb' -> (* RET FAR *) return s ([Return ; set_esp Add (T esp) s.addr_sz; ] @ (pop_stmts false s [V (T cs), 16]))
      | '\xcc' -> (* INT 3 *) int_3 s ctx
      | '\xcd' -> (* INT *) let c = getchar s in error s.a (Printf.sprintf "INT %d decoded. Interpreter halts" (Char.code c))

      | '\xce' -> (* INTO *) into s ctx
         
      | '\xcf' -> (* IRET *) iret s

      | '\xd0' -> (* grp2 shift with one on byte size *) grp2 s 8 (Some (const1 8))
      | '\xd1' -> (* grp2 shift with one on word or double size *) grp2 s s.operand_sz (Some (const1 8))
      | '\xd2' -> (* grp2 shift with CL and byte size *) grp2 s 8 (Some (Lval (V (to_reg ecx 8))))
      | '\xd3' -> (* grp2 shift with CL *) grp2 s s.operand_sz (Some (Lval (V (to_reg ecx 8))))
      | '\xd4' -> (* AAM *) aam s
      | '\xd5' -> (* AAD *) aad s
      | '\xd7' -> (* XLAT *) xlat s
      | c when '\xd8' <= c && c <= '\xdf' -> (* ESC (escape to coprocessor instruction set *) decode_coprocessor c s

      | c when '\xe0' <= c && c <= '\xe2' -> (* LOOPNE/LOOPE/LOOP *) loop s c
      | '\xe3' -> (* JCXZ *) jecxz s

      | '\xe8' -> (* relative call *) relative_call s 32 (* x64: immediates are of size 8 or 32 then sign extended to 64 *)
      | '\xe9' -> (* JMP to near relative address (offset has word or double word size) *) relative_jmp s s.imm_sz (* x64: immediates are of size 8 or 32 then sign extended to 64 *)
      | '\xea' -> (* JMP to near absolute address *) direct_jmp s
      | '\xeb' -> (* JMP to near relative address (offset has byte size) *) relative_jmp s 8


      | '\xf0' -> (* LOCK *) L.analysis(fun p -> p "x86 LOCK prefix ignored"); decode s
      | '\xf1' -> (* undefined *) error s.a "Undefined opcode 0xf1"
      | '\xf2' -> (* REPNE *) s.repne <- true; rep s Word.one
      | '\xf3' -> (* REP/REPE *) s.repe <- true; rep s Word.zero
      | '\xf4' -> (* HLT *) error s.a "Decoder stopped: HLT reached"
      | '\xf5' -> (* CMC *) let fcf' = V (T fcf) in return s [ Set (fcf', UnOp (Not, Lval fcf')) ]
      | '\xf6' -> (* shift to grp3  with byte size *) grp3 s 8
      | '\xf7' -> (* shift to grp3 with word or double word size *) grp3 s s.operand_sz
      | '\xf8' -> (* CLC *) return s (clear_flag fcf fcf_sz)
      | '\xf9' -> (* STC *) return s (set_flag fcf fcf_sz)
      | '\xfa' -> (* CLI *) L.decoder (fun p -> p "entering privilege mode (CLI instruction)"); return s (clear_flag fif fif_sz)
      | '\xfb' -> (* STI *) L.decoder (fun p -> p "entering privilege mode (STI instruction)"); return s (set_flag fif fif_sz)
      | '\xfc' -> (* CLD *) return s (clear_flag fdf fdf_sz)
      | '\xfd' -> (* STD *) return s (set_flag fdf fdf_sz)
      | '\xfe' -> (* INC/DEC grp4 *) grp4 s
      | '\xff' -> (* indirect grp5 *) grp5 s
      | c ->  error s.a (Printf.sprintf "Unknown opcode 0x%x" (Char.code c))

(* https://github.com/bcpierce00/unison/blob/273ec6298cd7f97f038ea8667701691c29b1b48f/src/uigtk3.ml#L2976-L4660 *)
(* bcpierce00/unison src/uigtk3.ml:2976-4660 *)
let createToplevelWindow () =
  let toplevelWindow =
    GWindow.window ~kind:`TOPLEVEL ~position:`CENTER
      ~title:myNameCapitalized ()
  in
  setToplevelWindow toplevelWindow;
  (* There is already a default icon under Windows, and transparent
     icons are not supported by all version of Windows *)
  if not Sys.win32 then toplevelWindow#set_icon (Some (Lazy.force icon));
  let toplevelVBox = GPack.vbox ~packing:toplevelWindow#add () in

  (*******************************************************************
   Statistic window
   *******************************************************************)

  let (statWin, startStats, stopStats) = statistics () in

  (*******************************************************************
   Groups of things that are sensitive to interaction at the same time
   *******************************************************************)
  let grAction = ref [] in
  let grDiff = ref [] in
  let grGo = ref [] in
  let grRescan = ref [] in
  let grStop = ref [] in
  let grDetail = ref [] in
  let grAdd gr w = gr := w#misc::!gr in
  let grSet gr st = Safelist.iter (fun x -> x#set_sensitive st) !gr in
  let grDisactivateAll () =
    grSet grAction false;
    grSet grDiff false;
    grSet grGo false;
    grSet grRescan false;
    grSet grStop false;
    grSet grDetail false
  in

  (*********************************************************************
    Create the menu bar
   *********************************************************************)
  let topHBox = GPack.hbox ~packing:(toplevelVBox#pack ~expand:false) () in

  let menuBar =
    GMenu.menu_bar ~border_width:0
      ~packing:(topHBox#pack ~expand:true) () in
  let menus = new gMenuFactory ~accel_modi:[] menuBar in
  let accel_group = menus#accel_group in
  toplevelWindow#add_accel_group accel_group;
  let add_submenu ?(modi=[]) label =
    let (menu, item) = menus#add_submenu label in
    (new gMenuFactory ~accel_group:(menus#accel_group)
       ~accel_path:(menus#accel_path ^ label ^ "/")
       ~accel_modi:modi menu,
     item)
  in
  let replace_submenu ?(modi=[]) label item =
    let menu = menus#replace_submenu item in
    new gMenuFactory ~accel_group:(menus#accel_group)
      ~accel_path:(menus#accel_path ^ label ^ "/")
      ~accel_modi:modi menu
  in

  let profileLabel =
    GMisc.label ~text:"" ~packing:(topHBox#pack ~expand:false ~padding:2) () in

  let displayNewProfileLabel () =
    let p = match !Prefs.profileName with None -> "" | Some p -> p in
    let label = Prefs.read Uicommon.profileLabel in
    let s =
      match p, label with
        "",        _  -> ""
      | _,         "" -> p
      | "default", _  -> label
      | _             -> Format.sprintf "%s (%s)" p label
    in
    let roots = String.concat " ↔ " (Globals.rawRoots ()) in
    let roots = if roots = "" then "" else "   |   " ^ roots in
    toplevelWindow#set_title
      (if s = "" then myNameCapitalized else
       Format.sprintf "%s [%s]%s" myNameCapitalized s roots);
    let s = if s="" then "No profile" else "Profile: " ^ s in
    profileLabel#set_text (transcode s)
  in
  displayNewProfileLabel ();

  (*********************************************************************
    Create the menus
   *********************************************************************)
  let (fileMenu, _) = add_submenu "_Synchronization" in
  let (actionMenu, actionItem) = add_submenu "_Actions" in
  let (ignoreMenu, _) = add_submenu ~modi:[`SHIFT] "_Ignore" in
  let (sortMenu, _) = add_submenu "S_ort" in
  let (helpMenu, _) = add_submenu "_Help" in
  let (expertMenu, expertItem) = add_submenu "Expert" in
  let () = expertItem#set_visible false in (* Expert menu hidden by default *)

  (*********************************************************************
    Action bar
   *********************************************************************)
  let actionBar =
    GButton.toolbar ~style:`BOTH
      (* 2003-0519 (stse): how to set space size in gtk 2.0? *)
      (* Answer from Jacques Garrigue: this can only be done in
         the user's.gtkrc, not programmatically *)
      ~orientation:`HORIZONTAL (* ~space_size:10 *)
      ~packing:(toplevelVBox#pack ~expand:false) () in
  actionBar#set_icon_size `SMALL_TOOLBAR;
  (* [show_arrow] is initially false to produce a better default width. *)
  actionBar#set_show_arrow false;
  ignore (toplevelWindow#misc#connect#show
    ~callback:(fun () -> actionBar#set_show_arrow true));

  (*********************************************************************
    Create the main window
   *********************************************************************)
  let mainWindowSW =
      GBin.scrolled_window ~packing:(toplevelVBox#pack ~expand:true)
        ~hpolicy:`AUTOMATIC ~vpolicy:`AUTOMATIC ()
  in
  let cols = new GTree.column_list in
  let c_replica1 = cols#add Gobject.Data.string in
  let c_action   = cols#add Gobject.Data.gobject in
  let c_replica2 = cols#add Gobject.Data.string in
  let c_status   = cols#add Gobject.Data.gobject_option in
  let c_statust  = cols#add Gobject.Data.string in
  let c_path     = cols#add Gobject.Data.string in
  (*let c_rowid    = cols#add Gobject.Data.uint in*)
  (* With current implementation the [list_store] view model and [theState]
     array have one-to-one correspondence, so that list_store's tree path index
     is the same as theState array index.
     This changes when, for example, [tree_store] would be used instead of
     list_store, or a separate view-only sorting is implemented without sorting
     the backing theState array. In that case, the column [c_rowid] must be
     used to store the index of [theState] array in the view model. Tree path
     index must not be used directly as [theState] array index and vice versa. *)
  let mainWindowModel = GTree.list_store cols in
  let mainWindow =
    GTree.view ~model:mainWindowModel ~packing:(mainWindowSW#add)
      ~headers_clickable:false ~enable_search:false () in
  mainWindow#selection#set_mode `MULTIPLE;
  ignore (mainWindow#append_column
    (GTree.view_column
       ~title:(" ")
       ~renderer:(GTree.cell_renderer_text [], ["text", c_replica1]) ()));
  ignore (mainWindow#append_column
    (GTree.view_column ~title:"  Action  "
       ~renderer:(GTree.cell_renderer_pixbuf [], ["pixbuf", c_action]) ()));
  ignore (mainWindow#append_column
    (GTree.view_column
       ~title:(" ")
       ~renderer:(GTree.cell_renderer_text [], ["text", c_replica2]) ()));
  let status_view_col = GTree.view_column ~title:"  Status  "
       ~renderer:(GTree.cell_renderer_pixbuf [], ["pixbuf", c_status]) () in
  let status_t_rend = GTree.cell_renderer_text [] in
  status_view_col#pack ~expand:false ~from:`END status_t_rend;
  status_view_col#add_attribute status_t_rend "text" c_statust;
  ignore (mainWindow#append_column status_view_col);
  ignore (mainWindow#append_column
    (GTree.view_column ~title:"  Path  "
       ~renderer:(GTree.cell_renderer_text [], ["text", c_path]) ()));

  let setMainWindowColumnHeaders roots =
    let escape s = String.split_on_char '_' s |> String.concat "__" in
    let (r1, r2) = Uicommon.roots2niceStrings 15 roots in
    Array.iteri
      (fun i data ->
         (mainWindow#get_column i)#set_title data)
      [| " " ^ Unicode.protect (escape r1) ^ " "; "  Action  ";
         " " ^ Unicode.protect (escape r2) ^ " "; "  Status  ";
         " Path" |];
  in

  (* See above for comment about tree path index and [theState] array index
     equivalence. *)
  let siOfRow f path =
    let row = mainWindowModel#get_iter path in
    let i = (GTree.Path.get_indices path).(0) in
    (*let i = mainWindowModel#get ~row ~column:c_rowid in*)
    f i !theState.(i) row
  in
  let rowOfSi i = GTree.Path.create [i] in
  let currentNumberRows () = mainWindow#selection#count_selected_rows in
  let currentRow () =
    match currentNumberRows () with
    | 1 -> siOfRow (fun i si row -> Some (i, !theState.(i), row))
             (List.hd mainWindow#selection#get_selected_rows)
    | _ -> None
  in
  let currentSelectedIter f =
    Safelist.iter (fun r -> siOfRow f r)
      mainWindow#selection#get_selected_rows
  in
  let currentSelectedFold f a =
    Safelist.fold_left (fun a r -> siOfRow (fun _ si _ -> f a si) r)
      a mainWindow#selection#get_selected_rows
  in
  let currentSelectedExists pred =
    Safelist.exists (fun r -> siOfRow (fun _ si _ -> pred si) r)
      mainWindow#selection#get_selected_rows
  in

  (*********************************************************************
    Create the details window
   *********************************************************************)

  let showDetCommand () =
    let details =
      match currentRow () with
        None ->
          None
      | Some (_, si, _) ->
          let path = Path.toString si.ri.path1 in
          match si.whatHappened with
            Some (Util.Failed _, Some det) ->
              Some ("Merge execution details for file" ^
                    transcodeFilename path,
                    det)
          | _ ->
              match si.ri.replicas with
                Problem err ->
                  Some ("Errors for file " ^ transcodeFilename path, err)
              | Different diff ->
                  let prefix s l =
                    Safelist.map (fun err -> Format.sprintf "%s%s\n" s err) l
                  in
                  let errors =
                    Safelist.append
                      (prefix "[root 1]: " diff.errors1)
                      (prefix "[root 2]: " diff.errors2)
                  in
                  let errors =
                    match si.whatHappened with
                       Some (Util.Failed err, _) -> err :: errors
                    |  _                         -> errors
                  in
                  Some ("Errors for file " ^ transcodeFilename path,
                        String.concat "\n" errors)
    in
    match details with
      None                  -> ((* Should not happen *))
    | Some (title, details) -> messageBox ~title (transcode details)
  in

  let detailsWindowSW =
    GBin.scrolled_window ~packing:(toplevelVBox#pack ~expand:false)
        ~shadow_type:`IN ~hpolicy:`AUTOMATIC ~vpolicy:`AUTOMATIC ()
  in
  let detailsWindow =
    GText.view ~editable:false ~packing:detailsWindowSW#add ()
  in
  let (width, height) = get_size_chars detailsWindow ~height:4 ~width:112 () in
  let () = detailsWindowSW#set_height_request height in
  (* width is set in [sizeMainWindow] *)

  let detailsWindowPath = detailsWindow#buffer#create_tag [] in
  let detailsWindowInfo =
    detailsWindow#buffer#create_tag [`FONT_DESC (Lazy.force fontMonospace)] in
  let detailsWindowError =
    detailsWindow#buffer#create_tag [`WRAP_MODE `WORD] in
  detailsWindow#misc#set_can_focus false;

  let updateButtons () =
    if not !busy then
      let actionPossible si =
        match si.whatHappened, si.ri.replicas with
          None, Different _ -> true
        | _                 -> false
      in
      match currentRow () with
        None ->
          grSet grAction (currentSelectedExists actionPossible);
          grSet grDiff false;
          grSet grDetail false
      | Some (_, si, _) ->
          let details =
            begin match si.ri.replicas with
              Different diff -> diff.errors1 <> [] || diff.errors2 <> []
            | Problem _      -> true
            end
              ||
            begin match si.whatHappened with
              Some (Util.Failed _, _) -> true
            | _                       -> false
            end
          in
          grSet grDetail details;
          let activateAction = actionPossible si in
          let activateDiff =
            activateAction &&
            match si.ri.replicas with
              Different {rc1 = {typ = `FILE}; rc2 = {typ = `FILE}} ->
                true
            | _ ->
                false
          in
          grSet grAction activateAction;
          grSet grDiff activateDiff
  in

  let makeRowVisible row =
    mainWindow#scroll_to_cell row status_view_col (* just a dummy column *)
  in

(*
  let makeFirstUnfinishedVisible pRiInFocus =
    let im = Array.length !theState in
    let rec find i =
      if i >= im then makeRowVisible im else
      match pRiInFocus (!theState.(i).ri), !theState.(i).whatHappened with
        true, None -> makeRowVisible i
      | _ -> find (i+1) in
    find 0
  in
*)

  let updateDetails () =
    begin match currentRow () with
      None ->
        detailsWindow#buffer#set_text ""
    | Some (_, si, _) ->
        let (formated, details) =
          match si.whatHappened with
          | Some(Util.Failed(s), _) ->
               (false, s)
          | None | Some(Util.Succeeded, _) ->
              match si.ri.replicas with
                Problem _ ->
                  (false, Uicommon.details2string si.ri "  ")
              | Different _ ->
                  (true, Uicommon.details2string si.ri "  ")
        in
        let path = Path.toString si.ri.path1 in
        detailsWindow#buffer#set_text "";
        detailsWindow#buffer#insert ~tags:[detailsWindowPath]
          (transcodeFilename path);
        let len = String.length details in
        let details =
          if details.[len - 1] = '\n' then String.sub details 0 (len - 1)
          else details
        in
        if details <> "" then
          detailsWindow#buffer#insert
             ~tags:[if formated then detailsWindowInfo else detailsWindowError]
             ("\n" ^ transcode details)
    end;
    (* Display text *)
    updateButtons () in

  (*********************************************************************
    Status window
   *********************************************************************)

  let statusHBox = GPack.hbox ~packing:(toplevelVBox#pack ~expand:false) () in

  let progressBar =
    GRange.progress_bar ~packing:(statusHBox#pack ~expand:false) () in

  progressBar#misc#modify_font detailsWindow#misc#pango_context#font_description;
  let (w, _) = get_size_chars progressBar ~width:28 ~height:1 () in
  progressBar#set_width_request w;
  progressBar#set_show_text true;
  progressBar#set_pulse_step 0.02;
  let progressBarPulse = ref false in

  let statusWindow =
    GMisc.statusbar ~packing:(statusHBox#pack ~expand:true) () in
  statusWindow#set_margin 0;
  let statusContext = statusWindow#new_context ~name:"status" in
  ignore (statusContext#push "");

  let displayStatus m =
    statusContext#pop ();
    if !progressBarPulse then progressBar#pulse ();
    ignore (statusContext#push (transcode m));
    (* Force message to be displayed immediately *)
    gtk_sync false
  in

  let formatStatus major minor = (Util.padto 30 (major ^ "  ")) ^ minor in

  (* Tell the Trace module about the status printer *)
  Trace.messageDisplayer := displayStatus;
  Trace.statusFormatter := formatStatus;
  Trace.sendLogMsgsToStderr := false;


  (* Window is created before initPrefs but we don't want the size to
     jump around after window has been shown (which is inevitable when
     height is specified in a profile). Scan the command line to check
     for height preference. *)
  begin try
    let prefName = List.hd (Prefs.name Uicommon.mainWindowHeight) in
    let clHeight = List.hd (Util.StringMap.find prefName (Prefs.scanCmdLine "")) in
    Prefs.set Uicommon.mainWindowHeight (int_of_string clHeight)
  with Not_found | Invalid_argument _ | Util.Fatal _ -> () end;

  let calcWinSize () =
    (* (Poor) approximation of row height. It is impossible to get real
       GTK TreeView row height (and it depends on theme). *)
    let row_height = (List.hd mainWindow#all_children)#misc#allocation.height in
    let height =
      if row_height < 2 then   (* Oops, sizes clearly not allocated yet *)
        let metrics = mainWindowSW#misc#pango_context#get_metrics () in
        let h = GPango.to_pixels (metrics#ascent + metrics#descent) in
        (h + 8) * (8 + (Prefs.read Uicommon.mainWindowHeight)) (* rought default *)
      else
          topHBox#misc#allocation.height
        + actionBar#misc#allocation.height
        + 2 * mainWindow#border_width  (* top and bottom *)
        + row_height  (* column headers *)
        + (row_height - 3) * (Prefs.read Uicommon.mainWindowHeight)
        + detailsWindowSW#misc#allocation.height
        + statusHBox#misc#allocation.height
    in
    let height = min height (Gdk.Screen.height ~screen:toplevelWindow#screen ()) in
    let width =
      let metrics = mainWindowSW#misc#pango_context#get_metrics () in
      let w = GPango.to_pixels metrics#approx_digit_width in
      max (w * 112) 860
    in
    let width = min width (Gdk.Screen.width ~screen:toplevelWindow#screen ()) in
    (height, width)
  in

  let prevHeightPref = ref 0 in

  let sizeMainWindow () =
    (* Only update height if the preference changed, otherwise risk undoing
       user's manual height adjustments. Also assume no change if the
       preference is at the default value. *)
    let prefHeight = Prefs.read Uicommon.mainWindowHeight in
    if !prevHeightPref <> prefHeight &&
        (!prevHeightPref = 0 ||
          prefHeight <> Prefs.readDefault Uicommon.mainWindowHeight) then begin
      let (height, _) = calcWinSize ()
      and width = toplevelWindow#misc#allocation.width in
      toplevelWindow#resize ~height ~width
    end;
    prevHeightPref := prefHeight
  in
  let (height, width) = calcWinSize () in
  toplevelWindow#set_default_size ~height ~width;
  ignore (toplevelWindow#misc#connect#show ~callback:sizeMainWindow);

  (*********************************************************************
    Functions used to print in the main window
   *********************************************************************)
  let delayUpdates = ref false in

  let select row scroll =
    delayUpdates := true;
    mainWindow#selection#unselect_all ();
    mainWindow#selection#select_path row;
    mainWindow#set_cursor row status_view_col (* just a dummy column *);
    delayUpdates := false;
    if scroll then makeRowVisible row;
    updateDetails ()
  in
  let selectI i scroll = select (rowOfSi i) scroll in

  ignore (mainWindow#selection#connect#changed ~callback:
      (fun () -> if not !delayUpdates then updateDetails ()));

  let nextInteresting () =
    let l = Array.length !theState in
    let start = match currentRow () with Some (i, _, _) -> i + 1 | None -> 0 in
    let rec loop i =
      if i < l then
        match !theState.(i).ri.replicas with
          Different {direction = dir}
              when not (Prefs.read Uicommon.auto) || isConflict dir ->
            selectI i true
        | _ ->
            loop (i + 1) in
    loop start in
  let selectSomethingIfPossible () =
    if currentNumberRows () = 0 then nextInteresting () in

  let columnsOf si =
    let oldPath = Path.empty in
    let status =
      match si.ri.replicas with
        Different {direction = Conflict _} | Problem _ ->
          NoStatus
      | _ ->
          match si.whatHappened with
            None                     -> NoStatus
          | Some (Util.Succeeded, _) -> Done
          | Some (Util.Failed _, _)  -> Failed
    in
    let (r1, action, r2, path) =
      Uicommon.reconItem2stringList oldPath si.ri in
    (r1, action, r2, status, path)
  in

  let greenPixel  = "00dd00" in
  let redPixel    = "ff2040" in
  let lightbluePixel = "8888FF" in
  let orangePixel = "ff9303" in
(*
  let yellowPixel = "999900" in
  let blackPixel  = "000000" in
*)
  let buildPixmap p =
    Pixmaps.to_pixbuf p in
  let buildPixmaps f c1 =
    (buildPixmap (f c1), buildPixmap (f lightbluePixel)) in

  let doneIcon = buildPixmap Pixmaps.success in
  let failedIcon = buildPixmap Pixmaps.failure in
  let rightArrow = buildPixmaps Pixmaps.copyAB greenPixel in
  let leftArrow = buildPixmaps Pixmaps.copyBA greenPixel in
  let orangeRightArrow = buildPixmaps Pixmaps.copyAB orangePixel in
  let orangeLeftArrow = buildPixmaps Pixmaps.copyBA orangePixel in
  let ignoreAct = buildPixmaps Pixmaps.ignore redPixel in
  let failedIcons = (failedIcon, failedIcon) in
  let mergeLogo = buildPixmaps Pixmaps.mergeLogo greenPixel in
(*
  let rightArrowBlack = buildPixmap (Pixmaps.copyAB blackPixel) in
  let leftArrowBlack = buildPixmap (Pixmaps.copyBA blackPixel) in
  let mergeLogoBlack = buildPixmap (Pixmaps.mergeLogo blackPixel) in
*)

  let getArrow j action =
    let changedFromDefault = match !theState.(j).ri.replicas with
        Different diff -> diff.direction <> diff.default_direction
      | _ -> false in
    let sel pixmaps =
      if changedFromDefault then snd pixmaps else fst pixmaps in
    let pixmaps =
      match action with
        Uicommon.AError      -> failedIcons
      | Uicommon.ASkip _     -> ignoreAct
      | Uicommon.ALtoR false -> rightArrow
      | Uicommon.ALtoR true  -> orangeRightArrow
      | Uicommon.ARtoL false -> leftArrow
      | Uicommon.ARtoL true  -> orangeLeftArrow
      | Uicommon.AMerge      -> mergeLogo
    in
    sel pixmaps
  in


  let getStatusIcon = function
    | Failed   -> Some failedIcon
    | Done     -> Some doneIcon
    | NoStatus -> None in

  let displayRowAction row i action =
    mainWindowModel#set ~row ~column:c_action (getArrow i action) in
  let displayRowStatus row status =
    mainWindowModel#set ~row ~column:c_status (getStatusIcon status);
    if status <> NoStatus then
      mainWindowModel#set ~row ~column:c_statust "" in
  let displayRowPath row path =
    mainWindowModel#set ~row ~column:c_path (transcodeFilename path) in
  let displayRow row i r1 r2 action status path =
    mainWindowModel#set ~row ~column:c_replica1 r1;
    mainWindowModel#set ~row ~column:c_replica2 r2;
    displayRowAction row i action;
    displayRowStatus row status;
    displayRowPath row path;
    (*mainWindowModel#set ~row ~column:c_rowid i;*)
  in

  let displayMain() =
    (* The call to mainWindow#clear below side-effect current,
       so we save the current value before we clear out the main window and
       rebuild it. *)
    let savedCurrent = mainWindow#selection#get_selected_rows in
    mainWindow#set_model None;
    mainWindowModel#clear ();
    let tot = Array.length !theState - 1 in
    let totf = float_of_int (tot + 1) in
    progressBar#set_text (Printf.sprintf "Displaying %i items..." (tot + 1));
    for i = 0 to tot do
      if i mod 1024 = 0 then begin
        progressBar#set_fraction (max 0. (min 1. ((float_of_int i) /. totf)));
        gtk_sync false
      end;

      let (r1, action, r2, status, path) = columnsOf !theState.(i) in

      let row = mainWindowModel#append () in
      displayRow row i r1 r2 action status path;
    done;
    mainWindow#set_model (Some mainWindowModel#coerce);
    begin match savedCurrent with
    | []  -> selectSomethingIfPossible ()
    | [x] -> select x true
    | _   -> Safelist.iter (fun p -> mainWindow#selection#select_path p) savedCurrent
    end;

    progressBar#set_text ""; progressBar#set_fraction 0.;
    updateDetails ();  (* Do we need this line? *)
 in

  let redisplay i si iter =
    let (_, action, _, status, path) = columnsOf si in
    displayRowAction iter i action;
    displayRowStatus iter status;
    if status = Failed then displayRowPath iter (path ^
               "       [failed: click on this line for details]");
  in

  let fastRedisplay i =
    let si = !theState.(i) in
    let iter = mainWindowModel#get_iter (rowOfSi i) in
    let (_, action, _, status, path) = columnsOf si in
    displayRowStatus iter status;
    if status = Failed then begin
      displayRowPath iter (path ^
               "       [failed: click on this line for details]");
      match currentRow () with
      | Some (_, csi, _) when csi = si -> updateDetails ()
      | Some _ | None -> ()
    end
  in

  let updateRowStatus i newstatus =
    let row = mainWindowModel#get_iter (rowOfSi i) in
    let oldstatus = mainWindowModel#get ~row ~column:c_statust in
    if oldstatus <> newstatus then mainWindowModel#set ~row ~column:c_statust newstatus
  in

  let totalBytesToTransfer = ref Uutil.Filesize.zero in
  let totalBytesTransferred = ref Uutil.Filesize.zero in

  let t1 = ref 0. in
  let lastFrac = ref 0. in
  let sta = ref (Uicommon.Stats.init (Uutil.Filesize.zero)) in
  let displayGlobalProgress v =
    if v = 0. || abs_float (v -. !lastFrac) > 1. then begin
      lastFrac := v;
      progressBar#set_fraction (max 0. (min 1. (v /. 100.)))
    end;
    if v < 0.001 then
      progressBar#set_text " "
    else begin
      let t = Unix.gettimeofday () in
      Uicommon.Stats.update !sta t !totalBytesTransferred;
      let delta = t -. !t1 in
      if delta >= 0.5 then begin
        t1 := t;
        let remTime =
          if v >= 100. then "00:00 remaining" else
          (Uicommon.Stats.eta !sta "--:--") ^ " remaining"
        in
        let rate = Uicommon.Stats.avgRate1 !sta in
        let txt =
          if rate > 99. then
            Format.sprintf "%s  (%s)" remTime (rate2str rate)
          else
            remTime
        in
        progressBar#set_text txt
      end
    end
  in

  let showGlobalProgress b =
    (* Concatenate the new message *)
    totalBytesTransferred := Uutil.Filesize.add !totalBytesTransferred b;
    let v =
      (Uutil.Filesize.percentageOfTotalSize
         !totalBytesTransferred !totalBytesToTransfer)
    in
    displayGlobalProgress v
  in

  let root1IsLocal = ref true in
  let root2IsLocal = ref true in

  let initGlobalProgress b =
    let (root1,root2) = Globals.roots () in
    root1IsLocal := fst root1 = Local;
    root2IsLocal := fst root2 = Local;
    totalBytesToTransfer := b;
    totalBytesTransferred := Uutil.Filesize.zero;
    t1 := Unix.gettimeofday ();
    sta := Uicommon.Stats.init !totalBytesToTransfer;
    displayGlobalProgress 0.
  in

  let showProgress i bytes dbg =
    let i = Uutil.File.toLine i in
    let item = !theState.(i) in
    item.bytesTransferred <- Uutil.Filesize.add item.bytesTransferred bytes;
    let b = item.bytesTransferred in
    let len = item.bytesToTransfer in
    let newstatus =
      if b = Uutil.Filesize.zero || len = Uutil.Filesize.zero then "start "
      else if len = Uutil.Filesize.zero then
        Printf.sprintf "%5s " (Uutil.Filesize.toString b)
      else Util.percent2string (Uutil.Filesize.percentageOfTotalSize b len) in
    let dbg = if Trace.enabled "progress" then dbg ^ "/" else "" in
    let newstatus = dbg ^ newstatus in
    updateRowStatus i newstatus;
    showGlobalProgress bytes;
    gtk_sync false;
    begin match item.ri.replicas with
      Different diff ->
        begin match diff.direction with
          Replica1ToReplica2 ->
            if !root2IsLocal then
              clientWritten := !clientWritten +. Uutil.Filesize.toFloat bytes
            else
              serverWritten := !serverWritten +. Uutil.Filesize.toFloat bytes
        | Replica2ToReplica1 ->
            if !root1IsLocal then
              clientWritten := !clientWritten +. Uutil.Filesize.toFloat bytes
            else
              serverWritten := !serverWritten +. Uutil.Filesize.toFloat bytes
        | Conflict _ | Merge ->
            (* Diff / merge *)
            clientWritten := !clientWritten +. Uutil.Filesize.toFloat bytes
        end
    | _ ->
        assert false
    end
  in

  (* Install showProgress so that we get called back by low-level
     file transfer stuff *)
  Uutil.setProgressPrinter showProgress;

  (* Apply new ignore patterns to the current state, expecting that the
     number of reconitems will grow smaller. Adjust the display, being
     careful to keep the cursor as near as possible to its position
     before the new ignore patterns take effect. *)
  let ignoreAndRedisplay () =
    let lst = Array.to_list !theState in
    (* FIX: we should actually test whether any prefix is now ignored *)
    let keep sI = not (Globals.shouldIgnore sI.ri.path1) in
    theState := Array.of_list (Safelist.filter keep lst);
    displayMain() in

  let sortAndRedisplay () =
    let compareRIs = Sortri.compareReconItems() in
    Array.stable_sort (fun si1 si2 -> compareRIs si1.ri si2.ri) !theState;
    displayMain() in

  (******************************************************************
   Main detect-updates-and-reconcile logic
   ******************************************************************)

  let commitUpdates () =
    Trace.status "Updating synchronizer state";
    let t = Trace.startTimer "Updating synchronizer state" in
    gtk_sync true;
    Update.commitUpdates();
    Trace.showTimer t
  in

  let clearMainWindow () =
    grDisactivateAll ();
    make_busy toplevelWindow;
    mainWindow#set_model None;
    mainWindowModel#clear ();
    mainWindow#set_model (Some mainWindowModel#coerce);
    theState := [||];
    detailsWindow#buffer#set_text ""
  in

  let detectUpdatesAndReconcile () =
    clearMainWindow ();
    startStats ();
    progressBarPulse := true;
    sync_action := Some (fun () -> progressBar#pulse ());
    let findUpdates () =
      let t = Trace.startTimer "Checking for updates" in
      Trace.status "Looking for changes";
      let updates = Update.findUpdates ~wantWatcher:true !unsynchronizedPaths in
      Trace.showTimer t;
      updates in
    let reconcile updates =
      let t = Trace.startTimer "Reconciling" in
      let reconRes = Recon.reconcileAll ~allowPartial:true updates in
      Trace.showTimer t;
      reconRes in
    let (reconItemList, thereAreEqualUpdates, dangerousPaths) =
      reconcile (findUpdates ()) in
    if not !Update.foundArchives then commitUpdates ();
    if reconItemList = [] then begin
      if !Update.foundArchives then commitUpdates ();
      if thereAreEqualUpdates then
        Trace.status
          "Replicas have been changed only in identical ways since last sync"
      else
        Trace.status "Everything is up to date"
    end else
      Trace.status "Check and/or adjust selected actions; then press Go";
    theState :=
      Array.of_list
         (Safelist.map
            (fun ri -> { ri = ri;
                         bytesTransferred = Uutil.Filesize.zero;
                         bytesToTransfer = Uutil.Filesize.zero;
                         whatHappened = None })
            reconItemList);
    unsynchronizedPaths :=
      Some (Safelist.map (fun ri -> ri.path1) reconItemList, []);
    progressBarPulse := false; sync_action := None; displayGlobalProgress 0.;
    displayMain();
    progressBarPulse := false; sync_action := None; displayGlobalProgress 0.;
    stopStats ();
    grSet grGo (Array.length !theState > 0);
    grSet grRescan true;
    make_interactive toplevelWindow;
    if Prefs.read Globals.confirmBigDeletes then begin
      if dangerousPaths <> [] then begin
        Prefs.set Globals.batch false;
        Util.warn (Uicommon.dangerousPathMsg dangerousPaths)
      end;
    end;
  in

  (*********************************************************************
    Help menu
   *********************************************************************)
  let addDocSection (shortname, (name, docstr)) =
    let parent = toplevelWindow in
    if shortname = "about" then
      ignore (helpMenu#add_image_item
                ~stock:`ABOUT ~callback:(fun () -> documentation ~parent shortname)
                name)
    else if shortname <> "" && name <> "" then
      ignore (helpMenu#add_item
                ~callback:(fun () -> documentation ~parent shortname)
                name) in
  Safelist.iter addDocSection Strings.docs;

  (*********************************************************************
    Ignore menu
   *********************************************************************)
  let addRegExpByPath pathfunc =
    Util.StringSet.iter (fun pat -> Uicommon.addIgnorePattern pat)
      (currentSelectedFold
         (fun s si -> Util.StringSet.add (pathfunc si.ri.path1) s)
         Util.StringSet.empty);
    ignoreAndRedisplay ()
  in
  grAdd grAction
    (ignoreMenu#add_item ~key:GdkKeysyms._i
       ~callback:(fun () -> getLock (fun () ->
          addRegExpByPath Uicommon.ignorePath))
       "Permanently Ignore This _Path");
  grAdd grAction
    (ignoreMenu#add_item ~key:GdkKeysyms._E
       ~callback:(fun () -> getLock (fun () ->
          addRegExpByPath Uicommon.ignoreExt))
       "Permanently Ignore Files with this _Extension");
  grAdd grAction
    (ignoreMenu#add_item ~key:GdkKeysyms._N
       ~callback:(fun () -> getLock (fun () ->
          addRegExpByPath Uicommon.ignoreName))
       "Permanently Ignore Files with this _Name (in any Dir)");

  (*
  grAdd grRescan
    (ignoreMenu#add_item ~callback:
       (fun () -> getLock ignoreDialog) "Edit ignore patterns");
  *)

  (*********************************************************************
    Sort menu
   *********************************************************************)
  grAdd grRescan
    (sortMenu#add_item
       ~callback:(fun () -> getLock (fun () ->
          Sortri.sortByName();
          sortAndRedisplay()))
       "Sort by _Name");
  grAdd grRescan
    (sortMenu#add_item
       ~callback:(fun () -> getLock (fun () ->
          Sortri.sortBySize();
          sortAndRedisplay()))
       "Sort by _Size");
  grAdd grRescan
    (sortMenu#add_item
       ~callback:(fun () -> getLock (fun () ->
          Sortri.sortNewFirst();
          sortAndRedisplay()))
       "Sort Ne_w Entries First (toggle)");
  grAdd grRescan
    (sortMenu#add_item
       ~callback:(fun () -> getLock (fun () ->
          Sortri.restoreDefaultSettings();
          sortAndRedisplay()))
       "_Default Ordering");

  (*********************************************************************
    Main function : synchronize
   *********************************************************************)
  let synchronize () =
    if Array.length !theState = 0 then
      Trace.status "Nothing to synchronize"
    else begin
      grDisactivateAll ();
      make_busy toplevelWindow;

      Trace.status "Propagating changes";
      Uicommon.transportStart ();
      grSet grStop true;
      let totalLength =
        Array.fold_left
          (fun l si ->
             si.bytesTransferred <- Uutil.Filesize.zero;
             let len =
               if si.whatHappened = None then Common.riLength si.ri else
               Uutil.Filesize.zero
             in
             si.bytesToTransfer <- len;
             Uutil.Filesize.add l len)
          Uutil.Filesize.zero !theState in
      initGlobalProgress totalLength;
      let t = Trace.startTimer "Propagating changes" in
      let uiWrapper i theSI =
        match theSI.whatHappened with
          None ->
            let textDetailed = ref None in
            catch (fun () ->
                     Transport.transportItem
                       theSI.ri (Uutil.File.ofLine i)
                       (fun title text ->
                         textDetailed := (Some text);
                         if Prefs.read Uicommon.confirmmerge then
                           twoBoxAdvanced
                             ~parent:toplevelWindow
                             ~title:title
                             ~message:("Do you want to commit the changes to"
                                       ^ " the replicas ?")
                             ~longtext:text
                             ~advLabel:"View details..."
                             ~astock:`YES
                             ~bstock:`NO
                         else
                           true)
                     >>= (fun () ->
                       return Util.Succeeded))
                   (fun e ->
                     match e with
                       Util.Transient s ->
                         return (Util.Failed s)
                     | _ ->
                         fail e)
              >>= (fun res ->
                let rem =
                  Uutil.Filesize.sub
                    theSI.bytesToTransfer theSI.bytesTransferred
                in
                if rem <> Uutil.Filesize.zero then
                  showProgress (Uutil.File.ofLine i) rem "done";
                theSI.whatHappened <- Some (res, !textDetailed);
            fastRedisplay i;
            gtk_sync false;
            return ())
        | Some _ ->
            return () (* Already processed this one (e.g. merged it) *)
      in
      startStats ();
      Uicommon.transportItems !theState (fun {ri; _} -> not (Common.isDeletion ri)) uiWrapper;
      Uicommon.transportItems !theState (fun {ri; _} -> Common.isDeletion ri) uiWrapper;
      Uicommon.transportFinish ();
      grSet grStop false;
      Trace.showTimer t;
      commitUpdates ();
      stopStats ();

      let failureList =
        Array.fold_right
          (fun si l ->
             match si.whatHappened with
               Some (Util.Failed err, _) ->
                 (si, [err], "transport failure") :: l
             | _ ->
                 l)
          !theState []
      in
      let failureCount = List.length failureList in
      let failures =
        if failureCount = 0 then [] else
        [Printf.sprintf "%d failure%s"
           failureCount (if failureCount = 1 then "" else "s")]
      in
      let partialList =
        Array.fold_right
          (fun si l ->
             match si.whatHappened with
               Some (Util.Succeeded, _)
               when partiallyProblematic si.ri &&
                    not (problematic si.ri) ->
                 let errs =
                   match si.ri.replicas with
                     Different diff -> diff.errors1 @ diff.errors2
                   | _              -> assert false
                 in
                 (si, errs,
                  "partial transfer (errors during update detection)") :: l
             | _ ->
                 l)
          !theState []
      in
      let partialCount = List.length partialList in
      let partials =
        if partialCount = 0 then [] else
        [Printf.sprintf "%d partially transferred" partialCount]
      in
      let skippedList =
        Array.fold_right
          (fun si l ->
             match si.ri.replicas with
               Problem err ->
                 (si, [err], "error during update detection") :: l
             | Different diff when isConflict diff.direction ->
                 (si, [],
                  if isConflict diff.default_direction then
                    "conflict"
                  else "skipped") :: l
             | _ ->
                 l)
          !theState []
      in
      let skippedCount = List.length skippedList in
      let skipped =
        if skippedCount = 0 then [] else
        [Printf.sprintf "%d skipped" skippedCount]
      in
      let nostartCount =
        if not (Abort.isAll ()) then 0 else
          Array.fold_left
            (fun c si -> if si.whatHappened = None then c + 1 else c)
            0 !theState
      in
      let nostart =
        if nostartCount = 0 then [] else
        [Printf.sprintf "%d not started" nostartCount]
      in
      unsynchronizedPaths :=
        Some (Safelist.map (fun (si, _, _) -> si.ri.path1)
                (failureList @ partialList @ skippedList),
              []);
      Trace.status
        (Printf.sprintf "Synchronization complete         %s"
           (String.concat ", " (failures @ partials @ skipped @ nostart)));
      displayGlobalProgress 0.;

      grSet grRescan true;
      make_interactive toplevelWindow;

      let totalCount = failureCount + partialCount + skippedCount + nostartCount in
      if totalCount > 0 then begin
        let format n item sing plur =
          match n with
            0 -> []
          | 1 -> [Format.sprintf "one %s%s" item sing]
          | n -> [Format.sprintf "%d %s%s" n item plur]
        in
        let infos =
          format failureCount "failure" "" "s" @
          format partialCount "partially transferred director" "y" "ies" @
          format skippedCount "skipped item" "" "s" @
          format nostartCount "not started item" "" "s"
        in
        let message =
          (if failureCount = 0 && nostartCount = 0 then
             "The synchronization was successful.\n\n"
           else "") ^
          "The replicas are not fully synchronized.\n" ^
          (if totalCount < 2 then "There was" else "There were") ^
          begin match infos with
            [] -> assert false
          | [x] -> " " ^ x
          | l -> ":\n  - " ^ String.concat ";\n  - " l
          end ^
          "."
        in
        summaryBox ~parent:toplevelWindow
          ~title:"Synchronization summary" ~message ~f:
          (fun t ->
             let bullet = "\xe2\x80\xa2 " in
             let layout = Pango.Layout.create t#misc#pango_context#as_context in
             Pango.Layout.set_text layout bullet;
             let (n, _) = Pango.Layout.get_pixel_size layout in
             let path =
               t#buffer#create_tag [`FONT_DESC (Lazy.force fontBold)] in
             let description =
               t#buffer#create_tag [`FONT_DESC (Lazy.force fontItalic)] in
             let errorFirstLine =
               t#buffer#create_tag [`LEFT_MARGIN (n); `INDENT (- n)] in
             let errorNextLines =
               t#buffer#create_tag [`LEFT_MARGIN (2 * n)] in
             List.iter
               (fun (si, errs, desc) ->
                  t#buffer#insert ~tags:[path]
                    (transcodeFilename (Path.toString si.ri.path1));
                  t#buffer#insert ~tags:[description]
                    (" \xe2\x80\x94 " ^ desc ^ "\n");
                  List.iter
                    (fun err ->
                       let errl =
                         Str.split (Str.regexp_string "\n") (transcode err) in
                       match errl with
                         [] ->
                           ()
                       | f :: rem ->
                           t#buffer#insert ~tags:[errorFirstLine]
                             (bullet ^ f ^ "\n");
                           List.iter
                             (fun n ->
                                t#buffer#insert ~tags:[errorNextLines]
                                  (n ^ "\n"))
                             rem)
                    errs)
               (failureList @ partialList @ skippedList))
      end

    end in

  (*********************************************************************
    Buttons for -->, M, <--, Skip
   *********************************************************************)
  let doActionOnRow f i theSI iter =
    begin match theSI.whatHappened, theSI.ri.replicas with
      None, Different diff ->
        f theSI.ri diff;
        redisplay i theSI iter
    | _ ->
        ()
    end
  in
  let doAction f =
    match currentRow () with
      Some (i, si, iter) ->
        doActionOnRow f i si iter;
        nextInteresting ()
    | None ->
        currentSelectedIter (fun i si iter -> doActionOnRow f i si iter);
        updateDetails ()
  in
  let leftAction _ =
    doAction (fun _ diff -> diff.direction <- Replica2ToReplica1) in
  let rightAction _ =
    doAction (fun _ diff -> diff.direction <- Replica1ToReplica2) in
  let questionAction _ = doAction (fun _ diff -> diff.direction <- Conflict "") in
  let mergeAction    _ = doAction (fun _ diff -> diff.direction <- Merge) in

  let insert_button (toolbar : #GButton.toolbar) ~stock ~text ~tooltip ~callback () =
    let b = GButton.tool_button ~stock ~label:text ~packing:toolbar#insert () in
    ignore (b#connect#clicked ~callback);
    b#misc#set_tooltip_text tooltip;
    b
  in

(*  actionBar#insert_space ();*)
  grAdd grAction
    (insert_button actionBar
       ~stock:`GO_FORWARD
       ~text:"Left to Right"
       ~tooltip:"Propagate selected items\n\
                 from the left replica to the right one"
       ~callback:rightAction ());
(*  actionBar#insert_space ();*)
  grAdd grAction
    (insert_button actionBar ~text:"Skip"
       ~stock:`NO
       ~tooltip:"Skip selected items"
       ~callback:questionAction ());
(*  actionBar#insert_space ();*)
  grAdd grAction
    (insert_button actionBar
       ~stock:`GO_BACK
       ~text:"Right to Left"
       ~tooltip:"Propagate selected items\n\
                 from the right replica to the left one"
       ~callback:leftAction ());
(*  actionBar#insert_space ();*)
  grAdd grAction
    (insert_button actionBar
       ~stock:`ADD
       ~text:"Merge"
       ~tooltip:"Merge selected files"
       ~callback:mergeAction ());

  (*********************************************************************
    Diff / merge buttons
   *********************************************************************)
  let diffCmd () =
    match currentRow () with
      Some (i, item, _) ->
        getLock (fun () ->
          let len =
            match item.ri.replicas with
              Problem _ ->
                Uutil.Filesize.zero
            | Different diff ->
                snd (if !root1IsLocal then diff.rc2 else diff.rc1).size
          in
          item.bytesTransferred <- Uutil.Filesize.zero;
          item.bytesToTransfer <- len;
          initGlobalProgress len;
          startStats ();
          let styleDiff (t_text : scrolled_text) =
            let diffAdd =
              t_text#text#buffer#create_tag [`FOREGROUND "green"] in
            let diffDel =
              t_text#text#buffer#create_tag [`FOREGROUND "red"] in
            let diffLoc =
              t_text#text#buffer#create_tag [`FOREGROUND "dark cyan"; `WEIGHT `BOLD] in
            let setStyle sty ~start ~stop =
              t_text#text#buffer#apply_tag sty ~start ~stop
            in
            let rec styleDiffLine ~start =
              let stop = start#forward_line in
              let styleLine tag = setStyle tag ~start ~stop in
              let () =
                match start#get_text ~stop:start#forward_char with
                | "+" -> styleLine diffAdd
                | "-" -> styleLine diffDel
                | "@" -> styleLine diffLoc
                | _ -> ()
              in
              if not (start#equal stop) then styleDiffLine ~start:stop
            in
            styleDiffLine ~start:(t_text#text#buffer#start_iter);
          in
          Uicommon.showDiffs item.ri
            (fun title text ->
               messageBox ~title:(transcode title) (transcode text)
                 ~styleText:styleDiff)
            Trace.status (Uutil.File.ofLine i);
          stopStats ();
          displayGlobalProgress 0.;
          fastRedisplay i)
    | None ->
        () in

  actionBar#insert (GButton.separator_tool_item ());
  grAdd grDiff (insert_button actionBar ~text:"Diff"
                  ~stock:`DIALOG_INFO
                  ~tooltip:"Compare the two files at each replica"
                  ~callback:diffCmd ());

  (*********************************************************************
    Detail button
   *********************************************************************)
(*  actionBar#insert_space ();*)
  grAdd grDetail (insert_button actionBar ~text:"Details"
                    ~stock:`INFO
                    ~tooltip:"Show detailed information about\n\
                              an item, when available"
                    ~callback:showDetCommand ());

  (*********************************************************************
    Quit button
   *********************************************************************)
(*  actionBar#insert_space ();
  ignore (actionBar#insert_button ~text:"Quit"
            ~icon:((GMisc.image ~stock:`QUIT ())#coerce)
            ~tooltip:"Exit Unison"
            ~callback:safeExit ());
*)

  (*********************************************************************
    go button
   *********************************************************************)
  actionBar#insert (GButton.separator_tool_item ());
  grAdd grGo
    (insert_button actionBar ~text:"Go"
       (* tooltip:"Go with displayed actions" *)
       ~stock:`EXECUTE
       ~tooltip:"Perform the synchronization"
       ~callback:(fun () ->
                    getLock synchronize) ());

  grAdd grStop
    (insert_button actionBar ~text:"Stop"
       ~stock:`STOP
       ~tooltip:"Stop update propagation"
       ~callback:Abort.all ());

  (*********************************************************************
    Rescan button
   *********************************************************************)
  let profileInitSuccess = ref false in
  let updateFromProfile = ref (fun () -> ()) in

  let loadProfile p reload =
    debug (fun()-> Util.msg "Loading profile %s..." p);
    Trace.status "Loading profile";
    unsynchronizedPaths := None;
    profileInitSuccess := false;
    Uicommon.initPrefs ~profileName:p ~promptForRoots ~prepDebug ();
    Uicommon.connectRoots
      ~displayWaitMessage:(fun () -> if not reload then displayWaitMessage ())
      ~termInteract ();
    profileInitSuccess := true;
    !updateFromProfile ()
  in

  let reloadProfile () =
    let n =
      match !Prefs.profileName with
        None   -> assert false
      | Some n -> n
    in
    clearMainWindow ();
    if not (Prefs.profileUnchanged ()) || not (!profileInitSuccess) then
      loadProfile n true
    else Uicommon.connectRoots ~displayWaitMessage ~termInteract ()
  in

  let detectCmd () =
    mainWindow#misc#grab_focus ();
    if !profileInitSuccess then begin
      getLock detectUpdatesAndReconcile;
      updateDetails ();
      if Prefs.read Globals.batch then begin
        Prefs.set Globals.batch false; synchronize()
      end
    end else begin
      grSet grRescan true;
      make_interactive toplevelWindow
    end
  in
  let loadAndRunProfile p =
    clearMainWindow ();
    loadProfile p false;
    detectCmd ()
  in

(*  actionBar#insert_space ();*)
  grAdd grRescan
    (insert_button actionBar ~text:"Rescan"
       ~stock:`REFRESH
       ~tooltip:"Check for updates"
       ~callback: (fun () -> reloadProfile(); detectCmd()) ());

  (*********************************************************************
    Profile change button
   *********************************************************************)
  actionBar#insert (GButton.separator_tool_item ());
  let profileChange _ =
    match getProfile false with
      None   -> ()
    | Some p -> loadAndRunProfile p
  in
  grAdd grRescan (insert_button actionBar ~text:"Change Profile"
                    ~stock:`OPEN
                    ~tooltip:"Select a different profile"
                    ~callback:profileChange ());

  (*********************************************************************
    Keyboard commands
   *********************************************************************)
  ignore
    (mainWindow#event#connect#key_press ~callback:
       begin fun ev ->
         let key = GdkEvent.Key.keyval ev in
         if key = GdkKeysyms._Left then begin
           leftAction (); GtkSignal.stop_emit (); true
         end else if key = GdkKeysyms._Right then begin
           rightAction (); GtkSignal.stop_emit (); true
         end else
           false
       end);

  (*********************************************************************
    Action menu
   *********************************************************************)
  let buildActionMenu init =
    let withDelayedUpdates f x =
      delayUpdates := true;
      f x;
      delayUpdates := false;
      updateDetails () in
    let actionMenu = replace_submenu "_Actions" actionItem in
    grAdd grRescan
      (actionMenu#add_image_item
         ~callback:(fun _ -> withDelayedUpdates mainWindow#selection#select_all ())
         ~image:((GMisc.image ~stock:`SELECT_ALL ~icon_size:`MENU ())#coerce)
         ~modi:[`CONTROL] ~key:GdkKeysyms._A
         "Select _All");
    grAdd grRescan
      (actionMenu#add_item
         ~callback:(fun _ -> withDelayedUpdates mainWindow#selection#unselect_all ())
         ~modi:[`SHIFT; `CONTROL] ~key:GdkKeysyms._A
         "_Deselect All");

    ignore (actionMenu#add_separator ());

    let (loc1, loc2) =
      if init then ("", "") else
      let (root1,root2) = Globals.roots () in
      (root2hostname root1, root2hostname root2)
    in
    let def_descr = "Left to Right" in
    let descr =
      if init || loc1 = loc2 then def_descr else
      Printf.sprintf "from %s to %s" loc1 loc2 in
    let left =
      actionMenu#add_image_item ~key:GdkKeysyms._greater ~callback:rightAction
        ~image:((GMisc.image ~stock:`GO_FORWARD ~icon_size:`MENU ())#coerce)
        ~name:("Propagate " ^ def_descr) ("Propagate " ^ descr) in
    grAdd grAction left;
    left#add_accelerator ~group:accel_group ~modi:[`SHIFT] GdkKeysyms._greater;
    left#add_accelerator ~group:accel_group GdkKeysyms._period;

    let def_descl = "Right to Left" in
    let descl =
      if init || loc1 = loc2 then def_descl else
      Printf.sprintf "from %s to %s"
        (Unicode.protect loc2) (Unicode.protect loc1) in
    let right =
      actionMenu#add_image_item ~key:GdkKeysyms._less ~callback:leftAction
        ~image:((GMisc.image ~stock:`GO_BACK ~icon_size:`MENU ())#coerce)
        ~name:("Propagate " ^ def_descl) ("Propagate " ^ descl) in
    grAdd grAction right;
    right#add_accelerator ~group:accel_group ~modi:[`SHIFT] GdkKeysyms._less;
    right#add_accelerator ~group:accel_group ~modi:[`SHIFT] GdkKeysyms._comma;

    let skip =
      actionMenu#add_image_item ~key:GdkKeysyms._slash ~callback:questionAction
        ~image:((GMisc.image ~stock:`NO ~icon_size:`MENU ())#coerce)
        "Do _Not Propagate Changes" in
    grAdd grAction skip;
    skip#add_accelerator ~group:accel_group ~modi:[`SHIFT] GdkKeysyms._minus;
    skip#add_accelerator ~group:accel_group GdkKeysyms._KP_Divide;

    let merge =
      actionMenu#add_image_item ~key:GdkKeysyms._m ~callback:mergeAction
        ~image:((GMisc.image ~stock:`ADD ~icon_size:`MENU ())#coerce)
        "_Merge the Files" in
    grAdd grAction merge;
  (* merge#add_accelerator ~group:accel_group ~modi:[`SHIFT] GdkKeysyms._m; *)

    (* Override actions *)
    ignore (actionMenu#add_separator ());
    grAdd grAction
      (actionMenu#add_item
         ~callback:(fun () ->
            doAction (fun ri _ ->
                        Recon.setDirection ri `Replica1ToReplica2 `Prefer))
         "Resolve Conflicts in Favor of First Root");
    grAdd grAction
      (actionMenu#add_item
         ~callback:(fun () ->
            doAction (fun ri _ ->
                        Recon.setDirection ri `Replica2ToReplica1 `Prefer))
         "Resolve Conflicts in Favor of Second Root");
    grAdd grAction
      (actionMenu#add_item
         ~callback:(fun () ->
            doAction (fun ri _ ->
                        Recon.setDirection ri `Newer `Prefer))
         "Resolve Conflicts in Favor of Most Recently Modified");
    grAdd grAction
      (actionMenu#add_item
         ~callback:(fun () ->
            doAction (fun ri _ ->
                        Recon.setDirection ri `Older `Prefer))
         "Resolve Conflicts in Favor of Least Recently Modified");
    ignore (actionMenu#add_separator ());
    grAdd grAction
      (actionMenu#add_item
         ~callback:(fun () ->
            doAction (fun ri _ -> Recon.setDirection ri `Newer `Force))
         "Force Newer Files to Replace Older Ones");
    grAdd grAction
      (actionMenu#add_item
         ~callback:(fun () ->
            doAction (fun ri _ -> Recon.setDirection ri `Older `Force))
         "Force Older Files to Replace Newer Ones");
    ignore (actionMenu#add_separator ());
    grAdd grAction
      (actionMenu#add_item
         ~callback:(fun () ->
            doAction (fun ri _ -> Recon.revertToDefaultDirection ri))
         "_Revert to Unison's Recommendation");
    grAdd grAction
      (actionMenu#add_item
         ~callback:(fun () ->
            doAction (fun ri _ -> Recon.setDirection ri `Merge `Force))
         "Revert to the Merging Default, if Available");

    (* Diff *)
    ignore (actionMenu#add_separator ());
    grAdd grDiff (actionMenu#add_image_item ~key:GdkKeysyms._d ~callback:diffCmd
        ~image:((GMisc.image ~stock:`DIALOG_INFO ~icon_size:`MENU ())#coerce)
        "Show _Diffs");

    (* Details *)
    grAdd grDetail
      (actionMenu#add_image_item ~key:GdkKeysyms._i ~callback:showDetCommand
        ~image:((GMisc.image ~stock:`INFO ~icon_size:`MENU ())#coerce)
        "Detailed _Information")

  in
  buildActionMenu true;

  (*********************************************************************
    Synchronization menu
   *********************************************************************)

  grAdd grGo
    (fileMenu#add_image_item ~key:GdkKeysyms._g
       ~image:(GMisc.image ~stock:`EXECUTE ~icon_size:`MENU () :> GObj.widget)
       ~callback:(fun () -> getLock synchronize)
       "_Go");
  grAdd grRescan
    (fileMenu#add_image_item ~key:GdkKeysyms._r
       ~image:(GMisc.image ~stock:`REFRESH ~icon_size:`MENU () :> GObj.widget)
       ~callback:(fun () -> reloadProfile(); detectCmd())
       "_Rescan");
  grAdd grRescan
    (fileMenu#add_item ~key:GdkKeysyms._a
       ~callback:(fun () ->
                    reloadProfile();
                    Prefs.set Globals.batch true;
                    detectCmd())
       "_Detect Updates and Proceed (Without Waiting)");
  grAdd grRescan
    (fileMenu#add_item ~key:GdkKeysyms._f
       ~callback:(
         fun () ->
           let rec loop i acc =
             if i >= Array.length (!theState) then acc else
             let notok =
               (match !theState.(i).whatHappened with
                   None-> true
                 | Some(Util.Failed _, _) -> true
                 | Some(Util.Succeeded, _) -> false)
              || match !theState.(i).ri.replicas with
                   Problem _ -> true
                 | Different diff -> isConflict diff.direction in
             if notok then loop (i+1) (i::acc)
             else loop (i+1) (acc) in
           let failedindices = loop 0 [] in
           let failedpaths =
             Safelist.map (fun i -> !theState.(i).ri.path1) failedindices in
           debug (fun()-> Util.msg "Rescaning with paths = %s\n"
                    (String.concat ", " (Safelist.map
                                           (fun p -> "'"^(Path.toString p)^"'")
                                           failedpaths)));
           let paths = Prefs.read Globals.paths in
           let confirmBigDeletes = Prefs.read Globals.confirmBigDeletes in
           Prefs.set Globals.paths failedpaths;
           Prefs.set Globals.confirmBigDeletes false;
           (* Modifying global paths does not play well with filesystem
              monitoring, so we disable it. *)
           unsynchronizedPaths := None;
           detectCmd();
           Prefs.set Globals.paths paths;
           Prefs.set Globals.confirmBigDeletes confirmBigDeletes;
           unsynchronizedPaths := None)
       "Re_check Unsynchronized Items");

  ignore (fileMenu#add_separator ());

  grAdd grRescan
    (fileMenu#add_image_item ~key:GdkKeysyms._p
       ~callback:profileChange
       ~image:(GMisc.image ~stock:`OPEN ~icon_size:`MENU () :> GObj.widget)
       "Change _Profile...");

  let fastProf i key =
    let item = fileMenu#add_item ~key:key ~bindname:(string_of_int i) "" in
    item#misc#hide ();
    grAdd grRescan item;
    let show name =
      match item#children with
      | [] | _::_::_ -> ()
      | [l] ->
          let label = (GMisc.label_cast l) in
          label#set_label ("Select profile " ^ name);
          ignore (item#connect#activate
            ~callback:(fun _ ->
               if System.file_exists (Prefs.profilePathname name) then begin
                 Trace.status ("Loading profile " ^ name);
                 loadProfile name false; detectCmd ()
               end else
                 Trace.status ("Profile " ^ name ^ " not found"))
            );
          item#misc#show ()
    in
    (item#misc#hide, show) in

  let fastKeysyms =
    [| GdkKeysyms._0; GdkKeysyms._1; GdkKeysyms._2; GdkKeysyms._3;
       GdkKeysyms._4; GdkKeysyms._5; GdkKeysyms._6; GdkKeysyms._7;
       GdkKeysyms._8; GdkKeysyms._9 |] in

  let fastKeyitems = Array.init 10 (fun i -> fastProf i fastKeysyms.(i)) in

  let updateProfileKeyMenu () =
    if !Uicommon.profilesAndRoots = [] then Uicommon.scanProfiles ();

    Array.iteri
      (fun i v -> match v with
      | None -> (fst fastKeyitems.(i)) ()
      | Some (profile, info) -> (snd fastKeyitems.(i)) profile)
      Uicommon.profileKeymap
  in

  ignore (fileMenu#add_separator ());
  ignore (fileMenu#add_item
            ~callback:(fun _ -> statWin#show ()) "Show _Statistics");

  ignore (fileMenu#add_separator ());
  let quit =
    fileMenu#add_image_item
      ~key:GdkKeysyms._q ~callback:safeExit
      ~image:((GMisc.image ~stock:`QUIT ~icon_size:`MENU ())#coerce)
      "_Quit"
  in
  quit#add_accelerator ~group:accel_group ~modi:[`CONTROL] GdkKeysyms._q;

  (*********************************************************************
    Expert menu
   *********************************************************************)
  let buildExpertMenu () =
    let addDebugToggle modname =
      ignore (expertMenu#add_check_item ~active:(Trace.enabled modname)
        ~callback:(fun b -> Trace.enable modname b)
        ("Debug '" ^ modname ^ "'")) in

    addDebugToggle "all";
    addDebugToggle "verbose";
    addDebugToggle "update";

    ignore (expertMenu#add_separator ());
    ignore (expertMenu#add_item
              ~callback:(fun () ->
                           Printf.fprintf stderr "\nGC stats now:\n";
                           Gc.print_stat stderr;
                           Printf.fprintf stderr "\nAfter major collection:\n";
                           Gc.full_major(); Gc.print_stat stderr;
                           flush stderr)
              "Show memory/GC stats")
  in
  buildExpertMenu ();

  let toggleExpertMenu enabled =
    expertItem#set_visible enabled
  in

  (*********************************************************************
    Finish up
   *********************************************************************)
  grDisactivateAll ();

  updateFromProfile :=
    (fun () ->
       displayNewProfileLabel ();
       setMainWindowColumnHeaders (Globals.roots ());
       sizeMainWindow ();
       toggleExpertMenu (Prefs.read Uicommon.expert);
       buildActionMenu false);

  fatalErrorHandler :=
    (fun err ->
       grDisactivateAll ();
       make_interactive toplevelWindow;
       Trace.status ("Fatal error: " ^ err);
       inExit := true;
       fatalError err;
       inExit := false;
       match !Prefs.profileName with
       | Some _ -> grSet grRescan true
       | None ->  (* Normally should never get here; exceptions loading the
                     very first profile are handled in the [start] function. *)
           begin match getProfile true with
           | None -> exit 1
           | Some p -> loadAndRunProfile p
           end
    );


  ignore (toplevelWindow#event#connect#delete ~callback:
            (fun _ -> safeExit (); true));
  toplevelWindow#show ();
  fun p ->
    updateProfileKeyMenu ();
    loadAndRunProfile p
