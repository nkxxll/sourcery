// https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/internal/op/setting.go#L184-L195
// AlistGo/alist internal/op/setting.go:184-195
func SaveSettingItem(item *model.SettingItem) (err error) {
	// hook
	if _, err := HandleSettingItemHook(item); err != nil {
		return err
	}
	// update
	if err = db.SaveSettingItem(item); err != nil {
		return err
	}
	SettingCacheUpdate()
	return nil
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/examples/plugin/frontend-auth-exclusive/go/main.go#L122-L133
// router-for-me/CLIProxyAPI examples/plugin/frontend-auth-exclusive/go/main.go:122-133
func handleMethod(method string, request []byte) ([]byte, error) {
	switch method {
	case pluginabi.MethodPluginRegister, pluginabi.MethodPluginReconfigure:
		return okEnvelope(exampleRegistration())
	case pluginabi.MethodFrontendAuthIdentifier:
		return okEnvelope(identifierResponse{Identifier: "example-frontend-auth-exclusive-go"})
	case pluginabi.MethodFrontendAuthAuthenticate:
		return authenticate(request)
	default:
		return errorEnvelope("unknown_method", "unknown method: "+method), nil
	}
}

// https://github.com/spf13/cobra/blob/ad460ea8f249db69c943a365fb84f3a59042d54e/bash_completions.go#L683-L694
// spf13/cobra bash_completions.go:683-694
func (c *Command) GenBashCompletion(w io.Writer) error {
	buf := new(bytes.Buffer)
	writePreamble(buf, c.Name())
	if len(c.BashCompletionFunction) > 0 {
		buf.WriteString(c.BashCompletionFunction + "\n")
	}
	gen(buf, c)
	writePostscript(buf, c.Name())

	_, err := buf.WriteTo(w)
	return err
}

// https://github.com/junegunn/fzf/blob/dea72834ed35b5d56634075fc7793aa4b9b1d697/src/chunklist.go#L104-L115
// junegunn/fzf src/chunklist.go:104-115
func (cl *ChunkList) ForEachItem(fn func(*Item), done func()) {
	cl.mutex.Lock()
	for _, chunk := range cl.chunks {
		for i := 0; i < chunk.count; i++ {
			fn(&chunk.items[i])
		}
	}
	if done != nil {
		done()
	}
	cl.mutex.Unlock()
}

// https://github.com/charmbracelet/bubbletea/blob/a23da80847e6fc928febc62114f761403ac5d2f1/cursed_renderer.go#L766-L777
// charmbracelet/bubbletea cursed_renderer.go:766-777
func (s *cursedRenderer) onMouse(m MouseMsg) Cmd {
	var onMouse func(MouseMsg) Cmd
	s.mu.Lock()
	if s.lastView != nil {
		onMouse = s.lastView.OnMouse
	}
	s.mu.Unlock()
	if onMouse != nil {
		return onMouse(m)
	}
	return nil
}

// https://github.com/wagoodman/dive/blob/d6c691947f8fda635c952a17ee3b7555379d58f0/cmd/dive/cli/internal/ui/v1/view/status.go#L30-L41
// wagoodman/dive cmd/dive/cli/internal/ui/v1/view/status.go:30-41
func newStatusView(gui *gocui.Gui) *Status {
	c := new(Status)

	// populate main fields
	c.name = "status"
	c.gui = gui
	c.helpKeys = make([]*key.Binding, 0)
	c.requestedHeight = 1
	c.logger = log.Nested("ui", "status")

	return c
}

// https://github.com/nektos/act/blob/4f411281417e88660bea1c1a1749aa71ae0bd60f/pkg/model/workflow.go#L279-L290
// nektos/act pkg/model/workflow.go:279-290
func (j *Job) Secrets() map[string]string {
	if j.RawSecrets.Kind != yaml.MappingNode {
		return nil
	}

	var val map[string]string
	if !decodeNode(j.RawSecrets, &val) {
		return nil
	}

	return val
}

// https://github.com/go-gorm/gorm/blob/d0ee5e2296150d691364c5f4f7f2b32abde04545/statement.go#L516-L527
// go-gorm/gorm statement.go:516-527
func (stmt *Statement) ParseWithSpecialTableName(value interface{}, specialTableName string) (err error) {
	if stmt.Schema, err = schema.ParseWithSpecialTableName(value, stmt.DB.cacheStore, stmt.DB.NamingStrategy, specialTableName); err == nil && stmt.Table == "" {
		if tables := strings.Split(stmt.Schema.Table, "."); len(tables) == 2 {
			stmt.TableExpr = &clause.Expr{SQL: stmt.Quote(stmt.Schema.Table)}
			stmt.Table = tables[1]
			return
		}

		stmt.Table = stmt.Schema.Table
	}
	return err
}

// https://github.com/gin-gonic/gin/blob/d75fcd4c9ab260e5225de590f1f0f8c0e0e12d11/gin.go#L630-L641
// gin-gonic/gin gin.go:630-641
func (engine *Engine) RunQUIC(addr, certFile, keyFile string) (err error) {
	debugPrint("Listening and serving QUIC on %s\n", addr)
	defer func() { debugPrintError(err) }()

	if engine.isUnsafeTrustedProxies() {
		debugPrint("[WARNING] You trusted all proxies, this is NOT safe. We recommend you to set a value.\n" +
			"Please check https://github.com/gin-gonic/gin/blob/master/docs/doc.md#dont-trust-all-proxies for details.")
	}

	err = http3.ListenAndServeQUIC(addr, certFile, keyFile, engine.Handler())
	return
}

// https://github.com/FiloSottile/mkcert/blob/d7ab78de71ad2e4d965446a6083ebb48150f0533/main.go#L345-L356
// FiloSottile/mkcert main.go:345-356
func storeEnabled(name string) bool {
	stores := os.Getenv("TRUST_STORES")
	if stores == "" {
		return true
	}
	for _, store := range strings.Split(stores, ",") {
		if store == name {
			return true
		}
	}
	return false
}

// https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/drivers/baidu_photo/utils.go#L244-L255
// AlistGo/alist drivers/baidu_photo/utils.go:244-255
func (d *BaiduPhoto) DeleteAlbumFile(ctx context.Context, file *AlbumFile) error {
	_, err := d.Post(ALBUM_API_URL+"/delfile", func(r *resty.Request) {
		r.SetContext(ctx)
		r.SetFormData(map[string]string{
			"album_id":   fmt.Sprint(file.AlbumID),
			"tid":        fmt.Sprint(file.Tid),
			"list":       fmt.Sprintf(`[{"fsid":%d,"uk":%d}]`, file.Fsid, file.Uk),
			"del_origin": BoolToIntStr(d.DeleteOrigin), // 是否删除原图 0 不删除 1 删除
		})
	}, nil)
	return err
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/internal/auth/gemini/gemini_token.go#L93-L104
// router-for-me/CLIProxyAPI internal/auth/gemini/gemini_token.go:93-104
func CredentialFileName(email, projectID string, includeProviderPrefix bool) string {
	email = strings.TrimSpace(email)
	project := strings.TrimSpace(projectID)
	if strings.EqualFold(project, "all") || strings.Contains(project, ",") {
		return fmt.Sprintf("gemini-%s-all.json", email)
	}
	prefix := ""
	if includeProviderPrefix {
		prefix = "gemini-"
	}
	return fmt.Sprintf("%s%s-%s.json", prefix, email, project)
}

// https://github.com/spf13/cobra/blob/ad460ea8f249db69c943a365fb84f3a59042d54e/command.go#L547-L558
// spf13/cobra command.go:547-558
func (c *Command) FlagErrorFunc() (f func(*Command, error) error) {
	if c.flagErrorFunc != nil {
		return c.flagErrorFunc
	}

	if c.HasParent() {
		return c.parent.FlagErrorFunc()
	}
	return func(c *Command, err error) error {
		return err
	}
}

// https://github.com/junegunn/fzf/blob/dea72834ed35b5d56634075fc7793aa4b9b1d697/src/terminal.go#L8274-L8285
// junegunn/fzf src/terminal.go:8274-8285
func (t *Terminal) promptLines() int {
	if t.inputless {
		return 0
	}
	if t.inputWindow != nil {
		return 0
	}
	if t.noSeparatorLine() {
		return 1
	}
	return 2
}

// https://github.com/charmbracelet/bubbletea/blob/a23da80847e6fc928febc62114f761403ac5d2f1/examples/textarea/main.go#L30-L41
// charmbracelet/bubbletea examples/textarea/main.go:30-41
func initialModel() model {
	ti := textarea.New()
	ti.Placeholder = "Once upon a time..."
	ti.SetVirtualCursor(false)
	ti.SetStyles(textarea.DefaultStyles(true)) // default to dark styles.
	ti.Focus()

	return model{
		textarea: ti,
		err:      nil,
	}
}

// https://github.com/wagoodman/dive/blob/d6c691947f8fda635c952a17ee3b7555379d58f0/dive/filetree/file_node.go#L125-L136
// wagoodman/dive dive/filetree/file_node.go:125-136
func (node *FileNode) String() string {
	var display string
	if node == nil {
		return ""
	}

	display = node.Name
	if node.Data.FileInfo.TypeFlag == tar.TypeSymlink || node.Data.FileInfo.TypeFlag == tar.TypeLink {
		display += " → " + node.Data.FileInfo.Linkname
	}
	return diffTypeColor[node.Data.DiffType].Sprint(display)
}

// https://github.com/nektos/act/blob/4f411281417e88660bea1c1a1749aa71ae0bd60f/pkg/runner/step_action_remote.go#L185-L196
// nektos/act pkg/runner/step_action_remote.go:185-196
func (sar *stepActionRemote) getGithubContext(ctx context.Context) *model.GithubContext {
	ghc := sar.getRunContext().getGithubContext(ctx)

	// extend github context if we already have an initialized remoteAction
	remoteAction := sar.remoteAction
	if remoteAction != nil {
		ghc.ActionRepository = fmt.Sprintf("%s/%s", remoteAction.Org, remoteAction.Repo)
		ghc.ActionRef = remoteAction.Ref
	}

	return ghc
}

// https://github.com/go-gorm/gorm/blob/d0ee5e2296150d691364c5f4f7f2b32abde04545/tests/joins_test.go#L467-L478
// go-gorm/gorm tests/joins_test.go:467-478
func TestJoinsPreload_Issue7013_NoEntries(t *testing.T) {
	var entries []User
	assert.NotPanics(t, func() {
		assert.NoError(t,
			DB.Preload("Manager.Team").
				Joins("Manager.Company").
				Where("1 <> 1").
				Find(&entries).Error)
	})

	AssertEqual(t, len(entries), 0)
}

// https://github.com/gin-gonic/gin/blob/d75fcd4c9ab260e5225de590f1f0f8c0e0e12d11/context_test.go#L1286-L1297
// gin-gonic/gin context_test.go:1286-1297
func TestContextRenderNoContentHTML(t *testing.T) {
	w := httptest.NewRecorder()
	c, router := CreateTestContext(w)
	templ := template.Must(template.New("t").Parse(`Hello {{.name}}`))
	router.SetHTMLTemplate(templ)

	c.HTML(http.StatusNoContent, "t", H{"name": "alexandernyquist"})

	assert.Equal(t, http.StatusNoContent, w.Code)
	assert.Empty(t, w.Body.String())
	assert.Equal(t, "text/html; charset=utf-8", w.Header().Get("Content-Type"))
}

// https://github.com/FiloSottile/mkcert/blob/d7ab78de71ad2e4d965446a6083ebb48150f0533/cert.go#L37-L48
// FiloSottile/mkcert cert.go:37-48
func init() {
	u, err := user.Current()
	if err == nil {
		userAndHostname = u.Username + "@"
	}
	if h, err := os.Hostname(); err == nil {
		userAndHostname += h
	}
	if err == nil && u.Name != "" && u.Name != u.Username {
		userAndHostname += " (" + u.Name + ")"
	}
}
