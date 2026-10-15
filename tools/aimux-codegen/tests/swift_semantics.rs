//! U2: the generated Swift (`generated/swift`), checked without a Swift
//! toolchain. Ground truth is only the manifest (`extract`, the serde
//! attributes) and serde's real behaviour (aimux-core values serialized and
//! deserialized by serde); the generator's wire IR is never consulted.
//!
//! The Swift text is parsed (structs, string enums, unions, aliases, `Lit*`)
//! and checked two ways:
//! - Against the manifest, per Swift declaration (every record, struct
//!   variant payload, `Tagged*` and flattened `Typed*` struct maps back to
//!   manifest fields; an unmapped one is an error):
//!   - `[1]` CodingKeys == serde keys;
//!   - `[2]` optionality from `null.input` (`asAbsent` → `T?`, `distinct` →
//!     `TriState<T>`, otherwise neither);
//!   - `[3]` decode and encode from `omittedWhen` / `null` / `default`:
//!     required → `decode`, `asAbsent` → `decodeIfPresent`, `distinct` →
//!     `decodeTriState`, `default` → `decodeDefault` / `decodeDefaulted`;
//!     `Option::is_none` → `encodeIfPresent` / `encodeTriState`, never
//!     omitted → `encode` (`encodeNullable` for `null.output = whenNone`),
//!     `is_empty` → written only when not empty, a custom predicate → a
//!     value comparison;
//!   - `[4]` a fixed `type` (`Tagged*`, `#[ts(type = "\"..\"")]`) is a
//!     `public let x = LitY()` constant, always written, and decoding checks
//!     the value;
//!   - `[T]` the Swift property type and decode type equal the type mapped
//!     from the manifest Rust type;
//!   - `[E]` string-enum raw values == manifest variant wire values;
//!   - `[U]` union tag key and tag values == manifest representation and
//!     variant wire names (with aliases).
//! - Against serde, by simulating the parsed Swift decode → encode on JSON
//!   (`[S]`): every serde fixture value (all variants), and per key of each
//!   object the value removed, `null`, and of the wrong JSON kind. serde
//!   accepting must equal Swift accepting, and the re-encoded JSON must be
//!   equal (values, `null`, absence).
//!
//! Every check has a mutation of the Swift text that must turn it red.

#![allow(clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use aimux_codegen::emit::go::pascal;
use aimux_codegen::extract::{self, CodecWire, FieldDef, Representation, Shape, TypeDef, TypeRef};
use aimux_codegen::manifest::Inputs;
use serde_json::{Map, Value, json};

#[path = "common/float_paths.rs"]
mod float_paths;
#[path = "common/go_fixtures.rs"]
mod go_fixtures;
#[path = "common/go_fixtures_empty.rs"]
mod go_fixtures_empty;
#[path = "common/go_fixtures_tri.rs"]
mod go_fixtures_tri;
#[path = "common/registry_fixtures.rs"]
mod registry_fixtures;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// See tests/wire_diff_go.rs; the fixture files name these.
pub type Back = fn(&Value) -> Result<Value, String>;

/// serde decode as `T`, re-serialized.
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

// ---- Swift parsing -----------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct SField {
    id: String,
    key: String,
    /// Property type, or for a constant `let x = LitY()` the `LitY`.
    ty: String,
    constant: bool,
    dec: String,
    enc: String,
}

#[derive(Debug, Clone)]
enum Decl {
    Struct(Vec<SField>),
    Closed {
        raws: BTreeMap<String, String>,
        aliases: BTreeMap<String, String>,
    },
    Open {
        cases: BTreeMap<String, String>,
        aliases: BTreeMap<String, String>,
    },
    Union(SUnion),
    Alias(String),
    Lit(String),
}

#[derive(Debug, Clone, Default)]
struct SUnion {
    /// `internal`, `adjacent`, `external` or `untagged`.
    kind: &'static str,
    tag: Option<String>,
    content: Option<String>,
    /// (tag values, case, payload type) in declaration order.
    arms: Vec<(Vec<String>, String, Option<String>)>,
    /// case → wire tag written on encode.
    enc_tags: BTreeMap<String, String>,
    other: Option<String>,
}

type Swift = BTreeMap<String, Decl>;

fn quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find('"') {
        let r = &rest[i..];
        let mut end = 1;
        let b = r.as_bytes();
        while end < b.len() && b[end] != b'"' {
            if b[end] == b'\\' {
                end += 1;
            }
            end += 1;
        }
        if end >= b.len() {
            break;
        }
        out.push(serde_json::from_str(&r[..=end]).unwrap());
        rest = &r[end + 1..];
    }
    out
}

fn block_lines<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out = Vec::new();
    for l in lines.by_ref() {
        if l == "}" {
            break;
        }
        out.push(l);
    }
    out
}

fn parse_struct(name: &str, body: &[&str]) -> Decl {
    if let Some(l) = body.iter().find(|l| l.contains("guard s == ")) {
        return Decl::Lit(quoted(l)[0].clone());
    }
    if body
        .iter()
        .any(|l| l.contains("public var rawValue: String"))
    {
        let mut cases = BTreeMap::new();
        let mut aliases = BTreeMap::new();
        for l in body {
            let t = l.trim();
            if let Some(r) = t.strip_prefix("public static let ")
                && let Some((c, _)) = r.split_once(" = ")
                && r.contains("(rawValue: ")
            {
                cases.insert(c.trim_matches('`').to_string(), quoted(r)[0].clone());
            }
            if t.starts_with("static let aliases") {
                let q = quoted(t);
                for p in q.chunks(2) {
                    aliases.insert(p[0].clone(), p[1].clone());
                }
            }
        }
        let _ = name;
        return Decl::Open { cases, aliases };
    }
    let mut props: BTreeMap<String, (String, bool)> = BTreeMap::new();
    let mut keys: Vec<(String, String)> = Vec::new();
    let mut dec: BTreeMap<String, String> = BTreeMap::new();
    let mut enc: BTreeMap<String, String> = BTreeMap::new();
    let mut section = "";
    for l in body {
        let t = l.trim();
        if t.starts_with("enum CodingKeys") {
            section = "keys";
            continue;
        } else if t.starts_with("public init(from") {
            section = "dec";
            continue;
        } else if t.starts_with("public func encode(to") {
            section = "enc";
            continue;
        } else if t.starts_with("public init(") {
            section = "";
            continue;
        }
        if section.is_empty() {
            if let Some(r) = t.strip_prefix("public var ") {
                let (id, ty) = r.split_once(": ").unwrap();
                props.insert(id.into(), (ty.into(), false));
            } else if let Some(r) = t.strip_prefix("public let ") {
                let (id, ty) = r.split_once(" = ").unwrap();
                props.insert(id.into(), (ty.trim_end_matches("()").into(), true));
            }
            continue;
        }
        if section == "keys" {
            if let Some(r) = t.strip_prefix("case ")
                && let Some((id, k)) = r.split_once(" = ")
            {
                keys.push((id.into(), serde_json::from_str(k).unwrap()));
            }
            continue;
        }
        for id in props.keys() {
            let bare = id.trim_matches('`');
            let hit = if section == "dec" {
                t.starts_with(&format!("{id} = "))
                    || t.contains(&format!("contains(.{id})"))
                    || t.contains(&format!("contains(.{bare})"))
            } else {
                t.contains(&format!("({id}, forKey:"))
                    || t.starts_with(&format!("if {id} "))
                    || t.starts_with(&format!("if !{id}."))
            };
            if hit {
                let m = if section == "dec" { &mut dec } else { &mut enc };
                m.entry(id.clone()).or_default().push_str(t);
            }
        }
    }
    let mut fields = Vec::new();
    for (id, key) in keys {
        let (ty, constant) = props.get(&id).cloned().unwrap_or_default();
        fields.push(SField {
            dec: dec.get(&id).cloned().unwrap_or_default(),
            enc: enc.get(&id).cloned().unwrap_or_default(),
            ty,
            constant,
            key,
            id,
        });
    }
    Decl::Struct(fields)
}

fn parse_enum(body: &[&str]) -> Decl {
    let mut raws = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    for l in body {
        let t = l.trim();
        if let Some(r) = t.strip_prefix("case ")
            && let Some((c, v)) = r.split_once(" = ")
        {
            raws.insert(c.trim_matches('`').to_string(), quoted(v)[0].clone());
        }
        if t.starts_with("static let aliases") {
            let q = quoted(t);
            for p in q.chunks(2) {
                aliases.insert(p[0].clone(), p[1].clone());
            }
        }
    }
    Decl::Closed { raws, aliases }
}

fn case_of(s: &str) -> String {
    // `.name` or `.name(` → name
    let r = s.split_once('.').map_or("", |x| x.1);
    r.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '`')
        .collect::<String>()
        .trim_matches('`')
        .to_string()
}

fn payload_of(s: &str) -> Option<String> {
    // `try P(from: d)` | `try c.decode(P.self, ...)` | `try? c.decode(P.self)`
    if let Some(i) = s.find("decode(") {
        let r = &s[i + 7..];
        return r.find(".self").map(|e| r[..e].to_string());
    }
    let i = s.find("(try ")?;
    let r = &s[i + 5..];
    r.find("(from: d)").map(|e| r[..e].to_string())
}

fn parse_union(body: &[&str]) -> Decl {
    let mut u = SUnion::default();
    let mut section = "";
    let mut cur = String::new();
    for l in body {
        let t = l.trim();
        if t.starts_with("public init(from") {
            section = "dec";
            continue;
        }
        if t.starts_with("public func encode(to") {
            section = "enc";
            continue;
        }
        if section == "dec" {
            if t.starts_with("let tag = try c.decode(String.self, forKey: AnyCodingKey(") {
                u.tag = quoted(t).first().cloned();
                if u.kind.is_empty() {
                    u.kind = "internal";
                }
            } else if t.starts_with("if let s = try? d.singleValueContainer().decode(String.self)")
            {
                u.kind = "external";
            } else if t.starts_with("let c = try d.singleValueContainer()") && u.kind.is_empty() {
                u.kind = "untagged";
            } else if let Some(r) = t.strip_prefix("case ")
                && let Some((tags, rest)) = r.split_once(": self = ")
            {
                let tags = quoted(tags);
                if let Some(c) = rest.find("AnyCodingKey(") {
                    let q = quoted(&rest[c..]);
                    if let Some(k) = q.first() {
                        u.content = Some(k.clone());
                        u.kind = "adjacent";
                    }
                }
                u.arms.push((tags, case_of(rest), payload_of(rest)));
            } else if let Some(r) = t.strip_prefix("default: self = ") {
                u.other = Some(case_of(r));
            } else if t.starts_with("if c.decodeNil() { self = ") {
                u.arms.push((vec![], case_of(&t[25..]), None));
            } else if t.starts_with("if let v = try? c.decode(") {
                let p = payload_of(t);
                let c = case_of(t.split_once("self = ").unwrap().1);
                u.arms.push((vec![], c, p));
            }
        } else if section == "enc" {
            // The first string literal after a `case .x` is the tag it writes.
            if t.starts_with("case .") {
                cur = case_of(t);
            }
            if !cur.is_empty()
                && !u.enc_tags.contains_key(&cur)
                && let Some(w) = quoted(t).first()
            {
                u.enc_tags.insert(cur.clone(), w.clone());
            }
        }
    }
    if u.kind.is_empty() {
        u.kind = "untagged";
    }
    Decl::Union(u)
}

