// https://github.com/junegunn/fzf/blob/dea72834ed35b5d56634075fc7793aa4b9b1d697/src/terminal.go#L5921-L8143
// junegunn/fzf src/terminal.go:5921-8143
func (t *Terminal) Loop() error {
	// prof := profile.Start(profile.ProfilePath("/tmp/"))
	fitpad := <-t.startChan
	fit := fitpad.fit
	if fit >= 0 {
		pad := fitpad.pad
		t.tui.Resize(func(termHeight int) int {
			contentHeight := fit + t.extraLines()
			if t.needPreviewWindow() {
				if t.activePreviewOpts.aboveOrBelow() {
					if t.activePreviewOpts.size.percent {
						newContentHeight := int(float64(contentHeight) * 100. / (100. - t.activePreviewOpts.size.size))
						contentHeight = max(contentHeight+1+borderLines(t.activePreviewOpts.Border(t.layout)), newContentHeight)
					} else {
						contentHeight += int(t.activePreviewOpts.size.size) + borderLines(t.activePreviewOpts.Border(t.layout))
					}
				} else {
					// Minimum height if preview window can appear
					contentHeight = max(contentHeight, 1+borderLines(t.activePreviewOpts.Border(t.layout)))
				}
			}
			return min(termHeight, contentHeight+pad)
		})
	}

	// Context
	ctx, cancel := context.WithCancel(context.Background())

	{ // Late initialization
		intChan := make(chan os.Signal, 1)
		signal.Notify(intChan, os.Interrupt, syscall.SIGTERM, syscall.SIGHUP)
		go func() {
			for {
				select {
				case <-ctx.Done():
					return
				case s := <-intChan:
					// Don't quit by SIGINT while executing because it should be for the executing command and not for fzf itself
					if !(s == os.Interrupt && t.executing.Get()) {
						t.reqBox.Set(reqQuit, nil)
					}
				}
			}
		}()

		if !t.tui.ShouldEmitResizeEvent() {
			resizeChan := make(chan os.Signal, 1)
			notifyOnResize(resizeChan) // Non-portable
			go func() {
				for {
					select {
					case <-ctx.Done():
						return
					case <-resizeChan:
						t.reqBox.Set(reqResize, nil)
					}
				}
			}()
		}

		t.mutex.Lock()
		if err := t.initFunc(); err != nil {
			t.mutex.Unlock()
			cancel()
			t.eventBox.Set(EvtQuit, quitSignal{ExitError, err})
			return err
		}
		t.termSize = t.tui.Size()
		t.resizeWindows(false, false)
		t.window.Erase()
		t.mutex.Unlock()

		t.reqBox.Set(reqPrompt, nil)
		t.reqBox.Set(reqInfo, nil)
		t.reqBox.Set(reqHeader, nil)
		t.reqBox.Set(reqFooter, nil)
		if t.initDelay > 0 {
			go func() {
				timer := time.NewTimer(t.initDelay)
				<-timer.C
				t.reqBox.Set(reqActivate, nil)
			}()
		}

		// Keep the spinner spinning
		go func() {
			for t.running.Get() {
				t.mutex.Lock()
				reading := t.reading
				t.mutex.Unlock()
				time.Sleep(spinnerDuration)
				if reading {
					t.reqBox.Set(reqInfo, nil)
				}
			}
		}()
	}

	if t.hasPreviewer() {
		go func() {
			var version int64
			stop := false
			t.previewBox.WaitFor(reqPreviewReady)
			for {
				requested := false
				var items [3][]*Item
				var commandTemplate string
				var env []string
				var query string
				initialOffset := 0
				t.previewBox.Wait(func(events *util.Events) {
					for req, value := range *events {
						switch req {
						case reqQuit:
							stop = true
							return
						case reqPreviewEnqueue:
							request := value.(previewRequest)
							commandTemplate = request.template
							initialOffset = request.scrollOffset
							items = request.list
							env = request.env
							query = request.query
							requested = true
						}
					}
					events.Clear()
				})
				if stop {
					break
				}
				if !requested {
					continue
				}
				version++
				// We don't display preview window if no match
				if items[0] != nil {
					command, tempFiles := t.replacePlaceholder(commandTemplate, false, query, items)
					cmd := t.executor.ExecCommand(command, true)
					cmd.Env = env

					out, _ := cmd.StdoutPipe()
					cmd.Stderr = cmd.Stdout
					reader := bufio.NewReader(out)
					eofChan := make(chan bool)
					finishChan := make(chan bool, 1)
					err := cmd.Start()
					if err == nil {
						reapChan := make(chan bool)
						lineChan := make(chan eachLine)
						// Goroutine 1 reads process output
						go func() {
							for {
								line, err := reader.ReadString('\n')
								lineChan <- eachLine{line, err}
								if err != nil {
									break
								}
							}
							eofChan <- true
						}()

						// Goroutine 2 periodically requests rendering
						rendered := util.NewAtomicBool(false)
						go func(version int64) {
							lines := []string{}
							spinner := makeSpinner(t.unicode)
							spinnerIndex := -1 // Delay initial rendering by an extra tick
							ticker := time.NewTicker(previewChunkDelay)
							offset := initialOffset
						Loop:
							for {
								select {
								case <-ticker.C:
									if len(lines) > 0 && len(lines) >= initialOffset {
										if spinnerIndex >= 0 {
											spin := spinner[spinnerIndex%len(spinner)]
											t.reqBox.Set(reqPreviewDisplay, previewResult{version, lines, offset, spin})
											rendered.Set(true)
											offset = -1
										}
										spinnerIndex++
									}
								case eachLine := <-lineChan:
									line := eachLine.line
									err := eachLine.err
									if len(line) > 0 {
										clearIndex := strings.Index(line, clearCode)
										if clearIndex >= 0 {
											lines = []string{}
											line = line[clearIndex+len(clearCode):]
											version--
											offset = 0
										}
										lines = append(lines, line)
									}
									if err != nil {
										t.reqBox.Set(reqPreviewDisplay, previewResult{version, lines, offset, ""})
										rendered.Set(true)
										break Loop
									}
								}
							}
							ticker.Stop()
							reapChan <- true
						}(version)

						// Goroutine 3 is responsible for cancelling running preview command
						go func(version int64) {
							timer := time.NewTimer(previewDelayed)
						Loop:
							for {
								select {
								case <-ctx.Done():
									break Loop
								case <-timer.C:
									t.reqBox.Set(reqPreviewDelayed, version)
								case immediately := <-t.killChan:
									if immediately {
										util.KillCommand(cmd)
										t.killedChan <- true
									} else {
										// We can immediately kill a long-running preview program
										// once we started rendering its partial output
										delay := previewCancelWait
										if rendered.Get() {
											delay = 0
										}
										timer := time.NewTimer(delay)
										select {
										case <-timer.C:
											util.KillCommand(cmd)
										case <-finishChan:
										}
										timer.Stop()
									}
									break Loop
								case <-finishChan:
									break Loop
								}
							}
							timer.Stop()
							reapChan <- true
						}(version)

						<-eofChan          // Goroutine 1 finished
						cmd.Wait()         // NOTE: We should not call Wait before EOF
						finishChan <- true // Tell Goroutine 3 to stop
						<-reapChan         // Goroutine 2 and 3 finished
						<-reapChan
						removeFiles(tempFiles)
					} else {
						// Failed to start the command. Report the error immediately.
						t.reqBox.Set(reqPreviewDisplay, previewResult{version, []string{err.Error()}, 0, ""})
					}
				} else {
					t.reqBox.Set(reqPreviewDisplay, previewResult{version, nil, 0, ""})
				}
			}
		}()
	}

	refreshPreview := func(command string) {
		if len(command) > 0 && t.canPreview() {
			_, list := t.buildPlusList(command, false)
			t.cancelPreview()
			t.previewBox.Set(reqPreviewEnqueue, previewRequest{command, t.evaluateScrollOffset(), list, t.environForPreview(), string(t.input)})
		}
	}

	go func() { // Render loop
		var focusedIndex = minItem.Index()
		var version int64 = -1
		running := true
		code := ExitError
		exit := func(getCode func() int) {
			if t.hasPreviewer() {
				t.previewBox.Set(reqQuit, nil)
			}
			if t.listener != nil {
				t.listener.Close()
			}
			t.tui.Close()
			code = getCode()
			if code <= ExitNoMatch && t.history != nil {
				t.history.append(string(t.input))
			}
			t.runningCmds.ForEach(func(cmd *runningCmd) {
				util.KillCommand(cmd.cmd)
				removeFiles(cmd.tempFiles)
			})
			running = false
			t.mutex.Unlock()
		}

		for running {
			t.reqBox.Wait(func(events *util.Events) {
				defer events.Clear()

				// Sort events.
				// e.g. Make sure that reqPrompt is processed before reqInfo
				keys := make([]int, 0, len(*events))
				for key := range *events {
					keys = append(keys, int(key))
				}
				sort.Ints(keys)

				// t.uiMutex must be locked first to avoid deadlock. Execute actions
				// will 1. unlock t.mutex to allow GET endpoint and 2. lock t.uiMutex
				// to block rendering during the execution.
				//
				// T1           T2 (good)       |  T1            T2 (bad)
				//               L t.uiMutex    |
				//  L t.mutex                   |   L t.mutex
				//  U t.mutex                   |   U t.mutex
				//               L t.mutex      |                 L t.mutex
				//               U t.mutex      |   L t.uiMutex
				//               U t.uiMutex    |                 L t.uiMutex!!
				//  L t.uiMutex                 |
				//                              |   L t.mutex!!
				//  L t.mutex                   |   U t.uiMutex
				//  U t.uiMutex                 |
				t.uiMutex.Lock()
				t.mutex.Lock()
				info := false
				header := false
				footer := false
				for _, key := range keys {
					req := util.EventType(key)
					value := (*events)[req]
					switch req {
					case reqPrompt:
						t.printPrompt()
						if t.infoStyle == infoInline || t.infoStyle == infoInlineRight {
							info = true
						}
					case reqInfo:
						info = true
					case reqList:
						t.printList()
						currentIndex := t.currentIndex()
						if t.track.Current() && t.track.index != currentIndex {
							t.track = trackDisabled
							info = true
						}
						focusChanged := focusedIndex != currentIndex
						if (t.hasFocusActions || t.infoCommand != "") && focusChanged && currentIndex != t.lastFocus {
							t.lastFocus = currentIndex
							t.eventChan <- tui.Focus.AsEvent()
							if t.infoCommand != "" {
								info = true
							}
						}
						if focusChanged || version != t.version {
							version = t.version
							focusedIndex = currentIndex
							refreshPreview(t.previewOpts.command)
						}
					case reqJump:
						if t.merger.Length() == 0 {
							t.jumping = jumpDisabled
						}
						t.printList()
					case reqHeader:
						header = true
					case reqFooter:
						footer = true
					case reqActivate:
						t.suppress = false
						if t.hasPreviewer() {
							t.previewBox.Set(reqPreviewReady, nil)
						}
					case reqRedrawInputLabel:
						t.printLabel(t.inputBorder, t.inputLabel, t.inputLabelOpts, t.inputLabelLen, t.inputBorderShape, true)
					case reqRedrawHeaderLabel:
						if t.headerBorderShape == tui.BorderInline {
							// Inline labels sit on the separator inside wborder; re-run the
							// full layout to repaint the separator + label together.
							t.printAll()
						} else {
							t.printLabel(t.headerBorder, t.headerLabel, t.headerLabelOpts, t.headerLabelLen, t.headerBorderShape, true)
						}
					case reqRedrawFooterLabel:
						if t.footerBorderShape == tui.BorderInline {
							t.printAll()
						} else {
							t.printLabel(t.footerBorder, t.footerLabel, t.footerLabelOpts, t.footerLabelLen, t.footerBorderShape, true)
						}
					case reqRedrawListLabel:
						// When inline sections are active, the label's bg depends on which
						// section owns the adjacent edge. Rerun the layout to reuse that
						// logic rather than duplicating it here.
						if t.headerBorderShape == tui.BorderInline || t.headerLinesShape == tui.BorderInline || t.footerBorderShape == tui.BorderInline {
							t.printAll()
						} else {
							t.printLabel(t.wborder, t.listLabel, t.listLabelOpts, t.listLabelLen, t.listBorderShape, true)
						}
					case reqRedrawBorderLabel:
						t.printLabel(t.border, t.borderLabel, t.borderLabelOpts, t.borderLabelLen, t.borderShape, true)
					case reqRedrawPreviewLabel:
						t.printLabel(t.pborder, t.previewLabel, t.previewLabelOpts, t.previewLabelLen, t.activePreviewOpts.Border(t.layout), true)
					case reqReinit, reqResize, reqFullRedraw, reqRedraw:
						if req == reqReinit {
							t.tui.Resume(t.fullscreen, true)
						}
						if req == reqResize {
							t.termSize = t.tui.Size()
						}
						wasHidden := t.pwindow == nil
						if req == reqRedraw {
							t.printAll()
						} else {
							t.fullRedraw()
						}
						if wasHidden && t.hasPreviewWindow() {
							refreshPreview(t.previewOpts.command)
						}
						if req == reqResize && t.hasResizeActions {
							t.eventChan <- tui.Resize.AsEvent()
						}
					case reqClose:
						exit(func() int {
							if t.output() {
								return ExitOk
							}
							return ExitNoMatch
						})
						return
					case reqPreviewDisplay:
						result := value.(previewResult)
						if t.previewer.version != result.version {
							t.previewer.version = result.version
							t.previewer.following.Force(t.activePreviewOpts.follow)
							if t.previewer.following.Enabled() {
								t.previewer.offset = 0
							}
						}
						t.previewer.lines = result.lines
						t.previewer.spinner = result.spinner
						if t.hasPreviewWindow() && t.previewer.following.Enabled() {
							t.previewer.offset = t.followOffset()
						} else if result.offset >= 0 {
							t.previewer.offset = util.Constrain(result.offset, t.activePreviewOpts.headerLines, len(t.previewer.lines)-1)
						}
						t.printPreview()
					case reqPreviewRefresh:
						t.printPreview()
					case reqPreviewDelayed:
						t.previewer.version = value.(int64)
						t.printPreviewDelayed()
					case reqPrintQuery:
						exit(func() int {
							t.printer(string(t.input))
							return ExitOk
						})
						return
					case reqBecome:
						exit(func() int { return ExitBecome })
						return
					case reqQuit:
						exit(func() int { return ExitInterrupt })
						return
					case reqFatal:
						exit(func() int { return ExitError })
						return
					}
				}
				if (info || header || footer) && !t.resizeIfNeeded() {
					if info {
						t.printInfo()
					}
					if header {
						t.printHeader()
					}
					if footer {
						t.printFooter()
					}
				}
				t.flush()
				t.mutex.Unlock()
				t.uiMutex.Unlock()
			})
		}

		t.running.Set(false)
		t.killPreview()
		cancel()
		t.eventBox.Set(EvtQuit, quitSignal{code, nil})
	}()

	looping := true
	barrier := make(chan bool)
	go func() {
		for {
			select {
			case <-ctx.Done():
				return
			case <-barrier:
			}
			select {
			case <-ctx.Done():
				return
			case t.keyChan <- t.tui.GetChar(t.listenAddr != nil):
			}
		}
	}()
	t.startTimers(ctx)
	previewDraggingPos := -1
	barDragging := false
	pbarDragging := false
	pborderDragging := -1
	wasDown := false
	pmx, pmy := -1, -1
	needBarrier := true

	// If an action is bound to 'start', we're going to process it before reading
	// user input.
	if !t.hasStartActions {
		barrier <- true
		needBarrier = false
	}

	// These variables are defined outside the loop to be accessible from closures.
	// In particular, async bg-transform callbacks run in a later iteration than
	// the one that scheduled them, but still need to mutate state that the
	// loop-end reload/event dispatch reads.
	events := []util.EventType{}
	changed := false
	var newNth *[]Range
	var newWithNth *withNthSpec
	var newHeaderLines *int
	var newCommand *commandSpec
	var reloadSync bool
	var denylist []int32
	req := func(evts ...util.EventType) {
		for _, event := range evts {
			events = append(events, event)
			if isTerminalEvent(event) {
				looping = false
			}
		}
	}

	// The main event loop
	for loopIndex := int64(0); looping; loopIndex++ {
		events = []util.EventType{}
		changed = false
		newNth = nil
		newWithNth = nil
		newHeaderLines = nil
		newCommand = nil
		reloadSync = false
		denylist = nil
		beof := false
		queryChanged := false

		// Special handling of --sync. Activate the interface on the second tick.
		if loopIndex == 1 && t.deferActivation() {
			t.reqBox.Set(reqActivate, nil)
		}

		if loopIndex > 0 && needBarrier {
			barrier <- true
			needBarrier = false
		}

		var event tui.Event
		actions := []*action{}
		callbacks := []versionedCallback{}
		select {
		case event = <-t.keyChan:
			needBarrier = true
		case event = <-t.timerChan:
		case event = <-t.eventChan:
			// Drain channel to process all queued events at once without rendering
			// the intermediate states
		Drain:
			for {
				if eventActions, prs := t.keymap[event]; prs {
					actions = append(actions, eventActions...)
				}
				for {
					select {
					case event = <-t.eventChan:
						continue Drain
					default:
						break Drain
					}
				}
			}
		case serverActions := <-t.serverInputChan:
			event = tui.Invalid.AsEvent()
			if t.listenAddr == nil || t.listenAddr.IsLocal() || t.listenUnsafe {
				actions = serverActions
			} else {
				for _, action := range serverActions {
					if !processExecution(action.t) {
						actions = append(actions, action)
					}
				}
			}
			for _, action := range actions {
				if action.t == actExecute {
					t.tui.CancelGetChar()
					break
				}
			}

		case callback := <-t.callbackChan:
			event = tui.Invalid.AsEvent()
			actions = append(actions, &action{t: actAsync})
			callbacks = append(callbacks, callback)
		DrainCallback:
			for {
				select {
				case callback = <-t.callbackChan:
					callbacks = append(callbacks, callback)
					continue DrainCallback
				default:
					break DrainCallback
				}
			}
		}

		t.mutex.Lock()
		for key, ret := range t.expect {
			if keyMatch(key, event) {
				t.pressed = ret
				t.mutex.Unlock()
				t.reqBox.Set(reqClose, nil)
				return nil
			}
		}
		triggering := map[tui.Event]struct{}{}
		previousInput := t.input
		previousCx := t.cx
		previousVersion := t.version
		if event.Type < tui.Invalid {
			t.lastKey = event.KeyName()
			t.lastActivity = time.Now()
		}
		updatePreviewWindow := func(forcePreview bool) {
			t.resizeWindows(forcePreview, false)
			req(reqPrompt, reqList, reqInfo, reqHeader, reqFooter)
		}
		toggle := func() bool {
			current := t.currentItem()
			if current != nil && t.toggleItem(current) {
				req(reqInfo)
				return true
			}
			return false
		}
		scrollPreviewTo := func(newOffset int) {
			if !t.previewer.scrollable {
				return
			}
			numLines := len(t.previewer.lines)
			headerLines := t.activePreviewOpts.headerLines
			if t.activePreviewOpts.cycle {
				offsetRange := numLines - headerLines
				newOffset = ((newOffset-headerLines)+offsetRange)%offsetRange + headerLines
			}
			newOffset = util.Constrain(newOffset, headerLines, numLines-1)
			if t.previewer.offset != newOffset {
				t.previewer.offset = newOffset
				t.previewer.following.Set(t.previewer.offset >= numLines-(t.pwindow.Height()-headerLines))
				req(reqPreviewRefresh)
			}
		}
		scrollPreviewBy := func(amount int) {
			scrollPreviewTo(t.previewer.offset + amount)
		}

		actionsFor := func(eventType tui.EventType) []*action {
			return t.keymap[eventType.AsEvent()]
		}

		var doAction func(*action) bool
		doActions := func(actions []*action) bool {
			for iter := 0; iter <= maxFocusEvents; iter++ {
				currentIndex := t.currentIndex()
				for _, action := range actions {
					if !doAction(action) {
						return false
					}
					// A terminal action performed. We should stop processing more.
					if !looping {
						break
					}
				}

				if onFocus, prs := t.keymap[tui.Focus.AsEvent()]; prs && iter < maxFocusEvents {
					if newIndex := t.currentIndex(); newIndex != currentIndex {
						t.lastFocus = newIndex
						if t.infoCommand != "" {
							req(reqInfo)
						}
						actions = onFocus
						continue
					}
				}
				break
			}
			return true
		}
		doAction = func(a *action) bool {
			// Keep track of the current query before the action is executed,
			// so we can restore it when the input section is hidden (--no-input).
			// * By doing this, we don't have to add a conditional branch to each
			//   query modifying action.
			// * We restore the query after each action instead of after a set of
			//   actions to allow changing the query even when the input is hidden
			//     e.g. fzf --no-input --bind 'space:show-input+change-query(foo)+hide-input'
			currentInput := t.input
			capture := func(firstLineOnly bool, callback func(string)) {
				if a.t >= actBgTransform {
					// bg-transform-*
					t.captureAsync(*a, firstLineOnly, callback)
				} else if a.t >= actTransform {
					// transform-*
					if firstLineOnly {
						callback(t.captureLine(a.a))
					} else {
						callback(t.captureLines(a.a))
					}
				} else {
					// change-*
					callback(a.a)
				}
			}
			// When track-blocked, only allow abort/cancel and track-disabling actions
			if t.trackBlocked && a.t != actToggleTrack && a.t != actToggleTrackCurrent && a.t != actUntrackCurrent {
				if a.t == actAbort || a.t == actCancel {
					t.unblockTrack()
					req(reqPrompt, reqInfo)
				}
				return true
			}
		Action:
			switch a.t {
			case actIgnore, actStart, actClick:
			case actAsync:
				for _, callback := range callbacks {
					if t.bgVersion == callback.version {
						callback.callback()
					}
				}
			case actBecome:
				valid, list := t.buildPlusList(a.a, false)
				if valid {
					// We do not remove temp files in this case
					command, _ := t.replacePlaceholder(a.a, false, string(t.input), list)
					t.tui.Close()
					if t.history != nil {
						t.history.append(string(t.input))
					}

					if len(t.proxyScript) > 0 {
						data := strings.Join(append([]string{command}, t.environ()...), "\x00")
						os.WriteFile(t.proxyScript+becomeSuffix, []byte(data), 0600)
						req(reqBecome)
					} else {
						t.executor.Become(t.ttyin, t.environ(), command)
					}
				}
			case actBell:
				t.tui.Bell()
			case actExcludeMulti:
				if len(t.selected) > 0 {
					for _, item := range t.sortSelected() {
						denylist = append(denylist, item.item.Index())
					}
					// Clear selected items
					t.selected = make(map[int32]selectedItem)
					t.version++
				} else {
					item := t.currentItem()
					if item != nil {
						denylist = append(denylist, item.Index())
					}
				}
				changed = true
			case actExclude:
				if item := t.currentItem(); item != nil {
					denylist = append(denylist, item.Index())
					t.deselectItem(item)
					changed = true
				}
			case actExecute, actExecuteSilent:
				t.executeCommand(a.a, false, a.t == actExecuteSilent, false, false, "")
			case actExecuteMulti:
				t.executeCommand(a.a, true, false, false, false, "")
			case actInvalid:
				t.mutex.Unlock()
				return false
			case actBracketedPasteBegin:
				current := []rune(t.input)
				t.pasting = &current
			case actBracketedPasteEnd:
				if t.pasting != nil {
					queryChanged = string(t.input) != string(*t.pasting)
					t.pasting = nil
				}
			case actTogglePreview, actShowPreview, actHidePreview:
				var act bool
				switch a.t {
				case actShowPreview:
					act = !t.hasPreviewWindow() && len(t.previewOpts.command) > 0
				case actHidePreview:
					act = t.hasPreviewWindow()
				case actTogglePreview:
					act = t.hasPreviewWindow() || len(t.previewOpts.command) > 0
				}
				if act {
					t.activePreviewOpts.Toggle()
					updatePreviewWindow(false)
					if t.canPreview() {
						valid, list := t.buildPlusList(t.previewOpts.command, false)
						if valid {
							t.cancelPreview()
							t.previewBox.Set(reqPreviewEnqueue,
								previewRequest{t.previewOpts.command, t.evaluateScrollOffset(), list, t.environForPreview(), string(t.input)})
						}
					} else {
						// Discard the preview content so that it won't accidentally appear
						// when preview window is re-enabled and previewDelay is triggered
						t.previewer.lines = nil

						// Also kill the preview process if it's still running
						t.cancelPreview()
					}
				}
			case actTogglePreviewWrap, actTogglePreviewWrapWord:
				if t.hasPreviewWindow() {
					if a.t == actTogglePreviewWrapWord {
						t.activePreviewOpts.wrapWord = !t.activePreviewOpts.wrapWord
						t.activePreviewOpts.wrap = t.activePreviewOpts.wrapWord
					} else {
						t.activePreviewOpts.wrap = !t.activePreviewOpts.wrap
						if !t.activePreviewOpts.wrap {
							t.activePreviewOpts.wrapWord = false
						}
					}
					// Reset preview version so that full redraw occurs
					t.previewed.version = 0
					req(reqPreviewRefresh)
				}
			case actTransformPrompt, actBgTransformPrompt:
				capture(true, func(prompt string) {
					t.promptString = prompt
					t.prompt, t.promptLen = t.parsePrompt(prompt)
					req(reqPrompt)
				})
			case actTransformQuery, actBgTransformQuery:
				capture(true, func(query string) {
					t.input = []rune(query)
					t.cx = len(t.input)
				})
			case actToggleSort:
				t.sort = !t.sort
				changed = true
			case actPreviewTop:
				if t.hasPreviewWindow() {
					scrollPreviewTo(0)
				}
			case actPreviewBottom:
				if t.hasPreviewWindow() {
					scrollPreviewTo(len(t.previewer.lines) - t.pwindow.Height())
				}
			case actPreviewUp:
				if t.hasPreviewWindow() {
					scrollPreviewBy(-1)
				}
			case actPreviewDown:
				if t.hasPreviewWindow() {
					scrollPreviewBy(1)
				}
			case actPreviewPageUp:
				if t.hasPreviewWindow() {
					scrollPreviewBy(-t.pwindow.Height())
				}
			case actPreviewPageDown:
				if t.hasPreviewWindow() {
					scrollPreviewBy(t.pwindow.Height())
				}
			case actPreviewHalfPageUp:
				if t.hasPreviewWindow() {
					scrollPreviewBy(-t.pwindow.Height() / 2)
				}
			case actPreviewHalfPageDown:
				if t.hasPreviewWindow() {
					scrollPreviewBy(t.pwindow.Height() / 2)
				}
			case actBeginningOfLine:
				t.cx = 0
				t.xoffset = 0
			case actBackwardChar:
				if t.cx > 0 {
					t.cx--
				}
			case actPrintQuery:
				req(reqPrintQuery)
			case actChangeHeaderLines, actTransformHeaderLines, actBgTransformHeaderLines:
				capture(true, func(expr string) {
					if n, err := strconv.Atoi(expr); err == nil && n >= 0 && n != t.headerLines {
						t.headerLines = n
						newHeaderLines = &n
						changed = true
						// Deselect items that are now part of the header
						for idx := range t.selected {
							if int(idx) < n {
								delete(t.selected, idx)
							}
						}
						// Tell UpdateList to reposition cursor to the current item
						t.targetIndex = t.currentIndex()
						req(reqList, reqPrompt, reqInfo, reqHeader)
					}
				})
			case actChangeMulti:
				multi := t.multi
				if a.a == "" {
					multi = maxMulti
				} else if n, e := strconv.Atoi(a.a); e == nil && n >= 0 {
					multi = n
				}
				if t.multi > 0 && multi != t.multi {
					t.selected = make(map[int32]selectedItem)
					t.version++
				}
				t.multi = multi
				req(reqList, reqInfo)
			case actChangeNth, actTransformNth, actBgTransformNth:
				capture(true, func(expr string) {
					// Split nth expression
					tokens := strings.Split(expr, "|")
					if nth, err := splitNth(tokens[0]); err == nil || len(expr) == 0 {
						// Changed
						newNth = &nth
					} else {
						// The default
						newNth = &t.nth
					}
					// Cycle
					if len(tokens) > 1 {
						a.a = strings.Join(append(tokens[1:], tokens[0]), "|")
					}
					if !compareRanges(t.nthCurrent, *newNth) {
						changed = true
						t.nthCurrent = *newNth
						t.forceRerenderList()
					}
				})
			case actChangeWithNth, actTransformWithNth, actBgTransformWithNth:
				if !t.withNthEnabled {
					break Action
				}
				capture(true, func(expr string) {
					tokens := strings.Split(expr, "|")
					withNthExpr := tokens[0]
					if len(tokens) > 1 {
						a.a = strings.Join(append(tokens[1:], tokens[0]), "|")
					}
					// Empty value restores the default --with-nth
					if len(withNthExpr) == 0 {
						withNthExpr = t.withNthDefault
					}
					if withNthExpr != t.withNthExpr {
						if factory, err := nthTransformer(withNthExpr); err == nil {
							newWithNth = &withNthSpec{fn: factory(t.delimiter)}
						} else {
							return
						}
						t.withNthExpr = withNthExpr
						t.filterSelection = true
						changed = true
						t.clearNumLinesCache()
						t.forceRerenderList()
					}
				})
			case actChangeQuery:
				t.input = []rune(a.a)
				t.cx = len(t.input)
			case actChangeHeader, actTransformHeader, actBgTransformHeader:
				capture(false, func(header string) {
					if t.changeHeader(header) {
						// resizeIfNeeded() tolerates a shorter-than-wanted inline
						// window, so a length change can leave the inline slot
						// stale. Force a redraw to re-run the layout. Non-inline
						// shapes are handled by resizeIfNeeded.
						if t.headerBorderShape == tui.BorderInline {
							req(reqRedraw)
						} else {
							req(reqList, reqPrompt, reqInfo)
						}
					}
					req(reqHeader)
				})
			case actChangeFooter, actTransformFooter, actBgTransformFooter:
				capture(false, func(footer string) {
					if t.changeFooter(footer) && t.footerBorderShape == tui.BorderInline {
						// resizeIfNeeded() tolerates a shorter-than-wanted inline
						// window, so a length change can leave the inline slot
						// stale. Force a redraw to re-run the layout.
						req(reqRedraw)
					}
					req(reqFooter)
				})
			case actChangeHeaderLabel, actTransformHeaderLabel, actBgTransformHeaderLabel:
				capture(true, func(label string) {
					t.headerLabelOpts.label = label
					t.headerLabel, t.headerLabelLen = t.ansiLabelPrinter(label, &tui.ColHeaderLabel, false)
					req(reqRedrawHeaderLabel)
				})
			case actChangeFooterLabel, actTransformFooterLabel, actBgTransformFooterLabel:
				capture(true, func(label string) {
					t.footerLabelOpts.label = label
					t.footerLabel, t.footerLabelLen = t.ansiLabelPrinter(label, &tui.ColFooterLabel, false)
					req(reqRedrawFooterLabel)
				})
			case actChangeInputLabel, actTransformInputLabel, actBgTransformInputLabel:
				capture(true, func(label string) {
					t.inputLabelOpts.label = label
					if t.inputBorder != nil {
						t.inputLabel, t.inputLabelLen = t.ansiLabelPrinter(label, &tui.ColInputLabel, false)
						req(reqRedrawInputLabel)
					}
				})
			case actChangeListLabel, actTransformListLabel, actBgTransformListLabel:
				capture(true, func(label string) {
					t.listLabelOpts.label = label
					if t.wborder != nil {
						t.listLabel, t.listLabelLen = t.ansiLabelPrinter(label, &tui.ColListLabel, false)
						req(reqRedrawListLabel)
					}
				})
			case actChangeBorderLabel, actTransformBorderLabel, actBgTransformBorderLabel:
				capture(true, func(label string) {
					t.borderLabelOpts.label = label
					if t.border != nil {
						t.borderLabel, t.borderLabelLen = t.ansiLabelPrinter(label, &tui.ColBorderLabel, false)
						req(reqRedrawBorderLabel)
					}
				})
			case actChangePreviewLabel, actTransformPreviewLabel, actBgTransformPreviewLabel:
				capture(true, func(label string) {
					t.previewLabelOpts.label = label
					if t.pborder != nil {
						t.previewLabel, t.previewLabelLen = t.ansiLabelPrinter(label, &tui.ColPreviewLabel, false)
						req(reqRedrawPreviewLabel)
					}
				})
			case actTransform, actBgTransform:
				capture(false, func(body string) {
					// Allow 'put' if the triggering key is a printable character
					if actions, err := parseSingleActionList(strings.Trim(body, "\r\n"), event.Printable()); err == nil {
						// NOTE: We're not properly passing the return value here
						doActions(actions)
					}
				})
			case actBgCancel:
				t.bgVersion++
				t.runningCmds.ForEach(func(cmd *runningCmd) {
					util.KillCommand(cmd.cmd)
				})
			case actChangePrompt:
				t.promptString = a.a
				t.prompt, t.promptLen = t.parsePrompt(a.a)
				req(reqPrompt)
			case actPreview:
				if !t.hasPreviewWindow() {
					updatePreviewWindow(true)
				}
				refreshPreview(a.a)
			case actRefreshPreview:
				refreshPreview(t.previewOpts.command)
			case actReplaceQuery:
				current := t.currentItem()
				if current != nil {
					t.input = current.text.ToRunes()
					t.cx = len(t.input)
				}
			case actFatal:
				req(reqFatal)
			case actAbort:
				req(reqQuit)
			case actDeleteChar:
				t.delChar()
			case actDeleteCharEof:
				if !t.delChar() && t.cx == 0 {
					req(reqQuit)
				}
			case actEndOfLine:
				t.cx = len(t.input)
			case actCancel:
				if len(t.input) == 0 {
					req(reqQuit)
				} else {
					t.yanked = t.input
					t.input = []rune{}
					t.cx = 0
				}
			case actBackwardDeleteCharEof:
				if len(t.input) == 0 {
					req(reqQuit)
				} else if t.cx > 0 {
					t.input = append(t.input[:t.cx-1], t.input[t.cx:]...)
					t.cx--
				}
			case actForwardChar:
				if t.cx < len(t.input) {
					t.cx++
				}
			case actBackwardDeleteChar:
				beof = len(t.input) == 0
				if t.cx > 0 {
					t.input = append(t.input[:t.cx-1], t.input[t.cx:]...)
					t.cx--
				}
			case actSelectAll:
				if t.multi > 0 {
					// Limit the scope only to the matching items
					for i := 0; i < t.resultMerger.Length(); i++ {
						if !t.selectItem(t.resultMerger.Get(i).item) {
							break
						}
					}
					req(reqList, reqInfo)
				}
			case actDeselectAll:
				if t.multi > 0 {
					// Also limit the scope only to the matching items, while this may
					// not be straightforward in raw mode.
					for i := 0; i < t.resultMerger.Length() && len(t.selected) > 0; i++ {
						t.deselectItem(t.resultMerger.Get(i).item)
					}
					req(reqList, reqInfo)
				}
			case actClose:
				if t.hasPreviewWindow() {
					t.activePreviewOpts.Toggle()
					updatePreviewWindow(false)
				} else {
					req(reqQuit)
				}
			case actSelect:
				current := t.currentItem()
				if t.multi > 0 && current != nil && t.selectItemChanged(current) {
					req(reqList, reqInfo)
				}
			case actDeselect:
				current := t.currentItem()
				if t.multi > 0 && current != nil && t.deselectItemChanged(current) {
					req(reqList, reqInfo)
				}
			case actToggle:
				if t.multi > 0 && t.merger.Length() > 0 && toggle() {
					req(reqList)
				}
			case actToggleAll:
				if t.multi > 0 {
					prevIndexes := make(map[int]struct{})
					for i := 0; i < t.resultMerger.Length() && len(t.selected) > 0; i++ {
						item := t.resultMerger.Get(i).item
						if _, found := t.selected[item.Index()]; found {
							prevIndexes[i] = struct{}{}
							t.deselectItem(item)
						}
					}

					for i := 0; i < t.resultMerger.Length(); i++ {
						if _, found := prevIndexes[i]; !found {
							item := t.resultMerger.Get(i).item
							if !t.selectItem(item) {
								break
							}
						}
					}
					req(reqList, reqInfo)
				}
			case actToggleIn:
				if t.layout != layoutDefault {
					return doAction(&action{t: actToggleUp})
				}
				return doAction(&action{t: actToggleDown})
			case actToggleOut:
				if t.layout != layoutDefault {
					return doAction(&action{t: actToggleDown})
				}
				return doAction(&action{t: actToggleUp})
			case actToggleDown:
				if t.multi > 0 && t.merger.Length() > 0 && toggle() {
					t.vmove(-1, true)
					req(reqList)
				}
			case actToggleUp:
				if t.multi > 0 && t.merger.Length() > 0 && toggle() {
					t.vmove(1, true)
					req(reqList)
				}
			case actDown, actDownMatch, actUp, actUpMatch:
				dir := -1
				if a.t == actUp || a.t == actUpMatch {
					dir = 1
				}
				if t.raw && (a.t == actDownMatch || a.t == actUpMatch) {
					if t.resultMerger.Length() > 0 {
						prevCy := t.cy
						for t.vmove(dir, true) && !t.isCurrentItemMatch() {
						}
						if !t.isCurrentItemMatch() {
							t.vset(prevCy)
						}
					}
				} else {
					t.vmove(dir, true)
				}
				req(reqList)
			case actToggleRaw, actEnableRaw, actDisableRaw:
				prevRaw := t.raw
				newRaw := t.raw
				switch a.t {
				case actEnableRaw:
					newRaw = true
				case actDisableRaw:
					newRaw = false
				case actToggleRaw:
					newRaw = !t.raw
				}
				if prevRaw == newRaw {
					break
				}
				prevPos := t.cy - t.offset
				prevIndex := t.currentIndex()
				if newRaw {
					// Build matchMap if not available
					if len(t.matchMap) == 0 {
						t.matchMap = t.resultMerger.ToMap()
					}
					t.merger = t.passMerger
				} else {
					// Find the closest matching item
					if !t.isCurrentItemMatch() && t.resultMerger.Length() > 1 {
						distance := 0
					Loop:
						for {
							distance++
							checks := 0
							for _, cy := range []int{t.cy + distance, t.cy - distance} {
								if cy >= 0 && cy < t.merger.Length() {
									checks++
									item := t.merger.Get(cy).item
									if t.isItemMatch(item) {
										prevIndex = item.Index()
										break Loop
									}
								}
							}
							if checks == 0 {
								break
							}
						}
					}

					t.merger = t.resultMerger

					// Need to remove non-matching items from the selection
					if t.multi > 0 && len(t.selected) > 0 {
						t.filterSelected()
						req(reqInfo)
					}
				}
				t.raw = newRaw

				// Try to retain position
				if prevIndex != minItem.Index() {
					t.cy = max(0, t.merger.FindIndex(prevIndex))
					t.offset = t.cy - prevPos
				}

				// List needs to be rerendered
				t.forceRerenderList()
				req(reqList)
			case actAccept:
				req(reqClose)
			case actAcceptNonEmpty:
				if len(t.selected) > 0 || t.merger.Length() > 0 || !t.reading && t.count == 0 {
					req(reqClose)
				}
			case actAcceptOrPrintQuery:
				if len(t.selected) > 0 || t.merger.Length() > 0 {
					req(reqClose)
				} else {
					req(reqPrintQuery)
				}
			case actClearScreen:
				req(reqFullRedraw)
			case actClearQuery:
				t.input = []rune{}
				t.cx = 0
			case actClearSelection:
				if t.multi > 0 {
					t.selected = make(map[int32]selectedItem)
					t.version++
					req(reqList, reqInfo)
				}
			case actFirst, actBest:
				if t.raw && a.t == actBest {
					if t.resultMerger.Length() > 0 {
						t.vset(t.merger.FindIndex(t.resultMerger.Get(0).item.Index()))
					}
				} else {
					t.vset(0)
				}
				t.constrain()
				req(reqList)
			case actLast:
				t.vset(t.merger.Length() - 1)
				t.constrain()
				req(reqList)
			case actPosition:
				if n, e := strconv.Atoi(a.a); e == nil {
					if n > 0 {
						n--
					} else if n < 0 {
						n += t.merger.Length()
					}
					t.vset(n)
					t.constrain()
					req(reqList)
				}
			case actPut:
				str := []rune(a.a)
				suffix := copySlice(t.input[t.cx:])
				t.input = append(append(t.input[:t.cx], str...), suffix...)
				t.cx += len(str)
			case actPrint:
				t.printQueue = append(t.printQueue, a.a)
			case actUnixLineDiscard:
				beof = len(t.input) == 0
				if t.cx > 0 {
					t.yanked = copySlice(t.input[:t.cx])
					t.input = t.input[t.cx:]
					t.cx = 0
				}
			case actUnixWordRubout:
				beof = len(t.input) == 0
				if t.cx > 0 {
					t.rubout("\\s\\S")
				}
			case actBackwardKillWord:
				beof = len(t.input) == 0
				if t.cx > 0 {
					t.rubout(t.wordRubout)
				}
			case actBackwardKillSubWord:
				beof = len(t.input) == 0
				if t.cx > 0 {
					t.rubout(t.subWordRubout)
				}
			case actYank:
				suffix := copySlice(t.input[t.cx:])
				t.input = append(append(t.input[:t.cx], t.yanked...), suffix...)
				t.cx += len(t.yanked)
			case actPageUp, actPageDown, actHalfPageUp, actHalfPageDown:
				// Calculate the number of lines to move
				maxItems := t.maxItems()
				linesToMove := maxItems - 1
				if a.t == actHalfPageUp || a.t == actHalfPageDown {
					linesToMove = maxItems / 2
				}
				// Move at least one line even in a very short window
				linesToMove = max(1, linesToMove)

				// Determine the direction of the movement
				direction := -1
				if a.t == actPageUp || a.t == actHalfPageUp {
					direction = 1
				}

				// In non-default layout, items are listed from top to bottom
				if t.layout != layoutDefault {
					direction *= -1
				}

				// We can simply add the number of lines to the current position in
				// single-line mode
				if !t.canSpanMultiLines() {
					t.vset(t.cy + direction*linesToMove)
					req(reqList)
					break
				}

				// But in multi-line mode, we need to carefully limit the amount of
				// vertical movement so that items are not skipped. In order to do
				// this, we calculate the minimum or maximum offset based on the
				// direction of the movement and the number of lines of the items
				// around the current scroll offset.
				var minOffset, maxOffset, lineSum int
				if direction > 0 {
					maxOffset = t.offset
					for ; maxOffset < t.merger.Length(); maxOffset++ {
						itemLines, _ := t.numItemLines(t.merger.Get(maxOffset).item, maxItems)
						lineSum += itemLines
						if lineSum >= maxItems {
							break
						}
					}
				} else {
					minOffset = t.offset
					for ; minOffset >= 0 && minOffset < t.merger.Length(); minOffset-- {
						itemLines, _ := t.numItemLines(t.merger.Get(minOffset).item, maxItems)
						lineSum += itemLines
						if lineSum >= maxItems {
							if lineSum > maxItems {
								minOffset++
							}
							break
						}
					}
				}

				for i := 0; i < linesToMove; i++ {
					cy, offset := t.cy, t.offset
					t.vset(cy + direction)
					t.constrain()
					if cy == t.cy {
						break
					}
					if i > 0 && (direction > 0 && t.offset > maxOffset ||
						direction < 0 && t.offset < minOffset) {
						t.cy, t.offset = cy, offset
						break
					}
				}
				req(reqList)
			case actOffsetUp, actOffsetDown:
				diff := 1
				if a.t == actOffsetDown {
					diff = -1
				}
				if t.layout != layoutDefault {
					diff *= -1
				}
				t.offset += diff
				before := t.offset
				t.constrain()
				if before != t.offset {
					t.offset = before
					if t.layout != layoutDefault {
						diff *= -1
					}
					t.vmove(diff, false)
				}
				req(reqList)
			case actOffsetMiddle:
				soff := t.scrollOff
				t.scrollOff = t.window.Height()
				t.constrain()
				t.scrollOff = soff
				req(reqList)
			case actJump:
				t.jumping = jumpEnabled
				req(reqJump)
			case actJumpAccept:
				t.jumping = jumpAcceptEnabled
				req(reqJump)
			case actBackwardWord:
				t.cx = findLastMatch(t.wordRubout, string(t.input[:t.cx])) + 1
			case actForwardWord:
				t.cx += findFirstMatch(t.wordNext, string(t.input[t.cx:])) + 1
			case actBackwardSubWord:
				t.cx = findLastMatch(t.subWordRubout, string(t.input[:t.cx])) + 1
			case actForwardSubWord:
				t.cx += findFirstMatch(t.subWordNext, string(t.input[t.cx:])) + 1
			case actKillWord:
				ncx := t.cx +
					findFirstMatch(t.wordNext, string(t.input[t.cx:])) + 1
				if ncx > t.cx {
					t.yanked = copySlice(t.input[t.cx:ncx])
					t.input = append(t.input[:t.cx], t.input[ncx:]...)
				}
			case actKillSubWord:
				ncx := t.cx +
					findFirstMatch(t.subWordNext, string(t.input[t.cx:])) + 1
				if ncx > t.cx {
					t.yanked = copySlice(t.input[t.cx:ncx])
					t.input = append(t.input[:t.cx], t.input[ncx:]...)
				}
			case actKillLine:
				if t.cx < len(t.input) {
					t.yanked = copySlice(t.input[t.cx:])
					t.input = t.input[:t.cx]
				}
			case actChar:
				prefix := copySlice(t.input[:t.cx])
				t.input = append(append(prefix, event.Char), t.input[t.cx:]...)
				t.cx++
			case actPrevHistory:
				if t.history != nil {
					t.history.override(string(t.input))
					t.input = trimQuery(t.history.previous())
					t.cx = len(t.input)
				}
			case actNextHistory:
				if t.history != nil {
					t.history.override(string(t.input))
					t.input = trimQuery(t.history.next())
					t.cx = len(t.input)
				}
			case actToggleSearch:
				t.paused = !t.paused
				changed = !t.paused
				req(reqPrompt)
			case actToggleTrack:
				switch t.track {
				case trackEnabled:
					t.track = trackDisabled
				case trackDisabled:
					t.track = trackEnabled
				}
				t.unblockTrack()
				req(reqPrompt, reqInfo)
			case actToggleTrackCurrent:
				if t.track.Current() {
					t.track = trackDisabled
				} else if t.track.Disabled() {
					t.track = trackCurrent(t.currentIndex())
				}
				t.unblockTrack()
				req(reqPrompt, reqInfo)
			case actShowHeader:
				t.headerVisible = true
				req(reqList, reqInfo, reqPrompt, reqHeader)
			case actHideHeader:
				t.headerVisible = false
				req(reqList, reqInfo, reqPrompt, reqHeader)
			case actToggleHeader:
				t.headerVisible = !t.headerVisible
				req(reqList, reqInfo, reqPrompt, reqHeader)
			case actToggleWrap, actToggleWrapWord:
				if a.t == actToggleWrapWord {
					t.wrapWord = !t.wrapWord
					t.wrap = t.wrapWord
				} else {
					t.wrap = !t.wrap
					if !t.wrap {
						t.wrapWord = false
					}
				}
				t.clearNumLinesCache()
				req(reqList, reqHeader)
			case actToggleMultiLine:
				t.multiLine = !t.multiLine
				t.clearNumLinesCache()
				req(reqList)
			case actToggleHscroll:
				// Force re-rendering of the list
				t.forceRerenderList()
				t.hscroll = !t.hscroll
				req(reqList)
			case actToggleInput, actShowInput, actHideInput:
				switch a.t {
				case actToggleInput:
					t.inputless = !t.inputless
				case actShowInput:
					if !t.inputless {
						break Action
					}
					t.inputless = false
				case actHideInput:
					if t.inputless {
						break Action
					}
					t.inputless = true
				}
				t.forceRerenderList()
				if t.inputless {
					t.tui.HideCursor()
				} else {
					t.tui.ShowCursor()
				}
				req(reqList, reqInfo, reqPrompt, reqHeader)
			case actTrackCurrent:
				// Global tracking has higher priority
				if !t.track.Global() {
					t.track = trackCurrent(t.currentIndex())
				}
				req(reqInfo)
			case actUntrackCurrent:
				if t.track.Current() {
					t.track = trackDisabled
				}
				t.unblockTrack()
				req(reqPrompt, reqInfo)
			case actSearch:
				override := []rune(a.a)
				t.inputOverride = &override
				changed = true
			case actTransformSearch, actBgTransformSearch:
				capture(true, func(query string) {
					override := []rune(query)
					t.inputOverride = &override
					changed = true
				})
			case actEnableSearch:
				t.paused = false
				changed = true
				req(reqPrompt)
			case actDisableSearch:
				t.paused = true
				req(reqPrompt)
			case actTrigger:
				if _, chords, err := parseKeyChords(a.a, ""); err == nil {
					for _, chord := range chords {
						if _, prs := triggering[chord]; prs {
							// Avoid recursive triggering
							continue
						}
						if acts, prs := t.keymap[chord]; prs {
							triggering[chord] = struct{}{}
							doActions(acts)
							delete(triggering, chord)
						}
					}
				}
			case actSigStop:
				p, err := os.FindProcess(os.Getpid())
				if err == nil {
					t.tui.Clear()
					t.tui.Pause(t.fullscreen)
					notifyStop(p)
					t.mutex.Unlock()
					t.reqBox.Set(reqReinit, nil)
					return false
				}
			case actMouse:
				me := event.MouseEvent
				mx, my := me.X, me.Y
				click := !wasDown && me.Down
				clicked := wasDown && !me.Down && (mx == pmx && my == pmy)
				wasDown = me.Down
				if click {
					pmx, pmy = mx, my
				}
				if !me.Down {
					barDragging = false
					pmx, pmy = -1, -1
				}
				if !me.Down || !t.hasPreviewWindow() {
					pbarDragging = false
					pborderDragging = -1
					previewDraggingPos = -1
				}

				// Scrolling
				if me.S != 0 {
					if t.window.Enclose(my, mx) && t.merger.Length() > 0 {
						evt := tui.ScrollUp
						if me.Mod() {
							evt = tui.SScrollUp
						}
						if me.S < 0 {
							evt = tui.ScrollDown
							if me.Mod() {
								evt = tui.SScrollDown
							}
						}
						return doActions(actionsFor(evt))
					} else if t.hasPreviewWindow() && t.pwindow.Enclose(my, mx) {
						evt := tui.PreviewScrollUp
						if me.S < 0 {
							evt = tui.PreviewScrollDown
						}
						return doActions(actionsFor(evt))
					}
					break
				}

				// Preview dragging
				if t.hasPreviewWindow() && me.Down && (previewDraggingPos >= 0 || click && t.pwindow.Enclose(my, mx)) {
					if previewDraggingPos > 0 {
						scrollPreviewBy(previewDraggingPos - my)
					}
					previewDraggingPos = my
					break
				}

				// Preview scrollbar dragging
				headerLines := t.activePreviewOpts.headerLines
				pbarDragging = t.hasPreviewWindow() && me.Down && (pbarDragging || click && my >= t.pwindow.Top()+headerLines && my < t.pwindow.Top()+t.pwindow.Height() && mx == t.pwindow.Left()+t.pwindow.Width())
				if pbarDragging {
					effectiveHeight := t.pwindow.Height() - headerLines
					numLines := len(t.previewer.lines) - headerLines
					barLength, _ := getScrollbar(1, numLines, effectiveHeight, min(numLines-effectiveHeight, t.previewer.offset-headerLines))
					if barLength > 0 {
						y := my - t.pwindow.Top() - headerLines - barLength/2
						y = util.Constrain(y, 0, effectiveHeight-barLength)
						// offset = (total - maxItems) * barStart / (maxItems - barLength)
						t.previewer.offset = headerLines + int(math.Ceil(float64(y)*float64(numLines-effectiveHeight)/float64(effectiveHeight-barLength)))
						t.previewer.following.Set(t.previewer.offset >= numLines-effectiveHeight)
						req(reqPreviewRefresh)
					}
					break
				}

				// Preview border dragging (resizing)
				if t.hasPreviewWindow() && pborderDragging < 0 && click {
					switch t.activePreviewOpts.position {
					case posUp:
						if t.pborder.Enclose(my, mx) && my == t.pborder.Top()+t.pborder.Height()-1 {
							pborderDragging = 0
						} else if t.listBorderShape.HasTop() && t.pborder.EncloseX(mx) && my == t.wborder.Top() {
							pborderDragging = 1
						}
					case posDown:
						if t.pborder.Enclose(my, mx) && my == t.pborder.Top() {
							pborderDragging = 0
						} else if t.listBorderShape.HasBottom() && t.pborder.EncloseX(mx) && my == t.wborder.Top()+t.wborder.Height()-1 {
							pborderDragging = 1
						}
					case posLeft:
						if t.pborder.Enclose(my, mx) && mx == t.pborder.Left()+t.pborder.Width()-1 {
							pborderDragging = 0
						} else if t.listBorderShape.HasLeft() && t.pborder.EncloseY(my) && mx == t.wborder.Left() {
							pborderDragging = 1
						}
					case posRight:
						if t.pborder.Enclose(my, mx) && mx == t.pborder.Left() {
							pborderDragging = 0
						} else if t.listBorderShape.HasRight() && t.pborder.EncloseY(my) && mx == t.wborder.Left()+t.wborder.Width()-1 {
							pborderDragging = 1
						}
					case posNext:
						if t.layout == layoutReverse {
							if t.pborder.Enclose(my, mx) && my == t.pborder.Top()+t.pborder.Height()-1 {
								pborderDragging = 0
							} else if t.listBorderShape.HasTop() && t.pborder.EncloseX(mx) && my == t.wborder.Top() {
								pborderDragging = 1
							}
						} else {
							if t.pborder.Enclose(my, mx) && my == t.pborder.Top() {
								pborderDragging = 0
							} else if t.listBorderShape.HasBottom() && t.pborder.EncloseX(mx) && my == t.wborder.Top()+t.wborder.Height()-1 {
								pborderDragging = 1
							}
						}
					}
				}

				if t.hasPreviewWindow() && pborderDragging >= 0 {
					var newSize int
					var prevSize int
					switch t.activePreviewOpts.position {
					case posLeft:
						prevSize = t.pwindow.Width()
						diff := t.pborder.Width() - prevSize
						newSize = mx - t.pborder.Left() - diff + 1
					case posUp:
						prevSize = t.pwindow.Height()
						diff := t.pborder.Height() - prevSize
						newSize = my - t.pborder.Top() - diff + 1
					case posDown:
						prevSize = t.pwindow.Height()
						offset := my - t.pborder.Top()
						newSize = prevSize - offset
					case posRight:
						prevSize = t.pwindow.Width()
						offset := mx - t.pborder.Left()
						newSize = prevSize - offset
					case posNext:
						prevSize = t.pwindow.Height()
						// In posNext, header/header-lines sections may sit
						// between preview and list. When the list border is
						// dragged (pborderDragging == 1), subtract that gap
						// so the initial click does not jump.
						headerGap := 0
						if pborderDragging == 1 && t.wborder != nil {
							if t.layout == layoutReverse {
								headerGap = t.wborder.Top() - (t.pborder.Top() + t.pborder.Height())
							} else {
								headerGap = t.pborder.Top() - (t.wborder.Top() + t.wborder.Height())
							}
						}
						if t.layout == layoutReverse {
							diff := t.pborder.Height() - prevSize
							newSize = my - t.pborder.Top() - diff + 1 - headerGap
						} else {
							offset := my - t.pborder.Top()
							newSize = prevSize - offset - headerGap
						}
					}
					newSize -= pborderDragging
					if newSize < 1 {
						newSize = 1
					}

					if prevSize == newSize {
						break
					}

					t.activePreviewOpts.size = sizeSpec{float64(newSize), false}
					updatePreviewWindow(true)
					req(reqPreviewRefresh)
					break
				}

				// Inside the input window
				if t.inputWindow != nil && t.inputWindow.Enclose(my, mx) {
					mx -= t.inputWindow.Left()
					my -= t.inputWindow.Top()
					y := t.inputWindow.Height() - 1
					if t.layout == layoutReverse {
						y = 0
					}
					mxCons := util.Constrain(mx-t.promptLen, 0, len(t.input))
					if my == y && mxCons >= 0 {
						t.cx = mxCons + t.xoffset
					}
					break
				}

				// Inside the header window
				if clicked && t.headerVisible && t.headerWindow != nil && t.headerWindow.Enclose(my, mx) {
					mx -= t.headerWindow.Left() + t.headerIndent(t.headerBorderShape)
					my -= t.headerWindow.Top()
					if mx < 0 {
						break
					}
					t.clickHeaderLine = my + 1
					if t.layout != layoutReverse && t.headerLinesWindow != nil {
						t.clickHeaderLine += t.headerLines
					}
					t.clickHeaderColumn = mx + 1
					return doActions(actionsFor(tui.ClickHeader))
				}

				if clicked && t.headerVisible && t.headerLinesWindow != nil && t.headerLinesWindow.Enclose(my, mx) {
					_, shape := t.determineHeaderLinesShape()
					mx -= t.headerLinesWindow.Left() + t.headerIndent(shape)
					my -= t.headerLinesWindow.Top()
					if mx < 0 {
						break
					}
					t.clickHeaderLine = my + 1
					if t.layout == layoutReverse {
						t.clickHeaderLine += len(t.header0)
					}
					t.clickHeaderColumn = mx + 1
					return doActions(actionsFor(tui.ClickHeader))
				}

				// Inside the footer window
				if clicked && t.footerWindow != nil && t.footerWindow.Enclose(my, mx) {
					mx -= t.footerWindow.Left() + t.headerIndent(t.footerBorderShape)
					my -= t.footerWindow.Top()
					if mx < 0 {
						break
					}
					t.clickFooterLine = my + 1
					t.clickFooterColumn = mx + 1
					return doActions(actionsFor(tui.ClickFooter))
				}

				// Ignored
				if !t.window.Enclose(my, mx) && !barDragging {
					break
				}

				// Translate coordinates
				mx -= t.window.Left()
				my -= t.window.Top()
				min := t.promptLines() + t.visibleHeaderLinesInList()
				h := t.window.Height()
				switch t.layout {
				case layoutDefault:
					my = h - my - 1
				case layoutReverseList:
					if my < h-min {
						my += min
					} else {
						my = h - my - 1
					}
				}

				// Scrollbar dragging
				barDragging = me.Down && (barDragging || click && my >= min && mx == t.window.Width()-1)
				if barDragging {
					barLength, barStart := t.getScrollbar()
					if barLength > 0 {
						maxItems := t.maxItems()
						if newBarStart := util.Constrain(my-min-barLength/2, 0, maxItems-barLength); newBarStart != barStart {
							total := t.merger.Length()
							prevOffset := t.offset
							// barStart = (maxItems - barLength) * t.offset / (total - maxItems)
							perLine := t.avgNumLines()
							t.offset = int(math.Ceil(float64(newBarStart) * float64(total*perLine-maxItems) / float64(maxItems*perLine-barLength)))
							t.cy = t.offset + t.cy - prevOffset
							req(reqList)
						}
					}
					break
				}

				// There can be empty lines after the list in multi-line mode
				prevLine := t.prevLines[my]
				if prevLine.empty {
					break
				}

				// Double-click on an item
				cy := prevLine.cy
				if me.Double && mx < t.window.Width()-1 {
					// Double-click
					if my >= min {
						if t.vset(cy) && t.cy < t.merger.Length() {
							return doActions(actionsFor(tui.DoubleClick))
						}
					}
				}

				if me.Down {
					mxCons := util.Constrain(mx-t.promptLen, 0, len(t.input))
					if !t.inputless && t.inputWindow == nil && my == 0 && mxCons >= 0 {
						// Prompt
						t.cx = mxCons + t.xoffset
					} else if my >= min {
						t.vset(cy)
						req(reqList)
						evt := tui.RightClick
						if me.Mod() {
							evt = tui.SRightClick
						}
						if me.Left {
							evt = tui.LeftClick
							if me.Mod() {
								evt = tui.SLeftClick
							}
						}
						return doActions(actionsFor(evt))
					}
				}
				if clicked && t.headerVisible && t.headerWindow == nil {
					// Header
					numLines := t.visibleHeaderLinesInList()
					lineOffset := 0
					if !t.inputless && t.inputWindow == nil && !t.headerFirst {
						// offset for info line
						if t.noSeparatorLine() {
							lineOffset = 1
						} else {
							lineOffset = 2
						}
					}
					my -= lineOffset
					mx -= t.pointerLen + t.markerLen
					if my >= 0 && my < numLines && mx >= 0 {
						if t.layout == layoutReverse {
							t.clickHeaderLine = my + 1
						} else {
							t.clickHeaderLine = numLines - my
						}
						t.clickHeaderColumn = mx + 1
						return doActions(actionsFor(tui.ClickHeader))
					}
				}
			case actReload, actReloadSync:
				t.failed = nil

				valid, list := t.buildPlusList(a.a, false)
				if !valid {
					// We run the command even when there's no match
					// 1. If the template doesn't have any slots
					// 2. If the template has {q}
					slot, _, _, forceUpdate := hasPreviewFlags(a.a)
					valid = !slot || forceUpdate
				}
				if valid {
					command, tempFiles := t.replacePlaceholder(a.a, false, string(t.input), list)
					newCommand = &commandSpec{command, tempFiles}
					reloadSync = a.t == actReloadSync
					t.reading = true

					if len(t.idNth) > 0 {
						t.trackSync = reloadSync
					}
					// Capture tracking key before reload
					if !t.track.Disabled() && len(t.idNth) > 0 {
						if item := t.currentItem(); item != nil {
							t.trackKey = t.trackKeyFor(item, t.idNth)
							t.trackKeyCache = make(map[int32]bool)
							t.trackBlocked = true
							if !t.inputless {
								t.tui.HideCursor()
							}
							req(reqPrompt, reqInfo)
						}
					}
				}
			case actUnbind:
				if keys, _, err := parseKeyChords(a.a, "PANIC"); err == nil {
					for key := range keys {
						delete(t.keymap, key)
					}
				}
			case actRebind:
				if keys, _, err := parseKeyChords(a.a, "PANIC"); err == nil {
					for key := range keys {
						if originalAction, found := t.keymapOrg[key]; found {
							t.keymap[key] = originalAction
						}
					}
				}
			case actToggleBind:
				if keys, _, err := parseKeyChords(a.a, "PANIC"); err == nil {
					for key := range keys {
						if _, bound := t.keymap[key]; bound {
							delete(t.keymap, key)
						} else if originalAction, found := t.keymapOrg[key]; found {
							t.keymap[key] = originalAction
						}
					}
				}
			case actChangeGhost, actTransformGhost, actBgTransformGhost:
				capture(true, func(ghost string) {
					t.ghost = ghost
					if len(t.input) == 0 {
						req(reqPrompt)
					}
				})
			case actChangePointer, actTransformPointer, actBgTransformPointer:
				capture(true, func(pointer string) {
					length := uniseg.StringWidth(pointer)
					if length <= 2 {
						if length != t.pointerLen {
							t.forceRerenderList()
						}
						t.pointer = pointer
						t.pointerLen = length
						t.pointerEmpty = strings.Repeat(" ", t.pointerLen)
						req(reqList)
					}
				})
			case actChangePreview:
				if t.previewOpts.command != a.a {
					t.previewOpts.command = a.a
					updatePreviewWindow(false)
					refreshPreview(t.previewOpts.command)
				}
			case actChangePreviewWindow:
				// NOTE: We intentionally use "previewOpts" instead of "activePreviewOpts" here
				currentPreviewOpts := t.previewOpts

				// Reset preview options and apply the additional options
				t.previewOpts = t.initialPreviewOpts
				t.previewOpts.command = currentPreviewOpts.command

				// Carry over toggle-driven state so toggle-preview-wrap survives
				// a change-preview-window. Tokens below can still override.
				t.previewOpts.wrap = currentPreviewOpts.wrap
				t.previewOpts.wrapWord = currentPreviewOpts.wrapWord

				// Split window options
				tokens := strings.Split(a.a, "|")
				if len(tokens[0]) > 0 && t.initialPreviewOpts.hidden {
					t.previewOpts.hidden = false
				}
				parsePreviewWindow(&t.previewOpts, tokens[0])
				if len(tokens) > 1 {
					a.a = strings.Join(append(tokens[1:], tokens[0]), "|")
				}

				// Full redraw
				switch currentPreviewOpts.compare(t.activePreviewOpts, &t.previewOpts) {
				case previewOptsDifferentLayout:
					// Preview command can be running in the background if the size of
					// the preview window is 0 but not 'hidden'
					wasHidden := currentPreviewOpts.hidden

					// FIXME: One-time preview window can't reappear once hidden
					// fzf --bind space:preview:ls --bind 'enter:change-preview-window:down|left|up|hidden|'
					updatePreviewWindow(t.hasPreviewWindow() && !t.activePreviewOpts.hidden)
					if wasHidden && t.hasPreviewWindow() {
						// Restart
						refreshPreview(t.previewOpts.command)
					} else if t.activePreviewOpts.hidden {
						// Cancel
						t.cancelPreview()
					} else {
						// Refresh
						req(reqPreviewRefresh)
					}
				case previewOptsDifferentContentLayout:
					t.previewed.version = 0
					req(reqPreviewRefresh)
				}

				// Adjust scroll offset
				if t.hasPreviewWindow() && currentPreviewOpts.scroll != t.activePreviewOpts.scroll {
					scrollPreviewTo(t.evaluateScrollOffset())
				}

				// Resume following
				t.previewer.following.Force(t.previewOpts.follow)
			case actNextSelected, actPrevSelected:
				if len(t.selected) > 0 {
					total := t.merger.Length()
					for i := 1; i < total; i++ {
						y := (t.cy + i) % total
						if t.layout == layoutDefault && a.t == actNextSelected ||
							t.layout != layoutDefault && a.t == actPrevSelected {
							y = (t.cy - i + total) % total
						}
						if _, found := t.selected[t.merger.Get(y).item.Index()]; found {
							t.vset(y)
							req(reqList)
							break
						}
					}
				}
			}

			if !processExecution(a.t) {
				t.lastAction = a.t
			}

			if t.inputless {
				// Always just discard the change
				t.input = currentInput
				t.cx = len(t.input)
				beof = false
			} else if string(t.input) != string(currentInput) {
				t.inputOverride = nil
			}
			return true
		}

		if t.jumping == jumpDisabled || len(actions) > 0 {
			// Break out of jump mode if any action is submitted to the server
			if t.jumping != jumpDisabled {
				t.jumping = jumpDisabled
				if acts, prs := t.keymap[tui.JumpCancel.AsEvent()]; prs && !doActions(acts) {
					continue
				}
				req(reqList)
			}
			if len(actions) == 0 {
				actions = t.keymap[event.Comparable()]
			}
			if len(actions) == 0 && event.Type == tui.Rune {
				doAction(&action{t: actChar})
			} else if !doActions(actions) {
				continue
			}
			if !t.inputless {
				t.truncateQuery()
			}
			queryChanged = queryChanged || t.pasting == nil && string(previousInput) != string(t.input)
			changed = changed || queryChanged
			if onChanges, prs := t.keymap[tui.Change.AsEvent()]; queryChanged && prs && !doActions(onChanges) {
				continue
			}
			if onEOFs, prs := t.keymap[tui.BackwardEOF.AsEvent()]; beof && prs && !doActions(onEOFs) {
				continue
			}
			if onMultis, prs := t.keymap[tui.Multi.AsEvent()]; t.version != previousVersion && prs && !doActions(onMultis) {
				continue
			}
		} else {
			jumpEvent := tui.JumpCancel
			if event.Type == tui.Rune {
				if idx := strings.IndexRune(t.jumpLabels, event.Char); idx >= 0 && idx < t.maxItems() && idx < t.merger.Length() {
					jumpEvent = tui.Jump
					t.cy = idx + t.offset
					if t.jumping == jumpAcceptEnabled {
						req(reqClose)
					}
				}
			}
			t.jumping = jumpDisabled
			if acts, prs := t.keymap[jumpEvent.AsEvent()]; prs && !doActions(acts) {
				continue
			}
			req(reqList)
		}

		if queryChanged && t.canPreview() && len(t.previewOpts.command) > 0 {
			_, _, _, forceUpdate := hasPreviewFlags(t.previewOpts.command)
			if forceUpdate {
				t.version++
			}
		}

		if queryChanged || t.cx != previousCx {
			req(reqPrompt)
		}

		reload := changed || newCommand != nil
		var reloadRequest *searchRequest
		if reload {
			reloadRequest = &searchRequest{sort: t.sort, sync: reloadSync, nth: newNth, withNth: newWithNth, headerLines: newHeaderLines, command: newCommand, environ: t.environ(), changed: changed, denylist: denylist, revision: t.resultMerger.Revision()}
		}

		// Dispatch queued background requests
		t.dispatchAsync()

		if t.pendingReqList {
			t.pendingReqList = false
			req(reqList)
		}

		t.mutex.Unlock() // Must be unlocked before touching reqBox

		if reload {
			t.eventBox.Set(EvtSearchNew, *reloadRequest)
		}
		for _, event := range events {
			t.reqBox.Set(event, nil)
		}
	}
	return nil
}

