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

// https://github.com/gin-gonic/gin/blob/d75fcd4c9ab260e5225de590f1f0f8c0e0e12d11/binding/msgpack.go#L31-L37
// gin-gonic/gin binding/msgpack.go:31-37
func decodeMsgPack(r io.Reader, obj any) error {
	cdc := new(codec.MsgpackHandle)
	if err := codec.NewDecoder(r, cdc).Decode(&obj); err != nil {
		return err
	}
	return validate(obj)
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/internal/translator/antigravity/openai/responses/antigravity_openai-responses_request_test.go#L161-L169
// router-for-me/CLIProxyAPI internal/translator/antigravity/openai/responses/antigravity_openai-responses_request_test.go:161-169
func testAntigravityResponsesGPTSignature() string {
	payload := make([]byte, 1+8+16+16+32)
	payload[0] = 0x80
	payload[8] = 1
	for i := 9; i < len(payload); i++ {
		payload[i] = byte(i)
	}
	return base64.URLEncoding.EncodeToString(payload)
}

// https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/internal/db/label_file_binding.go#L37-L42
// AlistGo/alist internal/db/label_file_binding.go:37-42
func GetLabelFileBinDingByLabelIdExists(labelId, userId uint) bool {
	var labelFileBinDing model.LabelFileBinding
	result := db.Where("label_id = ?", labelId).Where("user_id = ?", userId).First(&labelFileBinDing)
	exists := !errors.Is(result.Error, gorm.ErrRecordNotFound)
	return exists
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

// https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/pkg/singleflight/signleflight_test.go#L22-L33
// AlistGo/alist pkg/singleflight/signleflight_test.go:22-33
func TestDo(t *testing.T) {
	var g Group[string]
	v, err, _ := g.Do("key", func() (string, error) {
		return "bar", nil
	})
	if got, want := fmt.Sprintf("%v (%T)", v, v), "bar (string)"; got != want {
		t.Errorf("Do = %v; want %v", got, want)
	}
	if err != nil {
		t.Errorf("Do error = %v", err)
	}
}

// https://github.com/spf13/cobra/blob/ad460ea8f249db69c943a365fb84f3a59042d54e/args_test.go#L233-L237
// spf13/cobra args_test.go:233-237
func TestMinimumNArgs_WithValid__WithInvalidArgs(t *testing.T) {
	c := getCommand(MinimumNArgs(2), true)
	output, err := executeCommand(c, "a", "b")
	expectSuccess(output, err, t)
}

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/internal/runtime/executor/codex_websockets_executor.go#L1290-L1306
// router-for-me/CLIProxyAPI internal/runtime/executor/codex_websockets_executor.go:1290-1306
func executionSessionIDFromOptions(opts cliproxyexecutor.Options) string {
	if len(opts.Metadata) == 0 {
		return ""
	}
	raw, ok := opts.Metadata[cliproxyexecutor.ExecutionSessionMetadataKey]
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

// https://github.com/router-for-me/CLIProxyAPI/blob/ed52c6147cdffdf18a9fe0cea106616a83113412/internal/thinking/validate.go#L330-L336
// router-for-me/CLIProxyAPI internal/thinking/validate.go:330-336
func normalizeLevels(levels []string) []string {
	out := make([]string, len(levels))
	for i, l := range levels {
		out[i] = strings.ToLower(strings.TrimSpace(l))
	}
	return out
}

// https://github.com/AlistGo/alist/blob/d0cec67718d9b0f3750715fe850f6c0ba9e9e87f/drivers/wukong/driver.go#L824-L829
// AlistGo/alist drivers/wukong/driver.go:824-829
func getSigningKey(secret, dateStamp, region, service string) []byte {
	kDate := hmacSHA256([]byte("AWS4"+secret), dateStamp)
	kRegion := hmacSHA256(kDate, region)
	kService := hmacSHA256(kRegion, service)
	return hmacSHA256(kService, "aws4_request")
}
