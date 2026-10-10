//! `inputSchemaInput` (#200:1952 lists it on the model-message
//! `tool-approval-request` content part).
//!
//! - The prompt/response content part (`ContentPart::ToolApprovalRequest`)
//!   carries it, camelCase, optional, and round-trips it.
//! - `ToolApprovalRequestOutput` keeps it internally only; it is not part of
//!   that type's JSON.

use aimux_core::content::ContentPart;
use aimux_core::result::ToolApprovalRequestOutput;
use serde_json::json;

fn part_json(input_schema_input: Option<serde_json::Value>) -> serde_json::Value {
    let mut value = json!({
        "type": "tool-approval-request",
        "approvalId": "approval-1",
        "toolCallId": "call-1",
        "reason": "needs confirmation"
    });
    if let Some(raw) = input_schema_input {
        value["inputSchemaInput"] = raw;
    }
    value
}

#[test]
fn content_part_round_trips_with_input_schema_input() {
    for raw in [json!({"city": "paris"}), json!("{\"city\":\"paris\"")] {
        let wire = part_json(Some(raw.clone()));
        let part: ContentPart = serde_json::from_value(wire.clone()).unwrap();
        let ContentPart::ToolApprovalRequest {
            input_schema_input, ..
        } = &part
        else {
            panic!("expected a tool-approval-request part");
        };
        assert_eq!(input_schema_input.as_ref(), Some(&raw));
        assert_eq!(serde_json::to_value(&part).unwrap(), wire);
    }
}

#[test]
fn content_part_round_trips_without_input_schema_input() {
    let wire = part_json(None);
    let part: ContentPart = serde_json::from_value(wire.clone()).unwrap();
    let back = serde_json::to_value(&part).unwrap();
    assert!(back.get("inputSchemaInput").is_none());
    assert_eq!(back, wire);
}

#[test]
fn approval_output_json_has_no_input_schema_input() {
    let wire = json!({
        "approvalId": "approval-1",
        "toolCall": {"toolCallId": "call-1", "toolName": "weather", "input": {"city": "Paris"}},
        "inputSchemaInput": {"city": "paris"}
    });
    let mut approval: ToolApprovalRequestOutput = serde_json::from_value(wire).unwrap();
    assert_eq!(approval.input_schema_input, None, "not read from JSON");
    approval.input_schema_input = Some(json!({"city": "paris"}));
    let back = serde_json::to_value(&approval).unwrap();
    assert!(
        back.get("inputSchemaInput").is_none(),
        "not written to JSON"
    );
}
