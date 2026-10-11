//! Phase 1E shared wire samples (A7): `fixtures/v4/enum-state/*.jsonl`.
//!
//! Each line is deserialized into its type and serialized again; the output is
//! compared byte for byte with `baseline/<name>.jsonl`, which was recorded at
//! `exp/restart@8455db9a` (before any enum change). Units E.1-E.4 run the same
//! comparison; where A7 §D25 changes the canonical output, they compare with
//! `normalized/<name>.jsonl` instead and say so. E.2: `ToolCall` compares
//! with `normalized/` (D25); its D26 rows now fail conversion.
//!
//! Re-record the baseline (only at 8455db9a): `AIMUX_BLESS_ENUM_STATE=1
//! cargo test -p aimux-core --test enum_state_wire_test`.

use std::path::PathBuf;

use aimux_core::stream_part::{StreamPart, TextStreamPart};
use aimux_core::tool::{DynamicToolCall, RawToolCall, ToolCall, ToolCallValidity, ToolResult};
use serde::Serialize;
use serde::de::DeserializeOwned;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/v4/enum-state")
}

fn lines(path: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `serialize(deserialize(line))` for every line of `file`.
fn round_trip<T: Serialize + DeserializeOwned>(file: &PathBuf) -> Vec<String> {
    lines(file)
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let value: T = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{}:{}: {e}: {line}", file.display(), i + 1));
            serde_json::to_string(&value).unwrap()
        })
        .collect()
}

fn check<T: Serialize + DeserializeOwned>(name: &str) {
    let sample = dir().join(format!("{name}.jsonl"));
    let baseline = dir().join("baseline").join(format!("{name}.jsonl"));
    let out = round_trip::<T>(&sample);
    if std::env::var_os("AIMUX_BLESS_ENUM_STATE").is_some() {
        std::fs::create_dir_all(baseline.parent().unwrap()).unwrap();
        std::fs::write(
            &baseline,
            out.iter().map(|l| format!("{l}\n")).collect::<String>(),
        )
        .unwrap();
        return;
    }
    let expected = lines(&baseline);
    assert_eq!(out.len(), expected.len(), "{name}: line count");
    for (i, (got, want)) in out.iter().zip(&expected).enumerate() {
        assert_eq!(
            got,
            want,
            "{name}.jsonl line {}: differs from baseline",
            i + 1
        );
    }
}

/// The normalized samples are a fixed point of the round trip.
fn check_normalized<T: Serialize + DeserializeOwned>(name: &str) {
    let normalized = dir().join("normalized").join(format!("{name}.jsonl"));
    let out = round_trip::<T>(&normalized);
    for (i, (got, want)) in out.iter().zip(lines(&normalized)).enumerate() {
        assert_eq!(*got, want, "normalized/{name}.jsonl line {}", i + 1);
    }
}

/// E.2: `ToolCall` is the Static/Dynamic enum, so its output is the A7 D25
/// canonical form: every sample serializes to its `normalized/` line, and
/// normalized lines are a fixed point.
#[test]
fn tool_call_samples_match_normalized() {
    let sample = dir().join("tool_call.jsonl");
    let normalized = lines(&dir().join("normalized").join("tool_call.jsonl"));
    let out = round_trip::<ToolCall>(&sample);
    assert_eq!(out.len(), normalized.len());
    for (i, (got, want)) in out.iter().zip(&normalized).enumerate() {
        assert_eq!(got, want, "tool_call.jsonl line {}", i + 1);
    }
    check_normalized::<ToolCall>("tool_call");
}

