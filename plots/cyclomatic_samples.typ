== OCaml sample file summary

#table(
  columns: (1fr, 1fr, 1fr),
  inset: 4pt,
  align: (center, center, center),
  table.header(
    [*`.mli` files*], [*`.mli` files with total cyclomatic = 0*], [*Total OCaml files*],
  ),
  [850], [849], [2,292],
)

== Median cyclomatic-complexity sample functions

=== Golang (median: 3.00)

==== Median
- `3.00` `blockInfoForChunk:830:33` (AlistGo__alist (sample analysis: 10 samples): drivers/yunpan360/upload.go) #link("https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/drivers/yunpan360/upload.go#L830-L835")[GitHub]
- `3.00` `TestRegisteredProvidersIgnoresStaleExclusiveProvider:64:6` (router-for-me__CLIProxyAPI (sample analysis: 10 samples): sdk/access/registry_test.go) #link("https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/sdk/access/registry_test.go#L64-L81")[GitHub]
- `3.00` `TestUpdateName:2425:6` (spf13__cobra (sample analysis: 10 samples): command_test.go) #link("https://github.com/spf13/cobra/blob/ad460ea8f249db69c943a365fb84f3a59042d54e/command_test.go#L2425-L2433")[GitHub]
- `3.00` `AsString:44:19` (junegunn__fzf (sample analysis: 10 samples): src/item.go) #link("https://github.com/junegunn/fzf/blob/dea72834ed35b5d56634075fc7793aa4b9b1d697/src/item.go#L44-L53")[GitHub]
- `3.00` `setupColors:68:17` (charmbracelet__bubbletea (sample analysis: 10 samples): examples/space/main.go) #link("https://github.com/charmbracelet/bubbletea/blob/a23da80847e6fc928febc62114f761403ac5d2f1/examples/space/main.go#L68-L86")[GitHub]
- `3.00` `GetSortOrderStrategy:20:6` (wagoodman__dive (sample analysis: 10 samples): dive/filetree/order_strategy.go) #link("https://github.com/wagoodman/dive/blob/d6c691947f8fda635c952a17ee3b7555379d58f0/dive/filetree/order_strategy.go#L20-L28")[GitHub]
- `3.00` `ToCache:9:19` (nektos__act (sample analysis: 10 samples): pkg/artifactcache/model.go) #link("https://github.com/nektos/act/blob/4f411281417e88660bea1c1a1749aa71ae0bd60f/pkg/artifactcache/model.go#L9-L24")[GitHub]
- `3.00` `QueryContext:181:27` (go-gorm__gorm (sample analysis: 10 samples): prepare_stmt.go) #link("https://github.com/go-gorm/gorm/blob/d0ee5e2296150d691364c5f4f7f2b32abde04545/prepare_stmt.go#L181-L190")[GitHub]
- `3.00` `createStaticHandler:216:27` (gin-gonic__gin (sample analysis: 10 samples): routergroup.go) #link("https://github.com/gin-gonic/gin/blob/d75fcd4c9ab260e5225de590f1f0f8c0e0e12d11/routergroup.go#L216-L239")[GitHub]
- `3.00` `openWindowsRootStore:71:6` (FiloSottile__mkcert (sample analysis: 10 samples): truststore_windows.go) #link("https://github.com/FiloSottile/mkcert/blob/d7ab78de71ad2e4d965446a6083ebb48150f0533/truststore_windows.go#L71-L81")[GitHub]

==== CC 5
- `5.00` `Link:46:20` (AlistGo__alist (sample analysis: 10 samples): drivers/189/driver.go) #link("https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/drivers/189/driver.go#L46-L80")[GitHub]
- `5.00` `TestSchedulerPick_FillFirstSticksToFirstReady:127:6` (router-for-me__CLIProxyAPI (sample analysis: 10 samples): sdk/cliproxy/auth/scheduler_test.go) #link("https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/sdk/cliproxy/auth/scheduler_test.go#L127-L149")[GitHub]
- `5.00` `findSuggestions:781:19` (spf13__cobra (sample analysis: 10 samples): command.go) #link("https://github.com/spf13/cobra/blob/ad460ea8f249db69c943a365fb84f3a59042d54e/command.go#L781-L796")[GitHub]
- `5.00` `dumpStatus:8328:20` (junegunn__fzf (sample analysis: 10 samples): src/terminal.go) #link("https://github.com/junegunn/fzf/blob/dea72834ed35b5d56634075fc7793aa4b9b1d697/src/terminal.go#L8328-L8366")[GitHub]
- `5.00` `Update:85:16` (charmbracelet__bubbletea (sample analysis: 10 samples): examples/tui-daemon-combo/main.go) #link("https://github.com/charmbracelet/bubbletea/blob/a23da80847e6fc928febc62114f761403ac5d2f1/examples/tui-daemon-combo/main.go#L85-L103")[GitHub]


