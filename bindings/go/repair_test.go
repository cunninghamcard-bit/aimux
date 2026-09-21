// Tool-call repair tests (RFC-0035): the shared contract fixture replayed
// against the three native functions, plus the host loop end-to-end against
// the mock provider server.

package aimux

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

// ── Contract fixture replay ─────────────────────────────────────────────────

type repairFixture struct {
	Cases []struct {
		Name          string          `json:"name"`
		Function      string          `json:"function"`
		Input         json.RawMessage `json:"input"`
		Expected      json.RawMessage `json:"expected"`
		ExpectedError string          `json:"expected_error"`
	} `json:"cases"`
}

// jsonString serializes a fixture value the way a host would before handing
// it to the C ABI. A null `opts` becomes "" (defaults).
func jsonString(v json.RawMessage) string {
	if len(v) == 0 || string(v) == "null" {
		return ""
	}
	return string(v)
}

func assertJSONEqual(t *testing.T, name, got string, want json.RawMessage) {
	t.Helper()
	var g, w any
	if err := json.Unmarshal([]byte(got), &g); err != nil {
		t.Fatalf("%s: output is not JSON: %v (%s)", name, err, got)
	}
	if err := json.Unmarshal(want, &w); err != nil {
		t.Fatalf("%s: fixture expectation is not JSON: %v", name, err)
	}
	if !reflect.DeepEqual(g, w) {
		t.Errorf("%s:\n got %s\nwant %s", name, got, want)
	}
}

func TestRepairContractFixture(t *testing.T) {
	path := filepath.Join("..", "..", "contract-tests", "fixtures", "tool-call-repair.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	var fixture repairFixture
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatalf("parse fixture: %v", err)
	}
	if len(fixture.Cases) == 0 {
		t.Fatal("fixture has no cases")
	}

	for _, c := range fixture.Cases {
		t.Run(c.Name, func(t *testing.T) {
			var in struct {
				ToolCall   json.RawMessage `json:"tool_call"`
				Prompt     json.RawMessage `json:"prompt"`
				Opts       json.RawMessage `json:"opts"`
				Reply      json.RawMessage `json:"reply"`
				Result     json.RawMessage `json:"result"`
				ToolCallID string          `json:"tool_call_id"`
			}
			if err := json.Unmarshal(c.Input, &in); err != nil {
				t.Fatalf("parse case input: %v", err)
			}

			var got string
			var callErr error
			switch c.Function {
			case "tool_call_repair_context":
				got, callErr = toolCallRepairContextJSON(string(in.ToolCall), string(in.Prompt), jsonString(in.Opts))
			case "apply_tool_call_repair":
				got, callErr = applyToolCallRepairJSON(string(in.ToolCall), jsonString(in.Opts), string(in.Reply))
			case "apply_tool_call_repair_to_result":
				got, callErr = applyToolCallRepairToResultJSON(string(in.Result), jsonString(in.Opts), in.ToolCallID, string(in.Reply))
			default:
				t.Fatalf("unknown fixture function %q", c.Function)
			}

			if c.ExpectedError != "" {
				var e *Error
				if !errors.As(callErr, &e) {
					t.Fatalf("expected %s, got err=%v out=%s", c.ExpectedError, callErr, got)
				}
				if e.Code.String() != c.ExpectedError {
					t.Fatalf("expected %s, got %s: %v", c.ExpectedError, e.Code, e)
				}
				return
			}
			if callErr != nil {
				t.Fatalf("unexpected error: %v", callErr)
			}
			assertJSONEqual(t, c.Name, got, c.Expected)
		})
	}
}

// ── End-to-end host loop ────────────────────────────────────────────────────

// strictWeatherOpts builds options whose weather tool rejects anything but
// {"city": …}, so a {"town": …} call comes back invalid.
func strictWeatherOpts(repair RepairToolCallFunc) *GenerateTextOptions {
	schema := json.RawMessage(`{"type":"object","properties":{"city":{"type":"string"}},` +
		`"required":["city"],"additionalProperties":false}`)
	return &GenerateTextOptions{
		Tools: []Tool{{
			Type:        "function",
			Name:        "weather",
			InputSchema: schema,
		}},
		RepairToolCall: repair,
	}
}

// townToolCallResponse is an OpenAI response calling `weather` with the wrong
// argument name.
const townToolCallResponse = `{
  "id": "chatcmpl-repair",
  "model": "gpt-4o",
  "choices": [{
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "id": "call-1",
        "type": "function",
        "function": {"name": "weather", "arguments": "{\"town\":\"Singapore\"}"}
      }]
    },
    "finish_reason": "tool_calls"
  }],
  "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
}`

func repairModel(t *testing.T, response string) *Model {
	t.Helper()
	srv := newMockServer()
	t.Cleanup(srv.Close)
	srv.SetResponse(response)
	m := OpenAIWithBase("sk-test-fake-key", "gpt-4o", srv.URL)
	t.Cleanup(func() { _ = m.Close() })
	return m
}

