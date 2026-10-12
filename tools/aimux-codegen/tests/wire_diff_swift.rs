//! Unit 2.2 (Swift): generated Swift vs the hand-written Swift types,
//! mirroring `wire_diff_go.rs`.
//!
//! Fixtures are the shared real aimux-core values (`common/fixtures.rs`)
//! plus Swift-only number fixtures. Each goes through the generated Swift
//! (`emit::swift`) and copies of `bindings/swift/Sources/Aimux/{Types,
//! Multimodal,DecisionModel}.swift` (never modified) in a scratch SwiftPM
//! package: decode, then encode. Outputs are compared as `serde_json::Value`.
//!
//! - `docs/plan/2.2-diff-swift.md` is rendered in memory and must equal the
//!   committed file (`AIMUX_BLESS_DIFF=1` rewrites it atomically).
//! - Verdicts: `docs/plan/2.2-diff-swift-verdicts.json`, verdict ∈
//!   {手写错, 生成器错, 已知差异}. 已知差异 needs `knownDifference`, an id
//!   in `docs/plan/2.2-known-differences.json` (language swift), and applies
//!   only when fixture label AND leaf path are in the entry's scope AND both
//!   sides are numbers AND they are equal as f64.
//! - Every other generated-vs-serde difference fails before the table.
//! - Real `swift` / `swiftc` are required: if missing the tests fail (not
//!   skipped).

#[path = "common/fixtures.rs"]
mod fixtures;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use aimux_codegen::emit::{Emitter, swift::Swift};
use aimux_codegen::manifest::Inputs;
use aimux_codegen::wire;
use serde_json::{Value, json};

use fixtures::{Back, Fixture, back, fixtures, null_parity_cases};

const DOC: &str = "docs/plan/2.2-diff-swift.md";
const VERDICTS: &str = "docs/plan/2.2-diff-swift-verdicts.json";
const KNOWN: &str = "docs/plan/2.2-known-differences.json";
const SWIFT_MISSING: &str =
    "`swift` is required for the Swift diff test (it is not skipped when missing)";

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ---- fixtures ----------------------------------------------------------------

/// A `Usage` whose `raw` map holds `{"n": <number text>}`, serialized by
/// serde (serde_json parses the text: i64 / u64 / f64 as serde keeps it).
fn usage_with_raw(n: &str) -> aimux_core::types::Usage {
    use aimux_core::types::{InputTokenUsage, OutputTokenUsage, Usage};
    let mut raw = serde_json::Map::new();
    raw.insert("n".into(), serde_json::from_str(n).unwrap());
    Usage {
        input_tokens: InputTokenUsage {
            total: Some(10),
            no_cache: None,
            cache_read: None,
            cache_write: None,
        },
        output_tokens: OutputTokenUsage {
            total: Some(5),
            text: None,
            reasoning: None,
        },
        raw: Some(raw),
    }
}

/// JSONValue number fixtures (mapping doc "Any JSON").
fn number_fixtures() -> Vec<Fixture> {
    [
        ("json-number-neg1", "-1"),
        ("json-number-i64min", "-9223372036854775808"),
        ("json-number-below-i64min", "-9223372036854775809"),
        ("json-number-2^53+1", "9007199254740993"),
        ("json-number-u64max", "18446744073709551615"),
        ("json-number-0.5", "0.5"),
        ("json-number-1.0", "1.0"),
        ("json-number-1e3", "1e3"),
        ("json-number-neg-zero", "-0.0"),
        ("json-number-0", "0"),
        ("json-number-1", "1"),
        ("json-number-true", "true"),
    ]
    .into_iter()
    .map(|(label, n)| fixtures::fxd(label, "Usage", &usage_with_raw(n)))
    .collect()
}

fn all_fixtures() -> Vec<Fixture> {
    fixtures().into_iter().chain(number_fixtures()).collect()
}

/// Decode-parity inputs: serde rejects each (asserted), the generated
/// Swift must reject it too.
fn reject_cases() -> Vec<Fixture> {
    use aimux_core::decision_model::DecisionCapabilities;
    use aimux_core::model_catalogue::ModelModalities;
    use aimux_core::types::Usage;
    let caps = json!({
        "probability_source": "Native",
        "supports_boolean": true,
        "supports_choice": true,
        "supports_score": false,
        "returns_distributions": false,
        "max_questions": null,
        "min_choices": null
    });
    let with = |base: &Value, path: &[&str], v: Value| {
        let mut x = base.clone();
        let mut at = &mut x;
        for k in &path[..path.len() - 1] {
            at = &mut at[*k];
        }
        at[path[path.len() - 1]] = v;
        x
    };
    let usage = serde_json::to_value(usage_with_raw("1")).unwrap();
    let dc: Back = back::<DecisionCapabilities>;
    let mm: Back = back::<ModelModalities>;
    let us: Back = back::<Usage>;
    [
        // #[serde(default)] non-Option (decision_model.rs:362-363).
        (
            "null-default-DecisionCapabilities.supports_images",
            "DecisionCapabilities",
            with(&caps, &["supports_images"], Value::Null),
            dc,
        ),
        // #[serde(default)] Named (kept as T? in Swift; null still rejected).
        (
            "null-default-DecisionCapabilities.rounding",
            "DecisionCapabilities",
            with(&caps, &["rounding"], Value::Null),
            dc,
        ),
        // skip_if_empty non-nullable (model_catalogue.rs:120-121).
        (
            "null-default-ModelModalities.input",
            "ModelModalities",
            json!({"input": null}),
            mm,
        ),
        // u32 overflow / negative / fraction (types.rs InputTokenUsage.total).
        (
            "int-overflow-u32",
            "Usage",
            with(&usage, &["inputTokens", "total"], json!(4_294_967_296_u64)),
            us,
        ),
        (
            "int-negative-u32",
            "Usage",
            with(&usage, &["inputTokens", "total"], json!(-1)),
            us,
        ),
        (
            "int-fraction-u32",
            "Usage",
            with(&usage, &["inputTokens", "total"], json!(1.5)),
            us,
        ),
    ]
    .into_iter()
    .map(|(label, ty, json, b)| Fixture {
        label,
        ty,
        json,
        back: Some(b),
    })
    .collect()
}

// ---- type names (swiftc -dump-parse) -----------------------------------------

const DECL_KINDS: &[&str] = &[
    "struct_decl",
    "enum_decl",
    "class_decl",
    "actor_decl",
    "protocol_decl",
    "typealias_decl",
    "extension_decl",
];

