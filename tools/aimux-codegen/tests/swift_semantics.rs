//! U2: the generated Swift (`generated/swift`), checked without a Swift
//! toolchain. The committed files must equal a fresh emission (snapshot),
//! and every record field of the parsed Swift is compared with the wire IR,
//! the manifest field facts (`omittedWhen`, `null.input` / `null.output`)
//! and the JSON serde really writes (go_fixtures.rs cov fixtures):
//! 1. CodingKey == serde key;
//! 2. optionality (`T?` / `TriState<T>` / neither);
//! 3. null handling: `omittedWhen = Option::is_none` → `encodeIfPresent`;
//!    `null.output = whenNone` (an `Option` without `skip_serializing_if`)
//!    → `encodeNullable` (an explicit `null`); `null.input = rejected` →
//!    not optional and not `decodeIfPresent`; `omittedWhen = Vec::is_empty`
//!    → written only when not empty;
//! 4. a `Literal` field is a constant (`public let x = LitY()`) written
//!    unconditionally.
//!
//! Each check has a mutation of the Swift text that must turn it red.

#![allow(clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use aimux_codegen::emit::go::pascal;
use aimux_codegen::extract::{self, FieldDef, Shape};
use aimux_codegen::manifest::Inputs;
use aimux_codegen::wire::{self, VariantBody, WireBody, WireField, WireTy};
use serde_json::Value;

#[path = "common/go_fixtures.rs"]
mod go_fixtures;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// See tests/wire_diff_go.rs; the fixture file names these.
pub type Back = fn(&Value) -> Result<Value, String>;

/// See tests/wire_diff_go.rs.
///
/// # Errors
///
/// serde rejects the value.
pub fn back<T: serde::Serialize + serde::de::DeserializeOwned>(v: &Value) -> Result<Value, String> {
    let t: T = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(&t).map_err(|e| e.to_string())
}