fn parse(text: &str) -> Swift {
    let mut out = Swift::new();
    let mut lines = text.lines();
    while let Some(l) = lines.next() {
        if let Some(r) = l.strip_prefix("public typealias ") {
            let (n, t) = r.split_once(" = ").unwrap();
            out.insert(n.into(), Decl::Alias(t.into()));
        } else if let Some(r) = l.strip_prefix("public struct ") {
            let name = r.split(':').next().unwrap().trim().to_string();
            let body = block_lines(&mut lines);
            out.insert(name.clone(), parse_struct(&name, &body));
        } else if let Some(r) = l.strip_prefix("public indirect enum ") {
            let name = r.split(':').next().unwrap().trim().to_string();
            let body = block_lines(&mut lines);
            out.insert(name, parse_union(&body));
        } else if let Some(r) = l.strip_prefix("public enum ") {
            let name = r.split(':').next().unwrap().trim().to_string();
            let body = block_lines(&mut lines);
            out.insert(name, parse_enum(&body));
        }
    }
    out
}

// ---- manifest -------------------------------------------------------------------

struct Manifest {
    model: extract::Model,
    /// Swift declaration → source file (`/// X mirrors <source>.`).
    src: BTreeMap<String, String>,
    /// Swift payload struct → (variant rust name, Swift union).
    variant_of: BTreeMap<String, (String, String)>,
    /// (source, manifest name) → Swift declaration.
    swift_of_type: BTreeMap<(String, String), String>,
}

fn manifest(text: &str) -> Manifest {
    let inputs = Inputs::from_workspace(&workspace());
    let model = extract::extract(&inputs.src_root, aimux_codegen::manifest::SRC_PREFIX).unwrap();
    let mut src = BTreeMap::new();
    let mut variant_of = BTreeMap::new();
    for l in text.lines() {
        let Some(d) = l.strip_prefix("/// ") else {
            continue;
        };
        if let Some((n, s)) = d.split_once(" mirrors ") {
            src.insert(n.to_string(), s.trim_end_matches('.').to_string());
        } else if let Some((n, rest)) = d.split_once(" is the ")
            && let Some((v, u)) = rest.split_once(" variant of ")
        {
            variant_of.insert(
                n.to_string(),
                (v.to_string(), u.trim_end_matches('.').to_string()),
            );
        }
    }
    let mut m = Manifest {
        model,
        src,
        variant_of,
        swift_of_type: BTreeMap::new(),
    };
    let names: Vec<String> = m.src.keys().cloned().collect();
    for n in names {
        if let Some((t, false)) = m.mtype(&n) {
            let key = (t.source.clone(), t.name.clone());
            let exact = n == t.name;
            let e = m.swift_of_type.entry(key).or_insert_with(|| n.clone());
            if exact {
                e.clone_from(&n);
            }
        }
    }
    m
}

/// One manifest field a Swift struct field must match: the field with its
/// owner's source and generic substitutions, or a fixed `type` tag.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)] // test-only, a few thousand values
enum MField {
    Field(FieldDef, String, BTreeMap<String, String>),
    Tag(String),
}

impl MField {
    fn wire(&self) -> String {
        match self {
            MField::Field(f, ..) => f.wire.clone(),
            MField::Tag(_) => "type".into(),
        }
    }
}

impl Manifest {
    fn types(&self) -> impl Iterator<Item = &TypeDef> {
        self.model.types.iter().chain(&self.model.auxiliary_types)
    }

    /// The manifest type a Swift declaration mirrors, by its doc source:
    /// the same name, `Tagged<name>` (a `TypeTag` type), or a name the
    /// generator prefixed to resolve a clash.
    fn mtype(&self, swift: &str) -> Option<(&TypeDef, bool)> {
        let src = self.src.get(swift)?;
        let here: Vec<&TypeDef> = self.types().filter(|t| &t.source == src).collect();
        if let Some(t) = here.iter().find(|t| t.name == swift) {
            return Some((t, false));
        }
        if let Some(rest) = swift.strip_prefix("Tagged")
            && let Some(t) = here.iter().find(|t| t.name == rest && t.type_tag.is_some())
        {
            return Some((t, true));
        }
        here.iter()
            .filter(|t| swift.ends_with(t.name.as_str()))
            .max_by_key(|t| t.name.len())
            .map(|t| (*t, false))
    }

    /// The manifest type `name` means inside `from_source`: one defined
    /// there, else the one its `use crate::<module>::..` imports, else the
    /// only one with that name.
    fn by_ref(&self, from_source: &str, name: &str) -> Option<&TypeDef> {
        let all: Vec<&TypeDef> = self.types().filter(|t| t.name == name).collect();
        if let Some(t) = all.iter().find(|t| t.source == from_source) {
            return Some(t);
        }
        if all.len() > 1
            && let Some(module) = imported_from(from_source, name)
        {
            let want = format!("aimux-core/src/{module}.rs");
            if let Some(t) = all.iter().find(|t| t.source == want) {
                return Some(t);
            }
        }
        (all.len() == 1).then(|| all[0])
    }

    fn expand(
        &self,
        owner: &TypeDef,
        fields: &[FieldDef],
        out: &mut Vec<MField>,
        depth: usize,
    ) -> Result<(), String> {
        for f in fields {
            if f.flatten {
                let TypeRef::Ref { name, .. } = &f.ty else {
                    return Err(format!("{}.{}: flatten of a non-ref", owner.name, f.rust));
                };
                let t = self
                    .by_ref(&owner.source, name)
                    .ok_or(format!("flatten {name} unknown"))?;
                match &t.shape {
                    Shape::Struct { fields, .. } if depth < 8 => {
                        self.expand(t, fields, out, depth + 1)?
                    }
                    _ => {
                        return Err(format!(
                            "{}.{}: flatten of non-struct {name}",
                            owner.name, f.rust
                        ));
                    }
                }
            } else {
                out.push(MField::Field(
                    f.clone(),
                    owner.source.clone(),
                    owner.concrete.clone(),
                ));
            }
        }
        Ok(())
    }

    /// An enum a Swift union mirrors: the type itself, or the internally
    /// tagged enum a struct flattens (`TypedSource` over `Source`).
    fn enum_of<'a>(&'a self, t: &'a TypeDef) -> Option<(&'a TypeDef, Vec<FieldDef>)> {
        match &t.shape {
            Shape::Enum { .. } => Some((t, vec![])),
            Shape::Struct { fields, .. } => {
                let f = fields.iter().find(|f| f.flatten)?;
                let TypeRef::Ref { name, .. } = &f.ty else {
                    return None;
                };
                let e = self.by_ref(&t.source, name)?;
                matches!(e.shape, Shape::Enum { .. })
                    .then(|| (e, fields.iter().filter(|x| !x.flatten).cloned().collect()))
            }
            _ => None,
        }
    }

    /// Manifest fields of the Swift struct `name`; an unmapped struct is an
    /// error.
    fn fields_of(&self, name: &str) -> Result<Vec<MField>, String> {
        let mut out = Vec::new();
        if let Some((v, u)) = self.variant_of.get(name) {
            let (ut, tagged) = self
                .mtype(u)
                .ok_or(format!("{name}: union {u} maps to no manifest type"))?;
            let (e, own) = self
                .enum_of(ut)
                .ok_or(format!("{name}: {} is not an enum", ut.name))?;
            let Shape::Enum { variants, .. } = &e.shape else {
                unreachable!()
            };
            let var = variants
                .iter()
                .find(|x| x.rust == *v)
                .ok_or(format!("{name}: no variant {v} in {}", e.name))?;
            let Shape::Struct { fields, .. } = &var.shape else {
                return Err(format!("{name}: {}::{v} is not a struct variant", e.name));
            };
            if tagged {
                out.push(MField::Tag(ut.type_tag.clone().unwrap()));
            }
            self.expand(ut, &own, &mut out, 0)?;
            let mut inner = Vec::new();
            self.expand(e, fields, &mut inner, 0)?;
            out.extend(
                inner
                    .into_iter()
                    .filter(|f| !(tagged && f.wire() == "type")),
            );
            return Ok(out);
        }
        let (t, tagged) = self
            .mtype(name)
            .ok_or(format!("{name}: maps to no manifest type"))?;
        let Shape::Struct { fields, .. } = &t.shape else {
            return Err(format!("{name}: manifest {} is not a struct", t.name));
        };
        if tagged {
            out.push(MField::Tag(t.type_tag.clone().unwrap()));
        }
        let mut inner = Vec::new();
        self.expand(t, fields, &mut inner, 0)?;
        out.extend(
            inner
                .into_iter()
                .filter(|f| !(tagged && f.wire() == "type")),
        );
        Ok(out)
    }

    fn tag_of(&self, name: &str) -> Option<String> {
        self.types()
            .find(|t| t.name == name)
            .and_then(|t| t.type_tag.clone())
    }
}

/// The `crate::<module>` a source file imports `name` from.
fn imported_from(source: &str, name: &str) -> Option<String> {
    let text = std::fs::read_to_string(workspace().join(source)).ok()?;
    let ident = |s: &str| s.chars().all(|c| c.is_alphanumeric() || c == '_');
    let mut rest = text.as_str();
    while let Some(i) = rest.find("use crate::") {
        let r = &rest[i + 11..];
        let end = r.find(';')?;
        let stmt = &r[..end];
        let module: String = stmt
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let names = stmt.replace(['{', '}', '\n', ' '], ",");
        if names.split([',', ':']).any(|n| n == name && ident(n)) {
            return Some(module);
        }
        rest = &r[end..];
    }
    None
}

fn swift_prim(p: &str) -> &'static str {
    match p {
        "bool" => "Bool",
        "u8" => "UInt8",
        "u16" => "UInt16",
        "u32" => "UInt32",
        "u64" | "usize" => "UInt64",
        "i8" => "Int8",
        "i16" => "Int16",
        "i32" => "Int32",
        "i64" | "isize" => "Int64",
        "f32" => "Float",
        "f64" => "Double",
        _ => "String",
    }
}