/// Type names from a `swiftc -dump-parse` S-expression dump: nested types
/// as `Outer.Inner`, types inside `extension X { .. }` as `X.Inner`; the
/// extended type itself is not a declaration.
fn names_from_dump(dump: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    // (depth at which the scope opened, name)
    let mut scopes: Vec<(usize, String)> = Vec::new();
    let mut depth = 0usize;
    let b = dump.as_bytes();
    let mut i = 0;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            if c == b'\\' {
                i += 1;
            } else if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'(' => {
                depth += 1;
                let rest = &dump[i + 1..];
                if let Some(kind) = DECL_KINDS.iter().find(|k| {
                    rest.starts_with(**k) && rest[k.len()..].starts_with([' ', '\n', ')'])
                }) {
                    let line = rest[kind.len()..].lines().next().unwrap_or("");
                    let name = decl_name(line, *kind == "extension_decl");
                    if let Some(name) = name {
                        let full = scopes
                            .iter()
                            .map(|(_, n)| n.as_str())
                            .chain(std::iter::once(name.as_str()))
                            .collect::<Vec<_>>()
                            .join(".");
                        if *kind != "extension_decl" {
                            out.insert(full.clone());
                        }
                        scopes.push((depth, full));
                    }
                }
            }
            b')' => {
                while scopes.last().is_some_and(|(d, _)| *d == depth) {
                    scopes.pop();
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// The declared name on a decl line: the first quoted string, or for an
/// extension the first bare token that is not `key=value`.
fn decl_name(line: &str, extension: bool) -> Option<String> {
    if let Some(s) = line.find('"') {
        let rest = &line[s + 1..];
        if let Some(e) = rest.find('"') {
            return Some(rest[..e].to_string());
        }
    }
    if extension {
        return line
            .split_whitespace()
            .find(|t| !t.contains('=') && !t.starts_with('(') && !t.starts_with('['))
            .map(|t| t.trim_end_matches(')').to_string());
    }
    None
}

fn swift_type_names(src: &str) -> BTreeSet<String> {
    let dir = std::env::temp_dir().join(format!("aimux-swift-names-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join(format!("in-{}.swift", src.len()));
    std::fs::write(&f, src).unwrap();
    let out = Command::new("swiftc")
        .arg("-dump-parse")
        .arg(&f)
        .output()
        .expect(
            "`swiftc` is required for the Swift type-name reader (it is not skipped when missing)",
        );
    let _ = std::fs::remove_file(&f);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "swiftc -dump-parse failed:\n{text}");
    names_from_dump(&text)
}

/// The rejected approach (the mutation): `^public (struct|enum) X` per line.
fn line_reader(src: &str) -> BTreeSet<String> {
    src.lines()
        .filter_map(|l| {
            let r = l
                .strip_prefix("public struct ")
                .or_else(|| l.strip_prefix("public enum "))?;
            let n: String = r
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!n.is_empty()).then_some(n)
        })
        .collect()
}

const NAMES_SAMPLE: &str = "public struct A {}; public enum B {}\npublic struct Outer {\n    public struct Inner {}\n}\nextension Outer {\n    public enum Ext {}\n}\npublic typealias T = Int\n";

fn names_expected() -> BTreeSet<String> {
    ["A", "B", "Outer", "Outer.Inner", "Outer.Ext", "T"]
        .map(String::from)
        .into()
}

const HAND_FILES: [&str; 3] = ["Types.swift", "Multimodal.swift", "DecisionModel.swift"];

/// Top-level hand-written Swift type names.
fn hand_types() -> BTreeSet<String> {
    let dir = workspace().join("bindings/swift/Sources/Aimux");
    let mut out = BTreeSet::new();
    for f in HAND_FILES {
        let src = std::fs::read_to_string(dir.join(f)).unwrap();
        out.extend(
            swift_type_names(&src)
                .into_iter()
                .filter(|n| !n.contains('.')),
        );
    }
    out
}

fn generated_types() -> BTreeSet<String> {
    wire::from_inputs(&Inputs::from_workspace(&workspace()))
        .unwrap()
        .types
        .into_iter()
        .map(|t| t.name)
        .collect()
}

fn uncovered(fixtures: &[Fixture], hand: &BTreeSet<String>) -> Vec<String> {
    let gen_t = generated_types();
    hand.iter()
        .filter(|t| gen_t.contains(*t) && !fixtures.iter().any(|f| f.ty == t.as_str()))
        .cloned()
        .collect()
}

fn without_empty(fixtures: &[Fixture], hand: &BTreeSet<String>) -> Vec<String> {
    let gen_t = generated_types();
    hand.iter()
        .filter(|t| gen_t.contains(*t))
        .filter(|t| {
            !fixtures
                .iter()
                .any(|f| f.ty == t.as_str() && f.label == format!("empty-{t}"))
        })
        .cloned()
        .collect()
}

// ---- the Swift run -------------------------------------------------------------

#[derive(serde::Deserialize, Debug, Clone)]
struct RawResult {
    label: String,
    gen_out: Option<String>,
    gen_err: String,
    hand_out: Option<String>,
    hand_err: String,
}

#[derive(Debug, Clone)]
struct SwiftResult {
    label: String,
    gen_out: Option<Value>,
    gen_err: String,
    hand_out: Option<Value>,
    hand_err: String,
}

#[derive(Debug, Clone)]
struct SwiftRun {
    results: Vec<SwiftResult>,
    /// `swift build` warnings (reported, not fatal).
    warnings: Vec<String>,
}

fn hand_source(f: &str) -> String {
    let text =
        std::fs::read_to_string(workspace().join("bindings/swift/Sources/Aimux").join(f)).unwrap();
    // The C module is not built here; the wire types do not need it.
    text.lines()
        .filter(|l| l.trim() != "import CAimuxFFI")
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build the harness package and run it with real `swift`. `mutate` edits
/// each generated file (`(file name, content)`) before the build.
fn run_swift(
    label: &str,
    fixtures: &[Fixture],
    mutate: impl Fn(&str, String) -> String,
) -> SwiftRun {
    let dir = std::env::temp_dir().join(format!(
        "aimux-wire-diff-swift-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    for d in [
        "Sources/AimuxWire",
        "Sources/Hand",
        "Sources/Harness",
        "fixtures",
    ] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    std::fs::write(
        dir.join("Package.swift"),
        "// swift-tools-version:5.9\nimport PackageDescription\n\nlet package = Package(\n    name: \"Harness\",\n    targets: [\n        .target(name: \"AimuxWire\", path: \"Sources/AimuxWire\"),\n        .target(name: \"Hand\", path: \"Sources/Hand\"),\n        .executableTarget(name: \"Harness\", dependencies: [\"AimuxWire\", \"Hand\"], path: \"Sources/Harness\"),\n    ]\n)\n",
    )
    .unwrap();
    let model = wire::from_inputs(&Inputs::from_workspace(&workspace())).unwrap();
    Swift::check(&model).expect("the Swift emitter accepts the real model");
    for o in Swift.emit(&model) {
        let Some(name) = o.path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name == "Package.swift" {
            continue;
        }
        std::fs::write(
            dir.join("Sources/AimuxWire").join(name),
            mutate(name, o.content),
        )
        .unwrap();
    }
    for f in HAND_FILES {
        std::fs::write(dir.join("Sources/Hand").join(f), hand_source(f)).unwrap();
    }
    let hand = hand_types();
    let mut cases = String::new();
    for t in fixtures.iter().map(|f| f.ty).collect::<BTreeSet<_>>() {
        let _ = std::fmt::Write::write_fmt(
            &mut cases,
            format_args!(
                "    case {t:?}:\n        (o.gen_out, o.gen_err) = through(AimuxWire.{t}.self, data)\n{}",
                if hand.contains(t) {
                    format!("        (o.hand_out, o.hand_err) = through(Hand.{t}.self, data)\n")
                } else {
                    String::new()
                }
            ),
        );
    }
    let main = format!(
        r#"import Foundation
import AimuxWire
import Hand

struct Out: Encodable {{
    var label: String
    var gen_out: String?
    var gen_err: String
    var hand_out: String?
    var hand_err: String
}}

func through<T: Codable>(_ t: T.Type, _ d: Data) -> (String?, String) {{
    let v: T
    do {{ v = try JSONDecoder().decode(T.self, from: d) }} catch {{ return (nil, "decode: \(error)") }}
    do {{ return (String(decoding: try JSONEncoder().encode(v), as: UTF8.self), "") }} catch {{ return (nil, "encode: \(error)") }}
}}

let dir = CommandLine.arguments[1]
let index = try! String(contentsOfFile: dir + "/index.tsv", encoding: .utf8)
var outs: [Out] = []
for line in index.split(separator: "\n") {{
    let p = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
    let data = FileManager.default.contents(atPath: dir + "/" + p[0] + ".json")!
    var o = Out(label: p[1], gen_out: nil, gen_err: "", hand_out: nil, hand_err: "")
    switch p[2] {{
{cases}    default:
        fatalError("no Swift type for " + p[2])
    }}
    outs.append(o)
}}
FileHandle.standardOutput.write(try! JSONEncoder().encode(outs))
"#
    );
    std::fs::write(dir.join("Sources/Harness/main.swift"), main).unwrap();
    let mut index = String::new();
    for (i, f) in fixtures.iter().enumerate() {
        std::fs::write(
            dir.join("fixtures").join(format!("{i}.json")),
            f.json.to_string(),
        )
        .unwrap();
        index.push_str(&format!("{i}\t{}\t{}\n", f.label, f.ty));
    }
    std::fs::write(dir.join("fixtures/index.tsv"), index).unwrap();
    let build = Command::new("swift")
        .args(["build", "--product", "Harness"])
        .current_dir(&dir)
        .output()
        .expect(SWIFT_MISSING);
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(build.status.success(), "swift build failed:\n{log}");
    let warnings = log
        .lines()
        .filter(|l| l.contains("warning:"))
        .map(String::from)
        .collect();
    let run = Command::new("swift")
        .args(["run", "--skip-build", "Harness"])
        .arg(dir.join("fixtures"))
        .current_dir(&dir)
        .output()
        .expect(SWIFT_MISSING);
    assert!(
        run.status.success(),
        "swift run failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let raw: Vec<RawResult> = serde_json::from_slice(&run.stdout).unwrap();
    let parse = |s: &Option<String>| s.as_ref().map(|t| serde_json::from_str(t).unwrap());
    let results = raw
        .iter()
        .map(|r| SwiftResult {
            label: r.label.clone(),
            gen_out: parse(&r.gen_out),
            gen_err: r.gen_err.clone(),
            hand_out: parse(&r.hand_out),
            hand_err: r.hand_err.clone(),
        })
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    SwiftRun { results, warnings }
}

// ---- comparison ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Row {
    ty: String,
    field: String,
    generated: String,
    handwritten: String,
    truth: String,
}

const ABSENT: &str = "(absent)";

fn show(v: Option<&Value>) -> String {
    v.map_or_else(|| ABSENT.to_string(), Value::to_string)
}

fn child<'a>(v: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    v.and_then(Value::as_object).and_then(|o| o.get(key))
}

fn at(v: Option<&Value>, i: usize) -> Option<&Value> {
    v.and_then(Value::as_array).and_then(|a| a.get(i))
}

fn join(path: &str, k: &str) -> String {
    if path.is_empty() {
        k.to_string()
    } else {
        format!("{path}.{k}")
    }
}

/// A generated-vs-serde difference at one leaf of one fixture.
#[derive(Debug, Clone, PartialEq)]
struct GenDiff {
    label: String,
    ty: String,
    path: String,
    generated: Option<Value>,
    truth: Option<Value>,
}

/// Leaf differences between `truth` and `gen_v` (objects by key union,
/// arrays by index).
fn leaf_diffs(
    label: &str,
    ty: &str,
    path: &str,
    truth: Option<&Value>,
    gen_v: Option<&Value>,
    out: &mut Vec<GenDiff>,
) {
    match (truth, gen_v) {
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for k in keys {
                leaf_diffs(label, ty, &join(path, k), a.get(k), b.get(k), out);
            }
        }
        (Some(Value::Array(a)), Some(Value::Array(b))) => {
            for i in 0..a.len().max(b.len()) {
                leaf_diffs(label, ty, &format!("{path}[{i}]"), a.get(i), b.get(i), out);
            }
        }
        (t, g) if t != g => out.push(GenDiff {
            label: label.into(),
            ty: ty.into(),
            path: if path.is_empty() {
                "(value)".into()
            } else {
                path.into()
            },
            generated: g.cloned(),
            truth: t.cloned(),
        }),
        _ => {}
    }
}

/// Generated vs hand-written at every path (Go's `diff`).
fn diff(
    ty: &str,
    path: &str,
    truth: Option<&Value>,
    gen_v: Option<&Value>,
    hand: Option<&Value>,
    rows: &mut Vec<Row>,
) {
    let all = [truth, gen_v, hand];
    if all.iter().any(|v| v.is_some_and(Value::is_object))
        && all.iter().all(|v| v.is_none_or(Value::is_object))
    {
        let mut keys = BTreeSet::new();
        for v in all.into_iter().flatten() {
            keys.extend(v.as_object().unwrap().keys().cloned());
        }
        for k in keys {
            diff(
                ty,
                &join(path, &k),
                child(truth, &k),
                child(gen_v, &k),
                child(hand, &k),
                rows,
            );
        }
        return;
    }
    if all
        .iter()
        .any(|v| v.is_some_and(|x| x.as_array().is_some_and(|a| !a.is_empty())))
        && all.iter().all(|v| v.is_none_or(Value::is_array))
    {
        let len = all
            .iter()
            .map(|v| v.and_then(Value::as_array).map_or(0, Vec::len))
            .max()
            .unwrap_or(0);
        for i in 0..len {
            diff(
                ty,
                &format!("{path}[{i}]"),
                at(truth, i),
                at(gen_v, i),
                at(hand, i),
                rows,
            );
        }
        return;
    }
    if gen_v != hand {
        rows.push(Row {
            ty: ty.into(),
            field: if path.is_empty() {
                "(value)".into()
            } else {
                path.into()
            },
            generated: show(gen_v),
            handwritten: show(hand),
            truth: show(truth),
        });
    }
}

struct Compared {
    rows: Vec<Row>,
    gen_diffs: Vec<GenDiff>,
    errors: Vec<String>,
}

fn compare(fixtures: &[Fixture], run: &SwiftRun, hand: &BTreeSet<String>) -> Compared {
    let mut c = Compared {
        rows: Vec::new(),
        gen_diffs: Vec::new(),
        errors: Vec::new(),
    };
    for f in fixtures {
        let r = run.results.iter().find(|r| r.label == f.label).unwrap();
        if !r.gen_err.is_empty() {
            c.errors
                .push(format!("{} (generated): {}", f.label, r.gen_err));
        }
        leaf_diffs(
            f.label,
            f.ty,
            "",
            Some(&f.json),
            r.gen_out.as_ref(),
            &mut c.gen_diffs,
        );
        // Swift → Rust: serde must decode the generated output to the same value.
        if let (Some(b), Some(out)) = (f.back, r.gen_out.as_ref()) {
            match b(out) {
                Ok(v) => leaf_diffs(f.label, f.ty, "", Some(&f.json), Some(&v), &mut c.gen_diffs),
                Err(e) => c.errors.push(format!(
                    "{}: serde rejects the generated Swift output {out}: {e}",
                    f.label
                )),
            }
        }
        if !hand.contains(f.ty) {
            continue;
        }
        if !r.hand_err.is_empty() {
            c.rows.push(Row {
                ty: f.ty.into(),
                field: "(decode)".into(),
                generated: "ok".into(),
                handwritten: r.hand_err.clone(),
                truth: "ok".into(),
            });
            continue;
        }
        diff(
            f.ty,
            "",
            Some(&f.json),
            r.gen_out.as_ref(),
            r.hand_out.as_ref(),
            &mut c.rows,
        );
    }
    c.gen_diffs.dedup();
    c.rows.sort();
    c.rows.dedup_by(|a, b| a.ty == b.ty && a.field == b.field);
    c
}

// ---- verdicts and known differences -------------------------------------------

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
struct Verdict {
    #[serde(rename = "type")]
    ty: String,
    field: String,
    verdict: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default, rename = "knownDifference")]
    known_difference: Option<String>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KdEntry {
    id: String,
    language: String,
    /// e.g. `planned-unmeasured`; informational, not checked.
    #[serde(default)]
    #[allow(dead_code)]
    status: Option<String>,
    limitation_source: String,
    phases_excerpt: String,
    scope: Vec<KdScope>,
}

#[derive(serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
struct KdScope {
    fixture: String,
    path: String,
}

#[derive(serde::Deserialize)]
struct KdFile {
    #[serde(rename = "_comment")]
    _comment: String,
    entries: Vec<KdEntry>,
}

fn read_registry() -> Vec<KdEntry> {
    let text = std::fs::read_to_string(workspace().join(KNOWN)).unwrap();
    let f: KdFile = serde_json::from_str(&text).unwrap();
    f.entries
}

fn read_verdicts() -> Vec<Verdict> {
    let text = std::fs::read_to_string(workspace().join(VERDICTS)).unwrap();
    serde_json::from_str(&text).unwrap()
}

const KD: &str = "已知差异";

/// Why `d` is not covered by known difference `e` (`None`: it is).
fn kd_mismatch(e: &KdEntry, d: &GenDiff) -> Option<String> {
    let id = &e.id;
    let (label, path) = (&d.label, &d.path);
    if !e
        .scope
        .iter()
        .any(|s| s.fixture == *label && s.path == *path)
    {
        return Some(format!(
            "known difference {id} for {label} {path} is out of scope"
        ));
    }
    let (Some(g), Some(t)) = (
        d.generated.as_ref().and_then(Value::as_f64),
        d.truth.as_ref().and_then(Value::as_f64),
    ) else {
        return Some(format!(
            "known difference {id} for {label} {path}: not both numbers"
        ));
    };
    if !(d.generated.as_ref().is_some_and(Value::is_number)
        && d.truth.as_ref().is_some_and(Value::is_number))
    {
        return Some(format!(
            "known difference {id} for {label} {path}: not both numbers"
        ));
    }
    #[allow(clippy::float_cmp)]
    if g != t {
        return Some(format!(
            "known difference {id} for {label} {path}: not equal as f64 ({} vs {})",
            show(d.generated.as_ref()),
            show(d.truth.as_ref())
        ));
    }
    None
}

/// Three-way known-difference checks plus scope/numbers/f64 on every
/// generated difference a 已知差异 verdict claims. Returns the problems
/// and the generated differences no valid known difference covers
/// (generator errors).
fn known_difference_problems(
    diffs: &[GenDiff],
    verdicts: &[Verdict],
    registry: &[KdEntry],
) -> (Vec<String>, Vec<GenDiff>) {
    let mut problems = Vec::new();
    let mut valid: Vec<(&Verdict, &KdEntry)> = Vec::new();
    for v in verdicts {
        match (&v.known_difference, v.verdict == KD) {
            (None, false) => continue,
            (None, true) => {
                problems.push(format!(
                    "verdict {KD} for {} | {} has no knownDifference",
                    v.ty, v.field
                ));
                continue;
            }
            (Some(id), false) => {
                problems.push(format!(
                    "knownDifference {id} on a {} verdict for {} | {}",
                    v.verdict, v.ty, v.field
                ));
                continue;
            }
            (Some(_), true) => {}
        }
        let id = v.known_difference.as_ref().unwrap();
        match registry.iter().find(|e| &e.id == id) {
            None => problems.push(format!("known difference {id} not registered")),
            Some(e) if e.language != "swift" => {
                problems.push(format!(
                    "known difference {id} is registered for {}, not swift",
                    e.language
                ));
            }
            Some(e) => valid.push((v, e)),
        }
    }
    let mut uncovered = Vec::new();
    for d in diffs {
        let claims: Vec<&(&Verdict, &KdEntry)> = valid
            .iter()
            .filter(|(v, _)| v.ty == d.ty && v.field == d.path)
            .collect();
        if claims.is_empty() {
            uncovered.push(d.clone());
            continue;
        }
        let why: Vec<String> = claims
            .iter()
            .filter_map(|(_, e)| kd_mismatch(e, d))
            .collect();
        if why.len() == claims.len() {
            problems.extend(why);
            uncovered.push(d.clone());
        }
    }
    for e in registry.iter().filter(|e| e.language == "swift") {
        if !verdicts
            .iter()
            .any(|v| v.known_difference.as_deref() == Some(e.id.as_str()))
        {
            problems.push(format!(
                "known difference {} is not referenced by any verdict",
                e.id
            ));
        }
        if e.limitation_source.trim().is_empty() || e.phases_excerpt.trim().is_empty() {
            problems.push(format!(
                "known difference {} has no limitationSource/phasesExcerpt",
                e.id
            ));
        }
    }
    (problems, uncovered)
}

/// Stale exemptions: every scope item of a referenced entry, at a path a
/// 已知差异 verdict names, must still differ.
fn stale_known_differences(
    diffs: &[GenDiff],
    verdicts: &[Verdict],
    registry: &[KdEntry],
) -> Vec<String> {
    let mut out = Vec::new();
    for v in verdicts.iter().filter(|v| v.verdict == KD) {
        let Some(e) = registry
            .iter()
            .find(|e| Some(&e.id) == v.known_difference.as_ref())
        else {
            continue;
        };
        for s in e.scope.iter().filter(|s| s.path == v.field) {
            if !diffs
                .iter()
                .any(|d| d.label == s.fixture && d.path == s.path && d.ty == v.ty)
            {
                out.push(format!(
                    "known difference {}: {} {} no longer differs",
                    e.id, s.fixture, s.path
                ));
            }
        }
    }
    out
}

fn expected_verdict(r: &Row) -> &'static str {
    if r.generated == r.truth {
        "手写错"
    } else {
        "生成器错"
    }
}

/// Go's checks, plus the third verdict (rows under 已知差异 are checked by
/// `known_difference_problems`, not against the table).
fn verdict_problems(rows: &[Row], entries: &[Verdict]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for e in entries {
        if !seen.insert((e.ty.clone(), e.field.clone())) {
            problems.push(format!("duplicate verdict for {} | {}", e.ty, e.field));
        }
        if e.reason.as_deref().is_none_or(|r| r.trim().is_empty()) {
            problems.push(format!("verdict for {} | {} has no reason", e.ty, e.field));
        }
        if !["生成器错", "手写错", KD].contains(&e.verdict.as_str()) {
            problems.push(format!(
                "bad verdict `{}` for {} | {}",
                e.verdict, e.ty, e.field
            ));
        }
        if e.verdict != KD && !rows.iter().any(|r| r.ty == e.ty && r.field == e.field) {
            problems.push(format!(
                "orphan verdict for {} | {} (no such diff row)",
                e.ty, e.field
            ));
        }
    }
    for r in rows {
        match entries.iter().find(|e| e.ty == r.ty && e.field == r.field) {
            None => problems.push(format!("no verdict for {} | {}", r.ty, r.field)),
            Some(e)
                if ["生成器错", "手写错"].contains(&e.verdict.as_str())
                    && e.verdict != expected_verdict(r) =>
            {
                problems.push(format!(
                    "verdict `{}` for {} | {} contradicts the Rust output {}",
                    e.verdict, r.ty, r.field, r.truth
                ))
            }
            Some(_) => {}
        }
    }
    problems
}

fn write_atomic(path: &Path, text: &str) {
    let tmp = path.with_extension(format!("md.tmp-{}", std::process::id()));
    std::fs::write(&tmp, text).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

fn doc_problem(committed: Option<&str>, doc: &str) -> Option<String> {
    (committed != Some(doc)).then(|| {
        format!("{DOC} is out of date; run AIMUX_BLESS_DIFF=1 cargo test -p aimux-codegen --test wire_diff_swift")
    })
}

fn cell(s: &str) -> String {
    format!("`{}`", s.replace('|', "\\|"))
}

fn render(
    fixtures: &[Fixture],
    run: &SwiftRun,
    rows: &[Row],
    verdicts: &[Verdict],
    hand: &BTreeSet<String>,
) -> String {
    let v: BTreeMap<(String, String), &str> = verdicts
        .iter()
        .map(|e| ((e.ty.clone(), e.field.clone()), e.verdict.as_str()))
        .collect();
    let mut s = String::from(
        "<!-- Generated by tools/aimux-codegen/tests/wire_diff_swift.rs; do not edit.\n     Regenerate: AIMUX_BLESS_DIFF=1 cargo test -p aimux-codegen --test wire_diff_swift\n     Verdicts: docs/plan/2.2-diff-swift-verdicts.json; known differences: docs/plan/2.2-known-differences.json -->\n\n# 2.2: generated Swift vs hand-written Swift\n\n",
    );
    s.push_str("Each fixture is a real aimux-core value serialized with serde (the truth). It is decoded and re-encoded by the generated Swift (`aimux-codegen --emit swift`) and by copies of the hand-written `bindings/swift/Sources/Aimux/{Types,Multimodal,DecisionModel}.swift`. A row is a JSON path where the two outputs differ, compared as `serde_json::Value`.\n\n## Fixtures\n\n| label | type | serde output |\n|---|---|---|\n");
    for f in fixtures {
        s.push_str(&format!(
            "| {} | {} | {} |\n",
            f.label,
            f.ty,
            cell(&f.json.to_string())
        ));
    }
    s.push_str("\n## Diff\n\n| type | field | generated | handwritten | verdict |\n|---|---|---|---|---|\n");
    for r in rows {
        let verdict = v
            .get(&(r.ty.clone(), r.field.clone()))
            .copied()
            .unwrap_or("(missing)");
        s.push_str(&format!(
            "| {} | {} | {} | {} | {verdict} |\n",
            r.ty,
            cell(&r.field),
            cell(&r.generated),
            cell(&r.handwritten)
        ));
    }
    s.push_str(&format!(
        "\nRows: {}. Build warnings: {}.\n",
        rows.len(),
        run.warnings.len()
    ));
    let gen_t = generated_types();
    s.push_str(&format!(
        "\n## Coverage\n\nHand-written Swift types with a generated counterpart: {}.\n",
        hand.iter().filter(|t| gen_t.contains(*t)).count()
    ));
    s
}

/// One Swift run over all fixtures, shared by the tests below.
fn shared() -> &'static (Vec<Fixture>, SwiftRun, BTreeSet<String>) {
    static RUN: std::sync::OnceLock<(Vec<Fixture>, SwiftRun, BTreeSet<String>)> =
        std::sync::OnceLock::new();
    RUN.get_or_init(|| {
        let f = all_fixtures();
        let r = run_swift("shared", &f, |_, s| s);
        (f, r, hand_types())
    })
}

fn gen_out(label: &str) -> (&'static Value, Value) {
    let (f, r, _) = shared();
    let fx = f.iter().find(|x| x.label == label).unwrap();
    let out = r
        .results
        .iter()
        .find(|x| x.label == label)
        .and_then(|x| x.gen_out.clone())
        .unwrap_or(Value::Null);
    (&fx.json, out)
}

/// `(label, serde accepts, generated Swift accepts)` for `cases`.
fn parity(
    label: &str,
    cases: &[Fixture],
    mutate: impl Fn(&str, String) -> String,
) -> Vec<(String, bool, bool)> {
    let run = run_swift(label, cases, mutate);
    cases
        .iter()
        .zip(&run.results)
        .map(|(f, r)| {
            assert_eq!(f.label, r.label);
            (
                (f.label).to_string(),
                (f.back.unwrap())(&f.json).is_ok(),
                r.gen_err.is_empty(),
            )
        })
        .collect()
}

fn divergent(p: &[(String, bool, bool)]) -> Vec<String> {
    p.iter()
        .filter(|(_, s, g)| s != g)
        .map(|(l, s, g)| format!("{l}: serde accepts={s}, generated Swift accepts={g}"))
        .collect()
}

// ---- tests needing swift -------------------------------------------------------

#[test]
fn every_hand_written_type_with_a_generated_counterpart_has_a_fixture() {
    let (f, _, hand) = shared();
    assert_eq!(uncovered(f, hand), Vec::<String>::new());
}

#[test]
fn every_covered_type_has_an_empty_value_fixture() {
    let (f, _, hand) = shared();
    assert_eq!(without_empty(f, hand), Vec::<String>::new());
}

#[test]
fn decode_and_encode_directions_match_serde() {
    let (f, r, hand) = shared();
    let c = compare(f, r, hand);
    assert!(c.errors.is_empty(), "{:#?}", c.errors);
    let (problems, uncovered) =
        known_difference_problems(&c.gen_diffs, &read_verdicts(), &read_registry());
    assert!(
        uncovered.is_empty(),
        "generator errors (generated Swift != serde):\n{uncovered:#?}\n{problems:#?}"
    );
}

#[test]
fn null_fields_stay_null() {
    let (truth, out) = gen_out("chat-completion-chunk");
    assert_eq!(truth["choices"][0]["finish_reason"], Value::Null);
    assert!(
        out["choices"][0].as_object().unwrap().get("finish_reason") == Some(&Value::Null),
        "{out}"
    );
    // Mutation: encodeNil → encodeIfPresent; the key disappears.
    let f: Vec<Fixture> = all_fixtures()
        .into_iter()
        .filter(|x| x.label == "chat-completion-chunk")
        .collect();
    let run = run_swift("null-mut", &f, |n, s| {
        if n == "Types.swift" {
            s.replace("encodeNullable(", "encodeIfPresent(")
        } else {
            s
        }
    });
    let got = run.results[0].gen_out.clone().unwrap();
    assert!(
        got["choices"][0].get("finish_reason").is_none(),
        "mutation not caught: {got}"
    );
}

#[test]
fn tri_state_and_nullable_round_trip() {
    for l in ["tri-state-absent", "tri-state-null", "tri-state-present"] {
        let (truth, out) = gen_out(l);
        assert_eq!(&out, truth, "{l}");
    }
}

#[test]
fn proxied_and_generic_types_match_serde() {
    for l in ["proxy-aimux-error", "proxy-size"] {
        let (truth, out) = gen_out(l);
        assert_eq!(&out, truth, "{l}");
    }
}

#[test]
fn generated_swift_matches_rust_and_diff_doc_is_current() {
    let (f, r, hand) = shared();
    let c = compare(f, r, hand);
    let verdicts = read_verdicts();
    let (_, uncovered) = known_difference_problems(&c.gen_diffs, &verdicts, &read_registry());
    assert!(
        uncovered.is_empty(),
        "generator errors never enter the table: {uncovered:#?}"
    );
    let problems = verdict_problems(&c.rows, &verdicts);
    let doc = render(f, r, &c.rows, &verdicts, hand);
    let path = workspace().join(DOC);
    if std::env::var_os("AIMUX_BLESS_DIFF").is_some() {
        write_atomic(&path, &doc);
    }
    let committed = std::fs::read_to_string(&path).ok();
    assert!(problems.is_empty(), "{problems:#?}");
    if let Some(p) = doc_problem(committed.as_deref(), &doc) {
        panic!("{p}");
    }
}

#[test]
fn mutated_wire_name_is_reported_as_exactly_one_row() {
    let f: Vec<Fixture> = all_fixtures()
        .into_iter()
        .filter(|x| x.label == "tool-call")
        .collect();
    let hand = &shared().2;
    let base = compare(&f, &run_swift("wire-base", &f, |_, s| s), hand);
    let run = run_swift("wire-mut", &f, |n, s| {
        if n != "Types.swift" {
            return s;
        }
        // Encode side of ToolCall only: the key is written as `toolCallID`.
        let start = s.find("\npublic struct ToolCall:").unwrap();
        let end = start + s[start..].find("\n}\n").unwrap();
        let from = "try c.encode(toolCallId, forKey: .toolCallId)";
        let block = &s[start..end];
        assert!(block.contains(from), "{block}");
        let to = "do { var x = e.container(keyedBy: AnyCodingKey.self); try x.encode(toolCallId, forKey: AnyCodingKey(\"toolCallID\")) }";
        format!(
            "{}{}{}",
            &s[..start],
            block.replacen(from, to, 1),
            &s[end..]
        )
    });
    let mutated = compare(&f, &run, hand);
    let new: Vec<&Row> = mutated
        .rows
        .iter()
        .filter(|r| !base.rows.contains(r))
        .collect();
    assert_eq!(
        new,
        vec![&Row {
            ty: "ToolCall".into(),
            field: "toolCallId".into(),
            generated: ABSENT.into(),
            handwritten: "\"call_1\"".into(),
            truth: "\"call_1\"".into(),
        }]
    );
    assert_eq!(expected_verdict(new[0]), "生成器错");
    // Both directions catch it: a generated-vs-serde difference, and serde
    // rejects the output (missing `toolCallId`).
    assert!(
        mutated
            .gen_diffs
            .iter()
            .any(|d| d.path == "toolCallId" && d.generated.is_none()),
        "{:?}",
        mutated.gen_diffs
    );
    assert!(
        mutated
            .errors
            .iter()
            .any(|e| e.contains("missing field `toolCallId`")),
        "{:?}",
        mutated.errors
    );
}

#[test]
fn untagged_decode_rejects_null_exactly_like_serde() {
    let cases = null_parity_cases();
    let got = parity("nulls", &cases, |_, s| s);
    assert_eq!(divergent(&got), Vec::<String>::new());
    // Mutation: an Optional-typed FileBytes arm accepts null.
    let m = parity("nulls-mut", &cases, |n, s| {
        if n == "Types.swift" {
            s.replace(
                "c.decode(WireBytes.self) { self = .binary(v)",
                "c.decode(WireBytes?.self) { self = .binary(v ?? [])",
            )
        } else {
            s
        }
    });
    assert!(
        divergent(&m)
            .iter()
            .any(|d| d.starts_with("null-FileBytes:")),
        "{m:#?}"
    );
}

fn binary_labels() -> Vec<&'static str> {
    all_fixtures()
        .iter()
        .filter(|f| {
            (f.ty == "AudioData" || f.ty == "VideoData")
                && f.label.to_lowercase().contains("binary")
        })
        .map(|f| f.label)
        .collect()
}

