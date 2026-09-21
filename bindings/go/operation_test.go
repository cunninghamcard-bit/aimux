package aimux

import (
	"context"
	"encoding/json"
	"errors"
	"strings"
	"testing"
	"time"
)

func repairOptions(repair ToolCallRepairFunc) *GenerateTextOptions {
	return &GenerateTextOptions{Tools: []Tool{{Type: "function", Name: "get_weather", InputSchema: json.RawMessage(`{"type":"object"}`)}}, RepairToolCall: repair}
}
func repairFixed(c ToolCallRepairContext) (*RawToolCall, error) {
	v := c.ToolCall
	v.Input = `{"location":"北京"}`
	return &v, nil
}

func TestHostOperationNestedGenerate(t *testing.T) {
	srv := newMockServer()
	defer srv.Close()
	srv.SetResponse(strings.ReplaceAll(toolCallOpenAIResponse, `{\"location\":\"Tokyo\"}`, "{"))
	model := OpenAIWithBase("fake", "mock", srv.URL)
	defer model.Close()
	called := false
	result, err := model.Generate("hi", repairOptions(func(c ToolCallRepairContext) (*RawToolCall, error) {
		called = true
		nested, err := model.Generate("nested", repairOptions(repairFixed))
		if err != nil {
			return nil, err
		}
		if nested.ToolCalls[0].Invalid != nil && *nested.ToolCalls[0].Invalid {
			return nil, errors.New("nested call still invalid")
		}
		return repairFixed(c)
	}))
	if err != nil {
		t.Fatal(err)
	}
	if !called || string(result.ToolCalls[0].Input) != `{"location":"北京"}` {
		t.Fatalf("repair result: %+v", result.ToolCalls)
	}
}

func TestHostOperationDeadlineCancelsLocalContext(t *testing.T) {
	srv := newMockServer()
	defer srv.Close()
	srv.SetResponse(strings.ReplaceAll(toolCallOpenAIResponse, `{\"location\":\"Tokyo\"}`, "{"))
	model := OpenAIWithBase("fake", "mock", srv.URL)
	defer model.Close()
	exited := make(chan struct{})
	options := repairOptions(func(c ToolCallRepairContext) (*RawToolCall, error) {
		<-c.Context.Done()
		close(exited)
		return nil, c.Context.Err()
	})
	total := uint64(200)
	options.Timeout = &TimeoutConfiguration{TotalMs: &total}
	_, err := model.Generate("hi", options)
	var core *Error
	if !errors.As(err, &core) || core.Code != CodeTimeout {
		t.Fatalf("expected Timeout, got %v", err)
	}
	select {
	case <-exited:
	case <-time.After(time.Second):
		t.Fatal("host context was not cancelled")
	}
}

func TestHostOperationStreamCancelDuringRepair(t *testing.T) {
	srv := newMockServer()
	defer srv.Close()
	srv.SetContentType("text/event-stream")
	srv.SetResponse(strings.ReplaceAll(buildToolCallSSE(), `{\"location\":\"Tokyo\"}`, "{"))
	model := OpenAIWithBase("fake", "mock", srv.URL)
	defer model.Close()
	entered := make(chan struct{})
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	stream, err := model.StreamContext(ctx, "hi", repairOptions(func(c ToolCallRepairContext) (*RawToolCall, error) {
		close(entered)
		<-c.Context.Done()
		return repairFixed(c)
	}))
	if err != nil {
		t.Fatal(err)
	}
	select {
	case <-entered:
	case <-time.After(time.Second):
		t.Fatal("repair did not start")
	}
	cancel()
	for range stream.Parts() {
	}
	if !errors.Is(stream.Err(), context.Canceled) {
		t.Fatalf("expected cancellation, got %v", stream.Err())
	}
}
