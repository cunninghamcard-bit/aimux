//! Stateless host-side tool-call repair (RFC-0035).
//!
//! The closure path (`ToolCallRepair`) and the data path
//! (`apply_tool_call_repair`) must be indistinguishable: the bindings rely on
//! the second producing exactly what Rust callers get from the first.

use aimux_core::error::AiMuxError;
use aimux_core::generate::GenerateTextOptions;
use aimux_core::message::{ModelMessage, ModelPrompt};
use aimux_core::parse_tool_call::{
    RawToolCall, ToolCallRepair, ToolCallRepairReply, apply_tool_call_repair,
    apply_tool_call_repair_to_result, parse_tool_call, tool_call_repair_context,
    tool_call_repair_inputs,
};
use aimux_core::tool::{FunctionTool, Tool, ToolCall};
use serde_json::{Value, json};

fn weather_tool() -> Tool {
    FunctionTool::new(
        "weather",
        json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"],
        }),
    )
    .into()
}

fn raw(name: &str, input: &str) -> RawToolCall {
    RawToolCall {
        tool_call_id: "call-1".into(),
        tool_name: name.into(),
        input: input.into(),
        provider_executed: None,
        dynamic: None,
        thought_signature: None,
        provider_metadata: None,
    }
}

/// The invalid `ToolCall` a host reads off `GenerateTextResult.tool_calls`.
async fn invalid_call(input: &str) -> ToolCall {
    let call = parse_tool_call(
        raw("weather", input),
        Some(&[weather_tool()]),
        None,
        &[],
        None,
    )
    .await;
    assert_eq!(call.invalid, Some(true));
    call
}

/// Run the closure path with the callback result the given reply models.
async fn via_closure(
    input: &str,
    outcome: Result<Option<RawToolCall>, AiMuxError>,
) -> aimux_core::tool::ToolCall {
    let repair = ToolCallRepair::new(move |_| {
        let outcome = outcome.clone();
        async move { outcome }
    });
    parse_tool_call(
        raw("weather", input),
        Some(&[weather_tool()]),
        Some(&repair),
        &[],
        None,
    )
    .await
}

const BAD: &str = r#"{"town":"Singapore"}"#;

// ── reply branches ──────────────────────────────────────────────────────────