#[test]
fn binary_audio_and_video_equal_serde() {
    let labels = binary_labels();
    assert!(labels.len() >= 2, "{labels:?}");
    for l in &labels {
        let (truth, out) = gen_out(l);
        assert_eq!(&out, truth, "{l}");
    }
    // Mutation: WireBytes = Data (base64).
    let f: Vec<Fixture> = all_fixtures()
        .into_iter()
        .filter(|x| labels.contains(&x.label))
        .collect();
    let run = run_swift("bin-mut", &f, |n, s| {
        if n == "Runtime.swift" {
            s.replace(
                "public typealias WireBytes = [UInt8]",
                "public typealias WireBytes = Data",
            )
        } else {
            s
        }
    });
    for (fx, r) in f.iter().zip(&run.results) {
        assert_ne!(
            r.gen_out.as_ref(),
            Some(&fx.json),
            "{} not caught",
            fx.label
        );
    }
}

#[test]
fn json_value_numbers_match_serde() {
    for l in [
        "json-number-2^53+1",
        "json-number-u64max",
        "json-number-i64min",
        "json-number-neg1",
        "json-number-0",
        "json-number-1",
        "json-number-true",
    ] {
        let (truth, out) = gen_out(l);
        assert_eq!(&out, truth, "{l}");
    }
    // Mutation: drop the int/uint branches (everything goes through Double).
    let f: Vec<Fixture> = number_fixtures()
        .into_iter()
        .filter(|x| x.label == "json-number-2^53+1" || x.label == "json-number-u64max")
        .collect();
    let run = run_swift("num-mut", &f, |n, s| {
        if n == "Runtime.swift" {
            s.replace(
                "if let v = try? c.decode(Int64.self) { self = .int(v); return }\n",
                "",
            )
            .replace(
                "if let v = try? c.decode(UInt64.self) { self = .uint(v); return }\n",
                "",
            )
        } else {
            s
        }
    });
    for (fx, r) in f.iter().zip(&run.results) {
        assert_ne!(
            r.gen_out.as_ref(),
            Some(&fx.json),
            "{} not caught",
            fx.label
        );
    }
}

