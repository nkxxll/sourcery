// https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/internal/db/label_file_binding.go#L37-L42
// AlistGo/alist internal/db/label_file_binding.go:37-42
func GetLabelFileBinDingByLabelIdExists(labelId, userId uint) bool {
	var labelFileBinDing model.LabelFileBinding
	result := db.Where("label_id = ?", labelId).Where("user_id = ?", userId).First(&labelFileBinDing)
	exists := !errors.Is(result.Error, gorm.ErrRecordNotFound)
	return exists
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/internal/runtime/executor/codex_executor.go#L373-L389
// router-for-me/CLIProxyAPI internal/runtime/executor/codex_executor.go:373-389
func metadataString(metadata map[string]any, key string) string {
	if len(metadata) == 0 {
		return ""
	}
	raw, ok := metadata[key]
	if !ok || raw == nil {
		return ""
	}
	switch v := raw.(type) {
	case string:
		return strings.TrimSpace(v)
	case []byte:
		return strings.TrimSpace(string(v))
	default:
		return ""
	}
}

// https://github.com/spf13/cobra/blob/ad460ea8f249db69c943a365fb84f3a59042d54e/args_test.go#L233-L237
// spf13/cobra args_test.go:233-237
func TestMinimumNArgs_WithValid__WithInvalidArgs(t *testing.T) {
	c := getCommand(MinimumNArgs(2), true)
	output, err := executeCommand(c, "a", "b")
	expectSuccess(output, err, t)
}

// https://github.com/junegunn/fzf/blob/dea72834ed35b5d56634075fc7793aa4b9b1d697/src/util/chars.go#L243-L250
// junegunn/fzf src/util/chars.go:243-250
func (chars *Chars) Prepend(prefix string) {
	if runes := chars.optionalRunes(); runes != nil {
		runes = append([]rune(prefix), runes...)
		chars.slice = *(*[]byte)(unsafe.Pointer(&runes))
	} else {
		chars.slice = append([]byte(prefix), chars.slice...)
	}
}

// https://github.com/charmbracelet/bubbletea/blob/a23da80847e6fc928febc62114f761403ac5d2f1/examples/cursor-style/main.go#L73-L79
// charmbracelet/bubbletea examples/cursor-style/main.go:73-79
func main() {
	p := tea.NewProgram(model{blink: true})
	if _, err := p.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v", err)
		os.Exit(1)
	}
}

// https://github.com/wagoodman/dive/blob/d6c691947f8fda635c952a17ee3b7555379d58f0/cmd/dive/cli/internal/command/ci/rules.go#L129-L147
// wagoodman/dive cmd/dive/cli/internal/command/ci/rules.go:129-147
func NewHighestWastedBytesRule(configValue string) (Rule, error) {
	if isRuleDisabled(configValue) {
		return DisabledRule(ciKeyHighestWastedBytes), nil
	}

	threshold, err := humanize.ParseBytes(configValue)
	if err != nil {
		return nil, fmt.Errorf("invalid highestWastedBytes config value, given %q: %v",
			configValue, err)
	}

	return &HighestWastedBytesRule{
		BaseRule: BaseRule{
			key:         ciKeyHighestWastedBytes,
			configValue: configValue,
		},
		threshold: threshold,
	}, nil
}

// https://github.com/nektos/act/blob/4f411281417e88660bea1c1a1749aa71ae0bd60f/pkg/runner/action.go#L493-L504
// nektos/act pkg/runner/action.go:493-504
func shouldRunPreStep(step actionStep) common.Conditional {
	return func(ctx context.Context) bool {
		log := common.Logger(ctx)

		if step.getActionModel() == nil {
			log.Debugf("skip pre step for '%s': no action model available", step.getStepModel())
			return false
		}

		return true
	}
}

// https://github.com/go-gorm/gorm/blob/d0ee5e2296150d691364c5f4f7f2b32abde04545/callbacks/helper_test.go#L41-L97
// go-gorm/gorm callbacks/helper_test.go:41-97
func TestConvertMapToValuesForCreate(t *testing.T) {
	testCase := []struct {
		name   string
		input  map[string]interface{}
		expect clause.Values
	}{
		{
			name: "Test convert string value",
			input: map[string]interface{}{
				"name": "my name",
			},
			expect: clause.Values{
				Columns: []clause.Column{{Name: "name"}},
				Values:  [][]interface{}{{"my name"}},
			},
		},
		{
			name: "Test convert int value",
			input: map[string]interface{}{
				"age": 18,
			},
			expect: clause.Values{
				Columns: []clause.Column{{Name: "age"}},
				Values:  [][]interface{}{{18}},
			},
		},
		{
			name: "Test convert float value",
			input: map[string]interface{}{
				"score": 99.5,
			},
			expect: clause.Values{
				Columns: []clause.Column{{Name: "score"}},
				Values:  [][]interface{}{{99.5}},
			},
		},
		{
			name: "Test convert bool value",
			input: map[string]interface{}{
				"active": true,
			},
			expect: clause.Values{
				Columns: []clause.Column{{Name: "active"}},
				Values:  [][]interface{}{{true}},
			},
		},
	}

	for _, tc := range testCase {
		t.Run(tc.name, func(t *testing.T) {
			actual := ConvertMapToValuesForCreate(&gorm.Statement{}, tc.input)
			if !reflect.DeepEqual(actual, tc.expect) {
				t.Errorf("expect %v got %v", tc.expect, actual)
			}
		})
	}
}

// https://github.com/gin-gonic/gin/blob/d75fcd4c9ab260e5225de590f1f0f8c0e0e12d11/binding/msgpack.go#L31-L37
// gin-gonic/gin binding/msgpack.go:31-37
func decodeMsgPack(r io.Reader, obj any) error {
	cdc := new(codec.MsgpackHandle)
	if err := codec.NewDecoder(r, cdc).Decode(&obj); err != nil {
		return err
	}
	return validate(obj)
}

// https://github.com/FiloSottile/mkcert/blob/d7ab78de71ad2e4d965446a6083ebb48150f0533/truststore_nss.go#L73-L87
// FiloSottile/mkcert truststore_nss.go:73-87
func (m *mkcert) checkNSS() bool {
	if !hasCertutil {
		return false
	}
	success := true
	if m.forEachNSSProfile(func(profile string) {
		err := exec.Command(certutilPath, "-V", "-d", profile, "-u", "L", "-n", m.caUniqueName()).Run()
		if err != nil {
			success = false
		}
	}) == 0 {
		success = false
	}
	return success
}
