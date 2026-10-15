//! Ported from fork PR #55 (a3fee776) tests/emit_swift_test.rs: the Swift
//! emitter's mapping rows, its refusals, and IR facts the Swift mapping
//! relies on. Rust-side only: the emitted text is asserted, never compiled
//! (Swift compile is not run here).

use std::collections::BTreeMap;
use std::path::PathBuf;

use aimux_codegen::emit::{self, Emitter, swift::Swift};
use aimux_codegen::manifest::Inputs;
use aimux_codegen::wire::{
    self, Presence, UnionRepr, VariantBody, WireBody, WireField, WireModel, WireTy, WireType,
    WireVariant,
};

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn model() -> WireModel {
    wire::from_inputs(&Inputs::from_workspace(&workspace())).unwrap()
}

fn file(m: &WireModel, name: &str) -> String {
    Swift
        .emit(m)
        .into_iter()
        .find(|o| o.path.ends_with(name))
        .unwrap_or_else(|| panic!("{name} not emitted"))
        .content
}

/// The declaration block `public <kind> <name>...` up to its closing `}`.
fn block(src: &str, name: &str) -> String {
    let start = [
        format!("\npublic struct {name}:"),
        format!("\npublic enum {name}:"),
        format!("\npublic indirect enum {name}:"),
        format!("\npublic typealias {name} ="),
    ]
    .iter()
    .find_map(|p| src.find(p.as_str()))
    .unwrap_or_else(|| panic!("{name} not emitted"));
    let rest = &src[start + 1..];
    let end = if rest.starts_with("public typealias") {
        rest.find('\n').unwrap() + 1
    } else {
        rest.find("\n}\n").map_or(rest.len(), |e| e + 3)
    };
    rest[..end].to_string()
}

fn ty(name: &str, body: WireBody) -> WireType {
    WireType {
        name: name.into(),
        doc_source: "synthetic.rs".into(),
        body,
    }
}

fn field(wire: &str, t: WireTy, presence: Presence) -> WireField {
    WireField {
        wire: wire.into(),
        rust: wire.into(),
        ty: t,
        presence,
        aliases: vec![],
        skip_fn: None,
        default_fn: None,
    }
}

fn variant(rust: &str, wire: Option<&str>, body: VariantBody) -> WireVariant {
    WireVariant {
        rust: rust.into(),
        wire: wire.map(Into::into),
        aliases: vec![],
        other: false,
        body,
    }
}

fn prim(p: &str) -> WireTy {
    WireTy::Prim(p.into())
}

// ---- IR facts -------------------------------------------------------------

/// String enums counted from the IR directly (replaces the python
/// one-liner over the manifest in the mapping doc).
fn string_enums(m: &WireModel) -> Vec<String> {
    m.types
        .iter()
        .filter(|t| matches!(t.body, WireBody::StringEnum { .. }))
        .map(|t| t.name.clone())
        .collect()
}

#[test]
fn ir_has_17_string_enums() {
    // v4 (#55 had 19): DecisionQuestionType and the others below.
    let names = string_enums(&model());
    assert_eq!(names.len(), 17, "{names:?}");
    // The Go output's two extra `type X string` are aliases, not enums.
    let m = model();
    for a in ["Size", "AspectRatio"] {
        let t = m.types.iter().find(|t| t.name == a).unwrap();
        assert_eq!(t.body, WireBody::Alias(prim("String")), "{a}");
    }
}

/// Untagged variants that would accept `null`, after alias expansion.
fn null_accepting(m: &WireModel) -> Vec<String> {
    let bodies: BTreeMap<&str, &WireBody> =
        m.types.iter().map(|t| (t.name.as_str(), &t.body)).collect();
    let expand = |t: &WireTy| -> WireTy {
        let mut t = t.clone();
        for _ in 0..32 {
            match &t {
                WireTy::Named(n) => match bodies.get(n.as_str()) {
                    Some(WireBody::Alias(a)) => t = a.clone(),
                    _ => break,
                },
                _ => break,
            }
        }
        t
    };
    let mut out = Vec::new();
    for t in &m.types {
        let WireBody::Union {
            repr: UnionRepr::Untagged,
            variants,
        } = &t.body
        else {
            continue;
        };
        for v in variants {
            let why = match &v.body {
                VariantBody::Unit => Some("Unit"),
                VariantBody::Value(x) => match (x, expand(x)) {
                    (WireTy::Json, _) => Some("Json"),
                    (_, WireTy::Json) => Some("Json after alias expansion"),
                    (_, WireTy::Nullable(_)) => Some("Nullable"),
                    _ => None,
                },
                _ => None,
            };
            if let Some(why) = why {
                out.push(format!(
                    "untagged {}::{} accepts null ({why})",
                    t.name, v.rust
                ));
            }
        }
    }
    out
}