#[test]
fn known_differences_still_differ() {
    let (f, r, hand) = shared();
    let c = compare(f, r, hand);
    assert_eq!(
        stale_known_differences(&c.gen_diffs, &read_verdicts(), &read_registry()),
        Vec::<String>::new()
    );
}

#[test]
fn open_enum_keeps_unknown_value() {
    use aimux_core::model_catalogue::ModelType;
    let b: Back = back::<ModelType>;
    let cases = vec![
        Fixture {
            label: "open-enum-unknown",
            ty: "ModelType",
            json: json!("SomethingNew"),
            back: Some(b),
        },
        Fixture {
            label: "open-enum-alias",
            ty: "ModelType",
            json: json!("image-generation"),
            back: Some(b),
        },
    ];
    let run = run_swift("open", &cases, |_, s| s);
    // Ruled: the received string is kept (serde would write its `other` name).
    assert_eq!(
        run.results[0].gen_out,
        Some(json!("SomethingNew")),
        "{:?}",
        run.results[0]
    );
    // Alias maps to the main value, as serde does.
    assert_eq!(run.results[1].gen_out, Some(json!("ImageGen")));
    assert_eq!(b(&json!("image-generation")).unwrap(), json!("ImageGen"));
    // Mutation: emit ModelType closed.
    let m = run_swift("open-mut", &cases[..1], |n, s| {
        if n == "Types.swift" {
            s.replace("public init(from d: Decoder) throws {\n        let s = try d.singleValueContainer().decode(String.self)\n        rawValue = Self.aliases[s] ?? s", "public init(from d: Decoder) throws {\n        let s = try d.singleValueContainer().decode(String.self)\n        guard Self.known.contains(s) else { throw DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: \"unknown value \\\"\\(s)\\\"\")) }\n        rawValue = Self.aliases[s] ?? s")
        } else {
            s
        }
    });
    assert!(
        m.results[0]
            .gen_err
            .contains("unknown value \"SomethingNew\""),
        "{:?}",
        m.results[0]
    );
}