// https://github.com/go-gorm/gorm/blob/d0ee5e2296150d691364c5f4f7f2b32abde04545/schema/field.go#L451-L990
// go-gorm/gorm schema/field.go:451-990
func (field *Field) setupValuerAndSetter(modelType reflect.Type) {
	// Setup NewValuePool
	field.setupNewValuePool()

	// ValueOf returns field's value and if it is zero
	fieldIndex := field.StructField.Index[0]
	switch {
	case len(field.StructField.Index) == 1 && fieldIndex >= 0:
		field.ValueOf = func(ctx context.Context, v reflect.Value) (interface{}, bool) {
			v = reflect.Indirect(v)
			fieldValue := v.Field(fieldIndex)
			return fieldValue.Interface(), fieldValue.IsZero()
		}
	default:
		field.ValueOf = func(ctx context.Context, v reflect.Value) (interface{}, bool) {
			v = reflect.Indirect(v)
			for _, fieldIdx := range field.StructField.Index {
				if fieldIdx >= 0 {
					v = v.Field(fieldIdx)
				} else {
					v = v.Field(-fieldIdx - 1)

					if !v.IsNil() {
						v = v.Elem()
					} else {
						return nil, true
					}
				}
			}

			fv, zero := v.Interface(), v.IsZero()
			return fv, zero
		}
	}

	if field.Serializer != nil {
		oldValuerOf := field.ValueOf
		field.ValueOf = func(ctx context.Context, v reflect.Value) (interface{}, bool) {
			value, zero := oldValuerOf(ctx, v)

			s, ok := value.(SerializerValuerInterface)
			if !ok {
				s = field.Serializer
			}

			return &serializer{
				Field:           field,
				SerializeValuer: s,
				Destination:     v,
				Context:         ctx,
				fieldValue:      value,
			}, zero
		}
	}

	// ReflectValueOf returns field's reflect value
	switch {
	case len(field.StructField.Index) == 1 && fieldIndex >= 0:
		field.ReflectValueOf = func(ctx context.Context, v reflect.Value) reflect.Value {
			v = reflect.Indirect(v)
			return v.Field(fieldIndex)
		}
	default:
		field.ReflectValueOf = func(ctx context.Context, v reflect.Value) reflect.Value {
			v = reflect.Indirect(v)
			for idx, fieldIdx := range field.StructField.Index {
				if fieldIdx >= 0 {
					v = v.Field(fieldIdx)
				} else {
					v = v.Field(-fieldIdx - 1)

					if v.IsNil() {
						v.Set(reflect.New(v.Type().Elem()))
					}

					if idx < len(field.StructField.Index)-1 {
						v = v.Elem()
					}
				}
			}
			return v
		}
	}

	fallbackSetter := func(ctx context.Context, value reflect.Value, v interface{}, setter func(context.Context, reflect.Value, interface{}) error) (err error) {
		if v == nil {
			field.ReflectValueOf(ctx, value).Set(reflect.New(field.FieldType).Elem())
		} else {
			reflectV := reflect.ValueOf(v)
			// Optimal value type acquisition for v
			reflectValType := reflectV.Type()

			if reflectValType.AssignableTo(field.FieldType) {
				if reflectV.Kind() == reflect.Ptr && reflectV.Elem().Kind() == reflect.Ptr {
					reflectV = reflect.Indirect(reflectV)
				}
				field.ReflectValueOf(ctx, value).Set(reflectV)
				return
			} else if reflectValType.ConvertibleTo(field.FieldType) {
				field.ReflectValueOf(ctx, value).Set(reflectV.Convert(field.FieldType))
				return
			} else if field.FieldType.Kind() == reflect.Ptr {
				fieldValue := field.ReflectValueOf(ctx, value)
				fieldType := field.FieldType.Elem()

				if reflectValType.AssignableTo(fieldType) {
					if !fieldValue.IsValid() {
						fieldValue = reflect.New(fieldType)
					} else if fieldValue.IsNil() {
						fieldValue.Set(reflect.New(fieldType))
					}
					fieldValue.Elem().Set(reflectV)
					return
				} else if reflectValType.ConvertibleTo(fieldType) {
					if fieldValue.IsNil() {
						fieldValue.Set(reflect.New(fieldType))
					}

					fieldValue.Elem().Set(reflectV.Convert(fieldType))
					return
				}
			}

			if reflectV.Kind() == reflect.Ptr {
				if reflectV.IsNil() {
					field.ReflectValueOf(ctx, value).Set(reflect.New(field.FieldType).Elem())
				} else if reflectV.Type().Elem().AssignableTo(field.FieldType) {
					field.ReflectValueOf(ctx, value).Set(reflectV.Elem())
					return
				} else {
					err = setter(ctx, value, reflectV.Elem().Interface())
				}
			} else if valuer, ok := v.(driver.Valuer); ok {
				if v, err = valuer.Value(); err == nil {
					err = setter(ctx, value, v)
				}
			} else if _, ok := v.(clause.Expr); !ok {
				return fmt.Errorf("failed to set value %#v to field %s", v, field.Name)
			}
		}

		return
	}

	// Set
	switch field.FieldType.Kind() {
	case reflect.Bool:
		field.Set = func(ctx context.Context, value reflect.Value, v interface{}) error {
			switch data := v.(type) {
			case **bool:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetBool(**data)
				}
			case bool:
				field.ReflectValueOf(ctx, value).SetBool(data)
			case int64:
				field.ReflectValueOf(ctx, value).SetBool(data > 0)
			case string:
				b, _ := strconv.ParseBool(data)
				field.ReflectValueOf(ctx, value).SetBool(b)
			default:
				return fallbackSetter(ctx, value, v, field.Set)
			}
			return nil
		}
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		field.Set = func(ctx context.Context, value reflect.Value, v interface{}) (err error) {
			switch data := v.(type) {
			case **int64:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetInt(**data)
				}
			case **int:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetInt(int64(**data))
				}
			case **int8:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetInt(int64(**data))
				}
			case **int16:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetInt(int64(**data))
				}
			case **int32:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetInt(int64(**data))
				}
			case int64:
				field.ReflectValueOf(ctx, value).SetInt(data)
			case int:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case int8:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case int16:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case int32:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case uint:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case uint8:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case uint16:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case uint32:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case uint64:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case float32:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case float64:
				field.ReflectValueOf(ctx, value).SetInt(int64(data))
			case []byte:
				return field.Set(ctx, value, string(data))
			case string:
				if i, err := strconv.ParseInt(data, 0, 64); err == nil {
					field.ReflectValueOf(ctx, value).SetInt(i)
				} else {
					return err
				}
			case time.Time:
				if field.AutoCreateTime == UnixNanosecond || field.AutoUpdateTime == UnixNanosecond {
					field.ReflectValueOf(ctx, value).SetInt(data.UnixNano())
				} else if field.AutoCreateTime == UnixMillisecond || field.AutoUpdateTime == UnixMillisecond {
					field.ReflectValueOf(ctx, value).SetInt(data.UnixMilli())
				} else {
					field.ReflectValueOf(ctx, value).SetInt(data.Unix())
				}
			case *time.Time:
				if data != nil {
					if field.AutoCreateTime == UnixNanosecond || field.AutoUpdateTime == UnixNanosecond {
						field.ReflectValueOf(ctx, value).SetInt(data.UnixNano())
					} else if field.AutoCreateTime == UnixMillisecond || field.AutoUpdateTime == UnixMillisecond {
						field.ReflectValueOf(ctx, value).SetInt(data.UnixMilli())
					} else {
						field.ReflectValueOf(ctx, value).SetInt(data.Unix())
					}
				} else {
					field.ReflectValueOf(ctx, value).SetInt(0)
				}
			default:
				return fallbackSetter(ctx, value, v, field.Set)
			}
			return err
		}
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
		field.Set = func(ctx context.Context, value reflect.Value, v interface{}) (err error) {
			switch data := v.(type) {
			case **uint64:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetUint(**data)
				}
			case **uint:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetUint(uint64(**data))
				}
			case **uint8:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetUint(uint64(**data))
				}
			case **uint16:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetUint(uint64(**data))
				}
			case **uint32:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetUint(uint64(**data))
				}
			case uint64:
				field.ReflectValueOf(ctx, value).SetUint(data)
			case uint:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case uint8:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case uint16:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case uint32:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case int64:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case int:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case int8:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case int16:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case int32:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case float32:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case float64:
				field.ReflectValueOf(ctx, value).SetUint(uint64(data))
			case []byte:
				return field.Set(ctx, value, string(data))
			case time.Time:
				if field.AutoCreateTime == UnixNanosecond || field.AutoUpdateTime == UnixNanosecond {
					field.ReflectValueOf(ctx, value).SetUint(uint64(data.UnixNano()))
				} else if field.AutoCreateTime == UnixMillisecond || field.AutoUpdateTime == UnixMillisecond {
					field.ReflectValueOf(ctx, value).SetUint(uint64(data.UnixMilli()))
				} else {
					field.ReflectValueOf(ctx, value).SetUint(uint64(data.Unix()))
				}
			case string:
				if i, err := strconv.ParseUint(data, 0, 64); err == nil {
					field.ReflectValueOf(ctx, value).SetUint(i)
				} else {
					return err
				}
			default:
				return fallbackSetter(ctx, value, v, field.Set)
			}
			return err
		}
	case reflect.Float32, reflect.Float64:
		field.Set = func(ctx context.Context, value reflect.Value, v interface{}) (err error) {
			switch data := v.(type) {
			case **float64:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetFloat(**data)
				}
			case **float32:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetFloat(float64(**data))
				}
			case float64:
				field.ReflectValueOf(ctx, value).SetFloat(data)
			case float32:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case int64:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case int:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case int8:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case int16:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case int32:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case uint:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case uint8:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case uint16:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case uint32:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case uint64:
				field.ReflectValueOf(ctx, value).SetFloat(float64(data))
			case []byte:
				return field.Set(ctx, value, string(data))
			case string:
				if i, err := strconv.ParseFloat(data, 64); err == nil {
					field.ReflectValueOf(ctx, value).SetFloat(i)
				} else {
					return err
				}
			default:
				return fallbackSetter(ctx, value, v, field.Set)
			}
			return err
		}
	case reflect.String:
		field.Set = func(ctx context.Context, value reflect.Value, v interface{}) (err error) {
			switch data := v.(type) {
			case **string:
				if data != nil && *data != nil {
					field.ReflectValueOf(ctx, value).SetString(**data)
				}
			case string:
				field.ReflectValueOf(ctx, value).SetString(data)
			case []byte:
				field.ReflectValueOf(ctx, value).SetString(string(data))
			case int, int8, int16, int32, int64, uint, uint8, uint16, uint32, uint64:
				field.ReflectValueOf(ctx, value).SetString(utils.ToString(data))
			case float64, float32:
				field.ReflectValueOf(ctx, value).SetString(fmt.Sprintf("%."+strconv.Itoa(field.Precision)+"f", data))
			default:
				return fallbackSetter(ctx, value, v, field.Set)
			}
			return err
		}
	default:
		fieldValue := reflect.New(field.FieldType)
		switch fieldValue.Elem().Interface().(type) {
		case time.Time:
			field.Set = func(ctx context.Context, value reflect.Value, v interface{}) error {
				switch data := v.(type) {
				case **time.Time:
					if data != nil && *data != nil {
						field.Set(ctx, value, *data)
					}
				case time.Time:
					field.ReflectValueOf(ctx, value).Set(reflect.ValueOf(v))
				case *time.Time:
					if data != nil {
						field.ReflectValueOf(ctx, value).Set(reflect.ValueOf(data).Elem())
					} else {
						field.ReflectValueOf(ctx, value).Set(reflect.ValueOf(time.Time{}))
					}
				case string:
					if t, err := now.Parse(data); err == nil {
						field.ReflectValueOf(ctx, value).Set(reflect.ValueOf(t))
					} else {
						return fmt.Errorf("failed to set string %v to time.Time field %s, failed to parse it as time, got error %v", v, field.Name, err)
					}
				default:
					return fallbackSetter(ctx, value, v, field.Set)
				}
				return nil
			}
		case *time.Time:
			field.Set = func(ctx context.Context, value reflect.Value, v interface{}) error {
				switch data := v.(type) {
				case **time.Time:
					if data != nil && *data != nil {
						field.ReflectValueOf(ctx, value).Set(reflect.ValueOf(*data))
					}
				case time.Time:
					fieldValue := field.ReflectValueOf(ctx, value)
					if fieldValue.IsNil() {
						fieldValue.Set(reflect.New(field.FieldType.Elem()))
					}
					fieldValue.Elem().Set(reflect.ValueOf(v))
				case *time.Time:
					field.ReflectValueOf(ctx, value).Set(reflect.ValueOf(v))
				case string:
					if t, err := now.Parse(data); err == nil {
						fieldValue := field.ReflectValueOf(ctx, value)
						if fieldValue.IsNil() {
							if v == "" {
								return nil
							}
							fieldValue.Set(reflect.New(field.FieldType.Elem()))
						}
						fieldValue.Elem().Set(reflect.ValueOf(t))
					} else {
						return fmt.Errorf("failed to set string %v to time.Time field %s, failed to parse it as time, got error %v", v, field.Name, err)
					}
				default:
					return fallbackSetter(ctx, value, v, field.Set)
				}
				return nil
			}
		default:
			if _, ok := fieldValue.Elem().Interface().(sql.Scanner); ok {
				// pointer scanner
				field.Set = func(ctx context.Context, value reflect.Value, v interface{}) (err error) {
					reflectV := reflect.ValueOf(v)
					if !reflectV.IsValid() {
						field.ReflectValueOf(ctx, value).Set(reflect.New(field.FieldType).Elem())
					} else if reflectV.Kind() == reflect.Ptr && reflectV.IsNil() {
						return
					} else if reflectV.Type().AssignableTo(field.FieldType) {
						field.ReflectValueOf(ctx, value).Set(reflectV)
					} else if reflectV.Kind() == reflect.Ptr {
						return field.Set(ctx, value, reflectV.Elem().Interface())
					} else {
						fieldValue := field.ReflectValueOf(ctx, value)
						if fieldValue.IsNil() {
							fieldValue.Set(reflect.New(field.FieldType.Elem()))
						}

						if valuer, ok := v.(driver.Valuer); ok {
							v, _ = valuer.Value()
						}

						err = fieldValue.Interface().(sql.Scanner).Scan(v)
					}
					return
				}
			} else if _, ok := fieldValue.Interface().(sql.Scanner); ok {
				// struct scanner
				field.Set = func(ctx context.Context, value reflect.Value, v interface{}) (err error) {
					reflectV := reflect.ValueOf(v)
					if !reflectV.IsValid() {
						field.ReflectValueOf(ctx, value).Set(reflect.New(field.FieldType).Elem())
					} else if reflectV.Kind() == reflect.Ptr && reflectV.IsNil() {
						return
					} else if reflectV.Type().AssignableTo(field.FieldType) {
						field.ReflectValueOf(ctx, value).Set(reflectV)
					} else if reflectV.Kind() == reflect.Ptr {
						return field.Set(ctx, value, reflectV.Elem().Interface())
					} else {
						if valuer, ok := v.(driver.Valuer); ok {
							v, _ = valuer.Value()
						}

						err = field.ReflectValueOf(ctx, value).Addr().Interface().(sql.Scanner).Scan(v)
					}
					return
				}
			} else {
				field.Set = func(ctx context.Context, value reflect.Value, v interface{}) (err error) {
					return fallbackSetter(ctx, value, v, field.Set)
				}
			}
		}
	}

	if field.Serializer != nil {
		var (
			oldFieldSetter = field.Set
			sameElemType   bool
			sameType       = field.FieldType == reflect.ValueOf(field.Serializer).Type()
		)

		if reflect.ValueOf(field.Serializer).Kind() == reflect.Ptr {
			sameElemType = field.FieldType == reflect.ValueOf(field.Serializer).Type().Elem()
		}

		serializerValue := reflect.Indirect(reflect.ValueOf(field.Serializer))
		serializerType := serializerValue.Type()
		field.Set = func(ctx context.Context, value reflect.Value, v interface{}) (err error) {
			if s, ok := v.(*serializer); ok {
				if s.fieldValue == nil && s.Serializer == nil {
					rv := field.ReflectValueOf(ctx, value)
					if rv.IsValid() && rv.CanSet() {
						rv.Set(reflect.Zero(field.FieldType))
					}
					return nil
				}
				if s.fieldValue != nil {
					err = oldFieldSetter(ctx, value, s.fieldValue)
				} else if err = s.Serializer.Scan(ctx, field, value, s.value); err == nil {
					if sameElemType {
						field.ReflectValueOf(ctx, value).Set(reflect.ValueOf(s.Serializer).Elem())
					} else if sameType {
						field.ReflectValueOf(ctx, value).Set(reflect.ValueOf(s.Serializer))
					}
					si := reflect.New(serializerType)
					si.Elem().Set(serializerValue)
					s.Serializer = si.Interface().(SerializerInterface)
				}
			} else {
				err = oldFieldSetter(ctx, value, v)
			}
			return
		}
	}
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/internal/translator/gemini/openai/responses/gemini_openai-responses_response.go#L92-L568
// router-for-me/CLIProxyAPI internal/translator/gemini/openai/responses/gemini_openai-responses_response.go:92-568
func ConvertGeminiResponseToOpenAIResponses(_ context.Context, modelName string, originalRequestRawJSON, requestRawJSON, rawJSON []byte, param *any) [][]byte {
	if *param == nil {
		*param = &geminiToResponsesState{
			FuncArgsBuf:      make(map[int]*strings.Builder),
			FuncNames:        make(map[int]string),
			FuncCallIDs:      make(map[int]string),
			FuncDone:         make(map[int]bool),
			SanitizedNameMap: util.SanitizedToolNameMap(originalRequestRawJSON),
		}
	}
	st := (*param).(*geminiToResponsesState)
	if st.FuncArgsBuf == nil {
		st.FuncArgsBuf = make(map[int]*strings.Builder)
	}
	if st.FuncNames == nil {
		st.FuncNames = make(map[int]string)
	}
	if st.FuncCallIDs == nil {
		st.FuncCallIDs = make(map[int]string)
	}
	if st.FuncDone == nil {
		st.FuncDone = make(map[int]bool)
	}
	if st.SanitizedNameMap == nil {
		st.SanitizedNameMap = util.SanitizedToolNameMap(originalRequestRawJSON)
	}

	if bytes.HasPrefix(rawJSON, []byte("data:")) {
		rawJSON = bytes.TrimSpace(rawJSON[5:])
	}

	rawJSON = bytes.TrimSpace(rawJSON)
	if len(rawJSON) == 0 || bytes.Equal(rawJSON, []byte("[DONE]")) {
		return [][]byte{}
	}

	root := gjson.ParseBytes(rawJSON)
	if !root.Exists() {
		return [][]byte{}
	}
	root = unwrapGeminiResponseRoot(root)

	var out [][]byte
	nextSeq := func() int { st.Seq++; return st.Seq }

	// Helper to finalize reasoning summary events in correct order.
	// It emits response.reasoning_summary_text.done followed by
	// response.reasoning_summary_part.done exactly once.
	finalizeReasoning := func() {
		if !st.ReasoningOpened || st.ReasoningClosed {
			return
		}
		full := st.ReasoningBuf.String()
		textDone := []byte(`{"type":"response.reasoning_summary_text.done","sequence_number":0,"item_id":"","output_index":0,"summary_index":0,"text":""}`)
		textDone, _ = sjson.SetBytes(textDone, "sequence_number", nextSeq())
		textDone, _ = sjson.SetBytes(textDone, "item_id", st.ReasoningItemID)
		textDone, _ = sjson.SetBytes(textDone, "output_index", st.ReasoningIndex)
		textDone, _ = sjson.SetBytes(textDone, "text", full)
		out = append(out, emitEvent("response.reasoning_summary_text.done", textDone))

		partDone := []byte(`{"type":"response.reasoning_summary_part.done","sequence_number":0,"item_id":"","output_index":0,"summary_index":0,"part":{"type":"summary_text","text":""}}`)
		partDone, _ = sjson.SetBytes(partDone, "sequence_number", nextSeq())
		partDone, _ = sjson.SetBytes(partDone, "item_id", st.ReasoningItemID)
		partDone, _ = sjson.SetBytes(partDone, "output_index", st.ReasoningIndex)
		partDone, _ = sjson.SetBytes(partDone, "part.text", full)
		out = append(out, emitEvent("response.reasoning_summary_part.done", partDone))

		itemDone := []byte(`{"type":"response.output_item.done","sequence_number":0,"output_index":0,"item":{"id":"","type":"reasoning","encrypted_content":"","summary":[{"type":"summary_text","text":""}]}}`)
		itemDone, _ = sjson.SetBytes(itemDone, "sequence_number", nextSeq())
		itemDone, _ = sjson.SetBytes(itemDone, "item.id", st.ReasoningItemID)
		itemDone, _ = sjson.SetBytes(itemDone, "output_index", st.ReasoningIndex)
		itemDone, _ = sjson.SetBytes(itemDone, "item.encrypted_content", st.ReasoningEnc)
		itemDone, _ = sjson.SetBytes(itemDone, "item.summary.0.text", full)
		out = append(out, emitEvent("response.output_item.done", itemDone))

		st.ReasoningClosed = true
	}

	// Helper to finalize the assistant message in correct order.
	// It emits response.output_text.done, response.content_part.done,
	// and response.output_item.done exactly once.
	finalizeMessage := func() {
		if !st.MsgOpened || st.MsgClosed {
			return
		}
		fullText := st.ItemTextBuf.String()
		done := []byte(`{"type":"response.output_text.done","sequence_number":0,"item_id":"","output_index":0,"content_index":0,"text":"","logprobs":[]}`)
		done, _ = sjson.SetBytes(done, "sequence_number", nextSeq())
		done, _ = sjson.SetBytes(done, "item_id", st.CurrentMsgID)
		done, _ = sjson.SetBytes(done, "output_index", st.MsgIndex)
		done, _ = sjson.SetBytes(done, "text", fullText)
		out = append(out, emitEvent("response.output_text.done", done))
		partDone := []byte(`{"type":"response.content_part.done","sequence_number":0,"item_id":"","output_index":0,"content_index":0,"part":{"type":"output_text","annotations":[],"logprobs":[],"text":""}}`)
		partDone, _ = sjson.SetBytes(partDone, "sequence_number", nextSeq())
		partDone, _ = sjson.SetBytes(partDone, "item_id", st.CurrentMsgID)
		partDone, _ = sjson.SetBytes(partDone, "output_index", st.MsgIndex)
		partDone, _ = sjson.SetBytes(partDone, "part.text", fullText)
		out = append(out, emitEvent("response.content_part.done", partDone))
		final := []byte(`{"type":"response.output_item.done","sequence_number":0,"output_index":0,"item":{"id":"","type":"message","status":"completed","content":[{"type":"output_text","text":""}],"role":"assistant"}}`)
		final, _ = sjson.SetBytes(final, "sequence_number", nextSeq())
		final, _ = sjson.SetBytes(final, "output_index", st.MsgIndex)
		final, _ = sjson.SetBytes(final, "item.id", st.CurrentMsgID)
		final, _ = sjson.SetBytes(final, "item.content.0.text", fullText)
		out = append(out, emitEvent("response.output_item.done", final))

		st.MsgClosed = true
	}

	// Initialize per-response fields and emit created/in_progress once
	if !st.Started {
		st.ResponseID = root.Get("responseId").String()
		if st.ResponseID == "" {
			st.ResponseID = fmt.Sprintf("resp_%x_%d", time.Now().UnixNano(), atomic.AddUint64(&responseIDCounter, 1))
		}
		if !strings.HasPrefix(st.ResponseID, "resp_") {
			st.ResponseID = fmt.Sprintf("resp_%s", st.ResponseID)
		}
		if v := root.Get("createTime"); v.Exists() {
			if t, errParseCreateTime := time.Parse(time.RFC3339Nano, v.String()); errParseCreateTime == nil {
				st.CreatedAt = t.Unix()
			}
		}
		if st.CreatedAt == 0 {
			st.CreatedAt = time.Now().Unix()
		}

		created := []byte(`{"type":"response.created","sequence_number":0,"response":{"id":"","object":"response","created_at":0,"status":"in_progress","background":false,"error":null,"output":[]}}`)
		created, _ = sjson.SetBytes(created, "sequence_number", nextSeq())
		created, _ = sjson.SetBytes(created, "response.id", st.ResponseID)
		created, _ = sjson.SetBytes(created, "response.created_at", st.CreatedAt)
		out = append(out, emitEvent("response.created", created))

		inprog := []byte(`{"type":"response.in_progress","sequence_number":0,"response":{"id":"","object":"response","created_at":0,"status":"in_progress"}}`)
		inprog, _ = sjson.SetBytes(inprog, "sequence_number", nextSeq())
		inprog, _ = sjson.SetBytes(inprog, "response.id", st.ResponseID)
		inprog, _ = sjson.SetBytes(inprog, "response.created_at", st.CreatedAt)
		out = append(out, emitEvent("response.in_progress", inprog))

		st.Started = true
		st.NextIndex = 0
	}

	// Handle parts (text/thought/functionCall)
	if parts := root.Get("candidates.0.content.parts"); parts.Exists() && parts.IsArray() {
		parts.ForEach(func(_, part gjson.Result) bool {
			// Reasoning text
			if part.Get("thought").Bool() {
				if st.ReasoningClosed {
					// Ignore any late thought chunks after reasoning is finalized.
					return true
				}
				if sig := part.Get("thoughtSignature"); sig.Exists() && sig.String() != "" && sig.String() != geminiResponsesThoughtSignature {
					st.ReasoningEnc = sig.String()
				} else if sig = part.Get("thought_signature"); sig.Exists() && sig.String() != "" && sig.String() != geminiResponsesThoughtSignature {
					st.ReasoningEnc = sig.String()
				}
				if !st.ReasoningOpened {
					st.ReasoningOpened = true
					st.ReasoningIndex = st.NextIndex
					st.NextIndex++
					st.ReasoningItemID = fmt.Sprintf("rs_%s_%d", st.ResponseID, st.ReasoningIndex)
					item := []byte(`{"type":"response.output_item.added","sequence_number":0,"output_index":0,"item":{"id":"","type":"reasoning","status":"in_progress","encrypted_content":"","summary":[]}}`)
					item, _ = sjson.SetBytes(item, "sequence_number", nextSeq())
					item, _ = sjson.SetBytes(item, "output_index", st.ReasoningIndex)
					item, _ = sjson.SetBytes(item, "item.id", st.ReasoningItemID)
					item, _ = sjson.SetBytes(item, "item.encrypted_content", st.ReasoningEnc)
					out = append(out, emitEvent("response.output_item.added", item))
					partAdded := []byte(`{"type":"response.reasoning_summary_part.added","sequence_number":0,"item_id":"","output_index":0,"summary_index":0,"part":{"type":"summary_text","text":""}}`)
					partAdded, _ = sjson.SetBytes(partAdded, "sequence_number", nextSeq())
					partAdded, _ = sjson.SetBytes(partAdded, "item_id", st.ReasoningItemID)
					partAdded, _ = sjson.SetBytes(partAdded, "output_index", st.ReasoningIndex)
					out = append(out, emitEvent("response.reasoning_summary_part.added", partAdded))
				}
				if t := part.Get("text"); t.Exists() && t.String() != "" {
					st.ReasoningBuf.WriteString(t.String())
					msg := []byte(`{"type":"response.reasoning_summary_text.delta","sequence_number":0,"item_id":"","output_index":0,"summary_index":0,"delta":""}`)
					msg, _ = sjson.SetBytes(msg, "sequence_number", nextSeq())
					msg, _ = sjson.SetBytes(msg, "item_id", st.ReasoningItemID)
					msg, _ = sjson.SetBytes(msg, "output_index", st.ReasoningIndex)
					msg, _ = sjson.SetBytes(msg, "delta", t.String())
					out = append(out, emitEvent("response.reasoning_summary_text.delta", msg))
				}
				return true
			}

			// Assistant visible text
			if t := part.Get("text"); t.Exists() && t.String() != "" {
				// Before emitting non-reasoning outputs, finalize reasoning if open.
				finalizeReasoning()
				if !st.MsgOpened {
					st.MsgOpened = true
					st.MsgIndex = st.NextIndex
					st.NextIndex++
					st.CurrentMsgID = fmt.Sprintf("msg_%s_0", st.ResponseID)
					item := []byte(`{"type":"response.output_item.added","sequence_number":0,"output_index":0,"item":{"id":"","type":"message","status":"in_progress","content":[],"role":"assistant"}}`)
					item, _ = sjson.SetBytes(item, "sequence_number", nextSeq())
					item, _ = sjson.SetBytes(item, "output_index", st.MsgIndex)
					item, _ = sjson.SetBytes(item, "item.id", st.CurrentMsgID)
					out = append(out, emitEvent("response.output_item.added", item))
					partAdded := []byte(`{"type":"response.content_part.added","sequence_number":0,"item_id":"","output_index":0,"content_index":0,"part":{"type":"output_text","annotations":[],"logprobs":[],"text":""}}`)
					partAdded, _ = sjson.SetBytes(partAdded, "sequence_number", nextSeq())
					partAdded, _ = sjson.SetBytes(partAdded, "item_id", st.CurrentMsgID)
					partAdded, _ = sjson.SetBytes(partAdded, "output_index", st.MsgIndex)
					out = append(out, emitEvent("response.content_part.added", partAdded))
					st.ItemTextBuf.Reset()
				}
				st.TextBuf.WriteString(t.String())
				st.ItemTextBuf.WriteString(t.String())
				msg := []byte(`{"type":"response.output_text.delta","sequence_number":0,"item_id":"","output_index":0,"content_index":0,"delta":"","logprobs":[]}`)
				msg, _ = sjson.SetBytes(msg, "sequence_number", nextSeq())
				msg, _ = sjson.SetBytes(msg, "item_id", st.CurrentMsgID)
				msg, _ = sjson.SetBytes(msg, "output_index", st.MsgIndex)
				msg, _ = sjson.SetBytes(msg, "delta", t.String())
				out = append(out, emitEvent("response.output_text.delta", msg))
				return true
			}

			// Function call
			if fc := part.Get("functionCall"); fc.Exists() {
				// Before emitting function-call outputs, finalize reasoning and the message (if open).
				// Responses streaming requires message done events before the next output_item.added.
				finalizeReasoning()
				finalizeMessage()
				name := util.RestoreSanitizedToolName(st.SanitizedNameMap, fc.Get("name").String())
				idx := st.NextIndex
				st.NextIndex++
				// Ensure buffers
				if st.FuncArgsBuf[idx] == nil {
					st.FuncArgsBuf[idx] = &strings.Builder{}
				}
				if st.FuncCallIDs[idx] == "" {
					st.FuncCallIDs[idx] = fmt.Sprintf("call_%d_%d", time.Now().UnixNano(), atomic.AddUint64(&funcCallIDCounter, 1))
				}
				st.FuncNames[idx] = name

				argsJSON := "{}"
				if args := fc.Get("args"); args.Exists() {
					argsJSON = args.Raw
				}
				if st.FuncArgsBuf[idx].Len() == 0 && argsJSON != "" {
					st.FuncArgsBuf[idx].WriteString(argsJSON)
				}

				// Emit item.added for function call
				item := []byte(`{"type":"response.output_item.added","sequence_number":0,"output_index":0,"item":{"id":"","type":"function_call","status":"in_progress","arguments":"","call_id":"","name":""}}`)
				item, _ = sjson.SetBytes(item, "sequence_number", nextSeq())
				item, _ = sjson.SetBytes(item, "output_index", idx)
				item, _ = sjson.SetBytes(item, "item.id", fmt.Sprintf("fc_%s", st.FuncCallIDs[idx]))
				item, _ = sjson.SetBytes(item, "item.call_id", st.FuncCallIDs[idx])
				item, _ = sjson.SetBytes(item, "item.name", name)
				out = append(out, emitEvent("response.output_item.added", item))

				// Emit arguments delta (full args in one chunk).
				// When Gemini omits args, emit "{}" to keep Responses streaming event order consistent.
				if argsJSON != "" {
					ad := []byte(`{"type":"response.function_call_arguments.delta","sequence_number":0,"item_id":"","output_index":0,"delta":""}`)
					ad, _ = sjson.SetBytes(ad, "sequence_number", nextSeq())
					ad, _ = sjson.SetBytes(ad, "item_id", fmt.Sprintf("fc_%s", st.FuncCallIDs[idx]))
					ad, _ = sjson.SetBytes(ad, "output_index", idx)
					ad, _ = sjson.SetBytes(ad, "delta", argsJSON)
					out = append(out, emitEvent("response.function_call_arguments.delta", ad))
				}

				// Gemini emits the full function call payload at once, so we can finalize it immediately.
				if !st.FuncDone[idx] {
					fcDone := []byte(`{"type":"response.function_call_arguments.done","sequence_number":0,"item_id":"","output_index":0,"arguments":""}`)
					fcDone, _ = sjson.SetBytes(fcDone, "sequence_number", nextSeq())
					fcDone, _ = sjson.SetBytes(fcDone, "item_id", fmt.Sprintf("fc_%s", st.FuncCallIDs[idx]))
					fcDone, _ = sjson.SetBytes(fcDone, "output_index", idx)
					fcDone, _ = sjson.SetBytes(fcDone, "arguments", argsJSON)
					out = append(out, emitEvent("response.function_call_arguments.done", fcDone))

					itemDone := []byte(`{"type":"response.output_item.done","sequence_number":0,"output_index":0,"item":{"id":"","type":"function_call","status":"completed","arguments":"","call_id":"","name":""}}`)
					itemDone, _ = sjson.SetBytes(itemDone, "sequence_number", nextSeq())
					itemDone, _ = sjson.SetBytes(itemDone, "output_index", idx)
					itemDone, _ = sjson.SetBytes(itemDone, "item.id", fmt.Sprintf("fc_%s", st.FuncCallIDs[idx]))
					itemDone, _ = sjson.SetBytes(itemDone, "item.arguments", argsJSON)
					itemDone, _ = sjson.SetBytes(itemDone, "item.call_id", st.FuncCallIDs[idx])
					itemDone, _ = sjson.SetBytes(itemDone, "item.name", st.FuncNames[idx])
					out = append(out, emitEvent("response.output_item.done", itemDone))

					st.FuncDone[idx] = true
				}

				return true
			}

			return true
		})
	}

	// Finalization on finishReason
	if fr := root.Get("candidates.0.finishReason"); fr.Exists() && fr.String() != "" {
		// Finalize reasoning first to keep ordering tight with last delta
		finalizeReasoning()
		finalizeMessage()

		// Close function calls
		if len(st.FuncArgsBuf) > 0 {
			// sort indices (small N); avoid extra imports
			idxs := make([]int, 0, len(st.FuncArgsBuf))
			for idx := range st.FuncArgsBuf {
				idxs = append(idxs, idx)
			}
			for i := 0; i < len(idxs); i++ {
				for j := i + 1; j < len(idxs); j++ {
					if idxs[j] < idxs[i] {
						idxs[i], idxs[j] = idxs[j], idxs[i]
					}
				}
			}
			for _, idx := range idxs {
				if st.FuncDone[idx] {
					continue
				}
				args := "{}"
				if b := st.FuncArgsBuf[idx]; b != nil && b.Len() > 0 {
					args = b.String()
				}
				fcDone := []byte(`{"type":"response.function_call_arguments.done","sequence_number":0,"item_id":"","output_index":0,"arguments":""}`)
				fcDone, _ = sjson.SetBytes(fcDone, "sequence_number", nextSeq())
				fcDone, _ = sjson.SetBytes(fcDone, "item_id", fmt.Sprintf("fc_%s", st.FuncCallIDs[idx]))
				fcDone, _ = sjson.SetBytes(fcDone, "output_index", idx)
				fcDone, _ = sjson.SetBytes(fcDone, "arguments", args)
				out = append(out, emitEvent("response.function_call_arguments.done", fcDone))

				itemDone := []byte(`{"type":"response.output_item.done","sequence_number":0,"output_index":0,"item":{"id":"","type":"function_call","status":"completed","arguments":"","call_id":"","name":""}}`)
				itemDone, _ = sjson.SetBytes(itemDone, "sequence_number", nextSeq())
				itemDone, _ = sjson.SetBytes(itemDone, "output_index", idx)
				itemDone, _ = sjson.SetBytes(itemDone, "item.id", fmt.Sprintf("fc_%s", st.FuncCallIDs[idx]))
				itemDone, _ = sjson.SetBytes(itemDone, "item.arguments", args)
				itemDone, _ = sjson.SetBytes(itemDone, "item.call_id", st.FuncCallIDs[idx])
				itemDone, _ = sjson.SetBytes(itemDone, "item.name", st.FuncNames[idx])
				out = append(out, emitEvent("response.output_item.done", itemDone))

				st.FuncDone[idx] = true
			}
		}

		// Reasoning already finalized above if present

		// Build response.completed with aggregated outputs and request echo fields
		completed := []byte(`{"type":"response.completed","sequence_number":0,"response":{"id":"","object":"response","created_at":0,"status":"completed","background":false,"error":null}}`)
		completed, _ = sjson.SetBytes(completed, "sequence_number", nextSeq())
		completed, _ = sjson.SetBytes(completed, "response.id", st.ResponseID)
		completed, _ = sjson.SetBytes(completed, "response.created_at", st.CreatedAt)

		if reqJSON := pickRequestJSON(originalRequestRawJSON, requestRawJSON); len(reqJSON) > 0 {
			req := unwrapRequestRoot(gjson.ParseBytes(reqJSON))
			if v := req.Get("instructions"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.instructions", v.String())
			}
			if v := req.Get("max_output_tokens"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.max_output_tokens", v.Int())
			}
			if v := req.Get("max_tool_calls"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.max_tool_calls", v.Int())
			}
			if v := req.Get("model"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.model", v.String())
			}
			if v := req.Get("parallel_tool_calls"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.parallel_tool_calls", v.Bool())
			}
			if v := req.Get("previous_response_id"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.previous_response_id", v.String())
			}
			if v := req.Get("prompt_cache_key"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.prompt_cache_key", v.String())
			}
			if v := req.Get("reasoning"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.reasoning", v.Value())
			}
			if v := req.Get("safety_identifier"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.safety_identifier", v.String())
			}
			if v := req.Get("service_tier"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.service_tier", v.String())
			}
			if v := req.Get("store"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.store", v.Bool())
			}
			if v := req.Get("temperature"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.temperature", v.Float())
			}
			if v := req.Get("text"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.text", v.Value())
			}
			if v := req.Get("tool_choice"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.tool_choice", v.Value())
			}
			if v := req.Get("tools"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.tools", v.Value())
			}
			if v := req.Get("top_logprobs"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.top_logprobs", v.Int())
			}
			if v := req.Get("top_p"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.top_p", v.Float())
			}
			if v := req.Get("truncation"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.truncation", v.String())
			}
			if v := req.Get("user"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.user", v.Value())
			}
			if v := req.Get("metadata"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.metadata", v.Value())
			}
		}

		// Compose outputs in output_index order.
		outputsWrapper := []byte(`{"arr":[]}`)
		for idx := 0; idx < st.NextIndex; idx++ {
			if st.ReasoningOpened && idx == st.ReasoningIndex {
				item := []byte(`{"id":"","type":"reasoning","encrypted_content":"","summary":[{"type":"summary_text","text":""}]}`)
				item, _ = sjson.SetBytes(item, "id", st.ReasoningItemID)
				item, _ = sjson.SetBytes(item, "encrypted_content", st.ReasoningEnc)
				item, _ = sjson.SetBytes(item, "summary.0.text", st.ReasoningBuf.String())
				outputsWrapper, _ = sjson.SetRawBytes(outputsWrapper, "arr.-1", item)
				continue
			}
			if st.MsgOpened && idx == st.MsgIndex {
				item := []byte(`{"id":"","type":"message","status":"completed","content":[{"type":"output_text","annotations":[],"logprobs":[],"text":""}],"role":"assistant"}`)
				item, _ = sjson.SetBytes(item, "id", st.CurrentMsgID)
				item, _ = sjson.SetBytes(item, "content.0.text", st.TextBuf.String())
				outputsWrapper, _ = sjson.SetRawBytes(outputsWrapper, "arr.-1", item)
				continue
			}

			if callID, ok := st.FuncCallIDs[idx]; ok && callID != "" {
				args := "{}"
				if b := st.FuncArgsBuf[idx]; b != nil && b.Len() > 0 {
					args = b.String()
				}
				item := []byte(`{"id":"","type":"function_call","status":"completed","arguments":"","call_id":"","name":""}`)
				item, _ = sjson.SetBytes(item, "id", fmt.Sprintf("fc_%s", callID))
				item, _ = sjson.SetBytes(item, "arguments", args)
				item, _ = sjson.SetBytes(item, "call_id", callID)
				item, _ = sjson.SetBytes(item, "name", st.FuncNames[idx])
				outputsWrapper, _ = sjson.SetRawBytes(outputsWrapper, "arr.-1", item)
			}
		}
		if gjson.GetBytes(outputsWrapper, "arr.#").Int() > 0 {
			completed, _ = sjson.SetRawBytes(completed, "response.output", []byte(gjson.GetBytes(outputsWrapper, "arr").Raw))
		}

		// usage mapping
		if um := root.Get("usageMetadata"); um.Exists() {
			// input tokens = prompt only (thoughts go to output)
			input := um.Get("promptTokenCount").Int()
			completed, _ = sjson.SetBytes(completed, "response.usage.input_tokens", input)
			// cached token details: align with OpenAI "cached_tokens" semantics.
			completed, _ = sjson.SetBytes(completed, "response.usage.input_tokens_details.cached_tokens", um.Get("cachedContentTokenCount").Int())
			// output tokens
			if v := um.Get("candidatesTokenCount"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.usage.output_tokens", v.Int())
			} else {
				completed, _ = sjson.SetBytes(completed, "response.usage.output_tokens", 0)
			}
			if v := um.Get("thoughtsTokenCount"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.usage.output_tokens_details.reasoning_tokens", v.Int())
			} else {
				completed, _ = sjson.SetBytes(completed, "response.usage.output_tokens_details.reasoning_tokens", 0)
			}
			if v := um.Get("totalTokenCount"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.usage.total_tokens", v.Int())
			} else {
				completed, _ = sjson.SetBytes(completed, "response.usage.total_tokens", 0)
			}
		}

		out = append(out, emitEvent("response.completed", completed))
	}

	return out
}