=== Ocaml (median: 1.00)

==== Median
- `1.00` `rewrite_substitute:42:7` (comby-tools__comby (sample analysis: 10 samples): lib/kernel/matchers/evaluate.ml) #link("https://github.com/comby-tools/comby/blob/3b6bdff7bc3b50361b621da9db030152772e7e6b/lib/kernel/matchers/evaluate.ml#L42-L43")[GitHub]
- `1.00` `change_and_rebuild:116:19` (fastpack__fastpack (sample analysis: 10 samples): FastpackTest/Watch.ml) #link("https://github.com/fastpack/fastpack/blob/173e0a412474baefd1f0f22597274730a74f475d/FastpackTest/Watch.ml#L116-L121")[GitHub]
- `1.00` `to_dream_method:19:5` (camlworks__dream (sample analysis: 10 samples): src/mirage/mirage.ml) #link("https://github.com/camlworks/dream/blob/2ce65e1010f2501f9319e8735eec8e1eeb676d4e/src/mirage/mirage.ml#L19")[GitHub]
- `1.00` `print_indent:3:5` (batsh-dev-team__Batsh (sample analysis: 10 samples): src/formatutil.ml) #link("https://github.com/batsh-dev-team/Batsh/blob/e38abb082deb38fc2942248a4fd855a79d2a35ca/src/formatutil.ml#L3-L4")[GitHub]
- `1.00` `scale:176:18` (janestreet__core (sample analysis: 10 samples): core/src/span_ns.ml) #link("https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/core/src/span_ns.ml#L176")[GitHub]
- `1.00` `reply:2154:7` (bcpierce00__unison (sample analysis: 10 samples): src/remote.ml) #link("https://github.com/bcpierce00/unison/blob/273ec6298cd7f97f038ea8667701691c29b1b48f/src/remote.ml#L2154")[GitHub]
- `1.00` `cmpdi:19:5` (BinaryAnalysisPlatform__bap (sample analysis: 10 samples): plugins/powerpc/powerpc_compare.ml) #link("https://github.com/BinaryAnalysisPlatform/bap/blob/034bbd09646a589d5eb1f10b740177d628da80fd/plugins/powerpc/powerpc_compare.ml#L19-L28")[GitHub]
- `1.00` `init:141:7` (airbus-seclab__bincat (sample analysis: 10 samples): ocaml/src/disassembly/x86Imports.ml) #link("https://github.com/airbus-seclab/bincat/blob/5d0ee3b56867059427eb0f4123c4d9de0b8059dd/ocaml/src/disassembly/x86Imports.ml#L141-L143")[GitHub]
- `1.00` `gen_method_decl:657:5` (austral__austral (sample analysis: 10 samples): lib/CodeGen.ml) #link("https://github.com/austral/austral/blob/0962d2a8a5d77f7daacd7f696819520733f4897d/lib/CodeGen.ml#L657-L659")[GitHub]
- `1.00` `prepare_update_state_stmt:151:7` (astrada__google-drive-ocamlfuse (sample analysis: 10 samples): src/dbCache.ml) #link("https://github.com/astrada/google-drive-ocamlfuse/blob/6076cd802709191528a227a9e5e0823f2c708ec8/src/dbCache.ml#L151-L153")[GitHub]

==== CC 5
- `5.00` `add_entry:618:5` (comby-tools__comby (sample analysis: 10 samples): lib/app/vendored/camlzip/zip.ml) #link("https://github.com/comby-tools/comby/blob/3b6bdff7bc3b50361b621da9db030152772e7e6b/lib/app/vendored/camlzip/zip.ml#L618-L645")[GitHub]
- `5.00` `show:9:5` (fastpack__fastpack (sample analysis: 10 samples): FastpackTest/Resolver.ml) #link("https://github.com/fastpack/fastpack/blob/173e0a412474baefd1f0f22597274730a74f475d/FastpackTest/Resolver.ml#L9-L34")[GitHub]
- `5.00` `forward:315:7` (camlworks__dream (sample analysis: 10 samples): src/server/log.ml) #link("https://github.com/camlworks/dream/blob/2ce65e1010f2501f9319e8735eec8e1eeb676d4e/src/server/log.ml#L315-L329")[GitHub]
- `5.00` `check_toplevel:17:5` (batsh-dev-team__Batsh (sample analysis: 10 samples): src/semantic_checker.ml) #link("https://github.com/batsh-dev-team/Batsh/blob/e38abb082deb38fc2942248a4fd855a79d2a35ca/src/semantic_checker.ml#L17-L26")[GitHub]
- `5.00` `complete:272:9` (janestreet__core (sample analysis: 10 samples): command/src/command.ml) #link("https://github.com/janestreet/core/blob/5c2e82c0c0258262b20850aaba4de71d4df91e42/command/src/command.ml#L272-L284")[GitHub]