/// Expand Swift typealiases, recursively, inside a Swift type.
fn canon(swift: &Swift, ty: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut String| {
        if !w.is_empty() {
            let mut t = w.clone();
            for _ in 0..16 {
                match swift.get(&t) {
                    Some(Decl::Alias(a)) => t = canon(swift, a),
                    _ => break,
                }
            }
            out.push_str(&t);
            w.clear();
        }
    };
    for c in ty.chars() {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// Swift type for a manifest Rust type (the wire type serde writes),
/// referenced from `source`.
#[allow(clippy::only_used_in_recursion)] // `swift` is passed down only; kept for a uniform signature
fn swift_of(
    m: &Manifest,
    swift: &Swift,
    t: &TypeRef,
    source: &str,
    conc: &BTreeMap<String, String>,
    f: Option<&FieldDef>,
) -> String {
    let named = |n: &str| -> String {
        if let Some(a) = m.model.aliases.get(n) {
            return swift_of(m, swift, &a.ty, source, conc, None);
        }
        match m.by_ref(source, n) {
            Some(t) => m
                .swift_of_type
                .get(&(t.source.clone(), t.name.clone()))
                .cloned()
                .unwrap_or_else(|| "JSONValue".into()),
            None => "JSONValue".into(),
        }
    };
    match t {
        TypeRef::Prim(p) => swift_prim(p).into(),
        TypeRef::Json | TypeRef::Tuple(_) => "JSONValue".into(),
        TypeRef::Option(i) | TypeRef::TriState(i) => {
            format!("{}?", swift_of(m, swift, i, source, conc, None))
        }
        TypeRef::Array(i) if matches!(i.as_ref(), TypeRef::Prim(p) if p == "u8") => {
            "WireBytes".into()
        }
        TypeRef::Array(i) => format!("[{}]", swift_of(m, swift, i, source, conc, None)),
        TypeRef::Map { value, .. } => format!(
            "[String: {}]",
            swift_of(m, swift, value, source, conc, None)
        ),
        TypeRef::Generic(g) => conc.get(g).map_or_else(|| "JSONValue".into(), |c| named(c)),
        TypeRef::Ref { name, .. } => named(name),
        TypeRef::Codec { rust, wire } => match wire {
            CodecWire::Type(d) => swift_of(m, swift, d, source, conc, None),
            CodecWire::Literal(l) => format!("Lit{}", pascal(l)),
            CodecWire::Ts(_) => {
                if f.is_some_and(|f| f.serde_with.is_some()) {
                    return "JSONValue".into();
                }
                // No serde fn: the wire is the Rust type's own serde form.
                rust_text(&rust.replace(' ', ""))
            }
        },
    }
}

/// Swift type of a Rust type written as text (a `#[ts(type)]` codec field
/// without a serde fn): `Option<..>`, `Vec<..>`, `serde_json::Map` /
/// `HashMap<String, ..>`, `Value`, primitives.
fn rust_text(r: &str) -> String {
    let inner = |p: &str| r.strip_prefix(p).and_then(|x| x.strip_suffix('>'));
    if let Some(i) = inner("Option<") {
        return format!("{}?", rust_text(i));
    }
    if let Some(i) = inner("Vec<") {
        return format!("[{}]", rust_text(i));
    }
    for p in [
        "serde_json::Map<String,",
        "Map<String,",
        "HashMap<String,",
        "BTreeMap<String,",
    ] {
        if let Some(i) = inner(p) {
            return format!("[String: {}]", rust_text(i));
        }
    }
    let prims = [
        "String", "bool", "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize",
        "f32", "f64",
    ];
    if prims.contains(&r) {
        return swift_prim(r).into();
    }
    "JSONValue".into()
}

// ---- checks 1-4, T, E, U --------------------------------------------------------

fn generic(ty: &str, w: &str) -> Option<String> {
    ty.strip_prefix(&format!("{w}<"))
        .and_then(|x| x.strip_suffix('>'))
        .map(String::from)
}

fn decode_type(dec: &str) -> Option<String> {
    let i = dec.find("decode")?;
    let r = &dec[i..];
    let j = r.find('(')?;
    let r = &r[j + 1..];
    r.find(".self").map(|e| r[..e].to_string())
}

#[derive(Default)]
struct Report {
    bad: Vec<String>,
    structs: usize,
    fields: usize,
    enums: usize,
    unions: usize,
    sims: usize,
    perturbed: usize,
    encode_only: usize,
    known: BTreeMap<&'static str, usize>,
}

fn check_struct(m: &Manifest, swift: &Swift, name: &str, sf: &[SField], r: &mut Report) {
    let mfs = match m.fields_of(name) {
        Ok(x) => x,
        Err(e) => {
            r.bad.push(format!("[1] {e}"));
            return;
        }
    };
    r.structs += 1;
    let want: BTreeSet<String> = mfs.iter().map(MField::wire).collect();
    let got: BTreeSet<String> = sf.iter().map(|f| f.key.clone()).collect();
    if want != got {
        r.bad.push(format!(
            "[1] {name}: CodingKeys {got:?} != serde keys {want:?}"
        ));
    }
    for mf in &mfs {
        let Some(s) = sf.iter().find(|s| s.key == mf.wire()) else {
            continue;
        };
        r.fields += 1;
        let at = format!("{name}.{}", s.key);
        let id = &s.id;
        let lit = match mf {
            MField::Tag(t) => Some(t.clone()),
            MField::Field(f, ..) => match &f.ty {
                TypeRef::Codec {
                    wire: CodecWire::Literal(l),
                    ..
                } => Some(l.clone()),
                _ => None,
            },
        };
        if let Some(l) = lit {
            let n = format!("Lit{}", pascal(&l));
            if !s.constant || s.ty != n {
                r.bad.push(format!(
                    "[4] {at}: Literal is `{}`, want constant `{n}()`",
                    s.ty
                ));
            }
            if s.enc != format!("try c.encode({id}, forKey: .{})", id.trim_matches('`')) {
                r.bad
                    .push(format!("[4] {at}: Literal not always written: `{}`", s.enc));
            }
            if !(s.dec.contains("contains(") && s.dec.contains(&format!("decode({n}.self"))) {
                r.bad.push(format!(
                    "[4] {at}: Literal value not checked on decode: `{}`",
                    s.dec
                ));
            }
            match swift.get(&n) {
                Some(Decl::Lit(v)) if *v == l => {}
                other => r.bad.push(format!(
                    "[4] {at}: {n} is {other:?}, want the literal {l:?}"
                )),
            }
            continue;
        }
        let MField::Field(f, source, conc) = mf else {
            continue;
        };
        if s.constant {
            r.bad
                .push(format!("[4] {at}: constant for a non-Literal field"));
        }
        let opt = s.ty.ends_with('?');
        let tri = generic(&s.ty, "TriState");
        let dflt = generic(&s.ty, "Defaulted");
        // [T] property type vs the manifest Rust type.
        let mapped = canon(swift, &swift_of(m, swift, &f.ty, source, conc, Some(f)));
        let base_prop = canon(
            swift,
            &tri.clone()
                .or_else(|| dflt.clone())
                .unwrap_or_else(|| s.ty.clone()),
        );
        let want_prop = if tri.is_some() {
            mapped.strip_suffix('?').unwrap_or(&mapped).to_string()
        } else {
            mapped.clone()
        };
        if base_prop != want_prop {
            r.bad
                .push(format!("[T] {at}: Swift `{}`, manifest `{mapped}`", s.ty));
        }
        let dt = decode_type(&s.dec).map(|t| canon(swift, &t));
        let want_dec = if opt {
            base_prop.strip_suffix('?').map(String::from)
        } else {
            Some(base_prop.clone())
        };
        if dt != want_dec {
            r.bad
                .push(format!("[T] {at}: decodes `{dt:?}`, property `{}`", s.ty));
        }
        // [2] optionality.
        match f.null.input {
            "asAbsent" if !opt => r
                .bad
                .push(format!("[2] {at}: null.input asAbsent but `{}`", s.ty)),
            "distinct" if tri.is_none() => r
                .bad
                .push(format!("[2] {at}: null.input distinct but `{}`", s.ty)),
            "rejected" | "asValue" | "byType" if opt || tri.is_some() => {
                r.bad.push(format!(
                    "[2] {at}: null.input {} but optional `{}`",
                    f.null.input, s.ty
                ));
            }
            _ => {}
        }
        // [3] decode.
        let d = &s.dec;
        let dk = if f.null.input == "distinct" {
            "decodeTriState("
        } else if f.null.input == "rejected" && s.ty == "JSONValue" {
            // nonNullJson: a JSON value whose `null` serde rejects.
            "decodeNonNull("
        } else if opt {
            "decodeIfPresent("
        } else if dflt.is_some() {
            "decodeDefaulted("
        } else if f.default {
            "decodeDefault("
        } else {
            "decode("
        };
        if !d.contains(&format!(".{dk}")) {
            r.bad.push(format!(
                "[3] {at}: null.input {} default={} wants `{dk}`: `{d}`",
                f.null.input, f.default
            ));
        }
        if dflt.is_some() && !f.default {
            r.bad
                .push(format!("[3] {at}: Defaulted without #[serde(default)]"));
        }
        // [3] encode.
        let e = &s.enc;
        let pred = f
            .omitted_when
            .as_deref()
            .map(|p| p.rsplit("::").next().unwrap_or(p));
        let ok = match pred {
            None if f.null.output == "whenNone" => e.starts_with("try c.encodeNullable("),
            None if dflt.is_some() => e.starts_with("try c.encodeDefaulted("),
            None => e.starts_with(&format!("try c.encode({id},")),
            Some("is_none") if tri.is_some() => e.starts_with("try c.encodeTriState("),
            Some("is_none") => e.starts_with("try c.encodeIfPresent("),
            Some("is_empty") => e.starts_with(&format!("if !{id}.isEmpty {{ try c.encode({id},")),
            Some(_) => {
                e.starts_with(&format!("if {id} != ."))
                    && e.contains(&format!("{{ try c.encode({id},"))
            }
        };
        if !ok {
            r.bad.push(format!(
                "[3] {at}: omittedWhen {:?} null.output {} but `{e}`",
                f.omitted_when, f.null.output
            ));
        }
    }
}

fn check_decls(m: &Manifest, swift: &Swift, r: &mut Report) {
    for (name, d) in swift {
        match d {
            Decl::Struct(sf) => check_struct(m, swift, name, sf, r),
            Decl::Closed { raws, .. } | Decl::Open { cases: raws, .. } => {
                r.enums += 1;
                let Some((t, _)) = m.mtype(name) else {
                    r.bad.push(format!("[E] {name}: maps to no manifest type"));
                    continue;
                };
                let Shape::Enum { variants, .. } = &t.shape else {
                    r.bad
                        .push(format!("[E] {name}: manifest {} is not an enum", t.name));
                    continue;
                };
                // A `#[serde(other)]` unit variant is written as its name.
                let want: BTreeSet<String> =
                    variants.iter().filter_map(|v| v.wire.clone()).collect();
                let got: BTreeSet<String> = raws.values().cloned().collect();
                if want != got {
                    r.bad
                        .push(format!("[E] {name}: raw values {got:?} != serde {want:?}"));
                }
            }
            Decl::Union(u) => {
                r.unions += 1;
                let Some((e, _)) = m.mtype(name).and_then(|(t, _)| m.enum_of(t)) else {
                    r.bad.push(format!("[U] {name}: maps to no manifest enum"));
                    continue;
                };
                let Shape::Enum {
                    representation,
                    variants,
                } = &e.shape
                else {
                    unreachable!()
                };
                let (kind, tag, content) = match representation {
                    Representation::Internal { tag } => ("internal", Some(tag.clone()), None),
                    Representation::Adjacent { tag, content } => {
                        ("adjacent", Some(tag.clone()), Some(content.clone()))
                    }
                    Representation::External => ("external", None, None),
                    Representation::Untagged => ("untagged", None, None),
                };
                if u.kind != kind || u.tag != tag || (kind == "adjacent" && u.content != content) {
                    r.bad.push(format!(
                        "[U] {name}: Swift {} tag {:?} content {:?}, serde {kind} tag {tag:?} content {content:?}",
                        u.kind, u.tag, u.content
                    ));
                }
                let cases: Vec<String> = variants.iter().map(|v| v.rust.clone()).collect();
                let got_cases: Vec<String> = u
                    .arms
                    .iter()
                    .map(|a| a.1.clone())
                    .chain(u.other.clone())
                    .collect();
                if got_cases.len() != cases.len() {
                    r.bad.push(format!(
                        "[U] {name}: Swift cases {got_cases:?}, serde variants {cases:?}"
                    ));
                }
                if kind == "untagged" {
                    // serde tries untagged arms in declaration order, the
                    // first that decodes wins; Swift must try them in the
                    // same order.
                    let norm = |c: &str| c.trim_matches('`').to_ascii_lowercase();
                    let want: Vec<String> = cases.iter().map(|c| norm(c)).collect();
                    let got: Vec<String> = got_cases.iter().map(|c| norm(c)).collect();
                    if want != got {
                        r.bad.push(format!(
                            "[U] {name}: untagged arm order {got:?} != serde declaration order {want:?}"
                        ));
                    }
                    continue;
                }
                let want: BTreeSet<String> = variants
                    .iter()
                    .filter(|v| !v.other)
                    .flat_map(|v| v.wire.iter().chain(&v.aliases).cloned())
                    .collect();
                let got: BTreeSet<String> =
                    u.arms.iter().flat_map(|a| a.0.iter().cloned()).collect();
                if want != got {
                    r.bad
                        .push(format!("[U] {name}: decode tags {got:?} != serde {want:?}"));
                }
                let wenc: BTreeSet<String> =
                    variants.iter().filter_map(|v| v.wire.clone()).collect();
                let genc: BTreeSet<String> = u.enc_tags.values().cloned().collect();
                if wenc != genc {
                    r.bad.push(format!(
                        "[U] {name}: encode tags {genc:?} != serde {wenc:?}"
                    ));
                }
            }
            Decl::Alias(_) | Decl::Lit(_) => {}
        }
    }
}

// ---- [S] simulation of the parsed Swift vs serde -------------------------------

#[derive(Clone, Debug, PartialEq)]
enum FV {
    Absent,
    Null,
    Val(Value),
}

struct Sim<'a> {
    swift: &'a Swift,
    /// Every type name [`Sim::value`] was asked for.
    seen: std::cell::RefCell<BTreeSet<String>>,
}

impl Sim<'_> {
    fn value(&self, ty: &str, v: &Value, depth: usize) -> Result<Value, String> {
        self.seen.borrow_mut().insert(ty.to_string());
        if depth > 64 {
            return Err("depth".into());
        }
        if let Some(i) = ty.strip_suffix('?') {
            return if v.is_null() {
                Ok(Value::Null)
            } else {
                self.value(i, v, depth + 1)
            };
        }
        if let Some(inner) = ty.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            if let Some(val) = inner.strip_prefix("String: ") {
                let o = v.as_object().ok_or("expected object")?;
                let mut out = Map::new();
                for (k, x) in o {
                    out.insert(k.clone(), self.value(val, x, depth + 1)?);
                }
                return Ok(Value::Object(out));
            }
            let a = v.as_array().ok_or("expected array")?;
            return a
                .iter()
                .map(|x| self.value(inner, x, depth + 1))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array);
        }
        let int = |lo: i128, hi: i128| -> Result<Value, String> {
            let n = v
                .as_i64()
                .map(i128::from)
                .or_else(|| v.as_u64().map(i128::from))
                .ok_or(format!("{ty}: not an integer"))?;
            if n < lo || n > hi {
                return Err(format!("{ty}: out of range"));
            }
            Ok(v.clone())
        };
        match ty {
            "JSONValue" => Ok(v.clone()),
            "String" => v
                .as_str()
                .map(|_| v.clone())
                .ok_or_else(|| "expected string".into()),
            "Bool" => v
                .as_bool()
                .map(|_| v.clone())
                .ok_or_else(|| "expected bool".into()),
            "UInt8" => int(0, 255),
            "UInt16" => int(0, 65535),
            "UInt32" => int(0, i128::from(u32::MAX)),
            "UInt64" => int(0, i128::from(u64::MAX)),
            "Int8" => int(-128, 127),
            "Int16" => int(-32768, 32767),
            "Int32" => int(i128::from(i32::MIN), i128::from(i32::MAX)),
            "Int64" => int(i128::from(i64::MIN), i128::from(i64::MAX)),
            "WireBytes" => self.value("[UInt8]", v, depth),
            "Double" => v
                .as_f64()
                .map(|x| json!(x))
                .ok_or_else(|| "expected number".into()),
            #[allow(clippy::cast_possible_truncation)]
            "Float" => v
                .as_f64()
                .map(|x| json!(f64::from(x as f32)))
                .ok_or_else(|| "expected number".into()),
            n => match self.swift.get(n) {
                None => Err(format!("{n}: no Swift declaration")),
                Some(Decl::Alias(t)) => self.value(t, v, depth + 1),
                Some(Decl::Lit(l)) => (v.as_str() == Some(l))
                    .then(|| v.clone())
                    .ok_or_else(|| format!("{n}: expected {l:?}")),
                Some(Decl::Closed { raws, aliases }) => {
                    let s = v.as_str().ok_or("expected string")?;
                    let s = aliases.get(s).map_or(s, String::as_str);
                    raws.values()
                        .any(|r| r == s)
                        .then(|| json!(s))
                        .ok_or_else(|| format!("{n}: unknown value {s}"))
                }
                Some(Decl::Open { aliases, .. }) => {
                    let s = v.as_str().ok_or("expected string")?;
                    Ok(json!(aliases.get(s).map_or(s, String::as_str)))
                }
                Some(Decl::Struct(fs)) => self.record(n, fs, v, depth + 1),
                Some(Decl::Union(u)) => self.union(n, u, v, depth + 1),
            },
        }
    }

    fn default_value(&self, ty: &str, expr: &str) -> Result<FV, String> {
        Ok(FV::Val(match expr {
            "false" => json!(false),
            "\"\"" => json!(""),
            "0" => json!(0),
            "[]" => json!([]),
            "[:]" => json!({}),
            "JSONValue.null" => Value::Null,
            "nil" => return Ok(FV::Null),
            "true" => json!(true),
            e if e.parse::<i64>().is_ok() => json!(e.parse::<i64>().unwrap()),
            e if e.len() > 1 && e.starts_with('"') && e.ends_with('"') => json!(e[1..e.len() - 1]),
            e if e.starts_with('.') => {
                let c = e[1..].trim_matches('`');
                match self.swift.get(ty) {
                    Some(Decl::Closed { raws, .. }) => {
                        json!(raws.get(c).ok_or(format!("{ty}.{c}"))?)
                    }
                    Some(Decl::Open { cases, .. }) => {
                        json!(cases.get(c).ok_or(format!("{ty}.{c}"))?)
                    }
                    _ => return Err(format!("{ty}: no case {c}")),
                }
            }
            e => return Err(format!("unknown default `{e}`")),
        }))
    }

    fn record(&self, name: &str, fs: &[SField], v: &Value, depth: usize) -> Result<Value, String> {
        let o = v.as_object().ok_or(format!("{name}: expected object"))?;
        let mut out = Map::new();
        for f in fs {
            // Key read: the first present alias, else the wire key.
            let key = if f.dec.contains("AnyCodingKey([") {
                let q = quoted(&f.dec[f.dec.find("AnyCodingKey([").unwrap()..]);
                q.iter()
                    .find(|k| o.contains_key(k.as_str()))
                    .cloned()
                    .unwrap_or_else(|| f.key.clone())
            } else {
                f.key.clone()
            };
            let raw = o.get(&key);
            let d = &f.dec;
            let ty = decode_type(d).unwrap_or_default();
            let state = if f.constant {
                if let Some(x) = raw
                    && d.contains(&format!("decode({}.self", f.ty))
                {
                    self.value(&f.ty, x, depth)?;
                }
                FV::Val(Value::Null)
            } else if d.contains(".decodeTriState(") {
                match raw {
                    None => FV::Absent,
                    Some(Value::Null) => FV::Null,
                    Some(x) => FV::Val(self.value(&ty, x, depth)?),
                }
            } else if d.contains(".decodeIfPresent(") {
                match raw {
                    None | Some(Value::Null) => FV::Null,
                    Some(x) => FV::Val(self.value(&ty, x, depth)?),
                }
            } else if d.contains(".decodeDefaulted(") {
                match raw {
                    None => FV::Absent,
                    Some(x) => FV::Val(self.value(&ty, x, depth)?),
                }
            } else if d.contains(".decodeDefault(") {
                match raw {
                    None => {
                        let expr = d
                            .rsplit_once(", ")
                            .map_or("", |x| x.1)
                            .trim_end_matches(')');
                        self.default_value(&ty, expr)?
                    }
                    Some(x) => FV::Val(self.value(&ty, x, depth)?),
                }
            } else if d.contains(".decodeNonNull(") {
                match raw {
                    None => return Err(format!("{name}.{}: missing key", f.key)),
                    Some(Value::Null) => return Err(format!("{name}.{}: null is rejected", f.key)),
                    Some(x) => FV::Val(self.value(&ty, x, depth)?),
                }
            } else if d.contains(".decode(") {
                let x = raw.ok_or(format!("{name}.{}: missing key", f.key))?;
                FV::Val(self.value(&ty, x, depth)?)
            } else {
                return Err(format!("{name}.{}: no decode", f.key));
            };
            // Encode.
            let e = &f.enc;
            let wrote = if f.constant {
                match self.swift.get(&f.ty) {
                    Some(Decl::Lit(l)) if e.starts_with("try c.encode(") => Some(json!(l)),
                    _ => None,
                }
            } else if e.starts_with("try c.encodeIfPresent(") {
                match state {
                    FV::Val(x) => Some(x),
                    _ => None,
                }
            } else if e.starts_with("try c.encodeTriState(")
                || e.starts_with("try c.encodeDefaulted(")
            {
                match state {
                    FV::Absent => None,
                    FV::Null => Some(Value::Null),
                    FV::Val(x) => Some(x),
                }
            } else if e.starts_with("try c.encodeNullable(") || e.starts_with("try c.encode(") {
                match state {
                    FV::Val(x) => Some(x),
                    _ => Some(Value::Null),
                }
            } else if let Some(r) = e.strip_prefix("if !") {
                let _ = r;
                match state {
                    FV::Val(x)
                        if !(x.as_str() == Some("")
                            || x.as_array().is_some_and(Vec::is_empty)
                            || x.as_object().is_some_and(Map::is_empty)) =>
                    {
                        Some(x)
                    }
                    _ => None,
                }
            } else if let Some(r) = e.strip_prefix("if ")
                && let Some((_, rest)) = r.split_once(" != ")
            {
                let c = rest.split_whitespace().next().unwrap_or_default();
                let skip = match self.default_value(&f.ty, c)? {
                    FV::Val(x) => x,
                    _ => Value::Null,
                };
                match state {
                    FV::Val(x) if x != skip => Some(x),
                    _ => None,
                }
            } else {
                return Err(format!("{name}.{}: no encode", f.key));
            };
            if let Some(w) = wrote {
                out.insert(f.key.clone(), w);
            }
        }
        Ok(Value::Object(out))
    }

    fn union(&self, name: &str, u: &SUnion, v: &Value, depth: usize) -> Result<Value, String> {
        let arm_by_tag = |s: &str| u.arms.iter().find(|a| a.0.iter().any(|t| t == s));
        let enc_tag = |c: &str| {
            u.enc_tags
                .get(c)
                .cloned()
                .ok_or(format!("{name}.{c}: no encode tag"))
        };
        match u.kind {
            "internal" => {
                let tk = u.tag.as_deref().unwrap_or_default();
                let s = v
                    .get(tk)
                    .and_then(Value::as_str)
                    .ok_or(format!("{name}: no tag {tk}"))?;
                let (case, payload) = match arm_by_tag(s) {
                    Some((_, c, p)) => (c.clone(), p.clone()),
                    None => (
                        u.other.clone().ok_or(format!("{name}: unknown tag {s}"))?,
                        None,
                    ),
                };
                let mut out = match payload {
                    Some(p) => self.value(&p, v, depth)?,
                    None => json!({}),
                };
                let w = enc_tag(&case)?;
                out.as_object_mut()
                    .ok_or(format!("{name}: payload not an object"))?
                    .insert(tk.into(), json!(w));
                Ok(out)
            }
            "adjacent" => {
                let tk = u.tag.as_deref().unwrap_or_default();
                let ck = u.content.as_deref().unwrap_or_default();
                let s = v
                    .get(tk)
                    .and_then(Value::as_str)
                    .ok_or(format!("{name}: no tag"))?;
                let (_, case, p) = arm_by_tag(s).ok_or(format!("{name}: unknown tag {s}"))?;
                let mut out = Map::new();
                out.insert(tk.into(), json!(enc_tag(case)?));
                if let Some(p) = p {
                    out.insert(
                        ck.into(),
                        self.value(p, v.get(ck).unwrap_or(&Value::Null), depth)?,
                    );
                }
                Ok(Value::Object(out))
            }
            "external" => {
                if let Some(s) = v.as_str() {
                    let (_, case, p) = arm_by_tag(s)
                        .filter(|a| a.2.is_none())
                        .ok_or(format!("{name}: unknown {s}"))?;
                    let _ = p;
                    return Ok(json!(enc_tag(case)?));
                }
                let o = v
                    .as_object()
                    .filter(|o| o.len() == 1)
                    .ok_or(format!("{name}: expected one key"))?;
                let (k, x) = o.iter().next().unwrap();
                let (_, case, p) = arm_by_tag(k).ok_or(format!("{name}: unknown {k}"))?;
                let p = p.as_ref().ok_or(format!("{name}: unit as object"))?;
                Ok(json!({ enc_tag(case)?: self.value(p, x, depth)? }))
            }
            _ => {
                for (_, _, p) in &u.arms {
                    match p {
                        None if v.is_null() => return Ok(Value::Null),
                        None => {}
                        Some(p) => {
                            if let Ok(x) = self.value(p, v, depth) {
                                return Ok(x);
                            }
                        }
                    }
                }
                Err(format!("{name}: no variant matches"))
            }
        }
    }
}