// toCity is the canonical repair: rewrite the arguments to the schema.
func toCity(ctx *ToolCallRepairContext) (*RawToolCall, error) {
	return &RawToolCall{
		ToolCallID: ctx.ToolCall.ToolCallID,
		ToolName:   ctx.ToolCall.ToolName,
		Input:      `{"city":"Singapore"}`,
	}, nil
}

func TestRepairGenerateRepairs(t *testing.T) {
	m := repairModel(t, townToolCallResponse)

	var seen *ToolCallRepairContext
	opts := strictWeatherOpts(func(ctx *ToolCallRepairContext) (*RawToolCall, error) {
		seen = ctx
		return toCity(ctx)
	})

	result, err := m.Generate("weather in Singapore?", opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	if seen == nil {
		t.Fatal("repair function was not invoked")
	}
	// The context carries the provider's raw argument text and the transcript.
	if seen.ToolCall.Input != `{"town":"Singapore"}` {
		t.Errorf("raw input: got %q", seen.ToolCall.Input)
	}
	if len(seen.Tools) != 1 || seen.Tools[0].Name != "weather" {
		t.Errorf("tools: got %+v", seen.Tools)
	}
	if len(seen.Messages) != 1 {
		t.Errorf("messages: got %+v", seen.Messages)
	}
	if len(seen.Error) == 0 {
		t.Error("expected the original error in the context")
	}

	if len(result.ToolCalls) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(result.ToolCalls))
	}
	call := result.ToolCalls[0]
	if call.Invalid != nil && *call.Invalid {
		t.Errorf("expected a valid call after repair, got %+v", call)
	}
	if string(call.Input) != `{"city":"Singapore"}` {
		t.Errorf("repaired input: got %s", call.Input)
	}
	// The transcript must agree, or the next turn resends the bad arguments.
	replayed, _ := json.Marshal(result.ResponseMessages)
	if !strings.Contains(string(replayed), `"city":"Singapore"`) || strings.Contains(string(replayed), `"town"`) {
		t.Errorf("response_messages not patched: %s", replayed)
	}
}