#[test]
fn integer_overflow_and_fraction_match_serde() {
    let cases: Vec<Fixture> = reject_cases()
        .into_iter()
        .filter(|f| f.label.starts_with("int-"))
        .collect();
    let got = parity("ints", &cases, |_, s| s);
    assert!(
        got.iter().all(|(_, s, _)| !*s),
        "serde must reject all: {got:?}"
    );
    assert_eq!(divergent(&got), Vec::<String>::new());
    // Mutation: UInt32 → Int widens; overflow and negative are accepted.
    let m = parity("ints-mut", &cases, |n, s| {
        if n == "Types.swift" {
            s.replace("UInt32", "Int")
        } else {
            s
        }
    });
    let d = divergent(&m);
    assert!(
        d.iter().any(|x| x.starts_with("int-overflow-u32"))
            && d.iter().any(|x| x.starts_with("int-negative-u32")),
        "{d:?}"
    );
}

#[test]
fn default_fields_reject_null_like_serde() {
    let cases: Vec<Fixture> = reject_cases()
        .into_iter()
        .filter(|f| f.label.starts_with("null-default-"))
        .collect();
    let got = parity("defaults", &cases, |_, s| s);
    assert!(
        got.iter().all(|(_, s, _)| !*s),
        "serde must reject all: {got:?}"
    );
    assert_eq!(divergent(&got), Vec::<String>::new());
    // Mutation: contains ? decode : default → decodeIfPresent ?? default.
    let m = parity("defaults-mut", &cases, |n, s| {
        if n == "Runtime.swift" {
            s.replace(
                "contains(k) ? try decode(t, forKey: k) : def()",
                "try decodeIfPresent(t, forKey: k) ?? def()",
            )
        } else {
            s
        }
    });
    assert!(
        divergent(&m)
            .iter()
            .any(|x| x.starts_with("null-default-DecisionCapabilities.supports_images")),
        "{m:?}"
    );
}