/// See tests/wire_diff_go.rs.
///
/// # Errors
///
/// serde rejects the value.
pub fn back_tagged<T>(v: &Value) -> Result<Value, String>
where
    T: serde::Serialize + serde::de::DeserializeOwned + aimux_core::wire::TypeTag,
{
    let t: T = aimux_core::wire::typed::deserialize(v.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(aimux_core::wire::Tagged(&t)).map_err(|e| e.to_string())
}

fn committed_types() -> String {
    std::fs::read_to_string(
        workspace().join("tools/aimux-codegen/generated/swift/Sources/AimuxWire/Types.swift"),
    )
    .unwrap()
}

/// One parsed Swift field.
#[derive(Debug, Default, Clone)]
struct SwiftField {
    id: String,
    /// `var` type, or for `let x = T()` the `= T()` text.
    ty: String,
    constant: bool,
    dec: String,
    enc: String,
}

/// struct name → (CodingKey wire → field).
type Parsed = BTreeMap<String, BTreeMap<String, SwiftField>>;

fn parse(text: &str) -> Parsed {
    let mut out = Parsed::new();
    let mut lines = text.lines().peekable();
    while let Some(l) = lines.next() {
        let Some(rest) = l.strip_prefix("public struct ") else {
            continue;
        };
        let name = rest.split(':').next().unwrap().trim().to_string();
        let mut props: BTreeMap<String, (String, bool)> = BTreeMap::new();
        let mut keys: Vec<(String, String)> = Vec::new();
        let mut dec: BTreeMap<String, String> = BTreeMap::new();
        let mut enc: BTreeMap<String, String> = BTreeMap::new();
        let mut section = "";
        for l in lines.by_ref() {
            if l == "}" {
                break;
            }
            let t = l.trim();
            if t.starts_with("enum CodingKeys") {
                section = "keys";
            } else if t.starts_with("public init(from") {
                section = "dec";
            } else if t.starts_with("public func encode(to") {
                section = "enc";
            } else if t.starts_with("public init(") {
                section = "";
            }
            if let Some(r) = t.strip_prefix("public var ") {
                let (id, ty) = r.split_once(": ").unwrap();
                props.insert(id.into(), (ty.into(), false));
            } else if let Some(r) = t.strip_prefix("public let ") {
                let (id, ty) = r.split_once(" = ").unwrap();
                props.insert(id.into(), (ty.into(), true));
            } else if section == "keys"
                && let Some(r) = t.strip_prefix("case ")
                && let Some((id, k)) = r.split_once(" = ")
            {
                keys.push((id.into(), serde_json::from_str(k).unwrap()));
            } else if section == "dec" || section == "enc" {
                for id in props.keys() {
                    let hit = [id.as_str(), id.trim_matches('`')].iter().any(|k| {
                        t.contains(&format!("forKey: .{k})"))
                            || t.contains(&format!("forKey: .{k},"))
                            || t.contains(&format!("contains(.{k})"))
                    });
                    if hit {
                        let m = if section == "dec" { &mut dec } else { &mut enc };
                        m.entry(id.clone()).or_default().push_str(t);
                    }
                }
            }
        }
        let mut fields = BTreeMap::new();
        for (id, k) in keys {
            let (ty, constant) = props.get(&id).cloned().unwrap_or_default();
            let f = SwiftField {
                ty,
                constant,
                dec: dec.get(&id).cloned().unwrap_or_default(),
                enc: enc.get(&id).cloned().unwrap_or_default(),
                id,
            };
            fields.insert(k, f);
        }
        out.insert(name, fields);
    }
    out
}

/// IR records and struct-variant payloads, by Swift struct name.
fn ir_records(model: &wire::WireModel) -> BTreeMap<String, (String, String, Vec<WireField>)> {
    let names: BTreeSet<&str> = model.types.iter().map(|t| t.name.as_str()).collect();
    let mut out = BTreeMap::new();
    for t in &model.types {
        match &t.body {
            WireBody::Record { fields } => {
                out.insert(
                    t.name.clone(),
                    (t.doc_source.clone(), t.name.clone(), fields.clone()),
                );
            }
            WireBody::Union { variants, .. } => {
                for v in variants {
                    if let VariantBody::Fields(fs) = &v.body {
                        let mut n = format!("{}{}", t.name, pascal(&v.rust));
                        if names.contains(n.as_str()) {
                            n.push_str("Variant");
                        }
                        out.insert(
                            n,
                            (
                                t.doc_source.clone(),
                                format!("{}::{}", t.name, v.rust),
                                fs.clone(),
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Manifest field facts by `(source file, owner, wire key)`; a
/// `Tagged<T>` owner reads `T`'s facts. Two Rust types with one name in
/// different files stay apart; an IR type renamed for a clash matches none
/// and is checked against the IR only.
type Facts = BTreeMap<(String, String, String), FieldDef>;

fn manifest_fields(m: &extract::Model) -> Facts {
    let mut out = Facts::new();
    let mut src = String::new();
    let mut add = |src: &str, owner: String, fs: &[FieldDef]| {
        for f in fs {
            out.insert((src.into(), owner.clone(), f.wire.clone()), f.clone());
            out.insert(
                (src.into(), format!("Tagged{owner}"), f.wire.clone()),
                f.clone(),
            );
        }
    };
    for t in m.types.iter().chain(&m.auxiliary_types) {
        src.clone_from(&t.source);
        match &t.shape {
            Shape::Struct { fields, .. } => add(&src, t.name.clone(), fields),
            Shape::Enum { variants, .. } => {
                for v in variants {
                    if let Shape::Struct { fields, .. } = &v.shape {
                        add(&src, format!("{}::{}", t.name, v.rust), fields);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Violations of checks 1–4, each tagged `[1]`..`[4]`.
fn violations(text: &str) -> Vec<String> {
    let inputs = Inputs::from_workspace(&workspace());
    let model = wire::from_inputs(&inputs).unwrap();
    let facts = manifest_fields(
        &extract::extract(&inputs.src_root, aimux_codegen::manifest::SRC_PREFIX).unwrap(),
    );
    let swift = parse(text);
    let mut bad = Vec::new();
    let mut matched = 0;
    for (sname, (source, owner, fields)) in ir_records(&model) {
        let Some(sf) = swift.get(&sname) else {
            bad.push(format!("[1] {sname}: no Swift struct"));
            continue;
        };
        let want: BTreeSet<&str> = fields.iter().map(|f| f.wire.as_str()).collect();
        let got: BTreeSet<&str> = sf.keys().map(String::as_str).collect();
        if want != got {
            bad.push(format!(
                "[1] {sname}: CodingKeys {got:?} != serde keys {want:?}"
            ));
        }
        for f in &fields {
            let Some(s) = sf.get(&f.wire) else { continue };
            let at = format!("{sname}.{}", f.wire);
            let p = f.presence;
            let opt = s.ty.ends_with('?');
            let tri = s.ty.starts_with("TriState<");
            // 4. Literal.
            if let WireTy::Literal(l) = &f.ty {
                let n = format!("Lit{}()", pascal(l));
                if !s.constant || s.ty != n {
                    bad.push(format!(
                        "[4] {at}: Literal is `{}`, want constant `{n}`",
                        s.ty
                    ));
                }
                if s.enc
                    != format!(
                        "try c.encode({}, forKey: .{})",
                        s.id,
                        s.id.trim_matches('`')
                    )
                {
                    bad.push(format!("[4] {at}: Literal not always written: `{}`", s.enc));
                }
                continue;
            }
            if s.constant {
                bad.push(format!("[4] {at}: constant for a non-Literal field"));
            }
            // 2. Optionality from the IR presence.
            // A codec `Option<T>` lowers to a `Nullable` type, not presence.
            let nullable_ty = matches!(f.ty, WireTy::Nullable(_));
            if tri != p.tri_state || opt != ((p.nullable && !p.tri_state) || nullable_ty) {
                bad.push(format!(
                    "[2] {at}: Swift `{}` vs IR nullable={} tri_state={}",
                    s.ty, p.nullable, p.tri_state
                ));
            }
            // Manifest facts.
            let Some(fd) = facts.get(&(source.clone(), owner.clone(), f.wire.clone())) else {
                continue;
            };
            matched += 1;
            match fd.null.input {
                "asAbsent" if !opt => {
                    bad.push(format!("[2] {at}: null.input asAbsent but `{}`", s.ty))
                }
                "distinct" if !tri => {
                    bad.push(format!("[2] {at}: null.input distinct but `{}`", s.ty))
                }
                "rejected" => {
                    if opt || tri {
                        bad.push(format!(
                            "[3] {at}: null.input rejected but optional `{}`",
                            s.ty
                        ));
                    }
                    if s.dec.contains("decodeIfPresent") {
                        bad.push(format!("[3] {at}: null.input rejected but decodeIfPresent"));
                    }
                }
                _ => {}
            }
            match fd.omitted_when.as_deref() {
                Some("Option::is_none")
                    if !(s.enc.contains("encodeIfPresent")
                        || (tri && s.enc.contains("encodeTriState"))) =>
                {
                    bad.push(format!(
                        "[3] {at}: omittedWhen Option::is_none but `{}`",
                        s.enc
                    ));
                }
                Some("Vec::is_empty") if !s.enc.starts_with("if !") => {
                    bad.push(format!(
                        "[3] {at}: omittedWhen Vec::is_empty but `{}`",
                        s.enc
                    ));
                }
                None if s.enc.contains("encodeIfPresent") => {
                    bad.push(format!("[3] {at}: never omitted by serde but `{}`", s.enc));
                }
                _ => {}
            }
            if fd.null.output == "whenNone" && !s.enc.contains("encodeNullable") {
                bad.push(format!(
                    "[3] {at}: null.output whenNone (explicit null) but `{}`",
                    s.enc
                ));
            }
        }
    }
    // 1. Keys serde really writes (filled fixtures) are Swift CodingKeys.
    let tagged: BTreeSet<String> = facts
        .keys()
        .filter(|(_, o, _)| o.starts_with("Tagged"))
        .map(|(_, o, _)| o.clone())
        .collect();
    let mut sampled = 0;
    for (label, ty, json, _) in go_fixtures::coverage_fixtures() {
        let Some(obj) = json.as_object() else {
            continue;
        };
        let name = if obj.contains_key("type")
            && tagged.contains(&format!("Tagged{ty}"))
            && swift.contains_key(&format!("Tagged{ty}"))
        {
            format!("Tagged{ty}")
        } else {
            ty.to_string()
        };
        let Some(sf) = swift.get(&name) else { continue };
        sampled += 1;
        for k in obj.keys() {
            if !sf.contains_key(k) {
                bad.push(format!(
                    "[1] {label}: serde writes `{k}`, no {name} CodingKey"
                ));
            }
        }
    }
    eprintln!("manifest facts matched: {matched} fields; serde output sampled: {sampled} structs");
    assert!(
        matched > 600,
        "manifest facts matched for only {matched} fields"
    );
    assert!(
        sampled > 40,
        "serde output sampled for only {sampled} structs"
    );
    bad
}

#[test]
fn generated_swift_snapshot_is_current() {
    let inputs = Inputs::from_workspace(&workspace());
    let out = workspace().join("tools/aimux-codegen/generated");
    if let Err(p) = aimux_codegen::emit::swift::check_committed(&inputs, &out) {
        panic!("{p:#?}\nrun `cargo run -p aimux-codegen` to regenerate generated/swift");
    }
}

#[test]
fn generated_swift_fields_match_serde() {
    let v = violations(&committed_types());
    assert!(v.is_empty(), "{v:#?}");
}

/// Replace the first line matching `pick` (after `after`) with `f(line)`.
fn mutate(
    text: &str,
    after: &str,
    pick: impl Fn(&str) -> bool,
    f: impl Fn(&str) -> String,
) -> String {
    let start = text.find(after).expect("anchor");
    let (head, tail) = text.split_at(start);
    let mut done = false;
    let tail: Vec<String> = tail
        .lines()
        .map(|l| {
            if !done && pick(l) {
                done = true;
                f(l)
            } else {
                l.to_string()
            }
        })
        .collect();
    assert!(done, "mutation found no line");
    format!("{head}{}\n", tail.join("\n"))
}

fn count(v: &[String], tag: &str) -> usize {
    v.iter().filter(|s| s.starts_with(tag)).count()
}

#[test]
fn each_semantic_check_catches_its_swift_mutation() {
    let text = committed_types();
    // 1. A CodingKey renamed.
    let m1 = mutate(
        &text,
        "public struct TaggedToolCall:",
        |l| l.contains("case toolCallId = \"toolCallId\""),
        |l| l.replace("\"toolCallId\"", "\"toolCallID\""),
    );
    // 2. A required field made optional.
    let m2 = mutate(
        &text,
        "public struct TaggedToolCall:",
        |l| l.trim() == "public var toolName: String",
        |l| format!("{l}?"),
    );
    // 3. An `encodeIfPresent` (omittedWhen Option::is_none) → `encode`.
    let m3 = mutate(
        &text,
        "public struct TaggedToolCall:",
        |l| l.contains("encodeIfPresent(providerExecuted"),
        |l| l.replace("encodeIfPresent", "encode"),
    );
    // 4. The Literal made an optional var.
    let m4 = mutate(
        &text,
        "public struct TaggedToolCall:",
        |l| l.trim() == "public let typeTag = LitToolCall()",
        |_| "    public var typeTag: LitToolCall?".to_string(),
    );
    let mut report = Vec::new();
    for (name, m, tag) in [
        ("codingkey", m1, "[1]"),
        ("optional", m2, "[2]"),
        ("encodeIfPresent", m3, "[3]"),
        ("literal", m4, "[4]"),
    ] {
        let v = violations(&m);
        let n = count(&v, tag);
        assert!(n >= 1, "{name}: check {tag} stayed green: {v:#?}");
        report.push(format!("{name}: {n} {tag} red, {} total", v.len()));
    }
    eprintln!("{}", report.join("\n"));
}
