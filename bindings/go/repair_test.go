package aimux

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

type repairFixture struct {
	Cases []struct {
		Name          string          `json:"name"`
		Function      string          `json:"function"`
		Input         json.RawMessage `json:"input"`
		Expected      json.RawMessage `json:"expected"`
		ExpectedError string          `json:"expected_error"`
	} `json:"cases"`
}

func jsonString(v json.RawMessage) string {
	if len(v) == 0 || string(v) == "null" {
		return ""
	}
	return string(v)
}

func TestRepairContractFixture(t *testing.T) {
	raw, err := os.ReadFile(filepath.Join("..", "..", "contract-tests", "fixtures", "tool-call-repair.json"))
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
				if !errors.As(callErr, &e) || e.Code.String() != c.ExpectedError {
					t.Fatalf("expected %s, got err=%v out=%s", c.ExpectedError, callErr, got)
				}
				return
			}
			if callErr != nil {
				t.Fatalf("unexpected error: %v", callErr)
			}
			var actual, expected any
			if err := json.Unmarshal([]byte(got), &actual); err != nil {
				t.Fatalf("output is not JSON: %v", err)
			}
			if err := json.Unmarshal(c.Expected, &expected); err != nil {
				t.Fatalf("fixture expectation is not JSON: %v", err)
			}
			if !reflect.DeepEqual(actual, expected) {
				t.Errorf("got %s\nwant %s", got, c.Expected)
			}
		})
	}
}

func strictWeatherOpts(repair RepairToolCallFunc) *GenerateTextOptions {
	return &GenerateTextOptions{
		Tools: []Tool{{Type: "function", Name: "weather", InputSchema: json.RawMessage(
			`{"type":"object","properties":{"city":{"type":"string"}},"required":["city"],"additionalProperties":false}`)}},
		RepairToolCall: repair,
	}
}

const townToolCallResponse = `{"id":"chatcmpl-repair","model":"gpt-4o","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"weather","arguments":"{\"town\":\"Singapore\"}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}`

func toCity(ctx *ToolCallRepairContext) (*RawToolCall, error) {
	return &RawToolCall{ToolCallID: ctx.ToolCall.ToolCallID, ToolName: ctx.ToolCall.ToolName, Input: `{"city":"Singapore"}`}, nil
}

func TestRepairGenerateRepairs(t *testing.T) {
	srv := newMockServer()
	defer srv.Close()
	srv.SetResponse(townToolCallResponse)
	m := OpenAIWithBase("sk-test-fake-key", "gpt-4o", srv.URL)
	defer m.Close()

	var seen *ToolCallRepairContext
	result, err := m.Generate("weather in Singapore?", strictWeatherOpts(func(ctx *ToolCallRepairContext) (*RawToolCall, error) {
		seen = ctx
		return toCity(ctx)
	}))
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	if seen == nil || seen.ToolCall.Input != `{"town":"Singapore"}` {
		t.Fatalf("repair did not receive provider raw input: %+v", seen)
	}
	if len(result.ToolCalls) != 1 || string(result.ToolCalls[0].Input) != `{"city":"Singapore"}` {
		t.Fatalf("unexpected repaired calls: %+v", result.ToolCalls)
	}
	transcript, _ := json.Marshal(result.ResponseMessages)
	if !strings.Contains(string(transcript), `"city":"Singapore"`) || strings.Contains(string(transcript), `"town"`) {
		t.Errorf("response_messages disagree with tool_calls: %s", transcript)
	}
}

const townToolCallSSE = "data: " + `{"id":"1","model":"gpt-4o","choices":[{"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"weather","arguments":""}}]}}]}` + "\n\n" +
	"data: " + `{"id":"1","model":"gpt-4o","choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"town\":\"Singapore\"}"}}]}}]}` + "\n\n" +
	"data: " + `{"id":"1","model":"gpt-4o","choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}` + "\n\n" +
	"data: [DONE]\n\n"

func TestRepairStreamReplacesToolCallPart(t *testing.T) {
	srv := newMockServer()
	defer srv.Close()
	srv.SetContentType("text/event-stream")
	srv.SetResponse(townToolCallSSE)
	m := OpenAIWithBase("sk-test-fake-key", "gpt-4o", srv.URL)
	defer m.Close()

	stream, err := m.Stream("weather in Singapore?", strictWeatherOpts(toCity))
	if err != nil {
		t.Fatalf("Stream: %v", err)
	}
	var toolCall string
	var deltas []string
	for part := range stream.Parts() {
		if part.Tag == "ToolCall" {
			toolCall = string(part.Payload)
		} else if part.Tag == "ToolInputDelta" {
			var delta TextDeltaPayload
			if err := json.Unmarshal(part.Payload, &delta); err != nil {
				t.Fatalf("decode ToolInputDelta: %v", err)
			}
			deltas = append(deltas, delta.Delta)
		}
	}
	if err := stream.Err(); err != nil {
		t.Fatalf("stream error: %v", err)
	}
	if !strings.Contains(toolCall, `"city":"Singapore"`) || strings.Contains(toolCall, `"invalid"`) {
		t.Errorf("expected repaired ToolCall, got %s", toolCall)
	}
	if got := strings.Join(deltas, ""); got != `{"town":"Singapore"}` {
		t.Errorf("provider deltas were changed: %s", got)
	}
}