func TestRepairUnchangedKeepsOriginalError(t *testing.T) {
	m := repairModel(t, townToolCallResponse)
	called := false
	opts := strictWeatherOpts(func(ctx *ToolCallRepairContext) (*RawToolCall, error) {
		called = true
		return nil, nil
	})

	result, err := m.Generate("weather in Singapore?", opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	if !called {
		t.Fatal("repair function was not invoked")
	}
	call := result.ToolCalls[0]
	if call.Invalid == nil || !*call.Invalid {
		t.Fatalf("expected the call to stay invalid, got %+v", call)
	}
	if !strings.Contains(string(call.Error), "InvalidToolInput") {
		t.Errorf("expected the original error, got %s", call.Error)
	}
}

func TestRepairFailedCarriesCause(t *testing.T) {
	m := repairModel(t, townToolCallResponse)
	opts := strictWeatherOpts(func(ctx *ToolCallRepairContext) (*RawToolCall, error) {
		return nil, errors.New("repair model unavailable")
	})

	result, err := m.Generate("weather in Singapore?", opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	call := result.ToolCalls[0]
	if call.Invalid == nil || !*call.Invalid {
		t.Fatalf("expected the call to stay invalid, got %+v", call)
	}
	if !strings.Contains(string(call.Error), "ToolCallRepair") ||
		!strings.Contains(string(call.Error), "repair model unavailable") {
		t.Errorf("expected a ToolCallRepair error with the cause, got %s", call.Error)
	}
	if !strings.Contains(string(call.Error), "original_error") {
		t.Errorf("expected the original error to be preserved, got %s", call.Error)
	}
}

func TestRepairStillInvalidAfterRepair(t *testing.T) {
	m := repairModel(t, townToolCallResponse)
	opts := strictWeatherOpts(func(ctx *ToolCallRepairContext) (*RawToolCall, error) {
		// Still wrong: a second attempt is never made (AI SDK rule).
		return &RawToolCall{
			ToolCallID: ctx.ToolCall.ToolCallID,
			ToolName:   ctx.ToolCall.ToolName,
			Input:      `{"village":"Singapore"}`,
		}, nil
	})

	result, err := m.Generate("weather in Singapore?", opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	call := result.ToolCalls[0]
	if call.Invalid == nil || !*call.Invalid {
		t.Fatalf("expected the call to stay invalid, got %+v", call)
	}
	if !strings.Contains(string(call.Error), "ToolCallRepair") {
		t.Errorf("expected a ToolCallRepair error, got %s", call.Error)
	}
}

func TestRepairSkippedWithoutTools(t *testing.T) {
	m := repairModel(t, townToolCallResponse)
	called := false
	// No tools: the call is never a repair candidate (the context function
	// answers null), so the host must not invoke the repair function.
	opts := &GenerateTextOptions{RepairToolCall: func(ctx *ToolCallRepairContext) (*RawToolCall, error) {
		called = true
		return nil, nil
	}}

	if _, err := m.Generate("weather in Singapore?", opts); err != nil {
		t.Fatalf("Generate: %v", err)
	}
	if called {
		t.Error("repair function must not run for a call made without a tool set")
	}
}

func TestRepairSkippedForValidCall(t *testing.T) {
	m := repairModel(t, townToolCallResponse)
	called := false
	// A permissive schema makes the same call valid.
	opts := &GenerateTextOptions{
		Tools: []Tool{{Type: "function", Name: "weather", InputSchema: json.RawMessage(`{"type":"object"}`)}},
		RepairToolCall: func(ctx *ToolCallRepairContext) (*RawToolCall, error) {
			called = true
			return nil, nil
		},
	}

	result, err := m.Generate("weather in Singapore?", opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	if called {
		t.Error("repair function must not run for a valid call")
	}
	if c := result.ToolCalls[0]; c.Invalid != nil && *c.Invalid {
		t.Errorf("expected a valid call, got %+v", c)
	}
}

// TestRepairFunctionMayCallAimux proves the repair function runs outside any
// native call: it can drive a second generation to produce the fix.
func TestRepairFunctionMayCallAimux(t *testing.T) {
	m := repairModel(t, townToolCallResponse)
	helper := repairModel(t, plainOpenAIResponse)

	opts := strictWeatherOpts(func(ctx *ToolCallRepairContext) (*RawToolCall, error) {
		if _, err := helper.Generate("fix these arguments: "+ctx.ToolCall.Input, nil); err != nil {
			return nil, err
		}
		return toCity(ctx)
	})

	result, err := m.Generate("weather in Singapore?", opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	if string(result.ToolCalls[0].Input) != `{"city":"Singapore"}` {
		t.Errorf("expected the repaired input, got %s", result.ToolCalls[0].Input)
	}
}

// ── Streaming ───────────────────────────────────────────────────────────────

func buildTownToolCallSSE() string {
	return "data: " + `{"id":"1","model":"gpt-4o","choices":[{"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"weather","arguments":""}}]}}]}` + "\n\n" +
		"data: " + `{"id":"1","model":"gpt-4o","choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"town\":\"Singapore\"}"}}]}}]}` + "\n\n" +
		"data: " + `{"id":"1","model":"gpt-4o","choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}` + "\n\n" +
		"data: [DONE]\n\n"
}

func TestRepairStreamReplacesToolCallPart(t *testing.T) {
	srv := newMockServer()
	defer srv.Close()
	srv.SetContentType("text/event-stream")
	srv.SetResponse(buildTownToolCallSSE())

	m := OpenAIWithBase("sk-test-fake-key", "gpt-4o", srv.URL)
	defer m.Close()

	opts := strictWeatherOpts(toCity)
	stream, err := m.Stream("weather in Singapore?", opts)
	if err != nil {
		t.Fatalf("Stream: %v", err)
	}

	var toolCalls, deltas []string
	for part := range stream.Parts() {
		switch part.Tag {
		case "ToolCall":
			toolCalls = append(toolCalls, string(part.Payload))
		case "ToolInputDelta":
			deltas = append(deltas, string(part.Payload))
		}
	}
	if err := stream.Err(); err != nil {
		t.Fatalf("stream error: %v", err)
	}
	if len(toolCalls) != 1 {
		t.Fatalf("expected 1 ToolCall part, got %d", len(toolCalls))
	}
	if !strings.Contains(toolCalls[0], `"city":"Singapore"`) || strings.Contains(toolCalls[0], `"invalid"`) {
		t.Errorf("expected the repaired call, got %s", toolCalls[0])
	}
	// Input deltas are forwarded verbatim — the provider's text, not the fix.
	for _, d := range deltas {
		if strings.Contains(d, "city") {
			t.Errorf("input delta was rewritten: %s", d)
		}
	}
	if len(deltas) > 0 && !strings.Contains(fmt.Sprint(deltas), "town") {
		t.Errorf("expected the provider's raw deltas, got %v", deltas)
	}
}

// TestRepairConsumeStreamAggregate covers the host-side aggregate: the
// repaired call must be the one the aggregated result carries.
func TestRepairConsumeStreamAggregate(t *testing.T) {
	srv := newMockServer()
	defer srv.Close()
	srv.SetContentType("text/event-stream")
	srv.SetResponse(buildTownToolCallSSE())

	m := OpenAIWithBase("sk-test-fake-key", "gpt-4o", srv.URL)
	defer m.Close()

	result, err := m.ConsumeStream("weather in Singapore?", strictWeatherOpts(toCity))
	if err != nil {
		t.Fatalf("ConsumeStream: %v", err)
	}
	if len(result.ToolCalls) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(result.ToolCalls))
	}
	if string(result.ToolCalls[0].Input) != `{"city":"Singapore"}` {
		t.Errorf("expected the repaired input, got %s", result.ToolCalls[0].Input)
	}
}

// TestRepairIsNotSerialized guards the one thing that would break every call:
// a func field reaching json.Marshal.
func TestRepairIsNotSerialized(t *testing.T) {
	optsJSON, err := MarshalOptions(strictWeatherOpts(toCity))
	if err != nil {
		t.Fatalf("MarshalOptions: %v", err)
	}
	if strings.Contains(optsJSON, "epair") {
		t.Errorf("repair function leaked into the options JSON: %s", optsJSON)
	}
}