/// The wire IR by type name, for [`float_paths::float_paths_in`].
fn ir() -> &'static float_paths::Ir {
    static IR: std::sync::OnceLock<float_paths::Ir> = std::sync::OnceLock::new();
    IR.get_or_init(|| {
        aimux_codegen::wire::from_inputs(&Inputs::from_workspace(&workspace()))
            .unwrap()
            .types
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect()
    })
}

/// Equality as U1 (tests/common/float_paths.rs `json_eq_at`): numbers
/// compare by value only at JSON paths of `truth` whose IR type is `f32` /
/// `f64`; every other number, integers included, must be the same JSON
/// number (`1` ≠ `1.0`).
fn same(ty: &str, truth: &Value, got: &Value) -> bool {
    let floats = float_paths::float_paths_in(ir(), ty, truth);
    float_paths::json_eq_at(truth, got, "", &floats)
}

fn model_spec_non_default() -> Value {
    use aimux_core::model_catalogue::{
        CatalogueSource, Modality, ModelCapabilities, ModelLimits, ModelModalities, ModelSpec,
        ModelType,
    };
    serde_json::to_value(ModelSpec {
        r#type: ModelType::Embedding,
        limits: ModelLimits {
            context: Some(1000),
            ..ModelLimits::default()
        },
        modalities: ModelModalities {
            input: vec![Modality::Image],
            output: vec![],
        },
        capabilities: ModelCapabilities {
            tool_call: true,
            ..ModelCapabilities::default()
        },
        source: CatalogueSource::Manual,
        ..ModelSpec::default()
    })
    .unwrap()
}

