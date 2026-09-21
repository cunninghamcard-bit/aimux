package aimux

/*
#include <stdlib.h>
#include "aimux-ffi.h"
*/
import "C"

import (
	"context"
	"encoding/json"
	"fmt"
	"runtime"
	"unsafe"
)

// RawToolCall is the provider call before argument parsing and validation.
type RawToolCall struct {
	ToolCallID       string          `json:"tool_call_id"`
	ToolName         string          `json:"tool_name"`
	Input            string          `json:"input"`
	ProviderExecuted *bool           `json:"provider_executed,omitempty"`
	Dynamic          *bool           `json:"dynamic,omitempty"`
	ThoughtSignature *string         `json:"thought_signature,omitempty"`
	ProviderMetadata json.RawMessage `json:"provider_metadata,omitempty"`
}

// ToolCallRepairContext is an owned snapshot. Context is cancelled when the
// operation ends; it is local to Go and never crosses the native boundary.
type ToolCallRepairContext struct {
	Context      context.Context `json:"-"`
	ToolCall     RawToolCall     `json:"tool_call"`
	Error        json.RawMessage `json:"error"`
	InputSchema  json.RawMessage `json:"input_schema"`
	Tools        []Tool          `json:"tools"`
	Messages     []ModelMessage  `json:"messages"`
	Instructions *string         `json:"instructions,omitempty"`
}

// ToolCallRepairFunc executes in a Go goroutine, outside a cgo callback.
// Return nil, nil to retain the original invalid call. Observe Context for
// cancellation; Go cannot forcibly stop a function that ignores it.
type ToolCallRepairFunc func(ToolCallRepairContext) (*RawToolCall, error)

type operationEvent struct {
	Type      string                `json:"type"`
	RequestID string                `json:"request_id"`
	Context   ToolCallRepairContext `json:"context"`
	Part      json.RawMessage       `json:"part"`
	Result    json.RawMessage       `json:"result"`
}

func operationNext(handle uint64, lane int) (*operationEvent, error) {
	var out *C.char
	var state C.int32_t
	if err := expectAimuxError(C.aimux_operation_next(C.uint64_t(handle), C.int32_t(lane), -1, &out, &state)); err != nil {
		return nil, err
	}
	if state == 2 {
		return nil, nil
	}
	if state != 0 {
		return nil, fmt.Errorf("aimux: unexpected operation receive state %d", state)
	}
	var event operationEvent
	if err := json.Unmarshal([]byte(cstr(out)), &event); err != nil {
		return nil, err
	}
	return &event, nil
}

func operationReply(handle uint64, id string, value *RawToolCall, failure error) error {
	reply := map[string]any{"type": "unchanged"}
	if failure != nil {
		reply = map[string]any{"type": "failed", "message": failure.Error()}
	} else if value != nil {
		reply = map[string]any{"type": "repaired", "tool_call": value}
	}
	data, err := json.Marshal(reply)
	if err != nil {
		return err
	}
	cID, cReply := C.CString(id), C.CString(string(data))
	defer C.free(unsafe.Pointer(cID))
	defer C.free(unsafe.Pointer(cReply))
	var status C.int32_t
	if err := expectAimuxError(C.aimux_operation_reply(C.uint64_t(handle), cID, cReply, &status)); err != nil {
		return err
	}
	if status != 0 && status != 3 {
		return fmt.Errorf("aimux: unexpected operation reply status %d", status)
	}
	return nil
}

// runOperation owns every native call until drop. User functions receive only
// data, so a late return cannot touch a released operation handle.
func (m *Model) runOperation(ctx context.Context, mode, prompt, options string, repair ToolCallRepairFunc, emit func(operationEvent) error) (resultErr error) {
	model, err := m.handle()
	if err != nil {
		return err
	}
	defer runtime.KeepAlive(m)
	if options == "" {
		options = "{}"
	}
	request, err := json.Marshal(map[string]any{"protocol_version": 1, "mode": mode, "prompt": json.RawMessage(prompt), "options": json.RawMessage(options), "repair_tool_call": true})
	if err != nil {
		return err
	}
	cRequest := C.CString(string(request))
	defer C.free(unsafe.Pointer(cRequest))
	var native C.uint64_t
	if err := expectAimuxError(C.aimux_operation_start(C.uint64_t(model), cRequest, &native)); err != nil {
		return err
	}
	handle := uint64(native)
	cancelNative := func() { _ = expectAimuxError(C.aimux_operation_cancel(native)) }
	hostCtx, stopHost := context.WithCancel(ctx)
	terminalDone := make(chan struct{})
	go func() { defer close(terminalDone); _, _ = operationNext(handle, 3); stopHost() }()
	watchDone := make(chan struct{})
	stopWatch := context.AfterFunc(ctx, func() {
		defer close(watchDone)
		cancelNative()
	})
	controlDone := make(chan error, 1)
	go func() {
		var err error
		defer func() {
			if err != nil {
				cancelNative()
			}
			controlDone <- err
		}()
		for {
			var event *operationEvent
			event, err = operationNext(handle, 0)
			if err != nil || event == nil {
				return
			}
			event.Context.Context = hostCtx
			type answer struct {
				value *RawToolCall
				err   error
			}
			answered := make(chan answer, 1)
			go func(context ToolCallRepairContext) {
				var a answer
				defer func() {
					if p := recover(); p != nil {
						a.err = fmt.Errorf("repair panic: %v", p)
					}
					answered <- a
				}()
				a.value, a.err = repair(context)
			}(event.Context)
			select {
			case a := <-answered:
				err = operationReply(handle, event.RequestID, a.value, a.err)
				if err != nil {
					return
				}
			case <-hostCtx.Done():
				return
			}
		}
	}()
	defer func() {
		cancelNative()
		stopHost()
		if err := <-controlDone; err != nil {
			resultErr = err
		}
		<-terminalDone
		if !stopWatch() {
			<-watchDone
		}
		C.aimux_operation_drop(native)
		if cause := context.Cause(ctx); cause != nil {
			resultErr = cause
		}
	}()
	for {
		event, err := operationNext(handle, 1)
		if err != nil {
			return err
		}
		if event == nil {
			return nil
		}
		if err := emit(*event); err != nil {
			return err
		}
	}
}

func (m *Model) generateOperation(mode, prompt, options string, repair ToolCallRepairFunc) (string, error) {
	var result string
	err := m.runOperation(context.Background(), mode, prompt, options, repair, func(e operationEvent) error { result = string(e.Result); return nil })
	return result, err
}

func (m *Model) streamOperation(ctx context.Context, mode, prompt, options string, repair ToolCallRepairFunc) *Stream {
	if ctx == nil {
		ctx = context.Background()
	}
	cancelCtx, cancel := context.WithCancelCause(context.Background())
	stream := &Stream{parts: make(chan string, 64), cancelCtx: cancelCtx, cancelFn: cancel}
	stopContext := context.AfterFunc(ctx, func() { cancel(context.Cause(ctx)) })
	go func() {
		err := m.runOperation(cancelCtx, mode, prompt, options, repair, func(e operationEvent) error {
			select {
			case stream.parts <- string(e.Part):
				return nil
			case <-cancelCtx.Done():
				return context.Cause(cancelCtx)
			}
		})
		stopContext()
		if cause := context.Cause(ctx); cause != nil {
			cancel(cause)
		}
		stream.finish(err)
	}()
	return stream
}