#[test]
fn swift_type_names_counts_nested_extension_and_one_line() {
    assert_eq!(swift_type_names(NAMES_SAMPLE), names_expected());
}

// ---- Rust-only tests -------------------------------------------------------------

/// The line-by-line reader (the mutation of the dump-parse reader) misses
/// the second one-line declaration, the nested type and the extension type.
#[test]
fn line_reader_misses_nested_extension_and_one_line() {
    let got = line_reader(NAMES_SAMPLE);
    let missed: BTreeSet<String> = names_expected().difference(&got).cloned().collect();
    assert_eq!(
        missed,
        ["B", "Outer.Inner", "Outer.Ext", "T"]
            .map(String::from)
            .into()
    );
}

#[test]
fn reject_cases_are_rejected_by_serde() {
    for f in reject_cases() {
        assert!(
            (f.back.unwrap())(&f.json).is_err(),
            "serde accepts {}",
            f.label
        );
    }
}

/// The real registry and verdict files. Intentionally red until the
/// toolchain measurement: KD-SWIFT-NUM-1 is registered (planned-unmeasured)
/// and no verdict references it yet; no exemption.
#[test]
fn known_differences_are_registered_scoped_and_used() {
    let (problems, _) = known_difference_problems(&[], &read_verdicts(), &read_registry());
    assert!(problems.is_empty(), "{problems:#?}");
}