/// Each ModelSpec Defaulted field: absent, serde writes a value different
/// from the fixture's, so the `defaulted-ModelSpec/-x` perturbation is a
/// real difference, not one that happens to round-trip.
#[test]
fn model_spec_defaulted_fields_absent_give_a_different_value() {
    let full = model_spec_non_default();
    for k in ["type", "limits", "modalities", "capabilities", "source"] {
        let mut v = full.clone();
        v.as_object_mut().unwrap().remove(k);
        let out = back::<aimux_core::model_catalogue::ModelSpec>(&v).unwrap();
        assert!(out.get(k).is_some(), "{k}: serde writes the default");
        assert_ne!(out[k], full[k], "{k}: absent gives the same value");
    }
}

type Fx = (&'static str, &'static str, Value, Option<Back>);

fn all_fixtures() -> Vec<Fx> {
    let mut v = go_fixtures::coverage_fixtures();
    v.extend(go_fixtures_empty::empty_fixtures());
    v.extend(go_fixtures_tri::tri_state_fixtures());
    // The custom skip_serializing_if (aimux-core/src/recording.rs:184): no
    // generated fixture writes InputRecord, so these come from serde here.
    let input: Back = back::<aimux_core::recording::InputRecord>;
    for (label, op) in [
        ("skip-language_model", Some("language_model")),
        ("skip-decision", Some("decision")),
        ("skip-absent", None),
    ] {
        let mut j = json!({"prompt": [], "options": {}});
        if let Some(op) = op {
            j["operation"] = json!(op);
        }
        v.push((label, "InputRecord", j, Some(input)));
    }
    // The nonNullJson codec on its own types: ToolResult (`result` rejects
    // null) and TaggedToolResult (`output` is a plain JSON value).
    v.push((
        "nonnull-ToolResult",
        "ToolResult",
        json!({"toolCallId": "c", "toolName": "t", "result": {"k": 1}}),
        Some(back::<aimux_core::tool::ToolResult>),
    ));
    v.push((
        "nonnull-TaggedToolResult",
        "TypedToolResult",
        json!({"type": "tool-result", "toolCallId": "c", "toolName": "t", "output": {"k": 1}}),
        Some(back_tagged::<aimux_core::result::TypedToolResult>),
    ));
    // ModelSpec's five `#[serde(default)]` fields Swift spells Defaulted<T>
    // (aimux-core/src/model_catalogue.rs:244-259), each set to a
    // non-default value, so dropping one (`/-type` …) changes serde's output.
    v.push((
        "defaulted-ModelSpec",
        "ModelSpec",
        model_spec_non_default(),
        Some(back::<aimux_core::model_catalogue::ModelSpec>),
    ));
    // Hand-written Swift types (bindings/swift/Sources/Aimux) no generated
    // fixture reaches (see
    // every_hand_written_type_with_a_generated_counterpart_has_a_fixture).
    let hw: [(&'static str, &'static str, Value, Back); 11] = [
        (
            "hand-ModelPrompt-text",
            "ModelPrompt",
            json!("hi"),
            back::<aimux_core::message::ModelPrompt>,
        ),
        (
            "hand-ResponseFormat-json",
            "ResponseFormat",
            json!({"type": "json", "name": "n"}),
            back::<aimux_core::options::ResponseFormat>,
        ),
        (
            "hand-ResponseFormat-text",
            "ResponseFormat",
            json!({"type": "text"}),
            back::<aimux_core::options::ResponseFormat>,
        ),
        (
            "hand-ToolResultContent-text",
            "ToolResultContent",
            json!({"type": "text", "text": "t"}),
            back::<aimux_core::language_model_message::ToolResultContent>,
        ),
        (
            "hand-ToolResultContent-file",
            "ToolResultContent",
            json!({"type": "file", "data": {"type": "url", "url": "https://x"}, "mediaType": "image/png"}),
            back::<aimux_core::language_model_message::ToolResultContent>,
        ),
        (
            "hand-FilePartData-tagged",
            "FilePartData",
            json!({"type": "url", "url": "https://x"}),
            back::<aimux_core::content::FilePartData>,
        ),
        (
            "hand-FilePartData-string",
            "FilePartData",
            json!("abc"),
            back::<aimux_core::content::FilePartData>,
        ),
        (
            "hand-ReasoningFilePartData-string",
            "ReasoningFilePartData",
            json!("abc"),
            back::<aimux_core::content::ReasoningFilePartData>,
        ),
        (
            "hand-ReasoningPart-text",
            "ReasoningPart",
            json!({"type": "reasoning", "text": "t"}),
            back::<aimux_core::result::ReasoningPart>,
        ),
        (
            "hand-TextStreamPart-TextStart",
            "TextStreamPart",
            json!({"type": "text-start", "id": "a"}),
            back::<aimux_core::stream_part::TextStreamPart>,
        ),
        (
            "hand-ToolOutputDenied",
            "ToolOutputDenied",
            json!({"toolCallId": "c", "toolName": "t"}),
            back::<aimux_core::result::ToolOutputDenied>,
        ),
    ];
    for (label, ty, j, b) in hw {
        v.push((label, ty, j, Some(b)));
    }
    // One generated field-absent fixture per emit::registry entry (the
    // same ones the Go round trip runs).
    for (label, ty, j) in registry_fixtures::registry_fixtures() {
        let b = registry_fixtures::registry_back(&ty);
        v.push((label.leak(), ty.leak(), j, Some(b)));
    }
    v
}

