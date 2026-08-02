// https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/drivers/189/driver.go#L46-L80
// AlistGo/alist drivers/189/driver.go:46-80
func (d *Cloud189) Link(ctx context.Context, file model.Obj, args model.LinkArgs) (*model.Link, error) {
	var resp DownResp
	u := "https://cloud.189.cn/api/portal/getFileInfo.action"
	_, err := d.request(u, http.MethodGet, func(req *resty.Request) {
		req.SetQueryParam("fileId", file.GetID())
	}, &resp)
	if err != nil {
		return nil, err
	}
	client := resty.NewWithClient(d.client.GetClient()).SetRedirectPolicy(
		resty.RedirectPolicyFunc(func(req *http.Request, via []*http.Request) error {
			return http.ErrUseLastResponse
		}))
	res, err := client.R().SetHeader("User-Agent", base.UserAgent).Get("https:" + resp.FileDownloadUrl)
	if err != nil {
		return nil, err
	}
	log.Debugln(res.Status())
	log.Debugln(res.String())
	link := model.Link{}
	log.Debugln("first url:", resp.FileDownloadUrl)
	if res.StatusCode() == 302 {
		link.URL = res.Header().Get("location")
		log.Debugln("second url:", link.URL)
		_, _ = client.R().Get(link.URL)
		if res.StatusCode() == 302 {
			link.URL = res.Header().Get("location")
		}
		log.Debugln("third url:", link.URL)
	} else {
		link.URL = resp.FileDownloadUrl
	}
	link.URL = strings.Replace(link.URL, "http://", "https://", 1)
	return &link, nil
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/sdk/cliproxy/auth/scheduler_test.go#L127-L149
// router-for-me/CLIProxyAPI sdk/cliproxy/auth/scheduler_test.go:127-149
func TestSchedulerPick_FillFirstSticksToFirstReady(t *testing.T) {
	t.Parallel()

	scheduler := newSchedulerForTest(
		&FillFirstSelector{},
		&Auth{ID: "b", Provider: "gemini"},
		&Auth{ID: "a", Provider: "gemini"},
		&Auth{ID: "c", Provider: "gemini"},
	)

	for index := 0; index < 3; index++ {
		got, errPick := scheduler.pickSingle(context.Background(), "gemini", "", cliproxyexecutor.Options{}, nil)
		if errPick != nil {
			t.Fatalf("pickSingle() #%d error = %v", index, errPick)
		}
		if got == nil {
			t.Fatalf("pickSingle() #%d auth = nil", index)
		}
		if got.ID != "a" {
			t.Fatalf("pickSingle() #%d auth.ID = %q, want %q", index, got.ID, "a")
		}
	}
}

// https://github.com/spf13/cobra/blob/ad460ea8f249db69c943a365fb84f3a59042d54e/command.go#L781-L796
// spf13/cobra command.go:781-796
func (c *Command) findSuggestions(arg string) string {
	if c.DisableSuggestions {
		return ""
	}
	if c.SuggestionsMinimumDistance <= 0 {
		c.SuggestionsMinimumDistance = 2
	}
	var sb strings.Builder
	if suggestions := c.SuggestionsFor(arg); len(suggestions) > 0 {
		sb.WriteString("\n\nDid you mean this?\n")
		for _, s := range suggestions {
			_, _ = fmt.Fprintf(&sb, "\t%v\n", s)
		}
	}
	return sb.String()
}

// https://github.com/junegunn/fzf/blob/dea72834ed35b5d56634075fc7793aa4b9b1d697/src/terminal.go#L8328-L8366
// junegunn/fzf src/terminal.go:8328-8366
func (t *Terminal) dumpStatus(params getParams) string {
	if !t.tryLock(channelTimeout) {
		return ""
	}
	defer t.mutex.Unlock()

	selectedItems := t.sortSelected()
	selected := make([]StatusItem, max(0, min(params.limit, len(selectedItems)-params.offset)))
	for i := range selected {
		selected[i] = t.dumpItem(selectedItems[i+params.offset].item)
	}

	matches := make([]StatusItem, max(0, min(params.limit, t.resultMerger.Length()-params.offset)))
	for i := range matches {
		matches[i] = t.dumpItem(t.resultMerger.Get(i + params.offset).item)
	}

	var current *StatusItem
	currentItem := t.currentItem()
	if currentItem != nil {
		item := t.dumpItem(currentItem)
		current = &item
	}

	dump := Status{
		Reading:    t.reading,
		Progress:   t.progress,
		Query:      string(t.input),
		Position:   t.cy,
		Sort:       t.sort,
		TotalCount: t.count,
		MatchCount: t.resultMerger.Length(),
		Current:    current,
		Matches:    matches,
		Selected:   selected,
	}
	bytes, _ := json.Marshal(&dump) // TODO: Errors?
	return string(bytes)
}

// https://github.com/charmbracelet/bubbletea/blob/a23da80847e6fc928febc62114f761403ac5d2f1/examples/tui-daemon-combo/main.go#L85-L103
// charmbracelet/bubbletea examples/tui-daemon-combo/main.go:85-103
func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		m.quitting = true
		return m, tea.Quit
	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return m, cmd
	case processFinishedMsg:
		d := time.Duration(msg)
		res := result{emoji: randomEmoji(), duration: d}
		log.Printf("%s Job finished in %s", res.emoji, res.duration)
		m.results = append(m.results[1:], res)
		return m, runPretendProcess
	default:
		return m, nil
	}
}