// https://github.com/nektos/act/blob/4f411281417e88660bea1c1a1749aa71ae0bd60f/pkg/artifactcache/handler_test.go#L21-L529
// nektos/act pkg/artifactcache/handler_test.go:21-529
func TestHandler(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "artifactcache")
	handler, err := StartHandler(dir, "", "", 0, nil)
	require.NoError(t, err)

	base := fmt.Sprintf("%s%s", handler.ExternalURL(), apiPath)

	defer func() {
		t.Run("inpect db", func(t *testing.T) {
			db, err := handler.openDB()
			require.NoError(t, err)
			defer db.Close()
			require.NoError(t, db.Bolt().View(func(tx *bbolt.Tx) error {
				return tx.Bucket([]byte("Cache")).ForEach(func(k, v []byte) error {
					t.Logf("%s: %s", k, v)
					return nil
				})
			}))
		})
		t.Run("close", func(t *testing.T) {
			require.NoError(t, handler.Close())
			assert.Nil(t, handler.server)
			assert.Nil(t, handler.listener)
			_, err := http.Post(fmt.Sprintf("%s/caches/%d", base, 1), "", nil)
			assert.Error(t, err)
		})
	}()

	t.Run("get not exist", func(t *testing.T) {
		key := strings.ToLower(t.Name())
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		resp, err := http.Get(fmt.Sprintf("%s/cache?keys=%s&version=%s", base, key, version))
		require.NoError(t, err)
		require.Equal(t, 204, resp.StatusCode)
	})

	t.Run("reserve and upload", func(t *testing.T) {
		key := strings.ToLower(t.Name())
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		content := make([]byte, 100)
		_, err := rand.Read(content)
		require.NoError(t, err)
		uploadCacheNormally(t, base, key, version, content)
	})

	t.Run("clean", func(t *testing.T) {
		resp, err := http.Post(fmt.Sprintf("%s/clean", base), "", nil)
		require.NoError(t, err)
		assert.Equal(t, 200, resp.StatusCode)
	})

	t.Run("reserve with bad request", func(t *testing.T) {
		body := []byte(`invalid json`)
		require.NoError(t, err)
		resp, err := http.Post(fmt.Sprintf("%s/caches", base), "application/json", bytes.NewReader(body))
		require.NoError(t, err)
		assert.Equal(t, 400, resp.StatusCode)
	})

	t.Run("duplicate reserve", func(t *testing.T) {
		key := strings.ToLower(t.Name())
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		var first, second struct {
			CacheID uint64 `json:"cacheId"`
		}
		{
			body, err := json.Marshal(&Request{
				Key:     key,
				Version: version,
				Size:    100,
			})
			require.NoError(t, err)
			resp, err := http.Post(fmt.Sprintf("%s/caches", base), "application/json", bytes.NewReader(body))
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)

			require.NoError(t, json.NewDecoder(resp.Body).Decode(&first))
			assert.NotZero(t, first.CacheID)
		}
		{
			body, err := json.Marshal(&Request{
				Key:     key,
				Version: version,
				Size:    100,
			})
			require.NoError(t, err)
			resp, err := http.Post(fmt.Sprintf("%s/caches", base), "application/json", bytes.NewReader(body))
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)

			require.NoError(t, json.NewDecoder(resp.Body).Decode(&second))
			assert.NotZero(t, second.CacheID)
		}

		assert.NotEqual(t, first.CacheID, second.CacheID)
	})

	t.Run("upload with bad id", func(t *testing.T) {
		req, err := http.NewRequest(http.MethodPatch,
			fmt.Sprintf("%s/caches/invalid_id", base), bytes.NewReader(nil))
		require.NoError(t, err)
		req.Header.Set("Content-Type", "application/octet-stream")
		req.Header.Set("Content-Range", "bytes 0-99/*")
		resp, err := http.DefaultClient.Do(req)
		require.NoError(t, err)
		assert.Equal(t, 400, resp.StatusCode)
	})

	t.Run("upload without reserve", func(t *testing.T) {
		req, err := http.NewRequest(http.MethodPatch,
			fmt.Sprintf("%s/caches/%d", base, 1000), bytes.NewReader(nil))
		require.NoError(t, err)
		req.Header.Set("Content-Type", "application/octet-stream")
		req.Header.Set("Content-Range", "bytes 0-99/*")
		resp, err := http.DefaultClient.Do(req)
		require.NoError(t, err)
		assert.Equal(t, 400, resp.StatusCode)
	})

	t.Run("upload with complete", func(t *testing.T) {
		key := strings.ToLower(t.Name())
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		var id uint64
		content := make([]byte, 100)
		_, err := rand.Read(content)
		require.NoError(t, err)
		{
			body, err := json.Marshal(&Request{
				Key:     key,
				Version: version,
				Size:    100,
			})
			require.NoError(t, err)
			resp, err := http.Post(fmt.Sprintf("%s/caches", base), "application/json", bytes.NewReader(body))
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)

			got := struct {
				CacheID uint64 `json:"cacheId"`
			}{}
			require.NoError(t, json.NewDecoder(resp.Body).Decode(&got))
			id = got.CacheID
		}
		{
			req, err := http.NewRequest(http.MethodPatch,
				fmt.Sprintf("%s/caches/%d", base, id), bytes.NewReader(content))
			require.NoError(t, err)
			req.Header.Set("Content-Type", "application/octet-stream")
			req.Header.Set("Content-Range", "bytes 0-99/*")
			resp, err := http.DefaultClient.Do(req)
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)
		}
		{
			resp, err := http.Post(fmt.Sprintf("%s/caches/%d", base, id), "", nil)
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)
		}
		{
			req, err := http.NewRequest(http.MethodPatch,
				fmt.Sprintf("%s/caches/%d", base, id), bytes.NewReader(content))
			require.NoError(t, err)
			req.Header.Set("Content-Type", "application/octet-stream")
			req.Header.Set("Content-Range", "bytes 0-99/*")
			resp, err := http.DefaultClient.Do(req)
			require.NoError(t, err)
			assert.Equal(t, 400, resp.StatusCode)
		}
	})

	t.Run("upload with invalid range", func(t *testing.T) {
		key := strings.ToLower(t.Name())
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		var id uint64
		content := make([]byte, 100)
		_, err := rand.Read(content)
		require.NoError(t, err)
		{
			body, err := json.Marshal(&Request{
				Key:     key,
				Version: version,
				Size:    100,
			})
			require.NoError(t, err)
			resp, err := http.Post(fmt.Sprintf("%s/caches", base), "application/json", bytes.NewReader(body))
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)

			got := struct {
				CacheID uint64 `json:"cacheId"`
			}{}
			require.NoError(t, json.NewDecoder(resp.Body).Decode(&got))
			id = got.CacheID
		}
		{
			req, err := http.NewRequest(http.MethodPatch,
				fmt.Sprintf("%s/caches/%d", base, id), bytes.NewReader(content))
			require.NoError(t, err)
			req.Header.Set("Content-Type", "application/octet-stream")
			req.Header.Set("Content-Range", "bytes xx-99/*")
			resp, err := http.DefaultClient.Do(req)
			require.NoError(t, err)
			assert.Equal(t, 400, resp.StatusCode)
		}
	})

	t.Run("commit with bad id", func(t *testing.T) {
		{
			resp, err := http.Post(fmt.Sprintf("%s/caches/invalid_id", base), "", nil)
			require.NoError(t, err)
			assert.Equal(t, 400, resp.StatusCode)
		}
	})

	t.Run("commit with not exist id", func(t *testing.T) {
		{
			resp, err := http.Post(fmt.Sprintf("%s/caches/%d", base, 100), "", nil)
			require.NoError(t, err)
			assert.Equal(t, 400, resp.StatusCode)
		}
	})

	t.Run("duplicate commit", func(t *testing.T) {
		key := strings.ToLower(t.Name())
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		var id uint64
		content := make([]byte, 100)
		_, err := rand.Read(content)
		require.NoError(t, err)
		{
			body, err := json.Marshal(&Request{
				Key:     key,
				Version: version,
				Size:    100,
			})
			require.NoError(t, err)
			resp, err := http.Post(fmt.Sprintf("%s/caches", base), "application/json", bytes.NewReader(body))
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)

			got := struct {
				CacheID uint64 `json:"cacheId"`
			}{}
			require.NoError(t, json.NewDecoder(resp.Body).Decode(&got))
			id = got.CacheID
		}
		{
			req, err := http.NewRequest(http.MethodPatch,
				fmt.Sprintf("%s/caches/%d", base, id), bytes.NewReader(content))
			require.NoError(t, err)
			req.Header.Set("Content-Type", "application/octet-stream")
			req.Header.Set("Content-Range", "bytes 0-99/*")
			resp, err := http.DefaultClient.Do(req)
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)
		}
		{
			resp, err := http.Post(fmt.Sprintf("%s/caches/%d", base, id), "", nil)
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)
		}
		{
			resp, err := http.Post(fmt.Sprintf("%s/caches/%d", base, id), "", nil)
			require.NoError(t, err)
			assert.Equal(t, 400, resp.StatusCode)
		}
	})

	t.Run("commit early", func(t *testing.T) {
		key := strings.ToLower(t.Name())
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		var id uint64
		content := make([]byte, 100)
		_, err := rand.Read(content)
		require.NoError(t, err)
		{
			body, err := json.Marshal(&Request{
				Key:     key,
				Version: version,
				Size:    100,
			})
			require.NoError(t, err)
			resp, err := http.Post(fmt.Sprintf("%s/caches", base), "application/json", bytes.NewReader(body))
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)

			got := struct {
				CacheID uint64 `json:"cacheId"`
			}{}
			require.NoError(t, json.NewDecoder(resp.Body).Decode(&got))
			id = got.CacheID
		}
		{
			req, err := http.NewRequest(http.MethodPatch,
				fmt.Sprintf("%s/caches/%d", base, id), bytes.NewReader(content[:50]))
			require.NoError(t, err)
			req.Header.Set("Content-Type", "application/octet-stream")
			req.Header.Set("Content-Range", "bytes 0-59/*")
			resp, err := http.DefaultClient.Do(req)
			require.NoError(t, err)
			assert.Equal(t, 200, resp.StatusCode)
		}
		{
			resp, err := http.Post(fmt.Sprintf("%s/caches/%d", base, id), "", nil)
			require.NoError(t, err)
			assert.Equal(t, 500, resp.StatusCode)
		}
	})

	t.Run("get with bad id", func(t *testing.T) {
		resp, err := http.Get(fmt.Sprintf("%s/artifacts/invalid_id", base))
		require.NoError(t, err)
		require.Equal(t, 400, resp.StatusCode)
	})

	t.Run("get with not exist id", func(t *testing.T) {
		resp, err := http.Get(fmt.Sprintf("%s/artifacts/%d", base, 100))
		require.NoError(t, err)
		require.Equal(t, 404, resp.StatusCode)
	})

	t.Run("get with not exist id", func(t *testing.T) {
		resp, err := http.Get(fmt.Sprintf("%s/artifacts/%d", base, 100))
		require.NoError(t, err)
		require.Equal(t, 404, resp.StatusCode)
	})

	t.Run("get with multiple keys", func(t *testing.T) {
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		key := strings.ToLower(t.Name())
		keys := [3]string{
			key + "_a_b_c",
			key + "_a_b",
			key + "_a",
		}
		contents := [3][]byte{
			make([]byte, 100),
			make([]byte, 200),
			make([]byte, 300),
		}
		for i := range contents {
			_, err := rand.Read(contents[i])
			require.NoError(t, err)
			uploadCacheNormally(t, base, keys[i], version, contents[i])
			time.Sleep(time.Second) // ensure CreatedAt of caches are different
		}

		reqKeys := strings.Join([]string{
			key + "_a_b_x",
			key + "_a_b",
			key + "_a",
		}, ",")

		resp, err := http.Get(fmt.Sprintf("%s/cache?keys=%s&version=%s", base, reqKeys, version))
		require.NoError(t, err)
		require.Equal(t, 200, resp.StatusCode)

		/*
			Expect `key_a_b` because:
			- `key_a_b_x" doesn't match any caches.
			- `key_a_b" matches `key_a_b` and `key_a_b_c`, but `key_a_b` is newer.
		*/
		except := 1

		got := struct {
			Result          string `json:"result"`
			ArchiveLocation string `json:"archiveLocation"`
			CacheKey        string `json:"cacheKey"`
		}{}
		require.NoError(t, json.NewDecoder(resp.Body).Decode(&got))
		assert.Equal(t, "hit", got.Result)
		assert.Equal(t, keys[except], got.CacheKey)

		contentResp, err := http.Get(got.ArchiveLocation)
		require.NoError(t, err)
		require.Equal(t, 200, contentResp.StatusCode)
		content, err := io.ReadAll(contentResp.Body)
		require.NoError(t, err)
		assert.Equal(t, contents[except], content)
	})

	t.Run("case insensitive", func(t *testing.T) {
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		key := strings.ToLower(t.Name())
		content := make([]byte, 100)
		_, err := rand.Read(content)
		require.NoError(t, err)
		uploadCacheNormally(t, base, key+"_ABC", version, content)

		{
			reqKey := key + "_aBc"
			resp, err := http.Get(fmt.Sprintf("%s/cache?keys=%s&version=%s", base, reqKey, version))
			require.NoError(t, err)
			require.Equal(t, 200, resp.StatusCode)
			got := struct {
				Result          string `json:"result"`
				ArchiveLocation string `json:"archiveLocation"`
				CacheKey        string `json:"cacheKey"`
			}{}
			require.NoError(t, json.NewDecoder(resp.Body).Decode(&got))
			assert.Equal(t, "hit", got.Result)
			assert.Equal(t, key+"_abc", got.CacheKey)
		}
	})

	t.Run("exact keys are preferred (key 0)", func(t *testing.T) {
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		key := strings.ToLower(t.Name())
		keys := [3]string{
			key + "_a",
			key + "_a_b_c",
			key + "_a_b",
		}
		contents := [3][]byte{
			make([]byte, 100),
			make([]byte, 200),
			make([]byte, 300),
		}
		for i := range contents {
			_, err := rand.Read(contents[i])
			require.NoError(t, err)
			uploadCacheNormally(t, base, keys[i], version, contents[i])
			time.Sleep(time.Second) // ensure CreatedAt of caches are different
		}

		reqKeys := strings.Join([]string{
			key + "_a",
			key + "_a_b",
		}, ",")

		resp, err := http.Get(fmt.Sprintf("%s/cache?keys=%s&version=%s", base, reqKeys, version))
		require.NoError(t, err)
		require.Equal(t, 200, resp.StatusCode)

		/*
			Expect `key_a` because:
			- `key_a` matches `key_a`, `key_a_b` and `key_a_b_c`, but `key_a` is an exact match.
			- `key_a_b` matches `key_a_b` and `key_a_b_c`, but previous key had a match
		*/
		expect := 0

		got := struct {
			ArchiveLocation string `json:"archiveLocation"`
			CacheKey        string `json:"cacheKey"`
		}{}
		require.NoError(t, json.NewDecoder(resp.Body).Decode(&got))
		assert.Equal(t, keys[expect], got.CacheKey)

		contentResp, err := http.Get(got.ArchiveLocation)
		require.NoError(t, err)
		require.Equal(t, 200, contentResp.StatusCode)
		content, err := io.ReadAll(contentResp.Body)
		require.NoError(t, err)
		assert.Equal(t, contents[expect], content)
	})

	t.Run("exact keys are preferred (key 1)", func(t *testing.T) {
		version := "c19da02a2bd7e77277f1ac29ab45c09b7d46a4ee758284e26bb3045ad11d9d20"
		key := strings.ToLower(t.Name())
		keys := [3]string{
			key + "_a",
			key + "_a_b_c",
			key + "_a_b",
		}
		contents := [3][]byte{
			make([]byte, 100),
			make([]byte, 200),
			make([]byte, 300),
		}
		for i := range contents {
			_, err := rand.Read(contents[i])
			require.NoError(t, err)
			uploadCacheNormally(t, base, keys[i], version, contents[i])
			time.Sleep(time.Second) // ensure CreatedAt of caches are different
		}

		reqKeys := strings.Join([]string{
			"------------------------------------------------------",
			key + "_a",
			key + "_a_b",
		}, ",")

		resp, err := http.Get(fmt.Sprintf("%s/cache?keys=%s&version=%s", base, reqKeys, version))
		require.NoError(t, err)
		require.Equal(t, 200, resp.StatusCode)

		/*
			Expect `key_a` because:
			- `------------------------------------------------------` doesn't match any caches.
			- `key_a` matches `key_a`, `key_a_b` and `key_a_b_c`, but `key_a` is an exact match.
			- `key_a_b` matches `key_a_b` and `key_a_b_c`, but previous key had a match
		*/
		expect := 0

		got := struct {
			ArchiveLocation string `json:"archiveLocation"`
			CacheKey        string `json:"cacheKey"`
		}{}
		require.NoError(t, json.NewDecoder(resp.Body).Decode(&got))
		assert.Equal(t, keys[expect], got.CacheKey)

		contentResp, err := http.Get(got.ArchiveLocation)
		require.NoError(t, err)
		require.Equal(t, 200, contentResp.StatusCode)
		content, err := io.ReadAll(contentResp.Body)
		require.NoError(t, err)
		assert.Equal(t, contents[expect], content)
	})
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/internal/translator/claude/openai/responses/claude_openai-responses_response.go#L115-L531
// router-for-me/CLIProxyAPI internal/translator/claude/openai/responses/claude_openai-responses_response.go:115-531
func ConvertClaudeResponseToOpenAIResponses(ctx context.Context, modelName string, originalRequestRawJSON, requestRawJSON, rawJSON []byte, param *any) [][]byte {
	if *param == nil {
		*param = &claudeToResponsesState{FuncArgsBuf: make(map[int]*strings.Builder), FuncNames: make(map[int]string), FuncCallIDs: make(map[int]string)}
	}
	st := (*param).(*claudeToResponsesState)

	// Expect `data: {..}` from Claude clients
	if !bytes.HasPrefix(rawJSON, dataTag) {
		return [][]byte{}
	}
	rawJSON = bytes.TrimSpace(rawJSON[5:])
	root := gjson.ParseBytes(rawJSON)
	ev := root.Get("type").String()
	var out [][]byte

	nextSeq := func() int { st.Seq++; return st.Seq }

	switch ev {
	case "message_start":
		if msg := root.Get("message"); msg.Exists() {
			st.ResponseID = msg.Get("id").String()
			st.CreatedAt = time.Now().Unix()
			// Reset per-message aggregation state
			st.TextBuf.Reset()
			st.CurrentTextBuf.Reset()
			st.MessageAnnotations = nil
			st.ReasoningBuf.Reset()
			st.ReasoningActive = false
			st.InTextBlock = false
			st.InFuncBlock = false
			st.MessageOpen = false
			st.ContentPartOpen = false
			st.CurrentMsgID = ""
			st.CurrentFCID = ""
			st.ReasoningItemID = ""
			st.ReasoningSignature = ""
			st.ReasoningIndex = 0
			st.ReasoningPartAdded = false
			st.FuncArgsBuf = make(map[int]*strings.Builder)
			st.FuncNames = make(map[int]string)
			st.FuncCallIDs = make(map[int]string)
			st.InputTokens = 0
			st.OutputTokens = 0
			st.UsageSeen = false
			if usage := msg.Get("usage"); usage.Exists() {
				if v := usage.Get("input_tokens"); v.Exists() {
					st.InputTokens = v.Int()
					st.UsageSeen = true
				}
				if v := usage.Get("output_tokens"); v.Exists() {
					st.OutputTokens = v.Int()
					st.UsageSeen = true
				}
			}
			// response.created
			created := []byte(`{"type":"response.created","sequence_number":0,"response":{"id":"","object":"response","created_at":0,"status":"in_progress","background":false,"error":null,"output":[]}}`)
			created, _ = sjson.SetBytes(created, "sequence_number", nextSeq())
			created, _ = sjson.SetBytes(created, "response.id", st.ResponseID)
			created, _ = sjson.SetBytes(created, "response.created_at", st.CreatedAt)
			out = append(out, emitEvent("response.created", created))
			// response.in_progress
			inprog := []byte(`{"type":"response.in_progress","sequence_number":0,"response":{"id":"","object":"response","created_at":0,"status":"in_progress"}}`)
			inprog, _ = sjson.SetBytes(inprog, "sequence_number", nextSeq())
			inprog, _ = sjson.SetBytes(inprog, "response.id", st.ResponseID)
			inprog, _ = sjson.SetBytes(inprog, "response.created_at", st.CreatedAt)
			out = append(out, emitEvent("response.in_progress", inprog))
		}
	case "content_block_start":
		cb := root.Get("content_block")
		if !cb.Exists() {
			return noSSEOutput(out)
		}
		idx := int(root.Get("index").Int())
		typ := cb.Get("type").String()
		if typ == "text" {
			st.InTextBlock = true
			if st.CurrentMsgID == "" {
				st.CurrentMsgID = fmt.Sprintf("msg_%s_0", st.ResponseID)
			}
			if !st.MessageOpen {
				item := []byte(`{"type":"response.output_item.added","sequence_number":0,"output_index":0,"item":{"id":"","type":"message","status":"in_progress","content":[],"role":"assistant"}}`)
				item, _ = sjson.SetBytes(item, "sequence_number", nextSeq())
				item, _ = sjson.SetBytes(item, "item.id", st.CurrentMsgID)
				out = append(out, emitEvent("response.output_item.added", item))
				st.MessageOpen = true
			}
			if !st.ContentPartOpen {
				part := []byte(`{"type":"response.content_part.added","sequence_number":0,"item_id":"","output_index":0,"content_index":0,"part":{"type":"output_text","annotations":[],"logprobs":[],"text":""}}`)
				part, _ = sjson.SetBytes(part, "sequence_number", nextSeq())
				part, _ = sjson.SetBytes(part, "item_id", st.CurrentMsgID)
				out = append(out, emitEvent("response.content_part.added", part))
				st.ContentPartOpen = true
			}
		} else if typ == "tool_use" {
			st.InFuncBlock = true
			st.CurrentFCID = cb.Get("id").String()
			name := cb.Get("name").String()
			item := []byte(`{"type":"response.output_item.added","sequence_number":0,"output_index":0,"item":{"id":"","type":"function_call","status":"in_progress","arguments":"","call_id":"","name":""}}`)
			item, _ = sjson.SetBytes(item, "sequence_number", nextSeq())
			item, _ = sjson.SetBytes(item, "output_index", idx)
			item, _ = sjson.SetBytes(item, "item.id", fmt.Sprintf("fc_%s", st.CurrentFCID))
			item, _ = sjson.SetBytes(item, "item.call_id", st.CurrentFCID)
			item, _ = sjson.SetBytes(item, "item.name", name)
			out = append(out, emitEvent("response.output_item.added", item))
			if st.FuncArgsBuf[idx] == nil {
				st.FuncArgsBuf[idx] = &strings.Builder{}
			}
			// record function metadata for aggregation
			st.FuncCallIDs[idx] = st.CurrentFCID
			st.FuncNames[idx] = name
		} else if typ == "thinking" {
			// start reasoning item
			st.ReasoningActive = true
			st.ReasoningIndex = idx
			st.ReasoningBuf.Reset()
			st.ReasoningSignature = ""
			if signature := cb.Get("signature"); signature.Exists() && signature.String() != "" {
				st.ReasoningSignature = signature.String()
			}
			st.ReasoningItemID = fmt.Sprintf("rs_%s_%d", st.ResponseID, idx)
			item := []byte(`{"type":"response.output_item.added","sequence_number":0,"output_index":0,"item":{"id":"","type":"reasoning","status":"in_progress","encrypted_content":"","summary":[]}}`)
			item, _ = sjson.SetBytes(item, "sequence_number", nextSeq())
			item, _ = sjson.SetBytes(item, "output_index", idx)
			item, _ = sjson.SetBytes(item, "item.id", st.ReasoningItemID)
			item, _ = sjson.SetBytes(item, "item.encrypted_content", st.ReasoningSignature)
			out = append(out, emitEvent("response.output_item.added", item))
			// add a summary part placeholder
			part := []byte(`{"type":"response.reasoning_summary_part.added","sequence_number":0,"item_id":"","output_index":0,"summary_index":0,"part":{"type":"summary_text","text":""}}`)
			part, _ = sjson.SetBytes(part, "sequence_number", nextSeq())
			part, _ = sjson.SetBytes(part, "item_id", st.ReasoningItemID)
			part, _ = sjson.SetBytes(part, "output_index", idx)
			out = append(out, emitEvent("response.reasoning_summary_part.added", part))
			st.ReasoningPartAdded = true
		}
	case "content_block_delta":
		d := root.Get("delta")
		if !d.Exists() {
			return noSSEOutput(out)
		}
		dt := d.Get("type").String()
		if dt == "text_delta" {
			if t := d.Get("text"); t.Exists() {
				msg := []byte(`{"type":"response.output_text.delta","sequence_number":0,"item_id":"","output_index":0,"content_index":0,"delta":"","logprobs":[]}`)
				msg, _ = sjson.SetBytes(msg, "sequence_number", nextSeq())
				msg, _ = sjson.SetBytes(msg, "item_id", st.CurrentMsgID)
				msg, _ = sjson.SetBytes(msg, "delta", t.String())
				out = append(out, emitEvent("response.output_text.delta", msg))
				// aggregate text for response.output
				st.TextBuf.WriteString(t.String())
				st.CurrentTextBuf.WriteString(t.String())
			}
		} else if dt == "input_json_delta" {
			if !st.InFuncBlock || st.CurrentFCID == "" {
				return [][]byte{}
			}
			idx := int(root.Get("index").Int())
			if pj := d.Get("partial_json"); pj.Exists() {
				if st.FuncArgsBuf[idx] == nil {
					st.FuncArgsBuf[idx] = &strings.Builder{}
				}
				st.FuncArgsBuf[idx].WriteString(pj.String())
				msg := []byte(`{"type":"response.function_call_arguments.delta","sequence_number":0,"item_id":"","output_index":0,"delta":""}`)
				msg, _ = sjson.SetBytes(msg, "sequence_number", nextSeq())
				msg, _ = sjson.SetBytes(msg, "item_id", fmt.Sprintf("fc_%s", st.CurrentFCID))
				msg, _ = sjson.SetBytes(msg, "output_index", idx)
				msg, _ = sjson.SetBytes(msg, "delta", pj.String())
				out = append(out, emitEvent("response.function_call_arguments.delta", msg))
			}
		} else if dt == "thinking_delta" {
			if st.ReasoningActive {
				if t := d.Get("thinking"); t.Exists() {
					st.ReasoningBuf.WriteString(t.String())
					msg := []byte(`{"type":"response.reasoning_summary_text.delta","sequence_number":0,"item_id":"","output_index":0,"summary_index":0,"delta":""}`)
					msg, _ = sjson.SetBytes(msg, "sequence_number", nextSeq())
					msg, _ = sjson.SetBytes(msg, "item_id", st.ReasoningItemID)
					msg, _ = sjson.SetBytes(msg, "output_index", st.ReasoningIndex)
					msg, _ = sjson.SetBytes(msg, "delta", t.String())
					out = append(out, emitEvent("response.reasoning_summary_text.delta", msg))
				}
			}
		} else if dt == "signature_delta" {
			if st.ReasoningActive {
				if signature := d.Get("signature"); signature.Exists() && signature.String() != "" {
					st.ReasoningSignature = signature.String()
				}
			}
			return [][]byte{}
		} else if dt == "citations_delta" {
			if citation := d.Get("citation"); citation.Exists() {
				st.appendMessageAnnotation(citation.Value())
			}
			return [][]byte{}
		}
	case "content_block_stop":
		idx := int(root.Get("index").Int())
		if st.InTextBlock {
			st.InTextBlock = false
		} else if st.InFuncBlock {
			args := "{}"
			if buf := st.FuncArgsBuf[idx]; buf != nil {
				if buf.Len() > 0 {
					args = buf.String()
				}
			}
			fcDone := []byte(`{"type":"response.function_call_arguments.done","sequence_number":0,"item_id":"","output_index":0,"arguments":""}`)
			fcDone, _ = sjson.SetBytes(fcDone, "sequence_number", nextSeq())
			fcDone, _ = sjson.SetBytes(fcDone, "item_id", fmt.Sprintf("fc_%s", st.CurrentFCID))
			fcDone, _ = sjson.SetBytes(fcDone, "output_index", idx)
			fcDone, _ = sjson.SetBytes(fcDone, "arguments", args)
			out = append(out, emitEvent("response.function_call_arguments.done", fcDone))
			itemDone := []byte(`{"type":"response.output_item.done","sequence_number":0,"output_index":0,"item":{"id":"","type":"function_call","status":"completed","arguments":"","call_id":"","name":""}}`)
			itemDone, _ = sjson.SetBytes(itemDone, "sequence_number", nextSeq())
			itemDone, _ = sjson.SetBytes(itemDone, "output_index", idx)
			itemDone, _ = sjson.SetBytes(itemDone, "item.id", fmt.Sprintf("fc_%s", st.CurrentFCID))
			itemDone, _ = sjson.SetBytes(itemDone, "item.arguments", args)
			itemDone, _ = sjson.SetBytes(itemDone, "item.call_id", st.CurrentFCID)
			itemDone, _ = sjson.SetBytes(itemDone, "item.name", st.FuncNames[idx])
			out = append(out, emitEvent("response.output_item.done", itemDone))
			st.InFuncBlock = false
		} else if st.ReasoningActive {
			full := st.ReasoningBuf.String()
			textDone := []byte(`{"type":"response.reasoning_summary_text.done","sequence_number":0,"item_id":"","output_index":0,"summary_index":0,"text":""}`)
			textDone, _ = sjson.SetBytes(textDone, "sequence_number", nextSeq())
			textDone, _ = sjson.SetBytes(textDone, "item_id", st.ReasoningItemID)
			textDone, _ = sjson.SetBytes(textDone, "output_index", st.ReasoningIndex)
			textDone, _ = sjson.SetBytes(textDone, "text", full)
			out = append(out, emitEvent("response.reasoning_summary_text.done", textDone))
			partDone := []byte(`{"type":"response.reasoning_summary_part.done","sequence_number":0,"item_id":"","output_index":0,"summary_index":0,"part":{"type":"summary_text","text":""}}`)
			partDone, _ = sjson.SetBytes(partDone, "sequence_number", nextSeq())
			partDone, _ = sjson.SetBytes(partDone, "item_id", st.ReasoningItemID)
			partDone, _ = sjson.SetBytes(partDone, "output_index", st.ReasoningIndex)
			partDone, _ = sjson.SetBytes(partDone, "part.text", full)
			out = append(out, emitEvent("response.reasoning_summary_part.done", partDone))
			itemDone := []byte(`{"type":"response.output_item.done","sequence_number":0,"output_index":0,"item":{"id":"","type":"reasoning","encrypted_content":"","summary":[]}}`)
			itemDone, _ = sjson.SetBytes(itemDone, "sequence_number", nextSeq())
			itemDone, _ = sjson.SetBytes(itemDone, "item.id", st.ReasoningItemID)
			itemDone, _ = sjson.SetBytes(itemDone, "output_index", st.ReasoningIndex)
			itemDone, _ = sjson.SetBytes(itemDone, "item.encrypted_content", st.ReasoningSignature)
			if full != "" {
				summary := []byte(`{"type":"summary_text","text":""}`)
				summary, _ = sjson.SetBytes(summary, "text", full)
				itemDone, _ = sjson.SetRawBytes(itemDone, "item.summary.-1", summary)
			}
			out = append(out, emitEvent("response.output_item.done", itemDone))
			st.ReasoningActive = false
			st.ReasoningPartAdded = false
		}
		return noSSEOutput(out)
	case "message_delta":
		if usage := root.Get("usage"); usage.Exists() {
			if v := usage.Get("output_tokens"); v.Exists() {
				st.OutputTokens = v.Int()
				st.UsageSeen = true
			}
			if v := usage.Get("input_tokens"); v.Exists() {
				st.InputTokens = v.Int()
				st.UsageSeen = true
			}
		}
		return [][]byte{}
	case "message_stop":
		out = append(out, st.finalizeAssistantMessage(nextSeq)...)

		completed := []byte(`{"type":"response.completed","sequence_number":0,"response":{"id":"","object":"response","created_at":0,"status":"completed","background":false,"error":null}}`)
		completed, _ = sjson.SetBytes(completed, "sequence_number", nextSeq())
		completed, _ = sjson.SetBytes(completed, "response.id", st.ResponseID)
		completed, _ = sjson.SetBytes(completed, "response.created_at", st.CreatedAt)
		// Inject original request fields into response as per docs/response.completed.json

		reqBytes := pickRequestJSON(originalRequestRawJSON, requestRawJSON)
		if len(reqBytes) > 0 {
			req := gjson.ParseBytes(reqBytes)
			if v := req.Get("instructions"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.instructions", v.String())
			}
			if v := req.Get("max_output_tokens"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.max_output_tokens", v.Int())
			}
			if v := req.Get("max_tool_calls"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.max_tool_calls", v.Int())
			}
			if v := req.Get("model"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.model", v.String())
			}
			if v := req.Get("parallel_tool_calls"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.parallel_tool_calls", v.Bool())
			}
			if v := req.Get("previous_response_id"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.previous_response_id", v.String())
			}
			if v := req.Get("prompt_cache_key"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.prompt_cache_key", v.String())
			}
			if v := req.Get("reasoning"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.reasoning", v.Value())
			}
			if v := req.Get("safety_identifier"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.safety_identifier", v.String())
			}
			if v := req.Get("service_tier"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.service_tier", v.String())
			}
			if v := req.Get("store"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.store", v.Bool())
			}
			if v := req.Get("temperature"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.temperature", v.Float())
			}
			if v := req.Get("text"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.text", v.Value())
			}
			if v := req.Get("tool_choice"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.tool_choice", v.Value())
			}
			if v := req.Get("tools"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.tools", v.Value())
			}
			if v := req.Get("top_logprobs"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.top_logprobs", v.Int())
			}
			if v := req.Get("top_p"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.top_p", v.Float())
			}
			if v := req.Get("truncation"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.truncation", v.String())
			}
			if v := req.Get("user"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.user", v.Value())
			}
			if v := req.Get("metadata"); v.Exists() {
				completed, _ = sjson.SetBytes(completed, "response.metadata", v.Value())
			}
		}

		// Build response.output from aggregated state
		outputsWrapper := []byte(`{"arr":[]}`)
		// reasoning item (if any)
		if st.ReasoningBuf.Len() > 0 || st.ReasoningPartAdded || st.ReasoningSignature != "" {
			item := []byte(`{"id":"","type":"reasoning","encrypted_content":"","summary":[]}`)
			item, _ = sjson.SetBytes(item, "id", st.ReasoningItemID)
			item, _ = sjson.SetBytes(item, "encrypted_content", st.ReasoningSignature)
			if st.ReasoningBuf.Len() > 0 {
				summary := []byte(`{"type":"summary_text","text":""}`)
				summary, _ = sjson.SetBytes(summary, "text", st.ReasoningBuf.String())
				item, _ = sjson.SetRawBytes(item, "summary.-1", summary)
			}
			outputsWrapper, _ = sjson.SetRawBytes(outputsWrapper, "arr.-1", item)
		}
		// assistant message item (if any text)
		if st.TextBuf.Len() > 0 || st.InTextBlock || st.CurrentMsgID != "" {
			item := []byte(`{"id":"","type":"message","status":"completed","content":[{"type":"output_text","annotations":[],"logprobs":[],"text":""}],"role":"assistant"}`)
			item, _ = sjson.SetBytes(item, "id", st.CurrentMsgID)
			item, _ = sjson.SetBytes(item, "content.0.text", st.TextBuf.String())
			if len(st.MessageAnnotations) > 0 {
				item, _ = sjson.SetBytes(item, "content.0.annotations", st.MessageAnnotations)
			}
			outputsWrapper, _ = sjson.SetRawBytes(outputsWrapper, "arr.-1", item)
		}
		// function_call items (in ascending index order for determinism)
		if len(st.FuncArgsBuf) > 0 {
			// collect indices
			idxs := make([]int, 0, len(st.FuncArgsBuf))
			for idx := range st.FuncArgsBuf {
				idxs = append(idxs, idx)
			}
			// simple sort (small N), avoid adding new imports
			for i := 0; i < len(idxs); i++ {
				for j := i + 1; j < len(idxs); j++ {
					if idxs[j] < idxs[i] {
						idxs[i], idxs[j] = idxs[j], idxs[i]
					}
				}
			}
			for _, idx := range idxs {
				args := ""
				if b := st.FuncArgsBuf[idx]; b != nil {
					args = b.String()
				}
				callID := st.FuncCallIDs[idx]
				name := st.FuncNames[idx]
				if callID == "" && st.CurrentFCID != "" {
					callID = st.CurrentFCID
				}
				item := []byte(`{"id":"","type":"function_call","status":"completed","arguments":"","call_id":"","name":""}`)
				item, _ = sjson.SetBytes(item, "id", fmt.Sprintf("fc_%s", callID))
				item, _ = sjson.SetBytes(item, "arguments", args)
				item, _ = sjson.SetBytes(item, "call_id", callID)
				item, _ = sjson.SetBytes(item, "name", name)
				outputsWrapper, _ = sjson.SetRawBytes(outputsWrapper, "arr.-1", item)
			}
		}
		if gjson.GetBytes(outputsWrapper, "arr.#").Int() > 0 {
			completed, _ = sjson.SetRawBytes(completed, "response.output", []byte(gjson.GetBytes(outputsWrapper, "arr").Raw))
		}

		reasoningTokens := int64(0)
		if st.ReasoningBuf.Len() > 0 {
			reasoningTokens = int64(st.ReasoningBuf.Len() / 4)
		}
		usagePresent := st.UsageSeen || reasoningTokens > 0
		if usagePresent {
			completed, _ = sjson.SetBytes(completed, "response.usage.input_tokens", st.InputTokens)
			completed, _ = sjson.SetBytes(completed, "response.usage.input_tokens_details.cached_tokens", 0)
			completed, _ = sjson.SetBytes(completed, "response.usage.output_tokens", st.OutputTokens)
			if reasoningTokens > 0 {
				completed, _ = sjson.SetBytes(completed, "response.usage.output_tokens_details.reasoning_tokens", reasoningTokens)
			}
			total := st.InputTokens + st.OutputTokens
			if total > 0 || st.UsageSeen {
				completed, _ = sjson.SetBytes(completed, "response.usage.total_tokens", total)
			}
		}
		out = append(out, emitEvent("response.completed", completed))
	}

	return noSSEOutput(out)
}