/// Swift type a fixture's value is: a `TypeTag` type sampled with its tag
/// is `Tagged<T>`.
fn swift_name(m: &Manifest, swift: &Swift, ty: &str, json: &Value) -> String {
    let tagged = format!("Tagged{ty}");
    if m.tag_of(ty).is_some() && json.get("type").is_some() && swift.contains_key(&tagged) {
        tagged
    } else {
        ty.to_string()
    }
}

fn flip(v: &Value) -> Value {
    match v {
        Value::String(_) => json!(12345),
        Value::Number(_) => json!("x"),
        Value::Bool(_) => json!("x"),
        Value::Array(_) => json!({"zz": 1}),
        Value::Object(_) => json!([1]),
        Value::Null => json!({"zz": 1}),
    }
}

fn sim_check(m: &Manifest, swift: &Swift, r: &mut Report) {
    let sim = Sim {
        swift,
        seen: Default::default(),
    };
    for (label, ty, json, back) in all_fixtures() {
        let name = swift_name(m, swift, ty, &json);
        if !swift.contains_key(&name) {
            r.bad.push(format!("[S] {label}: no Swift type {name}"));
            continue;
        }
        r.sims += 1;
        let got = sim.value(&name, &json, 0);
        // The fixture is serde output; re-reading it must give it back.
        let truth = match back {
            Some(b) => b(&json),
            None => {
                r.encode_only += 1;
                Ok(json.clone())
            }
        };
        compare(&name, label, &truth, &got, r);
        let (Some(b), Some(obj)) = (back, json.as_object()) else {
            continue;
        };
        let mut cases: Vec<(String, Value)> = Vec::new();
        for (k, x) in obj {
            let mut a = obj.clone();
            a.remove(k);
            cases.push((format!("{label}/-{k}"), Value::Object(a)));
            let mut n = obj.clone();
            n.insert(k.clone(), Value::Null);
            cases.push((format!("{label}/{k}=null"), Value::Object(n)));
            let mut w = obj.clone();
            w.insert(
                k.clone(),
                if x.is_string() {
                    json!("zz-not-a-value")
                } else {
                    flip(x)
                },
            );
            cases.push((format!("{label}/{k}=wrong"), Value::Object(w)));
        }
        for (l, p) in cases {
            r.perturbed += 1;
            compare(&name, &l, &b(&p), &sim.value(&name, &p, 0), r);
        }
    }
}

fn compare(
    ty: &str,
    label: &str,
    truth: &Result<Value, String>,
    got: &Result<Value, String>,
    r: &mut Report,
) {
    match (truth, got) {
        (Ok(t), Ok(g)) if same(ty, t, g) => {}
        (Err(_), Err(_)) => {}
        (Ok(t), Ok(g)) => r.bad.push(format!("[S] {label}: serde {t} vs Swift {g}")),
        (Ok(_), Err(e)) => r
            .bad
            .push(format!("[S] {label}: serde accepts, Swift rejects ({e})")),
        (Err(e), Ok(g)) => r.bad.push(format!(
            "[S] {label}: serde rejects ({e}), Swift accepts as {g}"
        )),
    }
}

/// Known serde/Swift differences: every label is listed exactly, must
/// still fail with the given message part, and is reported as known.
const KNOWN_DIFFS: &[(&str, &str, &str, &[&str])] = &[
    (
        "seq",
        "serde also reads a struct written as a JSON array (visit_seq); Swift Codable reads only objects",
        "serde accepts, Swift rejects",
        &[
            "cov-EmbeddingCallOptions/timeout=wrong",
            "cov-EmbeddingResult/usage=wrong",
            "cov-GenerateObjectResult/request=wrong",
            "cov-GenerateObjectResult/usage=wrong",
            "cov-GenerateResult/request=wrong",
            "cov-GenerateTextOptions/timeout=wrong",
            "cov-ImageCallOptions/timeout=wrong",
            "cov-ImageResult/usage=wrong",
            "cov-LanguageModelUsage/inputTokenDetails=wrong",
            "cov-LanguageModelUsage/outputTokenDetails=wrong",
            "cov-RerankingCallOptions/timeout=wrong",
            "cov-SearchCallOptions/timeout=wrong",
            "cov-SpeechCallOptions/timeout=wrong",
            "cov-SpeechResult/request=wrong",
            "cov-StepResult/request=wrong",
            "cov-StepResult/usage=wrong",
            "cov-TraceRecord/usage=wrong",
            "cov-TranscriptionCallOptions/timeout=wrong",
            "cov-Usage/inputTokens=wrong",
            "cov-Usage/outputTokens=wrong",
            "cov-VideoCallOptions/poll=wrong",
            "cov-VideoCallOptions/timeout=wrong",
            "defaulted-ModelSpec/limits=wrong",
            "empty-GenerateObjectResult/request=wrong",
            "empty-GenerateObjectResult/usage=wrong",
            "empty-LanguageModelUsage/inputTokenDetails=wrong",
            "empty-LanguageModelUsage/outputTokenDetails=wrong",
            "empty-StepResult/request=wrong",
            "empty-StepResult/usage=wrong",
            "empty-TraceRecord/usage=wrong",
            "empty-Usage/inputTokens=wrong",
            "empty-Usage/outputTokens=wrong",
            "registry-HttpExchange-absent:crate::recording::default_finalized/timing=wrong",
        ],
    ),
    (
        "validated",
        "a hand-written Deserialize validates the string (Timestamp, Size, AspectRatio, `<provider>.<name>` ids, ToolChoice); the Swift alias is a plain String/JSONValue",
        "serde rejects",
        &[
            "cov-ContentPart-Custom/kind=wrong",
            "cov-GenerateObjectResponse/timestamp=wrong",
            "cov-GenerateTextOptions/toolChoice=wrong",
            "cov-ImageCallOptions/aspectRatio=wrong",
            "cov-ImageCallOptions/size=wrong",
            "cov-ImageResponse/timestamp=wrong",
            "cov-RerankingResponse/timestamp=wrong",
            "cov-ResponseInfo/timestamp=wrong",
            "cov-ResponseMetadata/timestamp=wrong",
            "cov-SpeechResponse/timestamp=wrong",
            "cov-StepResponse/timestamp=wrong",
            "cov-StreamPart-Custom/kind=wrong",
            "cov-StreamPart-ResponseMetadata/timestamp=wrong",
            "cov-Tool-Provider/id=wrong",
            "cov-ToolChoice-3/-toolName",
            "cov-ToolChoice-3/-type",
            "cov-ToolChoice-3/toolName=null",
            "cov-ToolChoice-3/type=null",
            "cov-ToolChoice-3/type=wrong",
            "cov-TranscriptionResponse/timestamp=wrong",
            "cov-VideoCallOptions/aspectRatio=wrong",
            "cov-VideoCallOptions/resolution=wrong",
            "cov-VideoResponse/timestamp=wrong",
            "empty-GenerateObjectResponse/timestamp=wrong",
            "empty-StepResponse/timestamp=wrong",
            "tri-RerankingResponse/timestamp=wrong",
            "tri-ResponseInfo/timestamp=wrong",
            "tri-SpeechResponse/timestamp=wrong",
            "tri-StepResponse/timestamp=wrong",
            "tri-TranscriptionResponse/timestamp=wrong",
            "tri-TranscriptionStreamPart-ResponseMetadata/timestamp=wrong",
        ],
    ),
    (
        "nullalias",
        "a handWritten/codec type is a Swift `JSONValue` alias, which also holds null and other kinds",
        "serde rejects",
        &[
            "cov-GenerateObjectResponse/timestamp=null",
            "cov-GeneratedFile/base64=null",
            "cov-ImageResult/images=null",
            "cov-ImageResult/images=wrong",
            "cov-SpeechResult/audio=null",
            "cov-StepResponse/timestamp=null",
            "cov-TranscriptionCallOptions/audio=null",
            "cov-VideoData-Binary/data=null",
            "empty-GenerateObjectResponse/timestamp=null",
            "empty-GeneratedFile/base64=null",
            "empty-ImageResult/images=null",
            "empty-ImageResult/images=wrong",
            "empty-SpeechResult/audio=null",
            "empty-StepResponse/timestamp=null",
            "empty-TranscriptionCallOptions/audio=null",
            "tri-StepResponse/timestamp=null",
        ],
    ),
    (
        "defaulted",
        "absent `#[serde(default)]` field Swift spells as Defaulted<T>: Swift leaves it out, serde writes Default::default()",
        "serde {",
        &[
            "cov-GenerateObjectResult/-request",
            "cov-LanguageModelUsage/-inputTokenDetails",
            "cov-LanguageModelUsage/-outputTokenDetails",
            "cov-StepResult/-request",
            "defaulted-ModelSpec/-capabilities",
            "defaulted-ModelSpec/-limits",
            "defaulted-ModelSpec/-modalities",
            "defaulted-ModelSpec/-source",
            "defaulted-ModelSpec/-type",
            "empty-GenerateObjectResult/-request",
            "empty-LanguageModelUsage/-inputTokenDetails",
            "empty-LanguageModelUsage/-outputTokenDetails",
            "empty-StepResult/-request",
        ],
    ),
    (
        "other",
        "an unknown value of a `#[serde(other)]` enum: serde re-writes it as the `Other` variant name; the Swift open enum keeps the string (ModelType, aimux-core/src/model_catalogue.rs:75-76)",
        "vs Swift",
        &["defaulted-ModelSpec/type=wrong"],
    ),
    (
        "base64",
        "VideoData base64 is normalised by serde (url-safe to standard); Swift keeps the string",
        "vs Swift",
        &["cov-VideoData-Binary/data=wrong"],
    ),
    (
        "lenient",
        "InputRecord.prompt has deserialize_with = \"deserialize_prompt\" (aimux-core/src/recording.rs:187), which loads an unreadable prompt as empty; Swift decodes the typed array",
        "serde accepts, Swift rejects",
        &[
            "skip-absent/prompt=null",
            "skip-absent/prompt=wrong",
            "skip-decision/prompt=null",
            "skip-decision/prompt=wrong",
            "registry-InputRecord-absent:crate::recording::RecordingOperation::is_language_model/prompt=null",
            "registry-InputRecord-absent:crate::recording::RecordingOperation::is_language_model/prompt=wrong",
            "skip-language_model/prompt=null",
            "skip-language_model/prompt=wrong",
        ],
    ),
];
/// The exact `[S]` message each known difference has
/// (tests/swift_known_diffs.json, label → message). A registered label that
/// now fails with any other message is not known: it stays red.
/// `AIMUX_BLESS_KNOWN=1` rewrites the file from the current run.
fn known_messages() -> BTreeMap<String, String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/swift_known_diffs.json");
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn report(text: &str) -> Report {
    report_with(text, &known_messages())
}

