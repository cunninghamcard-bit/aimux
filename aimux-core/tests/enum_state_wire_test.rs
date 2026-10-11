//! Phase 1E shared wire samples (A7): `fixtures/v4/enum-state/*.jsonl`.
//!
//! Each line is deserialized into its type and serialized again; the output is
//! compared byte for byte with `baseline/<name>.jsonl`, which was recorded at
//! `exp/restart@8455db9a` (before any enum change). Units E.1-E.4 run the same
//! comparison; where A7 §D25 changes the canonical output, they compare with
//! `normalized/<name>.jsonl` instead and say so.
//!
//! Re-record the baseline (only at 8455db9a): `AIMUX_BLESS_ENUM_STATE=1
//! cargo test -p aimux-core --test enum_state_wire_test`.

use std::path::PathBuf;

use aimux_core::stream_part::{StreamPart, TextStreamPart};
use aimux_core::tool::{RawToolCall, ToolCall, ToolResult};
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

#[test]
fn tool_call_samples_match_baseline() {
    check::<ToolCall>("tool_call");
    check_normalized::<ToolCall>("tool_call");
}

/// D26 inputs: still accepted at 8455db9a; rejected by `try_from` from E.2.
#[test]
fn tool_call_rejected_samples_match_baseline() {
    check::<ToolCall>("tool_call_rejected");
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

/// At 8455db9a every sample already round-trips unchanged: the baseline is
/// the sample itself, byte for byte.
#[test]
fn baseline_equals_samples() {
    for name in [
        "tool_call",
        "tool_call_rejected",
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