fn kd_verdict(ty: &str, field: &str, id: &str) -> Verdict {
    Verdict {
        ty: ty.into(),
        field: field.into(),
        verdict: KD.into(),
        reason: Some(
            "Foundation JSONDecoder; 见 docs/plan/2.2-provenance.md § PHASES.md:307".into(),
        ),
        known_difference: Some(id.into()),
    }
}

fn gd(label: &str, path: &str, g: Option<Value>, t: Value) -> GenDiff {
    GenDiff {
        label: label.into(),
        ty: "Usage".into(),
        path: path.into(),
        generated: g,
        truth: Some(t),
    }
}

/// Every known-difference check catches its mutation (synthetic generated
/// differences shaped like the planned measurement: `1.0` → `1`).
#[test]
fn known_difference_checks_each_catch_their_mutation() {
    let reg = read_registry();
    let base = vec![
        gd("json-number-1.0", "raw.n", Some(json!(1)), json!(1.0)),
        gd("json-number-1e3", "raw.n", Some(json!(1000)), json!(1000.0)),
        gd("json-number-neg-zero", "raw.n", Some(json!(0)), json!(-0.0)),
    ];
    let v = vec![kd_verdict("Usage", "raw.n", "KD-SWIFT-NUM-1")];
    let ok = known_difference_problems(&base, &v, &reg);
    assert_eq!(ok.0, Vec::<String>::new());
    assert!(ok.1.is_empty());
    assert_eq!(
        stale_known_differences(&base, &v, &reg),
        Vec::<String>::new()
    );

    let first = |d: &[GenDiff], v: &[Verdict], r: &[KdEntry]| known_difference_problems(d, v, r).0;
    // ID exists.
    assert_eq!(
        first(
            &base,
            &[kd_verdict("Usage", "raw.n", "KD-SWIFT-NUM-9")],
            &reg
        )[0],
        "known difference KD-SWIFT-NUM-9 not registered"
    );
    // Language matches.
    let mut go = reg.clone();
    go[0].language = "go".into();
    assert_eq!(
        first(&base, &v, &go)[0],
        "known difference KD-SWIFT-NUM-1 is registered for go, not swift"
    );
    // Row in scope: a non-raw field of Usage.
    let d = vec![gd(
        "json-number-1.0",
        "inputTokens.total",
        Some(json!(10)),
        json!(10.0),
    )];
    assert_eq!(
        first(
            &d,
            &[kd_verdict("Usage", "inputTokens.total", "KD-SWIFT-NUM-1")],
            &reg
        ),
        vec![
            "known difference KD-SWIFT-NUM-1 for json-number-1.0 inputTokens.total is out of scope"
        ]
    );
    // Every ID referenced.
    let (p, unc) = known_difference_problems(&base, &[], &reg);
    assert_eq!(
        p,
        vec!["known difference KD-SWIFT-NUM-1 is not referenced by any verdict"]
    );
    assert_eq!(
        unc.len(),
        3,
        "unreferenced differences stay generator errors"
    );
    // Both numbers: generated as a string, or missing.
    let mut s = base.clone();
    s[0].generated = Some(json!("1.0"));
    assert_eq!(
        first(&s, &v, &reg),
        vec!["known difference KD-SWIFT-NUM-1 for json-number-1.0 raw.n: not both numbers"]
    );
    s[0].generated = None;
    assert_eq!(
        first(&s, &v, &reg),
        vec!["known difference KD-SWIFT-NUM-1 for json-number-1.0 raw.n: not both numbers"]
    );
    // Equal as f64.
    let mut t = base.clone();
    t[0].generated = Some(json!(2));
    let (p, unc) = known_difference_problems(&t, &v, &reg);
    assert_eq!(
        p,
        vec![
            "known difference KD-SWIFT-NUM-1 for json-number-1.0 raw.n: not equal as f64 (2 vs 1.0)"
        ]
    );
    assert_eq!(
        unc,
        vec![t[0].clone()],
        "the failing difference is a generator error"
    );
    // Stale exemption: 0.5 in scope but it matches serde (no difference).
    let mut r = reg.clone();
    r[0].scope.push(KdScope {
        fixture: "json-number-0.5".into(),
        path: "raw.n".into(),
    });
    assert_eq!(
        stale_known_differences(&base, &v, &r),
        vec!["known difference KD-SWIFT-NUM-1: json-number-0.5 raw.n no longer differs"]
    );
}