fn report_with(text: &str, expected: &BTreeMap<String, String>) -> Report {
    let m = manifest(text);
    let swift = parse(text);
    let mut r = Report::default();
    check_decls(&m, &swift, &mut r);
    sim_check(&m, &swift, &mut r);
    if std::env::var_os("AIMUX_BLESS_KNOWN").is_some() {
        let labels: BTreeSet<&str> = KNOWN_DIFFS
            .iter()
            .flat_map(|k| k.3.iter().copied())
            .collect();
        let msgs: BTreeMap<String, String> = r
            .bad
            .iter()
            .filter_map(|b| b.strip_prefix("[S] ")?.split_once(": "))
            .filter(|(l, _)| labels.contains(l))
            .map(|(l, m)| (l.to_string(), m.to_string()))
            .collect();
        std::fs::write(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/swift_known_diffs.json"),
            serde_json::to_string_pretty(&msgs).unwrap() + "\n",
        )
        .unwrap();
    }
    let mut hit: BTreeSet<&str> = BTreeSet::new();
    r.bad.retain(|b| {
        let Some(rest) = b.strip_prefix("[S] ") else {
            return true;
        };
        let Some((label, msg)) = rest.split_once(": ") else {
            return true;
        };
        for (cat, _, part, labels) in KNOWN_DIFFS {
            if let Some(l) = labels.iter().find(|l| **l == label)
                && msg.contains(part)
                && expected.get(label).is_some_and(|m| m == msg)
            {
                hit.insert(l);
                *r.known.entry(cat).or_default() += 1;
                return false;
            }
        }
        true
    });
    for (cat, _, _, labels) in KNOWN_DIFFS {
        for l in labels.iter().filter(|l| !hit.contains(**l)) {
            r.bad.push(format!(
                "[K] stale known difference {cat}: {l} no longer differs"
            ));
        }
    }
    r
}

// ---- tests -----------------------------------------------------------------------

#[test]
fn generated_swift_snapshot_is_current() {
    let inputs = Inputs::from_workspace(&workspace());
    let out = workspace().join("tools/aimux-codegen/generated");
    if let Err(p) = aimux_codegen::emit::swift::check_committed(&inputs, &out) {
        panic!("{p:#?}\nrun `cargo run -p aimux-codegen` to regenerate generated/swift");
    }
}

/// Exact coverage: every Swift struct maps to manifest fields, every
/// fixture is simulated, and nothing disagrees with serde.
#[test]
fn generated_swift_matches_manifest_and_serde() {
    let r = report(&committed_types());
    assert!(r.bad.is_empty(), "{:#?}", r.bad);
    let counts = (
        r.structs,
        r.fields,
        r.enums,
        r.unions,
        r.sims,
        r.encode_only,
        r.perturbed,
    );
    eprintln!("structs, fields, enums, unions, fixtures, encode-only, perturbed = {counts:?}");
    eprintln!("known differences = {:?}", r.known);
    assert_eq!(counts, EXPECTED_COUNTS);
    let known: Vec<(&str, usize)> = r.known.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(known, EXPECTED_KNOWN);
}

/// Filled in from the first green run; any change in coverage is a diff.
const EXPECTED_KNOWN: &[(&str, usize)] = &[
    ("base64", 1),
    ("defaulted", 13),
    ("lenient", 8),
    ("nullalias", 16),
    ("other", 1),
    ("seq", 33),
    ("validated", 31),
];

/// A known label failing with a different message than the registered one
/// (here: the same field, `cov-Usage/inputTokens=wrong`, with another
/// error) is not a known difference.
#[test]
fn known_difference_with_a_different_message_is_red() {
    let text = committed_types();
    let mut expected = known_messages();
    let l = "cov-Usage/inputTokens=wrong";
    let m = expected.get_mut(l).expect("registered");
    assert!(m.starts_with("serde accepts, Swift rejects"), "{m}");
    *m = "serde accepts, Swift rejects (InputTokenDetails: some other error)".into();
    let r = report_with(&text, &expected);
    assert!(
        r.bad.iter().any(|b| b.starts_with(&format!("[S] {l}: "))),
        "{:#?}",
        r.bad
    );
    assert!(
        r.bad
            .iter()
            .any(|b| b.contains(&format!("stale known difference seq: {l}")))
    );
}

const EXPECTED_COUNTS: (usize, usize, usize, usize, usize, usize, usize) =
    (309, 1083, 17, 45, 261, 26, 2253);

/// Every `Defaulted<T>` field (a `#[serde(default)]` field whose Rust
/// `Default` the Swift side cannot spell) rejects `null` like serde, and is
/// one of the registered known differences (absent is not re-written).
#[test]
fn defaulted_fields_reject_null() {
    let swift = parse(&committed_types());
    let sim = Sim {
        swift: &swift,
        seen: Default::default(),
    };
    let mut found = BTreeSet::new();
    for (name, d) in &swift {
        let Decl::Struct(fs) = d else { continue };
        for f in fs.iter().filter(|f| f.ty.starts_with("Defaulted<")) {
            found.insert(format!("{name}.{}", f.key));
            let one = vec![f.clone()];
            let r = sim.record(name, &one, &json!({ f.key.clone(): null }), 0);
            assert!(r.is_err(), "{name}.{}: Swift accepts null: {r:?}", f.key);
            let r = sim.record(name, &one, &json!({}), 0).unwrap();
            assert_eq!(r, json!({}), "{name}.{}: absent stays absent", f.key);
        }
    }
    assert_eq!(
        found,
        KNOWN_DEFAULTED.iter().map(|s| (*s).to_string()).collect()
    );
}

/// The known differences "absent `#[serde(default)]` field without a Swift
/// default": Swift leaves the key out, serde writes `Default::default()`.
const KNOWN_DEFAULTED: &[&str] = &[
    "GenerateObjectResult.request",
    "LanguageModelUsage.inputTokenDetails",
    "LanguageModelUsage.outputTokenDetails",
    "ModelSpec.capabilities",
    "ModelSpec.limits",
    "ModelSpec.modalities",
    "ModelSpec.source",
    "ModelSpec.type",
    "StepResult.request",
];

/// Replace the first line matching `pick` at or after `after` with `f`.
fn mutate(
    text: &str,
    after: &str,
    pick: impl Fn(&str) -> bool,
    f: impl Fn(&str) -> String,
) -> String {
    let start = text.find(after).unwrap_or_else(|| panic!("anchor {after}"));
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
    assert!(done, "mutation after {after} found no line");
    format!("{head}{}\n", tail.join("\n"))
}

/// Swap the first lines containing `a` and `b` after `anchor`.
fn swap_after(t: &str, anchor: &str, a: &str, b: &str) -> String {
    let start = t.find(anchor).expect("anchor");
    let mut lines: Vec<&str> = t[start..].split_inclusive('\n').collect();
    let i = lines.iter().position(|l| l.contains(a)).expect("a");
    let j = lines.iter().position(|l| l.contains(b)).expect("b");
    lines.swap(i, j);
    format!("{}{}", &t[..start], lines.concat())
}

fn mutations(text: &str) -> Vec<(&'static str, String)> {
    let t = text;
    vec![
        (
            "old-codingkey",
            mutate(
                t,
                "public struct TaggedToolCall:",
                |l| l.contains("case toolCallId = \"toolCallId\""),
                |l| l.replace("\"toolCallId\"", "\"toolCallID\""),
            ),
        ),
        (
            "old-optional",
            mutate(
                t,
                "public struct TaggedToolCall:",
                |l| l.trim() == "public var toolName: String",
                |l| format!("{l}?"),
            ),
        ),
        (
            "old-encodeIfPresent",
            mutate(
                t,
                "public struct TaggedToolCall:",
                |l| l.contains("encodeIfPresent(providerExecuted"),
                |l| l.replace("encodeIfPresent", "encode"),
            ),
        ),
        (
            "old-literal-optional",
            mutate(
                t,
                "public struct TaggedToolCall:",
                |l| l.trim() == "public let typeTag = LitToolCall()",
                |_| "    public var typeTag: LitToolCall?".into(),
            ),
        ),
        (
            "A1-enum-raw",
            mutate(
                t,
                "public enum FinishReasonUnified:",
                |l| l.contains("= \"stop\""),
                |l| l.replace("\"stop\"", "\"stopp\""),
            ),
        ),
        (
            "A2-union-tag",
            mutate(
                t,
                "public indirect enum Source:",
                |l| l.contains("case \"url\":"),
                |l| l.replace("\"url\"", "\"URL\""),
            ),
        ),
        (
            "A3-required-decodeDefault",
            mutate(
                t,
                "public struct InputTokenDetails:",
                |l| l.contains("= try c.decode(UInt32.self"),
                |l| {
                    l.replace(
                        "c.decode(UInt32.self, forKey: .",
                        "c.decodeDefault(UInt32.self, forKey: .",
                    )
                    .strip_suffix(')')
                    .map(|x| format!("{x}, 0)"))
                    .unwrap()
                },
            ),
        ),
        (
            "A4-type-UInt32-String",
            mutate(
                t,
                "public struct InputTokenDetails:",
                |l| l.contains("UInt32"),
                |l| l.replace("UInt32", "String"),
            ),
        ),
        (
            "A5-tristate-decodeIfPresent",
            mutate(
                t,
                "public struct ApiCallError:",
                |l| l.contains("decodeTriState(") && l.contains(".data"),
                |l| l.replace("decodeTriState(", "decodeIfPresent("),
            ),
        ),
        (
            "G1-providerMetadata-decode",
            mutate(
                t,
                "public struct StreamPartTextDelta:",
                |l| l.contains("decodeIfPresent(") && l.contains(".providerMetadata"),
                |l| l.replace("decodeIfPresent(", "decode("),
            ),
        ),
        (
            "G3-literal-decode-deleted",
            mutate(
                t,
                "public struct TaggedSourceUrl:",
                |l| l.contains("c.contains(.typeTag)"),
                |_| String::new(),
            ),
        ),
        (
            "G4-id-Int",
            mutate(
                t,
                "public struct TaggedSourceUrl:",
                |l| l.contains("id = try c.decode(String.self"),
                |l| l.replace("String.self", "Int.self"),
            ),
        ),
        (
            "G5-typed-providerExecuted-encode",
            mutate(
                t,
                "public struct TypedToolCall:",
                |l| l.contains("encodeIfPresent(providerExecuted"),
                |l| l.replace("encodeIfPresent", "encode"),
            ),
        ),
        (
            "S1-custom-skip-check",
            mutate(
                t,
                "public struct InputRecord:",
                |l| l.contains("if operation != .languageModel"),
                |l| l.replace(".languageModel", ".decision"),
            ),
        ),
        (
            "D-defaulted-decodeIfPresent",
            mutate(
                t,
                "\npublic struct ",
                |l| l.contains(".decodeDefaulted(RequestInfo.self"),
                |l| l.replace("decodeDefaulted(", "decodeIfPresent("),
            ),
        ),
        (
            "B3-untagged-arm-order",
            swap_after(
                t,
                "public indirect enum FilePartData:",
                "if let v = try? c.decode(String.self)",
                "if let v = try? c.decode(SharedProviderReference.self)",
            ),
        ),
        (
            "B2-nonnull-decode",
            mutate(
                t,
                "public struct ToolResult:",
                |l| l.contains(".decodeNonNull("),
                |l| l.replace("decodeNonNull(", "decode("),
            ),
        ),
    ]
}

