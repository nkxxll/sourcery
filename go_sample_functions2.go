// https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/drivers/yunpan360/upload.go#L830-L835
// AlistGo/alist drivers/yunpan360/upload.go:830-835
func (r *openUploadRequestResp) blockInfoForChunk(index int) blockInfoMap {
	if index <= 0 || index > len(r.Data.BlockInfo) {
		return blockInfoMap{}
	}
	return blockInfoMap(r.Data.BlockInfo[index-1])
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/sdk/access/registry_test.go#L64-L81
// router-for-me/CLIProxyAPI sdk/access/registry_test.go:64-81
func TestRegisteredProvidersIgnoresStaleExclusiveProvider(t *testing.T) {
	UnregisterProvider("test-a")
	UnregisterProvider("missing")
	ClearExclusiveProvider()
	defer UnregisterProvider("test-a")
	defer ClearExclusiveProvider()

	RegisterProvider("test-a", testProvider{id: "test-a"})
	SetExclusiveProvider("missing")

	providers := RegisteredProviders()
	if len(providers) != 1 {
		t.Fatalf("RegisteredProviders() len = %d, want 1", len(providers))
	}
	if providers[0].Identifier() != "test-a" {
		t.Fatalf("RegisteredProviders()[0] = %q, want test-a", providers[0].Identifier())
	}
}

// https://github.com/spf13/cobra/blob/ad460ea8f249db69c943a365fb84f3a59042d54e/command_test.go#L2425-L2433
// spf13/cobra command_test.go:2425-2433
func TestUpdateName(t *testing.T) {
	c := &Command{Use: "name xyz"}
	originalName := c.Name()

	c.Use = "changedName abc"
	if originalName == c.Name() || c.Name() != "changedName" {
		t.Error("c.Name() should be updated on changed c.Use")
	}
}

// https://github.com/junegunn/fzf/blob/dea72834ed35b5d56634075fc7793aa4b9b1d697/src/item.go#L44-L53
// junegunn/fzf src/item.go:44-53
func (item *Item) AsString(stripAnsi bool) string {
	if item.origText != nil {
		if stripAnsi {
			trimmed, _, _ := extractColor(string(*item.origText), nil, nil)
			return trimmed
		}
		return string(*item.origText)
	}
	return item.text.ToString()
}

// https://github.com/charmbracelet/bubbletea/blob/a23da80847e6fc928febc62114f761403ac5d2f1/examples/space/main.go#L68-L86
// charmbracelet/bubbletea examples/space/main.go:68-86
func (m *model) setupColors() {
	height := m.height * 2 // double height for half blocks
	m.colors = make([][]color.Color, height)

	for y := range height {
		m.colors[y] = make([]color.Color, m.width)
		randomnessFactor := float64(height-y) / float64(height)

		for x := range m.width {
			baseValue := randomnessFactor * (float64(height-y) / float64(height))
			randomOffset := (rand.Float64() * 0.2) - 0.1
			value := clamp(baseValue+randomOffset, 0, 1)

			// Convert value to grayscale color (0-255)
			gray := uint8(value * 255)
			m.colors[y][x] = lipgloss.Color(fmt.Sprintf("#%02x%02x%02x", gray, gray, gray))
		}
	}
}

// https://github.com/wagoodman/dive/blob/d6c691947f8fda635c952a17ee3b7555379d58f0/dive/filetree/order_strategy.go#L20-L28
// wagoodman/dive dive/filetree/order_strategy.go:20-28
func GetSortOrderStrategy(sortOrder SortOrder) OrderStrategy {
	switch sortOrder {
	case ByName:
		return orderByNameStrategy{}
	case BySizeDesc:
		return orderBySizeDescStrategy{}
	}
	return orderByNameStrategy{}
}

// https://github.com/nektos/act/blob/4f411281417e88660bea1c1a1749aa71ae0bd60f/pkg/artifactcache/model.go#L9-L24
// nektos/act pkg/artifactcache/model.go:9-24
func (c *Request) ToCache() *Cache {
	if c == nil {
		return nil
	}
	ret := &Cache{
		Key:     c.Key,
		Version: c.Version,
		Size:    c.Size,
	}
	if c.Size == 0 {
		// So the request comes from old versions of actions, like `actions/cache@v2`.
		// It doesn't send cache size. Set it to -1 to indicate that.
		ret.Size = -1
	}
	return ret
}

// https://github.com/go-gorm/gorm/blob/d0ee5e2296150d691364c5f4f7f2b32abde04545/prepare_stmt.go#L181-L190
// go-gorm/gorm prepare_stmt.go:181-190
func (tx *PreparedStmtTX) QueryContext(ctx context.Context, query string, args ...interface{}) (rows *sql.Rows, err error) {
	stmt, err := tx.PreparedStmtDB.prepare(ctx, tx.Tx, true, query)
	if err == nil {
		rows, err = tx.Tx.StmtContext(ctx, stmt.Stmt).QueryContext(ctx, args...)
		if errors.Is(err, driver.ErrBadConn) {
			tx.PreparedStmtDB.Stmts.Delete(query)
		}
	}
	return rows, err
}

// https://github.com/gin-gonic/gin/blob/d75fcd4c9ab260e5225de590f1f0f8c0e0e12d11/routergroup.go#L216-L239
// gin-gonic/gin routergroup.go:216-239
func (group *RouterGroup) createStaticHandler(relativePath string, fs http.FileSystem) HandlerFunc {
	absolutePath := group.calculateAbsolutePath(relativePath)
	fileServer := http.StripPrefix(absolutePath, http.FileServer(fs))

	return func(c *Context) {
		if _, noListing := fs.(*OnlyFilesFS); noListing {
			c.Writer.WriteHeader(http.StatusNotFound)
		}

		file := c.Param("filepath")
		// Check if file exists and/or if we have permission to access it
		f, err := fs.Open(file)
		if err != nil {
			c.Writer.WriteHeader(http.StatusNotFound)
			c.handlers = group.engine.noRoute
			// Reset index
			c.index = -1
			return
		}
		f.Close()

		fileServer.ServeHTTP(c.Writer, c.Request)
	}
}

// https://github.com/FiloSottile/mkcert/blob/d7ab78de71ad2e4d965446a6083ebb48150f0533/truststore_windows.go#L71-L81
// FiloSottile/mkcert truststore_windows.go:71-81
func openWindowsRootStore() (windowsRootStore, error) {
	rootStr, err := syscall.UTF16PtrFromString("ROOT")
	if err != nil {
		return 0, err
	}
	store, _, err := procCertOpenSystemStoreW.Call(0, uintptr(unsafe.Pointer(rootStr)))
	if store != 0 {
		return windowsRootStore(store), nil
	}
	return 0, fmt.Errorf("failed to open windows root store: %v", err)
}

// https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/drivers/onedrive_app/util.go#L54-L63
// AlistGo/alist drivers/onedrive_app/util.go:54-63
func (d *OnedriveAPP) accessToken() error {
	var err error
	for i := 0; i < 3; i++ {
		err = d._accessToken()
		if err == nil {
			break
		}
	}
	return err
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/internal/cache/signature_cache_test.go#L121-L144
// router-for-me/CLIProxyAPI internal/cache/signature_cache_test.go:121-144
func TestHasValidSignature(t *testing.T) {
	tests := []struct {
		name      string
		modelName string
		signature string
		expected  bool
	}{
		{"valid long signature", testModelName, "abc123validSignature1234567890123456789012345678901234567890", true},
		{"exactly 50 chars", testModelName, "12345678901234567890123456789012345678901234567890", true},
		{"49 chars - invalid", testModelName, "1234567890123456789012345678901234567890123456789", false},
		{"empty string", testModelName, "", false},
		{"short signature", testModelName, "abc", false},
		{"gemini sentinel", "gemini-3-pro-preview", "skip_thought_signature_validator", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := HasValidSignature(tt.modelName, tt.signature)
			if result != tt.expected {
				t.Errorf("HasValidSignature(%q) = %v, expected %v", tt.signature, result, tt.expected)
			}
		})
	}
}

// https://github.com/spf13/cobra/blob/ad460ea8f249db69c943a365fb84f3a59042d54e/doc/rest_docs.go#L30-L49
// spf13/cobra doc/rest_docs.go:30-49
func printOptionsReST(buf *bytes.Buffer, cmd *cobra.Command, name string) error {
	flags := cmd.NonInheritedFlags()
	flags.SetOutput(buf)
	if flags.HasAvailableFlags() {
		buf.WriteString("Options\n")
		buf.WriteString("~~~~~~~\n\n::\n\n")
		flags.PrintDefaults()
		buf.WriteString("\n")
	}

	parentFlags := cmd.InheritedFlags()
	parentFlags.SetOutput(buf)
	if parentFlags.HasAvailableFlags() {
		buf.WriteString("Options inherited from parent commands\n")
		buf.WriteString("~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~\n\n::\n\n")
		parentFlags.PrintDefaults()
		buf.WriteString("\n")
	}
	return nil
}

// https://github.com/junegunn/fzf/blob/dea72834ed35b5d56634075fc7793aa4b9b1d697/src/util/util_test.go#L105-L112
// junegunn/fzf src/util/util_test.go:105-112
func TestRepeatToFill(t *testing.T) {
	if RepeatToFill("abcde", 10, 50) != strings.Repeat("abcde", 5) {
		t.Error("Expected:", strings.Repeat("abcde", 5))
	}
	if RepeatToFill("abcde", 10, 42) != strings.Repeat("abcde", 4)+"abcde"[:2] {
		t.Error("Expected:", strings.Repeat("abcde", 4)+"abcde"[:2])
	}
}

// https://github.com/charmbracelet/bubbletea/blob/a23da80847e6fc928febc62114f761403ac5d2f1/exec_test.go#L121-L137
// charmbracelet/bubbletea exec_test.go:121-137
func TestTeaExecWithNilInput(t *testing.T) {
	t.Parallel()
	var buf bytes.Buffer

	m := &testExecNoInputModel{}
	p := NewProgram(m,
		WithInput(nil),
		WithOutput(&buf),
	)

	if _, err := p.Run(); err != nil {
		t.Fatal(err)
	}
	if m.err != nil {
		t.Fatalf("expected no error, got %v", m.err)
	}
}

// https://github.com/wagoodman/dive/blob/d6c691947f8fda635c952a17ee3b7555379d58f0/dive/filetree/file_tree_test.go#L9-L16
// wagoodman/dive dive/filetree/file_tree_test.go:9-16
func stringInSlice(a string, list []string) bool {
	for _, b := range list {
		if b == a {
			return true
		}
	}
	return false
}

// https://github.com/nektos/act/blob/4f411281417e88660bea1c1a1749aa71ae0bd60f/pkg/container/docker_cli.go#L974-L986
// nektos/act pkg/container/docker_cli.go:974-986
func parseSystemPaths(securityOpts []string) (filtered, maskedPaths, readonlyPaths []string) {
	filtered = securityOpts[:0]
	for _, opt := range securityOpts {
		if opt == "systempaths=unconfined" {
			maskedPaths = []string{}
			readonlyPaths = []string{}
		} else {
			filtered = append(filtered, opt)
		}
	}

	return filtered, maskedPaths, readonlyPaths
}

// https://github.com/go-gorm/gorm/blob/d0ee5e2296150d691364c5f4f7f2b32abde04545/tests/upsert_test.go#L360-L375
// go-gorm/gorm tests/upsert_test.go:360-375
func TestUpdateWithMissWhere(t *testing.T) {
	type User struct {
		ID   uint   `gorm:"column:id;<-:create"`
		Name string `gorm:"column:name"`
	}
	user := User{ID: 1, Name: "king"}
	tx := DB.Session(&gorm.Session{DryRun: true}).Save(&user)

	if err := tx.Error; err != nil {
		t.Fatalf("failed to update user,missing where condition,err=%+v", err)
	}

	if !regexp.MustCompile("WHERE .id. = [^ ]+$").MatchString(tx.Statement.SQL.String()) {
		t.Fatalf("invalid updating SQL, got %v", tx.Statement.SQL.String())
	}
}

// https://github.com/gin-gonic/gin/blob/d75fcd4c9ab260e5225de590f1f0f8c0e0e12d11/routes_test.go#L731-L772
// gin-gonic/gin routes_test.go:731-772
func TestRouteContextHoldsFullPath(t *testing.T) {
	router := New()

	// Test routes
	routes := []string{
		"/simple",
		"/project/:name",
		"/",
		"/news/home",
		"/news",
		"/simple-two/one",
		"/simple-two/one-two",
		"/project/:name/build/*params",
		"/project/:name/bui",
		"/user/:id/status",
		"/user/:id",
		"/user/:id/profile",
	}

	for _, route := range routes {
		actualRoute := route
		router.GET(route, func(c *Context) {
			// For each defined route context should contain its full path
			assert.Equal(t, actualRoute, c.FullPath())
			c.AbortWithStatus(http.StatusOK)
		})
	}

	for _, route := range routes {
		w := PerformRequest(router, http.MethodGet, route)
		assert.Equal(t, http.StatusOK, w.Code)
	}

	// Test not found
	router.Use(func(c *Context) {
		// For not found routes full path is empty
		assert.Empty(t, c.FullPath())
	})

	w := PerformRequest(router, http.MethodGet, "/not-found")
	assert.Equal(t, http.StatusNotFound, w.Code)
}

// https://github.com/FiloSottile/mkcert/blob/d7ab78de71ad2e4d965446a6083ebb48150f0533/truststore_nss.go#L89-L104
// FiloSottile/mkcert truststore_nss.go:89-104
func (m *mkcert) installNSS() bool {
	if m.forEachNSSProfile(func(profile string) {
		cmd := exec.Command(certutilPath, "-A", "-d", profile, "-t", "C,,", "-n", m.caUniqueName(), "-i", filepath.Join(m.CAROOT, rootName))
		out, err := execCertutil(cmd)
		fatalIfCmdErr(err, "certutil -A -d "+profile, out)
	}) == 0 {
		log.Printf("ERROR: no %s security databases found", NSSBrowsers)
		return false
	}
	if !m.checkNSS() {
		log.Printf("Installing in %s failed. Please report the issue with details about your environment at https://github.com/FiloSottile/mkcert/issues/new 👎", NSSBrowsers)
		log.Printf("Note that if you never started %s, you need to do that at least once.", NSSBrowsers)
		return false
	}
	return true
}
