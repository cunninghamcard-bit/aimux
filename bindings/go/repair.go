package aimux

/*
#include <stdint.h>
#include <stdlib.h>

#include "aimux-ffi.h"

// Go-side trampoline (//export below). user_data is the cgo.Handle of the
// *ToolCallRepair that owns the Go function.
extern char* goRepairToolCall(uintptr_t id, char* context_json);

static char* trampoline_repair(const char* context_json, void* user_data) {
    return goRepairToolCall((uintptr_t)user_data, (char*)context_json);
}

static uint64_t new_repair(uintptr_t id) {
    return aimux_tool_call_repair_new(trampoline_repair, (void*)id);
}
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"runtime/cgo"
	"sync"
	"sync/atomic"
	"unsafe"
)

// RawToolCall is a tool call before Core parses its input: Input is the
// model's argument text verbatim, possibly malformed.
type RawToolCall struct {
	ToolCallID       string          `json:"tool_call_id"`
	ToolName         string          `json:"tool_name"`
	Input            string          `json:"input"`
	ProviderExecuted *bool           `json:"provider_executed,omitempty"`
	Dynamic          *bool           `json:"dynamic,omitempty"`
	ThoughtSignature *string         `json:"thought_signature,omitempty"`
	ProviderMetadata json.RawMessage `json:"provider_metadata,omitempty"`
}

// ToolCallRepairContext is what a ToolCallRepairFunc receives — the AI SDK
// `repairToolCall` arguments.
type ToolCallRepairContext struct {
	// ToolCall is the call that failed lookup, JSON parsing, or schema validation.
	ToolCall RawToolCall `json:"tool_call"`
	// Error is the typed failure (NoSuchTool or InvalidToolInput) as wire JSON,
	// the same shape as ToolCall.Error.
	Error json.RawMessage `json:"error"`
	// InputSchema is the JSON Schema of the called tool; an empty-object
	// schema when the tool is unknown.
	InputSchema json.RawMessage `json:"input_schema"`
	Tools       []Tool          `json:"tools"`
	// Messages is the prompt of the current step as wire JSON.
	Messages     []json.RawMessage `json:"messages"`
	Instructions *string           `json:"instructions,omitempty"`
}

// ToolCallRepairFunc gets one attempt to fix an invalid tool call. Return the
// repaired call, or nil to keep the original validation error. Core parses
// and validates the returned call from scratch.
//
// It runs synchronously on the goroutine that called GenerateText /
// StreamText, while that call is in progress. It must not call back into
// aimux: the FFI layer rejects that as a re-entrant call.
type ToolCallRepairFunc func(ctx ToolCallRepairContext) (*RawToolCall, error)

// ToolCallRepair is a registered ToolCallRepairFunc. Assign it to
// GenerateTextOptions.RepairToolCall; it marshals as the FFI handle. Close it
// once no call that references it is in flight.
type ToolCallRepair struct {
	fn   ToolCallRepairFunc
	id   atomic.Uint64 // FFI handle; 0 once closed
	self cgo.Handle

	mu  sync.Mutex
	err error
}

// NewToolCallRepair registers fn with the FFI layer.
func NewToolCallRepair(fn ToolCallRepairFunc) *ToolCallRepair {
	r := &ToolCallRepair{fn: fn}
	r.self = cgo.NewHandle(r)
	r.id.Store(uint64(C.new_repair(C.uintptr_t(r.self))))
	return r
}

// MarshalJSON writes the FFI handle, which is how the function reaches
// opts_json (`"repair_tool_call": <handle>`). A closed repair marshals as null.
func (r *ToolCallRepair) MarshalJSON() ([]byte, error) {
	if r == nil {
		return []byte("null"), nil
	}
	id := r.id.Load()
	if id == 0 {
		return []byte("null"), nil
	}
	return json.Marshal(id)
}

// Err returns the last error the Go function returned (or the last panic it
// raised). The C contract carries only "repaired" or "not repaired", so a
// failing function leaves the original validation error on the tool call;
// this is where the cause is kept.
func (r *ToolCallRepair) Err() error {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.err
}

// Close releases the FFI handle. Calls already in flight keep their clone.
func (r *ToolCallRepair) Close() error {
	if id := r.id.Swap(0); id != 0 {
		C.aimux_tool_call_repair_drop(C.uint64_t(id))
		r.self.Delete()
	}
	return nil
}

func (r *ToolCallRepair) setErr(err error) {
	r.mu.Lock()
	r.err = err
	r.mu.Unlock()
}

// invoke runs the Go function; nil means "keep the original error".
func (r *ToolCallRepair) invoke(contextJSON string) (out []byte) {
	// A Go panic must not unwind through the C frames into Rust.
	defer func() {
		if p := recover(); p != nil {
			r.setErr(fmt.Errorf("aimux: repair function panicked: %v", p))
			out = nil
		}
	}()
	var ctx ToolCallRepairContext
	if err := json.Unmarshal([]byte(contextJSON), &ctx); err != nil {
		r.setErr(fmt.Errorf("aimux: repair context: %w", err))
		return nil
	}
	repaired, err := r.fn(ctx)
	if err != nil {
		r.setErr(err)
		return nil
	}
	if repaired == nil {
		return nil
	}
	b, err := json.Marshal(repaired)
	if err != nil {
		r.setErr(fmt.Errorf("aimux: repaired tool call: %w", err))
		return nil
	}
	return b
}

//export goRepairToolCall
func goRepairToolCall(id C.uintptr_t, contextJSON *C.char) *C.char {
	r, ok := cgo.Handle(id).Value().(*ToolCallRepair)
	if !ok || contextJSON == nil {
		return nil
	}
	out := r.invoke(C.GoString(contextJSON))
	if out == nil {
		return nil
	}
	// aimux owns the returned string, so it has to come from its allocator.
	cs := C.CString(string(out))
	defer C.free(unsafe.Pointer(cs))
	return C.aimux_string_new(cs)
}
