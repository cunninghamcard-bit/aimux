//! Unit 1.4 (T3 / D9): differential test against the pinned AI SDK tracker.
//!
//! `fixtures/aisdk/provider-utils/t3-unattributable-deltas.json` holds the
//! deltas for each case plus what `@ai-sdk/provider-utils@5.0.53`
//! `StreamingToolCallTracker` (typeValidation `'if-present'`, as the openai
//! chat model uses) emitted for them, recorded by
//! `scripts/gen-t3-tracker-fixture.mjs`. Each case asserts aimux's tracker,
//! configured as `openai/model.rs` configures it, matches per delta, on the
//! error, and on flush.

use aimux_core::error::AiMuxError;
use aimux_core::stream_part::StreamPart;
use aimux_core::tool::RawToolCall;
use aimux_provider_utils::{StreamingToolCallDelta, StreamingToolCallTracker, TypeValidation};
use serde_json::{Value, json};

const FIXTURE: &str =
    include_str!("../../fixtures/aisdk/provider-utils/t3-unattributable-deltas.json");

fn to_aisdk(part: StreamPart) -> Value {
    match part {
        StreamPart::ToolInputStart { id, tool_name, .. } => {
            json!({"type": "tool-input-start", "id": id, "toolName": tool_name})
        }
        StreamPart::ToolInputDelta { id, delta, .. } => {
            json!({"type": "tool-input-delta", "id": id, "delta": delta})
        }
        StreamPart::ToolInputEnd { id, .. } => json!({"type": "tool-input-end", "id": id}),
        StreamPart::ToolCall(RawToolCall {
            tool_call_id,
            tool_name,
            input,
            ..
        }) => json!({
            "type": "tool-call", "toolCallId": tool_call_id, "toolName": tool_name, "input": input
        }),
        other => panic!("unexpected part: {other:?}"),
    }
}

fn opt_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn run_case(case: &Value) {
    let name = case["name"].as_str().unwrap();
    let expected = &case["aisdk"];
    // No case reaches id generation (every started call carries a wire id).
    let mut tracker = StreamingToolCallTracker::new()
        .with_generate_id(|| "gen-0".to_string())
        .with_type_validation(TypeValidation::IfPresent);
    let mut steps = Vec::new();
    let mut error = Value::Null;
    for d in case["deltas"].as_array().unwrap() {
        let f = &d["function"];
        let delta = StreamingToolCallDelta {
            index: d.get("index").and_then(Value::as_u64).map(|i| i as usize),
            id: opt_str(d, "id"),
            r#type: opt_str(d, "type"),
            name: opt_str(f, "name"),
            arguments: opt_str(f, "arguments"),
            provider_metadata: None,
        };
        let mut out = Vec::new();
        let result = tracker.process(delta, &mut out);
        steps.push(json!({"parts": out.into_iter().map(to_aisdk).collect::<Vec<_>>()}));
        if let Err(e) = result {
            // The provider surfaces this as AiMuxError::InvalidResponseData.
            let AiMuxError::InvalidResponseData(message) = AiMuxError::from(e) else {
                panic!("{name}: tracker error is not InvalidResponseData");
            };
            error = json!({"name": "AI_InvalidResponseDataError", "message": message});
            break;
        }
    }
    let flush = if error.is_null() {
        let mut out = Vec::new();
        tracker.finish(&mut out);
        Value::Array(out.into_iter().map(to_aisdk).collect())
    } else {
        Value::Null
    };
    let actual = json!({"steps": steps, "error": error, "flush": flush});
    assert_eq!(&actual, expected, "{name} ({})", case["tracker"]);
}

fn case(name: &str) -> Value {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("missing fixture case {name}"))
        .clone()
}

#[test]
fn fixture_covers_exactly_the_four_t3_cases() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 4);
}

/// streaming-tool-call-tracker.ts:142-144.
#[test]
fn ambiguous_delta_across_open_calls_is_dropped() {
    run_case(&case("ambiguous_delta_across_open_calls_is_dropped"));
}

/// streaming-tool-call-tracker.ts:151-153.
#[test]
fn blank_name_matching_no_call_is_dropped() {
    run_case(&case("blank_name_matching_no_call_is_dropped"));
}

/// streaming-tool-call-tracker.ts:293-305, :379-384.
#[test]
fn nameless_delta_without_open_call_is_invalid_response_data() {
    run_case(&case(
        "nameless_delta_without_open_call_is_invalid_response_data",
    ));
}

/// streaming-tool-call-tracker.ts:370-376.
#[test]
fn non_function_type_is_invalid_response_data() {
    run_case(&case("non_function_type_is_invalid_response_data"));
}
