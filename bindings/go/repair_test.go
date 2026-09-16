package aimux

import (
	"encoding/json"
	"errors"
	"strings"
	"testing"
)

// The model drops the closing brace of the arguments.
const malformedToolCallOpenAIResponse = `{
  "id": "chatcmpl-bad",
  "model": "gpt-4o",
  "choices": [{
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "id": "call_bad",
        "type": "function",
        "function": {"name": "get_weather", "arguments": "{\"location\":\"Tokyo\""}
      }]
    },
    "finish_reason": "tool_calls"
  }],
  "usage": {"prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30}
}`

func weatherTools() []Tool {
	return []Tool{{
		Type:        "function",
		Name:        "get_weather",
		InputSchema: json.RawMessage(`{"type":"object","properties":{"location":{"type":"string"}},"required":["location"]}`),
	}}
}

func generateWithRepair(t *testing.T, repair *ToolCallRepair) *GenerateTextResult {
	t.Helper()
	srv := newMockServer()
	defer srv.Close()
	srv.SetResponse(malformedToolCallOpenAIResponse)

	m := OpenAIWithBase("sk-test-fake-key", "gpt-4o", srv.URL)
	defer m.Close()

	opts, err := MarshalOptions(&GenerateTextOptions{Tools: weatherTools(), RepairToolCall: repair})
	if err != nil {
		t.Fatal(err)
	}
	result, err := m.GenerateText(`"What is the weather in Tokyo?"`, opts)
	if err != nil {
		t.Fatalf("generate_text failed: %v", err)
	}
	parsed, err := ParseGenerateTextResult(result)
	if err != nil {
		t.Fatalf("parse failed: %v", err)
	}
	if len(parsed.ToolCalls) != 1 {
		t.Fatalf("expected one tool call, got %d", len(parsed.ToolCalls))
	}
	return parsed
}

func TestRepairToolCall_GoFunctionFixesMalformedArguments(t *testing.T) {
	calls := 0
	repair := NewToolCallRepair(func(ctx ToolCallRepairContext) (*RawToolCall, error) {
		calls++
		if ctx.ToolCall.ToolName != "get_weather" {
			t.Errorf("tool name: %q", ctx.ToolCall.ToolName)
		}
		if !json.Valid(ctx.InputSchema) || len(ctx.Tools) != 1 {
			t.Errorf("context missing schema/tools: %s / %d", ctx.InputSchema, len(ctx.Tools))
		}
		var e map[string]json.RawMessage
		if json.Unmarshal(ctx.Error, &e) != nil || e["InvalidToolInput"] == nil {
			t.Errorf("expected InvalidToolInput, got %s", ctx.Error)
		}
		fixed := ctx.ToolCall
		fixed.Input += "}"
		return &fixed, nil
	})
	defer repair.Close()

	tc := generateWithRepair(t, repair).ToolCalls[0]
	if calls != 1 {
		t.Errorf("repair called %d times, want 1", calls)
	}
	if tc.Invalid != nil {
		t.Errorf("tool call still invalid: %s", tc.Error)
	}
	var input map[string]string
	if err := json.Unmarshal(tc.Input, &input); err != nil || input["location"] != "Tokyo" {
		t.Errorf("input not repaired: %s (%v)", tc.Input, err)
	}
}

func TestRepairToolCall_NilKeepsTheOriginalError(t *testing.T) {
	repair := NewToolCallRepair(func(ToolCallRepairContext) (*RawToolCall, error) {
		return nil, nil
	})
	defer repair.Close()

	tc := generateWithRepair(t, repair).ToolCalls[0]
	if tc.Invalid == nil || !*tc.Invalid {
		t.Fatalf("expected invalid tool call, got %s", tc.Input)
	}
	if string(tc.Input) != `"{\"location\":\"Tokyo\""` {
		t.Errorf("raw text not preserved: %s", tc.Input)
	}
}

func TestRepairToolCall_GoErrorBecomesAToolCallRepairError(t *testing.T) {
	repair := NewToolCallRepair(func(ToolCallRepairContext) (*RawToolCall, error) {
		return nil, errors.New("boom")
	})
	defer repair.Close()

	tc := generateWithRepair(t, repair).ToolCalls[0]
	if tc.Invalid == nil || !*tc.Invalid {
		t.Fatalf("expected an invalid tool call, got %s", tc.Input)
	}
	var e struct {
		ToolCallRepair *struct {
			Cause json.RawMessage `json:"cause"`
		} `json:"ToolCallRepair"`
	}
	if err := json.Unmarshal(tc.Error, &e); err != nil || e.ToolCallRepair == nil {
		t.Fatalf("expected a ToolCallRepair error, got %s (%v)", tc.Error, err)
	}
	if !strings.Contains(string(e.ToolCallRepair.Cause), "boom") {
		t.Errorf("cause lost the Go error: %s", e.ToolCallRepair.Cause)
	}
}

