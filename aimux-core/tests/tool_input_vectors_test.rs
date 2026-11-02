//! RFC-0042 §3.2 tool-argument vectors (`fixtures/v4/tool-input-vectors.json`).
//!
//! For each vector:
//! - the provider DTO (`RawToolCall`) survives Rust -> JSON -> Rust with its
//!   argument text byte for byte;
//! - core parsing gives the expected value, or an invalid call whose error
//!   keeps the original text (never `{}`, never partial-JSON repair);
//! - the core DTO (`ToolCall`) survives Rust -> JSON -> Rust with its value
//!   equal under JSON semantics.

use aimux_core::error::AiMuxError;
use aimux_core::parse_tool_call::parse_tool_call;
use aimux_core::tool::{FunctionTool, RawToolCall, Tool, ToolCall};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("../../fixtures/v4/tool-input-vectors.json");

struct Vector {
    row: u64,
    provider_input: String,
    valid: bool,
    input: Option<Value>,
    ignore: Option<String>,
}

fn vectors() -> Vec<Vector> {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture is JSON");
    fixture["vectors"]
        .as_array()
        .expect("vectors array")
        .iter()
        .map(|v| Vector {
            row: v["row"].as_u64().expect("row"),
            provider_input: v["providerInput"]
                .as_str()
                .expect("providerInput")
                .to_owned(),
            valid: v["expect"]["valid"].as_bool().expect("expect.valid"),
            input: v["expect"].get("input").cloned(),
            ignore: v["ignore"].as_str().map(str::to_owned),
        })
        .collect()
}

/// A provider-executed dynamic call: core parses it with no tool set, so
/// only JSON parsing (no schema) decides validity.
fn raw(input: &str) -> RawToolCall {
    RawToolCall {
        tool_call_id: "call-1".into(),
        tool_name: "lookup".into(),
        input: input.into(),
        provider_executed: Some(true),
        dynamic: Some(true),
        provider_metadata: None,
    }
}

fn error_text(call: &ToolCall) -> Option<&str> {
    match call.error.as_ref()? {
        AiMuxError::InvalidToolInput { tool_input, .. } => Some(tool_input),
        _ => None,
    }
}

async fn check(vector: &Vector) {
    let label = format!("row {} {:?}", vector.row, vector.provider_input);

    // Provider DTO: byte-identical argument text across the binding wire.
    let wire = serde_json::to_string(&raw(&vector.provider_input)).unwrap();
    let back: RawToolCall = serde_json::from_str(&wire).unwrap();
    assert_eq!(
        back.input.as_bytes(),
        vector.provider_input.as_bytes(),
        "{label}: provider text"
    );

    // Core parsing.
    let call = parse_tool_call(back, None, None, &[], None).await;
    if vector.valid {
        assert_eq!(call.invalid, None, "{label}: expected a valid call");
        let expected = vector.input.as_ref().expect("valid vectors carry input");
        assert_eq!(&call.input, expected, "{label}: parsed value");
    } else {
        assert_eq!(
            call.invalid,
            Some(true),
            "{label}: expected an invalid call"
        );
        assert_eq!(
            error_text(&call),
            Some(vector.provider_input.as_str()),
            "{label}: error keeps the original text"
        );
        assert_ne!(call.input, json!({}), "{label}: not filled in as {{}}");
    }

    // Core DTO: JSON-semantic equality across the binding wire. A bare JSON
    // string stays a string (no second JSON.parse).
    let core_back: ToolCall = serde_json::from_str(&serde_json::to_string(&call).unwrap()).unwrap();
    assert_eq!(core_back.input, call.input, "{label}: core value");
    assert_eq!(
        core_back.invalid, call.invalid,
        "{label}: core invalid flag"
    );
}

#[tokio::test]
async fn rfc_0042_tool_input_vectors() {
    let enabled: Vec<Vector> = vectors()
        .into_iter()
        .filter(|v| v.ignore.is_none())
        .collect();
    // Rows 1-5 and the truncated-object half of row 6.
    assert_eq!(enabled.len(), 8);
    for vector in &enabled {
        check(vector).await;
    }
}

/// D18: empty argument text must be an invalid call (RFC-0042:91); the
/// current parser still maps it to `{}`. Enabled by the parser change in 3.3.
#[tokio::test]
#[ignore = "D18: parser change in 3.3"]
async fn rfc_0042_tool_input_vectors_pending() {
    let pending: Vec<Vector> = vectors()
        .into_iter()
        .filter(|v| v.ignore.is_some())
        .collect();
    assert_eq!(pending.len(), 1);
    for vector in &pending {
        check(vector).await;
    }
}

/// RFC-0042:89: an error on a duplicate-key call keeps the original text,
/// duplicate keys included.
#[tokio::test]
async fn duplicate_key_text_survives_in_the_error() {
    let text = r#"{"a":1,"a":2}"#;
    let tools = vec![Tool::Function(FunctionTool::new(
        "lookup",
        json!({"type": "object", "properties": {"a": {"type": "string"}}}),
    ))];
    let mut call = raw(text);
    call.provider_executed = None;
    call.dynamic = None;
    let parsed = parse_tool_call(call, Some(&tools), None, &[], None).await;
    assert_eq!(parsed.invalid, Some(true));
    assert_eq!(error_text(&parsed), Some(text));
}
