//! `ToolApprovalRequestOutput.inputSchemaInput` (#200:1952, AI SDK
//! `ToolApprovalRequest.inputSchemaInput`): optional, camelCase on the wire,
//! omitted when absent, and passed through unchanged.

use aimux_core::result::ToolApprovalRequestOutput;
use serde_json::json;

fn approval_json(input_schema_input: Option<serde_json::Value>) -> serde_json::Value {
    let mut value = json!({
        "approvalId": "approval-1",
        "toolCall": {
            "toolCallId": "call-1",
            "toolName": "weather",
            "input": {"city": "Paris", "unit": "celsius"},
            "providerExecuted": true
        },
        "reason": "needs confirmation"
    });
    if let Some(raw) = input_schema_input {
        value["inputSchemaInput"] = raw;
    }
    value
}

#[test]
fn round_trips_with_input_schema_input() {
    let wire = approval_json(Some(json!({"city": "paris"})));
    let approval: ToolApprovalRequestOutput = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(approval.input_schema_input, Some(json!({"city": "paris"})));
    assert_eq!(serde_json::to_value(&approval).unwrap(), wire);
}

#[test]
fn round_trips_without_input_schema_input() {
    let wire = approval_json(None);
    let approval: ToolApprovalRequestOutput = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(approval.input_schema_input, None);
    let back = serde_json::to_value(&approval).unwrap();
    assert!(back.get("inputSchemaInput").is_none());
    assert!(back.get("input_schema_input").is_none());
    assert_eq!(back, wire);
}

#[test]
fn non_object_input_schema_input_is_preserved() {
    // `unknown` upstream: any JSON value, including a raw string.
    let wire = approval_json(Some(json!("{\"city\":\"paris\"")));
    let approval: ToolApprovalRequestOutput = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&approval).unwrap(), wire);
}