#[test]
fn each_semantic_check_catches_its_swift_mutation() {
    let text = committed_types();
    let mut lines = Vec::new();
    let mut green = Vec::new();
    for (name, m) in mutations(&text) {
        assert_ne!(m, text, "{name}: no change");
        let r = report(&m);
        let mut tags: BTreeMap<String, usize> = BTreeMap::new();
        for b in &r.bad {
            *tags
                .entry(b[..b.find(']').unwrap() + 1].to_string())
                .or_default() += 1;
        }
        if r.bad.is_empty() {
            green.push(name);
        }
        lines.push(format!("{name}: {} red {tags:?}", r.bad.len()));
    }
    eprintln!("{}", lines.join("\n"));
    assert!(
        green.is_empty(),
        "stayed green: {green:?}\n{}",
        lines.join("\n")
    );
}

/// Ported from #55 wire_diff_swift.rs:96/:1528 for v4 types: values serde
/// rejects, which the simulated Swift must reject too.
#[test]
fn reject_cases_are_rejected_by_serde_and_swift() {
    use aimux_core::model_catalogue::ModelModalities;
    use aimux_core::types::{LanguageModelUsage, Usage};
    let swift = parse(&committed_types());
    let sim = Sim {
        swift: &swift,
        seen: Default::default(),
    };
    let usage = json!({"inputTokens": {"total": 1}, "outputTokens": {"total": 1}});
    let with = |path: &[&str], v: Value| {
        let mut x = usage.clone();
        let mut at = &mut x;
        for k in &path[..path.len() - 1] {
            at = &mut at[*k];
        }
        at[path[path.len() - 1]] = v;
        x
    };
    let cases: Vec<(&str, &str, Value, Back)> = vec![
        (
            "null-skip-if-empty-ModelModalities.input",
            "ModelModalities",
            json!({"input": null}),
            back::<ModelModalities>,
        ),
        (
            "null-defaulted-LanguageModelUsage.inputTokenDetails",
            "LanguageModelUsage",
            json!({"inputTokenDetails": null}),
            back::<LanguageModelUsage>,
        ),
        (
            "int-overflow-u32",
            "Usage",
            with(&["inputTokens", "total"], json!(4_294_967_296_u64)),
            back::<Usage>,
        ),
        (
            "int-negative-u32",
            "Usage",
            with(&["inputTokens", "total"], json!(-1)),
            back::<Usage>,
        ),
        (
            "int-fraction-u32",
            "Usage",
            with(&["inputTokens", "total"], json!(1.5)),
            back::<Usage>,
        ),
    ];
    assert!(back::<Usage>(&usage).is_ok());
    assert!(sim.value("Usage", &usage, 0).is_ok());
    for (label, ty, v, b) in cases {
        assert!(b(&v).is_err(), "serde accepts {label}");
        let s = sim.value(ty, &v, 0);
        assert!(s.is_err(), "Swift accepts {label}: {s:?}");
    }
}

/// Ported from #55 wire_diff_swift.rs:1516: the `^public (struct|enum) X`
/// line reader misses one-line, nested and extension declarations (why
/// #55 read type names with `swiftc -dump-parse`; that reader,
/// :1507, needs swiftc and is 未运行 here).
#[test]
fn line_reader_misses_nested_extension_and_one_line() {
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
    let sample = "public struct A {}; public enum B {}\npublic struct Outer {\n    public struct Inner {}\n}\nextension Outer {\n    public enum Ext {}\n}\npublic typealias T = Int\n";
    let expected: BTreeSet<String> = ["A", "B", "Outer", "Outer.Inner", "Outer.Ext", "T"]
        .map(String::from)
        .into();
    let missed: BTreeSet<String> = expected.difference(&line_reader(sample)).cloned().collect();
    assert_eq!(
        missed,
        ["B", "Outer.Inner", "Outer.Ext", "T"]
            .map(String::from)
            .into()
    );
    // The generated Swift has none of those forms, so this file's own
    // parser (one top-level declaration per line) sees every type.
    let text = committed_types();
    assert!(
        !text.lines().any(|l| l.starts_with("extension ")
            || (l.starts_with("public ") && l.contains("}; public ")))
    );
    assert!(
        !text
            .lines()
            .any(|l| l.starts_with("    public struct ") || l.starts_with("    public enum "))
    );
}

#[test]
fn same_compares_numbers_by_value_only_on_float_paths() {
    // RerankingRank: `index: u32`, `relevanceScore: f64`
    // (aimux-core/src/reranking_model.rs:98-100).
    let t = json!({"index": 1, "relevanceScore": 1.0});
    assert!(same(
        "RerankingRank",
        &t,
        &json!({"index": 1, "relevanceScore": 1})
    ));
    assert!(!same(
        "RerankingRank",
        &t,
        &json!({"index": 1.0, "relevanceScore": 1.0})
    ));
    // 2^53 + 1 and 2^53 are one f64 but different integers.
    assert!(!same(
        "RerankingRank",
        &json!({"index": 9_007_199_254_740_993_u64, "relevanceScore": 0.5}),
        &json!({"index": 9_007_199_254_740_992_u64, "relevanceScore": 0.5}),
    ));
}

/// Top-level type names of a Swift source (`struct` / `enum` / `class` /
/// `actor` / `typealias` / `protocol` at brace depth 0), skipping comments
/// and string literals. Nested and `extension` members are not top-level.
/// (#55 read these with `swiftc -dump-parse`; Swift is not run here.)
fn top_level_types(src: &str) -> BTreeSet<String> {
    let b = src.as_bytes();
    let mut out = BTreeSet::new();
    let (mut i, mut depth) = (0usize, 0usize);
    let mut word = String::new();
    let mut prev = String::new();
    let mut flush = |w: &mut String, prev: &mut String, depth: usize| {
        if !w.is_empty() {
            if depth == 0
                && matches!(
                    prev.as_str(),
                    "struct" | "enum" | "class" | "actor" | "typealias" | "protocol"
                )
            {
                out.insert(w.clone());
            }
            *prev = std::mem::take(w);
        }
    };
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if c == b'"' {
            flush(&mut word, &mut prev, depth);
            let triple = b[i..].starts_with(b"\"\"\"");
            i += if triple { 3 } else { 1 };
            while i < b.len() {
                if b[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if triple && b[i..].starts_with(b"\"\"\"") {
                    i += 3;
                    break;
                }
                if !triple && b[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            prev.clear();
            continue;
        }
        if c.is_ascii_alphanumeric() || c == b'_' {
            word.push(c as char);
        } else {
            flush(&mut word, &mut prev, depth);
            match c {
                b'{' => depth += 1,
                b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        i += 1;
    }
    flush(&mut word, &mut prev, depth);
    out
}

#[test]
fn top_level_reader_sees_one_line_and_skips_nested_and_extension() {
    let sample = "public struct A {}; public enum B {}\npublic struct Outer {\n    public struct Inner {}\n}\nextension Outer {\n    public enum Ext {}\n}\npublic typealias T = Int\n// struct Commented {}\nlet s = \"struct InString {\"\n";
    let want: BTreeSet<String> = ["A", "B", "Outer", "T"].map(String::from).into();
    assert_eq!(top_level_types(sample), want);
}

/// #55 wire_diff_swift.rs:1127: every hand-written Swift type
/// (bindings/swift/Sources/Aimux, the files #55 read) with a generated
/// counterpart is simulated by at least one fixture.
#[test]
fn every_hand_written_type_with_a_generated_counterpart_has_a_fixture() {
    let dir = workspace().join("bindings/swift/Sources/Aimux");
    let mut hand = BTreeSet::new();
    for f in ["Types.swift", "Multimodal.swift", "DecisionModel.swift"] {
        hand.extend(top_level_types(
            &std::fs::read_to_string(dir.join(f)).unwrap(),
        ));
    }
    let text = committed_types();
    let swift = parse(&text);
    let m = manifest(&text);
    // A type is simulated when any fixture's simulation reaches it (as a
    // fixture's own type or nested inside one).
    let sim = Sim {
        swift: &swift,
        seen: Default::default(),
    };
    for (_, ty, j, _) in all_fixtures() {
        let _ = sim.value(&swift_name(&m, &swift, ty, &j), &j, 0);
    }
    let simulated = sim.seen.into_inner();
    let both: Vec<&String> = hand.iter().filter(|t| swift.contains_key(*t)).collect();
    assert!(!both.is_empty());
    let uncovered: Vec<&&String> = both.iter().filter(|t| !simulated.contains(**t)).collect();
    eprintln!("hand-written with a generated counterpart: {}", both.len());
    assert_eq!(uncovered, Vec::<&&String>::new());
}