#[test]
fn untagged_variants_reject_null_premise() {
    let m = model();
    // The aliases the scan expands are really `Alias(Json)` in the IR: on
    // v4 ToolChoice and FileBytes are hand-written in the manifest, lowered
    // to Json (#55's DecisionState / TextContentBinding are gone).
    for a in ["ToolChoice", "FileBytes"] {
        let t = m
            .types
            .iter()
            .find(|t| t.name == a)
            .unwrap_or_else(|| panic!("{a}"));
        assert_eq!(t.body, WireBody::Alias(WireTy::Json), "{a}");
    }
    let untagged = m
        .types
        .iter()
        .filter(|t| {
            matches!(
                &t.body,
                WireBody::Union {
                    repr: UnionRepr::Untagged,
                    ..
                }
            )
        })
        .count();
    assert_eq!(untagged, 7);
    assert_eq!(null_accepting(&m), Vec::<String>::new());
}

#[test]
fn untagged_premise_catches_synthetic_null_accepting_variants() {
    let mut m = model();
    m.types.push(ty(
        "SyntheticUntagged",
        WireBody::Union {
            repr: UnionRepr::Untagged,
            variants: vec![
                variant(
                    "Bytes",
                    None,
                    VariantBody::Value(WireTy::Named("FileBytes".into())),
                ),
                variant(
                    "Maybe",
                    None,
                    VariantBody::Value(WireTy::Nullable(Box::new(prim("String")))),
                ),
                variant(
                    "ToolChoice",
                    None,
                    VariantBody::Value(WireTy::Named("ToolChoice".into())),
                ),
            ],
        },
    ));
    assert_eq!(
        null_accepting(&m),
        vec![
            "untagged SyntheticUntagged::Bytes accepts null (Json after alias expansion)",
            "untagged SyntheticUntagged::Maybe accepts null (Nullable)",
            "untagged SyntheticUntagged::ToolChoice accepts null (Json after alias expansion)",
        ]
    );
}

// ---- refusals -------------------------------------------------------------

fn u128_model() -> WireModel {
    WireModel {
        types: vec![ty(
            "Big",
            WireBody::Record {
                fields: vec![field("n", prim("u128"), Presence::default())],
            },
        )],
    }
}

/// `Swift::check` messages of the model itself: a synthetic model names no
/// registry function, so every emit::registry entry is reported unused.
fn own(e: Vec<String>) -> Vec<String> {
    let unused = e
        .iter()
        .filter(|x| {
            x.starts_with("emit::registry ")
                && x.ends_with("not found in the manifest (no field names it)")
        })
        .count();
    assert_eq!(
        unused,
        aimux_codegen::emit::registry::REGISTRY.len(),
        "{e:?}"
    );
    e.into_iter()
        .filter(|x| !x.starts_with("emit::registry "))
        .collect()
}