#[tokio::test]
async fn repaired_reply_revalidates_and_yields_a_valid_call() {
    let call = apply_tool_call_repair(
        &invalid_call(BAD).await,
        Some(&[weather_tool()]),
        ToolCallRepairReply::Repaired {
            tool_call: raw("weather", r#"{"city":"Singapore"}"#),
        },
    )
    .unwrap();
    assert_eq!(call.invalid, None);
    assert!(call.error.is_none());
    assert_eq!(call.input, json!({"city": "Singapore"}));
}

#[tokio::test]
async fn repaired_reply_that_still_fails_nests_both_errors() {
    let call = apply_tool_call_repair(
        &invalid_call(BAD).await,
        Some(&[weather_tool()]),
        ToolCallRepairReply::Repaired {
            tool_call: raw("weather", r#"{"city":42}"#),
        },
    )
    .unwrap();
    let Some(AiMuxError::ToolCallRepair {
        original_error,
        cause,
    }) = call.error
    else {
        panic!("expected ToolCallRepair, got {:?}", call.error);
    };
    assert!(matches!(
        *original_error,
        AiMuxError::InvalidToolInput { .. }
    ));
    assert!(matches!(*cause, AiMuxError::InvalidToolInput { .. }));
}

#[tokio::test]
async fn unchanged_reply_keeps_the_original_error() {
    let original = invalid_call(BAD).await;
    let call = apply_tool_call_repair(
        &original,
        Some(&[weather_tool()]),
        ToolCallRepairReply::Unchanged,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&call).unwrap(),
        serde_json::to_value(&original).unwrap()
    );
}

#[tokio::test]
async fn failed_reply_reports_the_host_message_as_the_cause() {
    let call = apply_tool_call_repair(
        &invalid_call(BAD).await,
        Some(&[weather_tool()]),
        ToolCallRepairReply::Failed {
            message: "LLM call failed".into(),
        },
    )
    .unwrap();
    let Some(AiMuxError::ToolCallRepair { cause, .. }) = call.error else {
        panic!("expected ToolCallRepair");
    };
    assert!(matches!(*cause, AiMuxError::Other(ref m) if m == "LLM call failed"));
}

#[tokio::test]
async fn a_valid_tool_call_is_rejected() {
    let valid = parse_tool_call(
        raw("weather", r#"{"city":"Singapore"}"#),
        Some(&[weather_tool()]),
        None,
        &[],
        None,
    )
    .await;
    let error = apply_tool_call_repair(
        &valid,
        Some(&[weather_tool()]),
        ToolCallRepairReply::Unchanged,
    )
    .unwrap_err();
    assert!(matches!(error, AiMuxError::InvalidArgument(_)));
}

// ── closure / stateless equivalence ─────────────────────────────────────────

#[tokio::test]
async fn both_paths_agree_on_every_branch() {
    let repaired = raw("weather", r#"{"city":"Singapore"}"#);
    let still_bad = raw("weather", r#"{"city":42}"#);
    let cases: Vec<(Result<Option<RawToolCall>, AiMuxError>, ToolCallRepairReply)> = vec![
        (
            Ok(Some(repaired.clone())),
            ToolCallRepairReply::Repaired {
                tool_call: repaired,
            },
        ),
        (
            Ok(Some(still_bad.clone())),
            ToolCallRepairReply::Repaired {
                tool_call: still_bad,
            },
        ),
        (Ok(None), ToolCallRepairReply::Unchanged),
        (
            Err(AiMuxError::Other("boom".into())),
            ToolCallRepairReply::Failed {
                message: "boom".into(),
            },
        ),
    ];

    for (outcome, reply) in cases {
        let from_closure = via_closure(BAD, outcome).await;
        let from_data =
            apply_tool_call_repair(&invalid_call(BAD).await, Some(&[weather_tool()]), reply)
                .unwrap();
        assert_eq!(
            serde_json::to_value(&from_closure).unwrap(),
            serde_json::to_value(&from_data).unwrap(),
        );
    }
}

// ── repair context ──────────────────────────────────────────────────────────

#[tokio::test]
async fn context_carries_the_raw_text_error_and_schema() {
    let messages = vec![ModelMessage::user("weather in Singapore?")];
    let context = tool_call_repair_context(
        &invalid_call(BAD).await,
        Some(&[weather_tool()]),
        &messages,
        Some("be helpful"),
    )
    .unwrap()
    .expect("a tool set was supplied");

    assert_eq!(context["tool_call"]["input"], json!(BAD));
    assert_eq!(context["tool_call"]["tool_name"], json!("weather"));
    assert_eq!(context["input_schema"]["required"], json!(["city"]));
    assert_eq!(context["instructions"], json!("be helpful"));
    assert_eq!(context["tools"].as_array().unwrap().len(), 1);
    assert_eq!(context["messages"].as_array().unwrap().len(), 1);
    assert!(context["error"]["InvalidToolInput"].is_object());
}

#[tokio::test]
async fn context_of_an_unknown_tool_falls_back_to_an_empty_schema() {
    let unknown = parse_tool_call(
        raw("wether", r#"{"city":"Singapore"}"#),
        Some(&[weather_tool()]),
        None,
        &[],
        None,
    )
    .await;
    let context = tool_call_repair_context(&unknown, Some(&[weather_tool()]), &[], None)
        .unwrap()
        .expect("a tool set was supplied");
    assert_eq!(context["input_schema"]["properties"], json!({}));
    assert!(context["error"]["NoSuchTool"].is_object());
}

// ── result patching ─────────────────────────────────────────────────────────

fn result_with(call: &ToolCall) -> Value {
    json!({
        "tool_calls": [serde_json::to_value(call).unwrap()],
        "response_messages": [{
            "role": "assistant",
            "content": [
                { "type": "text", "text": "checking" },
                {
                    "type": "tool_call",
                    "tool_call_id": call.tool_call_id,
                    "tool_name": call.tool_name,
                    "input": call.input,
                },
            ],
        }],
    })
}

#[tokio::test]
async fn patching_updates_tool_calls_and_response_messages() {
    let result = result_with(&invalid_call(BAD).await);
    let patched = apply_tool_call_repair_to_result(
        &result,
        Some(&[weather_tool()]),
        "call-1",
        ToolCallRepairReply::Repaired {
            tool_call: raw("weather", r#"{"city":"Singapore"}"#),
        },
    )
    .unwrap();

    assert_eq!(
        patched["tool_calls"][0]["input"],
        json!({"city":"Singapore"})
    );
    assert!(patched["tool_calls"][0].get("invalid").is_none());
    let part = &patched["response_messages"][0]["content"][1];
    assert_eq!(part["input"], json!({"city":"Singapore"}));
    assert_eq!(part["tool_name"], json!("weather"));
    // Unrelated parts and fields survive untouched.
    assert_eq!(
        patched["response_messages"][0]["content"][0]["text"],
        "checking"
    );
}

#[tokio::test]
async fn patching_a_generate_object_result_reaches_into_raw() {
    let result = json!({ "object": {"ok": true}, "raw": result_with(&invalid_call(BAD).await) });
    let patched = apply_tool_call_repair_to_result(
        &result,
        Some(&[weather_tool()]),
        "call-1",
        ToolCallRepairReply::Repaired {
            tool_call: raw("weather", r#"{"city":"Singapore"}"#),
        },
    )
    .unwrap();
    assert_eq!(patched["object"], json!({"ok": true}));
    assert_eq!(
        patched["raw"]["tool_calls"][0]["input"],
        json!({"city":"Singapore"})
    );
}

#[tokio::test]
async fn patching_a_primitive_input_is_not_replayed_into_the_transcript() {
    // A still-invalid call whose input is a bare string: `tool_calls` keeps it,
    // the replay transcript must not.
    let result = result_with(&invalid_call("not json at all").await);
    let patched = apply_tool_call_repair_to_result(
        &result,
        Some(&[weather_tool()]),
        "call-1",
        ToolCallRepairReply::Unchanged,
    )
    .unwrap();
    assert_eq!(patched["tool_calls"][0]["input"], json!("not json at all"));
    assert_eq!(
        patched["response_messages"][0]["content"][1]["input"],
        json!({})
    );
}

#[tokio::test]
async fn patching_follows_a_repair_that_changes_the_call_id() {
    let result = result_with(&invalid_call(BAD).await);
    let patched = apply_tool_call_repair_to_result(
        &result,
        Some(&[weather_tool()]),
        "call-1",
        ToolCallRepairReply::Repaired {
            tool_call: RawToolCall {
                tool_call_id: "call-2".into(),
                ..raw("weather", r#"{"city":"Singapore"}"#)
            },
        },
    )
    .unwrap();

    assert_eq!(patched["tool_calls"][0]["tool_call_id"], json!("call-2"));
    // The transcript part must follow, or the replayed message points at a
    // call id that is no longer in `tool_calls`.
    let part = &patched["response_messages"][0]["content"][1];
    assert_eq!(part["tool_call_id"], json!("call-2"));
    assert_eq!(part["input"], json!({"city":"Singapore"}));
}

#[tokio::test]
async fn patching_an_unknown_id_is_an_error() {
    let result = result_with(&invalid_call(BAD).await);
    let error = apply_tool_call_repair_to_result(
        &result,
        Some(&[weather_tool()]),
        "call-9",
        ToolCallRepairReply::Unchanged,
    )
    .unwrap_err();
    assert!(matches!(error, AiMuxError::InvalidArgument(_)));
}

// ── wire shapes ─────────────────────────────────────────────────────────────

#[test]
fn reply_wire_shapes_round_trip_and_reject_unknown_fields() {
    let repaired: ToolCallRepairReply = serde_json::from_str(
        r#"{"type":"repaired","tool_call":{"tool_call_id":"c1","tool_name":"weather","input":"{}"}}"#,
    )
    .unwrap();
    assert!(matches!(repaired, ToolCallRepairReply::Repaired { .. }));
    assert!(serde_json::from_str::<ToolCallRepairReply>(r#"{"type":"unchanged"}"#).is_ok());
    assert!(serde_json::from_str::<ToolCallRepairReply>(r#"{"type":"failed"}"#).is_err());
    // serde's `deny_unknown_fields` reaches the struct variants of an
    // internally tagged enum, but not its unit variants.
    assert!(
        serde_json::from_str::<ToolCallRepairReply>(r#"{"type":"failed","message":"m","x":1}"#)
            .is_err()
    );
}

// ── no tool set: not repairable, not an error ───────────────────────────────

#[tokio::test]
async fn a_call_made_without_tools_has_no_repair_context() {
    assert!(
        tool_call_repair_context(&invalid_call(BAD).await, None, &[], None)
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn applying_a_reply_without_tools_is_an_error() {
    let error = apply_tool_call_repair(
        &invalid_call(BAD).await,
        None,
        ToolCallRepairReply::Unchanged,
    )
    .unwrap_err();
    assert!(matches!(error, AiMuxError::InvalidArgument(_)));
}

#[test]
fn repair_inputs_come_from_the_prompt_and_options_the_call_used() {
    let options = GenerateTextOptions {
        tools: Some(vec![weather_tool()]),
        instructions: Some("be terse".into()),
        ..Default::default()
    };
    let (messages, instructions, tools) = tool_call_repair_inputs("weather?", &options);
    assert_eq!(messages.len(), 1);
    assert_eq!(instructions, Some("be terse"));
    assert_eq!(tools.map(<[_]>::len), Some(1));

    // A messages prompt passes through; options with no tools answer `None`.
    let defaults = GenerateTextOptions::default();
    let (messages, _, tools) = tool_call_repair_inputs(
        ModelPrompt::Messages(vec![ModelMessage::user("a"), ModelMessage::user("b")]),
        &defaults,
    );
    assert_eq!(messages.len(), 2);
    assert!(tools.is_none());
}

// ── shared contract fixture ─────────────────────────────────────────────────

/// Run `contract-tests/fixtures/tool-call-repair.json` against the Rust
/// implementation, so the file every binding asserts against cannot drift.
#[test]
fn contract_fixture_matches_the_implementation() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract-tests/fixtures/tool-call-repair.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

    for case in fixture["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let input = &case["input"];
        // Every case carries the prompt and options the call was generated
        // with, exactly as a host would hand them back.
        let options: GenerateTextOptions = match &input["opts"] {
            Value::Null => GenerateTextOptions::default(),
            opts => serde_json::from_value(opts.clone()).unwrap(),
        };
        let prompt: ModelPrompt = match &input["prompt"] {
            Value::Object(object) if object.len() == 1 && object.contains_key("prompt") => {
                serde_json::from_value(object["prompt"].clone()).unwrap()
            }
            Value::Null => ModelPrompt::Text(String::new()),
            prompt => serde_json::from_value(prompt.clone()).unwrap(),
        };
        let (messages, instructions, tools) = tool_call_repair_inputs(prompt, &options);
        let reply = || serde_json::from_value(input["reply"].clone()).unwrap();
        let tool_call = || serde_json::from_value::<ToolCall>(input["tool_call"].clone()).unwrap();

        let actual = match case["function"].as_str().unwrap() {
            "tool_call_repair_context" => {
                tool_call_repair_context(&tool_call(), tools, &messages, instructions)
                    .map(|context| serde_json::to_value(context).unwrap())
            }
            "apply_tool_call_repair" => apply_tool_call_repair(&tool_call(), tools, reply())
                .map(|call| serde_json::to_value(call).unwrap()),
            "apply_tool_call_repair_to_result" => apply_tool_call_repair_to_result(
                &input["result"],
                tools,
                input["tool_call_id"].as_str().unwrap(),
                reply(),
            ),
            other => panic!("{name}: unknown function {other}"),
        };

        match case.get("expected_error").and_then(Value::as_str) {
            Some(variant) => {
                let error = actual.unwrap_err();
                let key = serde_json::to_value(&error).unwrap();
                assert!(
                    key.get(variant).is_some(),
                    "{name}: expected {variant}, got {error}"
                );
            }
            None => assert_eq!(actual.unwrap(), case["expected"], "{name}"),
        }
    }
}