/// D25: absent and explicit `false` give the same variant and the same JSON,
/// for `dynamic` and for `invalid`.
#[test]
fn tool_call_absent_and_false_are_the_same_variant() {
    let base = r#""toolCallId":"call_1","toolName":"weather","input":{}"#;
    let parse =
        |extra: &str| -> ToolCall { serde_json::from_str(&format!("{{{base}{extra}}}")).unwrap() };
    for (absent, explicit_false) in [
        ("", r#","dynamic":false"#),
        ("", r#","invalid":false"#),
        (r#","dynamic":true"#, r#","dynamic":true,"invalid":false"#),
    ] {
        let (a, b) = (parse(absent), parse(explicit_false));
        assert_eq!(a.is_dynamic(), b.is_dynamic(), "{explicit_false}");
        assert!(matches!(
            (&a, &b),
            (ToolCall::Static(_), ToolCall::Static(_))
                | (
                    ToolCall::Dynamic(DynamicToolCall {
                        validity: ToolCallValidity::Valid,
                        ..
                    }),
                    ToolCall::Dynamic(DynamicToolCall {
                        validity: ToolCallValidity::Valid,
                        ..
                    })
                )
        ));
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }
}

/// `providerExecuted` true / false / absent round-trips byte for byte, on
/// static, dynamic and invalid calls.
#[test]
fn tool_call_provider_executed_is_passed_through() {
    for line in lines(&dir().join("normalized").join("tool_call.jsonl")) {
        let call: ToolCall = serde_json::from_str(&line).unwrap();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            call.provider_executed(),
            value
                .get("providerExecuted")
                .and_then(serde_json::Value::as_bool),
            "{line}"
        );
        assert_eq!(serde_json::to_string(&call).unwrap(), line);
    }
}

/// D26: `invalid: true` without `dynamic: true`, or without `error`, fails
/// conversion (`tool_call_rejected.jsonl`, E.0).
#[test]
fn tool_call_rejected_samples_fail_conversion() {
    let file = dir().join("tool_call_rejected.jsonl");
    let rows = lines(&file);
    assert_eq!(rows.len(), 3);
    for line in rows {
        let error = serde_json::from_str::<ToolCall>(&line).unwrap_err();
        assert!(
            error.to_string().contains("invalid: true"),
            "{error}: {line}"
        );
    }
}

/// An invalid call can only be built with its error: the variant holds it.
#[test]
fn invalid_tool_call_carries_its_error() {
    let line = &lines(&dir().join("normalized").join("tool_call.jsonl"))[12];
    let call: ToolCall = serde_json::from_str(line).unwrap();
    assert!(call.is_dynamic() && call.is_invalid());
    assert!(matches!(
        call.error(),
        Some(aimux_core::error::AiMuxError::InvalidToolInput { .. })
    ));
}

#[test]
fn tool_result_samples_match_baseline() {
    check::<ToolResult>("tool_result");
    check_normalized::<ToolResult>("tool_result");
}

#[test]
fn raw_tool_call_samples_match_baseline() {
    check::<RawToolCall>("raw_tool_call");
    check_normalized::<RawToolCall>("raw_tool_call");
}

#[test]
fn stream_tool_input_start_samples_match_baseline() {
    check::<StreamPart>("stream_tool_input_start");
    check_normalized::<StreamPart>("stream_tool_input_start");
}

#[test]
fn text_stream_tool_input_start_samples_match_baseline() {
    check::<TextStreamPart>("text_stream_tool_input_start");
    check_normalized::<TextStreamPart>("text_stream_tool_input_start");
}

/// At 8455db9a every sample already round-tripped unchanged: the baseline is
/// the sample itself, byte for byte. (The `tool_call` baselines stay as the
/// record of 8455db9a; E.2 compares `tool_call` with `normalized/`.)
#[test]
fn baseline_equals_samples() {
    for name in [
        "tool_result",
        "raw_tool_call",
        "stream_tool_input_start",
        "text_stream_tool_input_start",
    ] {
        let sample = lines(&dir().join(format!("{name}.jsonl")));
        let baseline = lines(&dir().join("baseline").join(format!("{name}.jsonl")));
        assert_eq!(sample, baseline, "{name}");
    }
}