#[test]
fn u128_is_an_emitter_error() {
    let m = u128_model();
    match Swift::check(&m) {
        Ok(()) => panic!("expected an emitter error for u128, got Ok"),
        Err(e) => assert_eq!(
            own(e),
            vec!["Big.n: u128/i128 has no lossless Swift type; the Swift emitter refuses it"]
        ),
    }
    // emit::write refuses before writing anything.
    let dir = std::env::temp_dir().join(format!("aimux-swift-u128-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let err = emit::write(&Swift, &m, &dir).unwrap_err();
    assert!(err.contains("swift emitter: Big.n: u128/i128"), "{err}");
    assert!(!dir.join("swift").exists(), "files were written");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn real_model_passes_the_swift_checks() {
    assert_eq!(Swift::check(&model()), Ok(()));
}

#[test]
fn variant_level_untagged_in_a_tagged_union_is_an_error() {
    let m = WireModel {
        types: vec![ty(
            "Mixed",
            WireBody::Union {
                repr: UnionRepr::Internal { tag: "type".into() },
                variants: vec![variant("Loose", None, VariantBody::Value(prim("String")))],
            },
        )],
    };
    let e = own(Swift::check(&m).unwrap_err());
    assert!(
        e[0].starts_with("Mixed::Loose: variant-level untagged"),
        "{e:?}"
    );
}

#[test]
fn record_cycle_by_value_is_an_error() {
    let opt = Presence {
        omittable: true,
        nullable: true,
        ..Presence::default()
    };
    let m = WireModel {
        types: vec![
            ty(
                "A",
                WireBody::Record {
                    fields: vec![field("b", WireTy::Named("B".into()), opt)],
                },
            ),
            ty(
                "B",
                WireBody::Record {
                    fields: vec![field("a", WireTy::Named("A".into()), Presence::default())],
                },
            ),
        ],
    };
    let e = Swift::check(&m).unwrap_err();
    assert!(
        e.iter()
            .any(|x| x.starts_with("record cycle by value: A -> B")),
        "{e:?}"
    );
}

// ---- mapping rows on the real model ----------------------------------------

#[test]
fn output_files_and_location() {
    let paths: Vec<String> = Swift
        .emit(&model())
        .iter()
        .map(|o| o.path.display().to_string())
        .collect();
    assert_eq!(
        paths,
        [
            "Package.swift",
            "Sources/AimuxWire/Runtime.swift",
            "Sources/AimuxWire/Types.swift"
        ]
    );
    let dir = std::env::temp_dir().join(format!("aimux-swift-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let wrote = emit::write(&Swift, &model(), &dir).unwrap();
    assert_eq!(wrote.len(), 3);
    assert!(wrote.iter().all(|p| p.starts_with(dir.join("swift"))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn records_follow_the_presence_matrix() {
    let src = file(&model(), "Types.swift");
    let ir = block(&src, "ImageResult");
    // required Vec: decode / encode (serde writes []).
    assert!(
        ir.contains("warnings = try c.decode([Warning].self, forKey: .warnings)"),
        "{ir}"
    );
    assert!(
        ir.contains("try c.encode(warnings, forKey: .warnings)"),
        "{ir}"
    );
    // Option with skip: decodeIfPresent / encodeIfPresent.
    assert!(
        ir.contains("usage = try c.decodeIfPresent(ImageUsage.self, forKey: .usage)"),
        "{ir}"
    );
    assert!(
        ir.contains("try c.encodeIfPresent(usage, forKey: .usage)"),
        "{ir}"
    );
    // Option without skip: serde writes null → encodeNullable (encodeNil).
    let ch = block(&src, "ChatCompletionChunkChoice");
    assert!(
        ch.contains("try c.encodeNullable(finishReason, forKey: .finishReason)"),
        "{ch}"
    );
    // #[serde(default)] non-Option: contains ? decode : default.
    // (v4 has no DecisionCapabilities / BooleanCriteria: ApiCallError.)
    let ae = block(&src, "ApiCallError");
    assert!(
        ae.contains("isRetryable = try c.decodeDefault(Bool.self, forKey: .isRetryable, false)"),
        "{ae}"
    );
    // Tri-state only where the IR says absent / null / value are distinct.
    assert!(ae.contains("public var data: TriState<JSONValue>"), "{ae}");
    assert!(
        ae.contains("try c.encodeTriState(data, forKey: .data)"),
        "{ae}"
    );
    let tri = model()
        .types
        .iter()
        .flat_map(|t| match &t.body {
            WireBody::Record { fields } => fields.clone(),
            WireBody::Union { variants, .. } => variants
                .iter()
                .flat_map(|v| match &v.body {
                    VariantBody::Fields(f) => f.clone(),
                    _ => vec![],
                })
                .collect(),
            _ => vec![],
        })
        .filter(|f| f.presence.tri_state)
        .count();
    assert_eq!(
        src.lines()
            .filter(|l| l.trim_start().starts_with("public var ") && l.contains(": TriState<"))
            .count(),
        tri,
        "TriState used outside tri-state fields"
    );
    // Runtime: default rejects a present null (no decodeIfPresent).
    let rt = file(&model(), "Runtime.swift");
    assert!(
        rt.contains("contains(k) ? try decode(t, forKey: k) : def()"),
        "{rt}"
    );
    assert!(rt.contains("if let v { try encode(v, forKey: k) } else { try encodeNil(forKey: k) }"));
}

#[test]
fn string_enums_open_and_closed() {
    let src = file(&model(), "Types.swift");
    // Open (#[serde(other)]): keeps the received string.
    for open in [
        "Modality",
        "ModelType",
        "ReasoningMode",
        "ReasoningVisibility",
    ] {
        let b = block(&src, open);
        assert!(
            b.starts_with(&format!("public struct {open}: RawRepresentable")),
            "{b}"
        );
        assert!(
            b.contains("let s = try d.singleValueContainer().decode(String.self)"),
            "{b}"
        );
    }
    // Variant aliases (ModelType, model_catalogue.rs:68-69) map to the main value.
    let mt = block(&src, "ModelType");
    assert!(
        mt.contains("\"image-generation\": \"ImageGen\", \"imageGeneration\": \"ImageGen\""),
        "{mt}"
    );
    assert!(mt.contains("rawValue = Self.aliases[s] ?? s"), "{mt}");
    // Closed: a String raw enum (synthesized decode rejects unknown values).
    let closed = src
        .matches(": String, Codable, Equatable, Sendable, CaseIterable {")
        .count();
    assert_eq!(closed, 13, "17 string enums - 4 open");
}

#[test]
fn unions_by_representation() {
    let src = file(&model(), "Types.swift");
    // Untagged tries in declaration order (v4: FileBytes is a JSONValue
    // alias, FilePartData is untagged Tagged | String | Reference).
    let fp = block(&src, "FilePartData");
    let tagged = fp.find("c.decode(FileData.self)").unwrap();
    let string = fp.find("c.decode(String.self)").unwrap();
    let reference = fp.find("c.decode(SharedProviderReference.self)").unwrap();
    assert!(
        tagged < string && string < reference,
        "untagged tries out of declaration order:\n{fp}"
    );
    assert!(!fp.contains("decodeNil"), "{fp}");
    let internal = src
        .matches("let tag = try c.decode(String.self, forKey: AnyCodingKey(")
        .count();
    let tagged_ir = model()
        .types
        .iter()
        .filter(|t| {
            matches!(
                &t.body,
                WireBody::Union {
                    repr: UnionRepr::Internal { .. } | UnionRepr::Adjacent { .. },
                    ..
                }
            )
        })
        .count();
    assert_eq!(internal, tagged_ir, "internal + adjacent unions");
    assert_eq!(
        src.matches(": no variant matches\")").count(),
        7,
        "untagged unions"
    );
    // Proxies by wire shape.
    assert!(src.contains("\npublic typealias Size = String\n"));
    assert!(src.contains("\npublic typealias AspectRatio = String\n"));
    assert!(
        block(&src, "AiMuxError")
            .contains("let tag = try c.decode(String.self, forKey: AnyCodingKey(\"name\"))")
    );
    // Bytes: WireBytes, never Data.
    assert!(file(&model(), "Runtime.swift").contains("public typealias WireBytes = [UInt8]"));
    for bad in ["(Data.self", ": Data\n", ": Data?\n", "[Data]", "<Data>"] {
        assert!(!src.contains(bad), "Foundation Data used: {bad}");
    }
}

#[test]
fn json_value_tries_bool_int64_uint64_double() {
    let rt = file(&model(), "Runtime.swift");
    let at = |s: &str| rt.find(s).unwrap_or_else(|| panic!("{s}"));
    let order = [
        at("c.decodeNil()"),
        at("decode(String.self) { self = .string"),
        at("decode([JSONValue].self)"),
        at("decode([String: JSONValue].self)"),
        at("decode(Bool.self)"),
        at("decode(Int64.self)"),
        at("decode(UInt64.self)"),
        at("decode(Double.self)"),
    ];
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
}

// ---- synthetic unions (no aimux-core type uses them) ------------------------

fn payload_union(repr: UnionRepr) -> WireModel {
    WireModel {
        types: vec![ty(
            "Shape",
            WireBody::Union {
                repr,
                variants: vec![
                    variant("Empty", Some("empty"), VariantBody::Unit),
                    variant("Circle", Some("circle"), VariantBody::Value(prim("f64"))),
                    variant(
                        "Rect",
                        Some("rect"),
                        VariantBody::Fields(vec![field("w", prim("u32"), Presence::default())]),
                    ),
                ],
            },
        )],
    }
}

#[test]
fn external_payload_union_synthetic() {
    let src = file(&payload_union(UnionRepr::External), "Types.swift");
    let b = block(&src, "Shape");
    // unit ↔ "empty"; payload ↔ {"circle": 1.5}; struct ↔ {"rect": {...}}.
    assert!(b.contains("case \"empty\": self = .empty; return"), "{b}");
    assert!(
        b.contains("guard c.allKeys.count == 1, let k = c.allKeys.first"),
        "{b}"
    );
    assert!(
        b.contains("case \"circle\": self = .circle(try c.decode(Double.self, forKey: k))"),
        "{b}"
    );
    assert!(
        b.contains("case \"rect\": self = .rect(try c.decode(ShapeRect.self, forKey: k))"),
        "{b}"
    );
    assert!(
        b.contains("case .empty: var s = e.singleValueContainer(); try s.encode(\"empty\")"),
        "{b}"
    );
    assert!(b.contains("case .circle(let v): var o = e.container(keyedBy: AnyCodingKey.self); try o.encode(v, forKey: AnyCodingKey(\"circle\"))"), "{b}");
    assert!(src.contains("public struct ShapeRect:"), "payload struct");
}

#[test]
fn adjacent_union_synthetic() {
    let src = file(
        &payload_union(UnionRepr::Adjacent {
            tag: "t".into(),
            content: "c".into(),
        }),
        "Types.swift",
    );
    let b = block(&src, "Shape");
    assert!(
        b.contains("let tag = try c.decode(String.self, forKey: AnyCodingKey(\"t\"))"),
        "{b}"
    );
    assert!(b.contains("case \"empty\": self = .empty\n"), "{b}");
    assert!(b.contains("case \"circle\": self = .circle(try c.decode(Double.self, forKey: AnyCodingKey(\"c\")))"), "{b}");
    assert!(
        b.contains("case .empty: try c.encode(\"empty\", forKey: tk)"),
        "{b}"
    );
    assert!(
        b.contains(
            "case .rect(let v): try c.encode(\"rect\", forKey: tk); try c.encode(v, forKey: ck)"
        ),
        "{b}"
    );
    assert!(
        b.contains("default: throw DecodingError"),
        "unknown tag must error: {b}"
    );
}

#[test]
fn other_variant_catches_unknown_tags() {
    let mut m = payload_union(UnionRepr::Internal { tag: "type".into() });
    if let WireBody::Union { variants, .. } = &mut m.types[0].body {
        variants.push(WireVariant {
            other: true,
            ..variant("Unknown", Some("unknown"), VariantBody::Unit)
        });
    }
    let b = block(&file(&m, "Types.swift"), "Shape");
    assert!(b.contains("default: self = .unknown"), "{b}");
}

// ---- registered predicates and default functions (this port) --------------

fn one_field(f: WireField, enum_cases: &[&str]) -> WireModel {
    let mut types = vec![ty("Rec", WireBody::Record { fields: vec![f] })];
    if !enum_cases.is_empty() {
        types.push(ty(
            "Op",
            WireBody::StringEnum {
                values: enum_cases.iter().map(|c| (*c).to_string()).collect(),
                open: false,
                aliases: vec![],
            },
        ));
    }
    WireModel { types }
}

#[test]
fn unregistered_default_fn_is_an_error() {
    let mut f = field(
        "n",
        prim("u32"),
        Presence {
            omittable: true,
            ..Presence::default()
        },
    );
    f.default_fn = Some("crate::x::default_mystery".into());
    let e = Swift::check(&one_field(f.clone(), &[])).unwrap_err();
    assert!(
        e.iter().any(|x| x.contains("crate::x::default_mystery")),
        "{e:?}"
    );
    // A registered one emits the decoded default (unused entries still error).
    f.default_fn = Some("crate::video_model::default_video_n".into());
    let m = one_field(f, &[]);
    let e = Swift::check(&m).unwrap_err();
    assert!(
        e.iter().all(|x| x.contains("not found in the manifest")),
        "{e:?}"
    );
    let b = block(&file(&m, "Types.swift"), "Rec");
    assert!(
        b.contains("n = try c.decodeDefault(UInt32.self, forKey: .n, 1)"),
        "{b}"
    );
}

#[test]
fn unregistered_custom_skip_is_an_error() {
    let omit = Presence {
        omittable: true,
        skip_if_empty: true,
        ..Presence::default()
    };
    // skip_serializing_if on a named enum with no registered skip value.
    let f = field("op", WireTy::Named("Op".into()), omit);
    let e = Swift::check(&one_field(f, &["a", "b"])).unwrap_err();
    assert!(
        e.iter()
            .any(|x| x.contains("unregistered custom predicate")),
        "{e:?}"
    );
    // A skip path that is not in the registry.
    let mut f = field(
        "op",
        WireTy::Named("Op".into()),
        Presence {
            omittable: true,
            ..Presence::default()
        },
    );
    f.skip_fn = Some("crate::x::bogus_skip".into());
    let e = Swift::check(&one_field(f.clone(), &["a", "b"])).unwrap_err();
    assert!(
        e.iter().any(|x| x.contains("crate::x::bogus_skip")),
        "{e:?}"
    );
    // A real registry path whose value is a case of the enum: skipped exactly when equal.
    f.skip_fn = Some("crate::recording::RecordingOperation::is_language_model".into());
    let m = one_field(f, &["language_model", "decision"]);
    // All other registry entries are unused on this synthetic model.
    let e = Swift::check(&m).unwrap_err();
    assert!(
        e.iter().all(|x| x.contains("not found in the manifest")),
        "{e:?}"
    );
    let b = block(&file(&m, "Types.swift"), "Rec");
    assert!(
        b.contains("if op != .languageModel { try c.encode(op, forKey: .op) }"),
        "{b}"
    );
}