#[test]
fn verdict_and_doc_checks_each_catch_their_mutation() {
    let row = Row {
        ty: "Usage".into(),
        field: "raw".into(),
        generated: "1".into(),
        handwritten: "(absent)".into(),
        truth: "1".into(),
    };
    let good = Verdict {
        ty: "Usage".into(),
        field: "raw".into(),
        verdict: "手写错".into(),
        reason: Some("r".into()),
        known_difference: None,
    };
    assert_eq!(
        verdict_problems(std::slice::from_ref(&row), std::slice::from_ref(&good)),
        Vec::<String>::new()
    );
    assert_eq!(
        verdict_problems(std::slice::from_ref(&row), &[]),
        vec!["no verdict for Usage | raw"]
    );
    let orphan = Verdict {
        field: "other".into(),
        ..good.clone()
    };
    assert!(
        verdict_problems(std::slice::from_ref(&row), &[good.clone(), orphan])
            .contains(&"orphan verdict for Usage | other (no such diff row)".to_string())
    );
    let wrong = Verdict {
        verdict: "生成器错".into(),
        ..good.clone()
    };
    assert_eq!(
        verdict_problems(std::slice::from_ref(&row), &[wrong]),
        vec!["verdict `生成器错` for Usage | raw contradicts the Rust output 1"]
    );
    let bad = Verdict {
        verdict: "maybe".into(),
        ..good.clone()
    };
    assert_eq!(
        verdict_problems(std::slice::from_ref(&row), &[bad]),
        vec!["bad verdict `maybe` for Usage | raw"]
    );
    let noreason = Verdict {
        reason: None,
        ..good.clone()
    };
    assert_eq!(
        verdict_problems(std::slice::from_ref(&row), &[noreason]),
        vec!["verdict for Usage | raw has no reason"]
    );
    // A 已知差异 entry without knownDifference.
    let kd = Verdict {
        verdict: KD.into(),
        ..good
    };
    assert_eq!(
        known_difference_problems(&[], &[kd], &[]).0,
        vec![format!(
            "verdict {KD} for Usage | raw has no knownDifference"
        )]
    );
    // Doc staleness compares the in-memory render.
    assert!(doc_problem(Some("a"), "a").is_none());
    assert!(doc_problem(Some("a"), "b").unwrap().contains("out of date"));
    assert!(doc_problem(None, "b").is_some());
}