func TestRepairToolCall_ClosedMarshalsAsNull(t *testing.T) {
	repair := NewToolCallRepair(func(ToolCallRepairContext) (*RawToolCall, error) { return nil, nil })
	if err := repair.Close(); err != nil {
		t.Fatal(err)
	}
	opts, err := MarshalOptions(&GenerateTextOptions{RepairToolCall: repair})
	if err != nil {
		t.Fatal(err)
	}
	if opts != `{"repair_tool_call":null}` {
		t.Errorf("closed repair marshaled as %s", opts)
	}
	// Closing twice is a no-op.
	_ = repair.Close()
}

// The model streams the same truncated arguments; repair must run on the
// streaming path too, and the repaired call is what reaches the Parts channel.
func buildMalformedToolCallSSE() string {
	chunk1 := `{"id":"1","model":"gpt-4o","choices":[{"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_bad","type":"function","function":{"name":"get_weather","arguments":""}}]}}]}`
	chunk2 := `{"id":"1","model":"gpt-4o","choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"location\":\"Tokyo\""}}]}}]}`
	chunk3 := `{"id":"1","model":"gpt-4o","choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}`
	var sb strings.Builder
	sb.WriteString("data: " + chunk1 + "\n\n")
	sb.WriteString("data: " + chunk2 + "\n\n")
	sb.WriteString("data: " + chunk3 + "\n\n")
	sb.WriteString("data: [DONE]\n\n")
	return sb.String()
}

func TestRepairToolCall_RunsOnTheStreamingPath(t *testing.T) {
	srv := newMockServer()
	defer srv.Close()
	srv.SetContentType("text/event-stream")
	srv.SetResponse(buildMalformedToolCallSSE())

	m := OpenAIWithBase("sk-test-fake-key", "gpt-4o", srv.URL)
	defer m.Close()

	calls := 0
	repair := NewToolCallRepair(func(ctx ToolCallRepairContext) (*RawToolCall, error) {
		calls++
		fixed := ctx.ToolCall
		fixed.Input += "}"
		return &fixed, nil
	})
	defer repair.Close()

	opts, err := MarshalOptions(&GenerateTextOptions{Tools: weatherTools(), RepairToolCall: repair})
	if err != nil {
		t.Fatal(err)
	}
	stream := m.StreamText(`"What is the weather in Tokyo?"`, opts)
	var toolCall json.RawMessage
	for part := range stream.Parts() {
		var sp struct {
			ToolCall json.RawMessage `json:"ToolCall"`
		}
		if json.Unmarshal([]byte(part), &sp) == nil && sp.ToolCall != nil {
			toolCall = sp.ToolCall
		}
	}
	if err := stream.Err(); err != nil {
		t.Fatalf("stream error: %v", err)
	}
	if calls != 1 {
		t.Fatalf("repair ran %d times, want 1", calls)
	}
	var tc ToolCall
	if err := json.Unmarshal(toolCall, &tc); err != nil {
		t.Fatalf("no ToolCall part: %v", err)
	}
	if tc.Invalid != nil && *tc.Invalid {
		t.Fatalf("tool call still invalid: %s", tc.Error)
	}
	if string(tc.Input) != `{"location":"Tokyo"}` {
		t.Errorf("repaired input = %s", tc.Input)
	}
}

// The repair function runs on the calling goroutine inside the FFI
// re-entrancy guard, so a nested aimux call is refused with code 204 rather
// than deadlocking.
func TestRepairToolCall_CallingBackIntoAimuxIsRejectedAsReentrant(t *testing.T) {
	srv := newMockServer()
	defer srv.Close()
	srv.SetResponse(malformedToolCallOpenAIResponse)

	m := OpenAIWithBase("sk-test-fake-key", "gpt-4o", srv.URL)
	defer m.Close()

	var nested error
	repair := NewToolCallRepair(func(ToolCallRepairContext) (*RawToolCall, error) {
		_, nested = m.GenerateText(`"hi"`, "")
		return nil, nested
	})
	defer repair.Close()

	opts, err := MarshalOptions(&GenerateTextOptions{Tools: weatherTools(), RepairToolCall: repair})
	if err != nil {
		t.Fatal(err)
	}
	result, err := m.GenerateText(`"What is the weather in Tokyo?"`, opts)
	if err != nil {
		t.Fatalf("outer generate_text failed: %v", err)
	}
	// Go maps every C ABI failure (200..206) to a plain error carrying the
	// Rust message, so 204 shows up as its text rather than as a code.
	if nested == nil || !strings.Contains(nested.Error(), "re-entrant") {
		t.Fatalf("nested call: want the re-entrant FFI error, got %v", nested)
	}
	parsed, err := ParseGenerateTextResult(result)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(parsed.ToolCalls[0].Error), "ToolCallRepair") {
		t.Errorf("outer tool call error = %s", parsed.ToolCalls[0].Error)
	}
}
