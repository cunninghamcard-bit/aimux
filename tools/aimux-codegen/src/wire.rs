//! Shared wire IR (unit 2.2): the extracted [`Model`] lowered into a
//! language-neutral description of what goes over the wire. Every emitter
//! (`crate::emit`) reads only this IR.
//!
//! - Types are sorted by name; union variants keep serde declaration order
//!   (untagged decoding tries them in that order, so order is semantic).
//! - Wire names are the manifest's as-is: no case conversion (decision D1 is
//!   open).
//! - `flatten` is expanded here: the flattened struct's fields are inlined.
//! - Presence has two dimensions, `omittable` (`skip_serializing_if` /
//!   `default`) and `nullable` (`Option`); a tri-state field is both plus the
//!   codec's `tri_state` flag.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::extract::{
    CodecWire, FieldDef, Model, Representation, Shape, TypeDef, TypeRef, parse_type_ref,
};

/// The whole IR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireModel {
    /// Sorted by `name`.
    pub types: Vec<WireType>,
}

/// One named wire type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireType {
    /// Unique IR name: the Rust name, or the `ts(rename)` name when the Rust
    /// name is defined in more than one file.
    pub name: String,
    /// Source file of the definition (e.g. `aimux-core/src/tool.rs`).
    pub doc_source: String,
    pub body: WireBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WireBody {
    /// A JSON object with named fields.
    Record { fields: Vec<WireField> },
    /// The wire is exactly this type (newtypes, aliases, proxies, codecs).
    Alias(WireTy),
    /// Externally tagged enum whose variants are all unit: a JSON string.
    /// `open`: a `#[serde(other)]` variant accepts unknown values.
    StringEnum { values: Vec<String>, open: bool },
    /// Any other enum. Variants in declaration order.
    Union {
        repr: UnionRepr,
        variants: Vec<WireVariant>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UnionRepr {
    External,
    Internal { tag: String },
    Adjacent { tag: String, content: String },
    Untagged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireVariant {
    pub rust: String,
    /// Tag value; `None` for an untagged variant.
    pub wire: Option<String>,
    pub aliases: Vec<String>,
    /// `#[serde(other)]` catch-all.
    pub other: bool,
    pub body: VariantBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VariantBody {
    Unit,
    /// Struct variant (flatten expanded).
    Fields(Vec<WireField>),
    /// Newtype variant: the payload is this type.
    Value(WireTy),
    /// Tuple variant: a JSON array.
    Tuple(Vec<WireTy>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireField {
    pub wire: String,
    /// Rust field name (for emitter identifiers only).
    pub rust: String,
    pub ty: WireTy,
    pub presence: Presence,
    pub aliases: Vec<String>,
}

/// Field presence. `omittable`: the key may be absent (`skip_serializing_if`
/// or `default`). `nullable`: the value may be `null` (`Option`).
/// `tri_state`: absent, `null` and a value are three distinct states.
/// `skip_if_empty`: serde omits the key when serializing an empty value
/// (`skip_serializing_if`), the only case a Go `omitempty` mirrors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Presence {
    pub omittable: bool,
    pub nullable: bool,
    pub tri_state: bool,
    pub skip_if_empty: bool,
}

/// A wire value type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WireTy {
    /// Rust primitive name (`String`, `bool`, `u32`, `f64`, ..).
    Prim(String),
    /// Any JSON value (also opaque codecs).
    Json,
    Array(Box<WireTy>),
    Map {
        key: Box<WireTy>,
        value: Box<WireTy>,
    },
    Tuple(Vec<WireTy>),
    /// Another IR type, by IR name.
    Named(String),
    /// `null` or the inner type, in a nested position (e.g. a map value);
    /// a field's own `Option` is [`Presence::nullable`] instead.
    Nullable(Box<WireTy>),
    /// A fixed string (`#[ts(type = "\"lit\"")]`, e.g. the `type` tag of
    /// v4 `TypedToolCall` / `TypedSource` / `TaggedToolResult`): always
    /// written with exactly this value; on input absent or this value
    /// (`crate::wire::untag`), anything else is rejected.
    Literal(String),
}

struct Ctx<'a> {
    /// (source, rust name) -> IR name.
    names: BTreeMap<(String, String), String>,
    /// rust name -> IR names defined under it.
    by_rust: BTreeMap<String, Vec<String>>,
    /// IR name -> definition.
    defs: BTreeMap<String, &'a TypeDef>,
    aliases: &'a BTreeMap<String, crate::extract::AliasDef>,
    errors: Vec<String>,
}

/// The body `aimux_core::wire::Tagged` writes for a [`TypeDef::type_tag`]
/// type: a fixed `type` field first, then the value's own fields (for an
/// internally tagged union, in every struct variant).
fn tagged(body: &WireBody, tag: &str) -> Option<WireBody> {
    let lit = WireField {
        wire: "type".into(),
        rust: "type_tag".into(),
        ty: WireTy::Literal(tag.into()),
        presence: Presence {
            omittable: true,
            ..Presence::default()
        },
        aliases: vec![],
    };
    let with = |fields: &[WireField]| {
        std::iter::once(lit.clone())
            .chain(fields.iter().filter(|f| f.wire != "type").cloned())
            .collect::<Vec<_>>()
    };
    match body {
        WireBody::Record { fields } => Some(WireBody::Record {
            fields: with(fields),
        }),
        WireBody::Union {
            repr: repr @ UnionRepr::Internal { .. },
            variants,
        } => Some(WireBody::Union {
            repr: repr.clone(),
            variants: variants
                .iter()
                .map(|v| {
                    let mut v = v.clone();
                    v.body = match &v.body {
                        VariantBody::Fields(f) => VariantBody::Fields(with(f)),
                        VariantBody::Unit => VariantBody::Fields(with(&[])),
                        other => other.clone(),
                    };
                    v
                })
                .collect(),
        }),
        _ => None,
    }
}

/// Lower the extracted model.
///
/// # Errors
///
/// A Rust name defined in several files without distinct `ts(rename)`
/// names, a `flatten` whose target is not a struct, a tri-state field
/// without `skip_serializing_if`, or a `TypeTag` on a non-record type.
pub fn lower(model: &Model) -> Result<WireModel, Vec<String>> {
    let all: Vec<&TypeDef> = model.types.iter().chain(&model.auxiliary_types).collect();
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for t in &all {
        *count.entry(t.name.as_str()).or_default() += 1;
    }
    let mut ctx = Ctx {
        names: BTreeMap::new(),
        by_rust: BTreeMap::new(),
        defs: BTreeMap::new(),
        aliases: &model.aliases,
        errors: Vec::new(),
    };
    for t in &all {
        let ir = if count[t.name.as_str()] > 1 {
            t.ts_name.clone().unwrap_or_else(|| t.name.clone())
        } else {
            t.name.clone()
        };
        if ctx.defs.contains_key(&ir) || (ir != t.name && count.contains_key(ir.as_str())) {
            ctx.errors.push(format!(
                "{}: wire name `{ir}` for {} is not unique",
                t.source, t.name
            ));
            continue;
        }
        ctx.names
            .insert((t.source.clone(), t.name.clone()), ir.clone());
        ctx.by_rust
            .entry(t.name.clone())
            .or_default()
            .push(ir.clone());
        ctx.defs.insert(ir, t);
    }
    let mut types = Vec::new();
    for (ir, t) in ctx.defs.clone() {
        let body = ctx.body(t);
        if let Some(tag) = &t.type_tag {
            // The standalone (FFI) form: `aimux_core::wire::Tagged`.
            let name = format!("Tagged{ir}");
            match tagged(&body, tag) {
                Some(_) if ctx.defs.contains_key(&name) => ctx.errors.push(format!(
                    "{}: wire name `{name}` (TypeTag form of {}) is not unique",
                    t.source, t.name
                )),
                Some(b) => types.push(WireType {
                    name,
                    doc_source: t.source.clone(),
                    body: b,
                }),
                None => ctx.errors.push(format!(
                    "{}: {}: `TypeTag` on a type that is not a record or an internally tagged union",
                    t.source, t.name
                )),
            }
        }
        types.push(WireType {
            name: ir,
            doc_source: t.source.clone(),
            body,
        });
    }
    for (name, a) in &model.aliases {
        if ctx.defs.contains_key(name) {
            continue;
        }
        let ty = ctx.ty(&a.ty, &a.source, &BTreeMap::new());
        types.push(WireType {
            name: name.clone(),
            doc_source: a.source.clone(),
            body: WireBody::Alias(ty),
        });
    }
    types.sort_by(|a, b| a.name.cmp(&b.name));
    if ctx.errors.is_empty() {
        Ok(WireModel { types })
    } else {
        Err(ctx.errors)
    }
}

impl<'a> Ctx<'a> {
    fn resolve(&self, name: &str, source: &str) -> String {
        if let Some(ir) = self.names.get(&(source.to_string(), name.to_string())) {
            return ir.clone();
        }
        match self.by_rust.get(name).map(Vec::as_slice) {
            Some([one]) => one.clone(),
            _ => name.to_string(),
        }
    }

    fn concrete(t: &TypeDef) -> BTreeMap<String, TypeRef> {
        t.concrete
            .iter()
            .filter_map(|(k, v)| parse_type_ref(v).map(|r| (k.clone(), r)))
            .collect()
    }

    fn body(&mut self, t: &TypeDef) -> WireBody {
        let conc = Self::concrete(t);
        match &t.shape {
            Shape::Enum {
                representation,
                variants,
            } => {
                let repr = match representation {
                    Representation::External => UnionRepr::External,
                    Representation::Internal { tag } => UnionRepr::Internal { tag: tag.clone() },
                    Representation::Adjacent { tag, content } => UnionRepr::Adjacent {
                        tag: tag.clone(),
                        content: content.clone(),
                    },
                    Representation::Untagged => UnionRepr::Untagged,
                };
                let all_unit = variants
                    .iter()
                    .all(|v| matches!(v.shape, Shape::Unit) && v.wire.is_some());
                if matches!(repr, UnionRepr::External) && all_unit && !variants.is_empty() {
                    return WireBody::StringEnum {
                        values: variants.iter().filter_map(|v| v.wire.clone()).collect(),
                        open: variants.iter().any(|v| v.other),
                    };
                }
                let variants = variants
                    .iter()
                    .map(|v| WireVariant {
                        rust: v.rust.clone(),
                        wire: v.wire.clone(),
                        aliases: v.aliases.clone(),
                        other: v.other,
                        body: match &v.shape {
                            Shape::Struct { fields, .. } => {
                                VariantBody::Fields(self.fields(t, fields, &conc))
                            }
                            Shape::Newtype { inner } => {
                                VariantBody::Value(self.ty(inner, &t.source, &conc))
                            }
                            Shape::Tuple { items } => VariantBody::Tuple(
                                items.iter().map(|i| self.ty(i, &t.source, &conc)).collect(),
                            ),
                            _ => VariantBody::Unit,
                        },
                    })
                    .collect();
                WireBody::Union { repr, variants }
            }
            other => self.plain(t, other, &conc),
        }
    }

    /// The internally tagged enum a struct field flattens (v4 `TypedSource`,
    /// result.rs:325, over `Source`), if any.
    fn flattened_union(&self, owner: &TypeDef, f: &FieldDef) -> Option<&'a TypeDef> {
        let TypeRef::Ref { name, .. } = &f.ty else {
            return None;
        };
        let def = self.defs.get(&self.resolve(name, &owner.source)).copied()?;
        matches!(
            def.shape,
            Shape::Enum {
                representation: Representation::Internal { .. },
                ..
            }
        )
        .then_some(def)
    }

    fn plain(&mut self, t: &TypeDef, shape: &Shape, conc: &BTreeMap<String, TypeRef>) -> WireBody {
        match shape {
            Shape::Struct { fields, .. } => {
                // A struct that flattens an internally tagged enum is that
                // union with the struct's other fields added to every variant.
                if let Some((i, def)) = fields
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.flatten)
                    .find_map(|(i, f)| self.flattened_union(t, f).map(|d| (i, d)))
                {
                    let rest: Vec<FieldDef> = fields
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, f)| f.clone())
                        .collect();
                    let extra = self.fields(t, &rest, conc);
                    let WireBody::Union { repr, variants } = self.body(def) else {
                        unreachable!("internally tagged enum lowers to a union");
                    };
                    let variants = variants
                        .into_iter()
                        .map(|mut v| {
                            v.body = match v.body {
                                VariantBody::Fields(own) => {
                                    VariantBody::Fields(extra.iter().cloned().chain(own).collect())
                                }
                                VariantBody::Unit => VariantBody::Fields(extra.clone()),
                                other => other,
                            };
                            v
                        })
                        .collect();
                    return WireBody::Union { repr, variants };
                }
                WireBody::Record {
                    fields: self.fields(t, fields, conc),
                }
            }
            Shape::Newtype { inner } => WireBody::Alias(self.ty(inner, &t.source, conc)),
            Shape::Tuple { items } => WireBody::Alias(WireTy::Tuple(
                items.iter().map(|i| self.ty(i, &t.source, conc)).collect(),
            )),
            // A unit struct is `null`; hand-written serde is opaque here.
            Shape::Unit | Shape::HandWritten { .. } | Shape::Enum { .. } => {
                WireBody::Alias(WireTy::Json)
            }
        }
    }

    fn fields(
        &mut self,
        owner: &TypeDef,
        fields: &[FieldDef],
        conc: &BTreeMap<String, TypeRef>,
    ) -> Vec<WireField> {
        let mut out = Vec::new();
        for f in fields {
            if f.flatten {
                out.extend(self.flattened(owner, f, 0));
                continue;
            }
            let w = self.field(f, &owner.source, conc);
            self.check_tri_state(owner, f, &w);
            out.push(w);
        }
        out
    }

    /// A tri-state field (`null.input = distinct`) must also be skipped when
    /// `None` (`skip_serializing_if`): the Go `TriState[T]` + `omitzero`
    /// (emit/go.rs) writes the absent state by omitting the key, which is
    /// only the Rust output when serde skips it too.
    fn check_tri_state(&mut self, owner: &TypeDef, f: &FieldDef, w: &WireField) {
        if w.presence.tri_state && !w.presence.skip_if_empty {
            self.errors.push(format!(
                "{}:{}: {}.{}: tri-state field (null input `distinct`) without `skip_serializing_if`: absent and null cannot both round-trip",
                owner.source, f.line, owner.name, f.rust
            ));
        }
    }

    fn flattened(&mut self, owner: &TypeDef, f: &FieldDef, depth: usize) -> Vec<WireField> {
        let mut ty = f.ty.clone();
        while let TypeRef::Ref { name, args } = &ty {
            match self.aliases.get(name) {
                Some(a) if args.is_empty() => ty = a.ty.clone(),
                _ => break,
            }
        }
        if let TypeRef::Ref { name, .. } = &ty {
            let ir = self.resolve(name, &owner.source);
            if let Some(def) = self.defs.get(&ir).copied()
                && let Shape::Struct { fields, .. } = &def.shape
                && depth < 16
            {
                let conc = Self::concrete(def);
                let mut out = Vec::new();
                for inner in fields {
                    if inner.flatten {
                        out.extend(self.flattened(def, inner, depth + 1));
                    } else {
                        let w = self.field(inner, &def.source, &conc);
                        self.check_tri_state(def, inner, &w);
                        out.push(w);
                    }
                }
                return out;
            }
        }
        self.errors.push(format!(
            "{}:{}: {}.{}: `flatten` target is not a struct",
            owner.source, f.line, owner.name, f.rust
        ));
        Vec::new()
    }

    fn field(&self, f: &FieldDef, source: &str, conc: &BTreeMap<String, TypeRef>) -> WireField {
        let mut presence = Presence {
            omittable: f.omit_when_empty || f.default,
            nullable: false,
            tri_state: false,
            skip_if_empty: f.omit_when_empty,
        };
        // Tri-state follows the manifest's input fact: `distinct` means an
        // absent key and `null` decode differently (`present` on
        // `Option<Option<T>>`, and on `Option<Value>`, where null is
        // `Some(Value::Null)`). A plain pointer would collapse the two.
        let distinct = f.null.input == "distinct";
        let inner = match &f.ty {
            TypeRef::Option(i) => {
                presence.nullable = true;
                presence.tri_state = distinct;
                i.as_ref().clone()
            }
            TypeRef::TriState(i) => {
                presence.nullable = true;
                presence.tri_state = f.codec.is_some() || distinct;
                i.as_ref().clone()
            }
            other => other.clone(),
        };
        if let TypeRef::Codec {
            wire: CodecWire::Literal(lit),
            ..
        } = &inner
        {
            // Read with the tag optional, never skipped when written.
            presence.omittable = true;
            presence.skip_if_empty = false;
            return WireField {
                wire: f.wire.clone(),
                rust: f.rust.clone(),
                ty: WireTy::Literal(lit.clone()),
                presence,
                aliases: f.aliases.clone(),
            };
        }
        let ty = if let TypeRef::Codec {
            wire: CodecWire::Type(declared),
            ..
        } = &inner
        {
            // `#[ts(as = ..)]` declares the wire type (v4 `TypedToolCall`
            // for a `ToolCall` written by `crate::wire::typed`).
            self.ty(declared, source, conc)
        } else if f.serde_with.is_some() && matches!(inner, TypeRef::Codec { .. }) {
            // A custom serde fn with a declared TS type: the Rust type is not
            // the wire, so the value stays opaque.
            WireTy::Json
        } else {
            self.ty(&inner, source, conc)
        };
        WireField {
            wire: f.wire.clone(),
            rust: f.rust.clone(),
            ty,
            presence,
            aliases: f.aliases.clone(),
        }
    }

    fn ty(&self, t: &TypeRef, source: &str, conc: &BTreeMap<String, TypeRef>) -> WireTy {
        match t {
            TypeRef::Prim(p) => WireTy::Prim(p.clone()),
            TypeRef::Json => WireTy::Json,
            TypeRef::Option(i) | TypeRef::TriState(i) => {
                WireTy::Nullable(Box::new(self.ty(i, source, conc)))
            }
            TypeRef::Array(i) => WireTy::Array(Box::new(self.ty(i, source, conc))),
            TypeRef::Map { key, value } => WireTy::Map {
                key: Box::new(self.ty(key, source, conc)),
                value: Box::new(self.ty(value, source, conc)),
            },
            TypeRef::Tuple(items) => {
                WireTy::Tuple(items.iter().map(|i| self.ty(i, source, conc)).collect())
            }
            TypeRef::Ref { name, .. } => WireTy::Named(self.resolve(name, source)),
            TypeRef::Generic(g) => conc
                .get(g)
                .map_or(WireTy::Json, |c| self.ty(c, source, &BTreeMap::new())),
            // The codec's Rust type, when it is structural or a known type;
            // otherwise opaque JSON.
            TypeRef::Codec {
                wire: CodecWire::Type(declared),
                ..
            } => self.ty(declared, source, conc),
            TypeRef::Codec {
                wire: CodecWire::Literal(lit),
                ..
            } => WireTy::Literal(lit.clone()),
            TypeRef::Codec { rust, .. } => match parse_type_ref(rust) {
                Some(TypeRef::Ref { name, .. }) => {
                    let ir = self.resolve(&name, source);
                    if self.defs.contains_key(&ir) {
                        WireTy::Named(ir)
                    } else {
                        WireTy::Json
                    }
                }
                Some(r) => self.ty(&r, source, conc),
                None => WireTy::Json,
            },
        }
    }
}

/// Extract `aimux-core/src` (as the manifest does) and lower it.
///
/// # Errors
///
/// Extraction or lowering errors.
pub fn from_inputs(inputs: &crate::manifest::Inputs) -> Result<WireModel, Vec<String>> {
    let model = crate::extract::extract(&inputs.src_root, crate::manifest::SRC_PREFIX)?;
    lower(&model)
}

/// Names referenced by a type (for emitters that need closure checks).
#[must_use]
pub fn referenced(t: &WireTy) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    fn walk(t: &WireTy, out: &mut BTreeSet<String>) {
        match t {
            WireTy::Named(n) => {
                out.insert(n.clone());
            }
            WireTy::Array(i) | WireTy::Nullable(i) => walk(i, out),
            WireTy::Map { key, value } => {
                walk(key, out);
                walk(value, out);
            }
            WireTy::Tuple(items) => items.iter().for_each(|i| walk(i, out)),
            WireTy::Prim(_) | WireTy::Json | WireTy::Literal(_) => {}
        }
    }
    walk(t, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::extract::{AliasDef, VariantDef};
    use crate::manifest::Inputs;

    fn real() -> WireModel {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        from_inputs(&Inputs::from_workspace(&root)).unwrap()
    }

    fn get<'a>(m: &'a WireModel, name: &str) -> &'a WireType {
        m.types.iter().find(|t| t.name == name).unwrap()
    }

    fn field(wire: &str, rust: &str, ty: TypeRef) -> FieldDef {
        FieldDef {
            wire: wire.into(),
            rust: rust.into(),
            ty,
            omit_when_empty: false,
            default: false,
            flatten: false,
            aliases: vec![],
            serde_with: None,
            codec: None,
            omitted_when: None,
            null: crate::extract::NullHandling {
                input: "rejected",
                output: "never",
            },
            line: 1,
        }
    }

    fn def(name: &str, shape: Shape) -> TypeDef {
        TypeDef {
            name: name.into(),
            ts_name: None,
            category: "core".into(),
            source: "x.rs".into(),
            generics: vec![],
            concrete: BTreeMap::new(),
            proxy: BTreeMap::new(),
            ts_override: None,
            type_tag: None,
            shape,
        }
    }

    fn record(fields: Vec<FieldDef>) -> Shape {
        Shape::Struct {
            fields,
            deny_unknown_fields: false,
            skipped: vec![],
        }
    }

    fn rf(name: &str) -> TypeRef {
        TypeRef::Ref {
            name: name.into(),
            args: vec![],
        }
    }

    #[test]
    fn types_are_sorted_and_unique() {
        let m = real();
        let names: Vec<&str> = m.types.iter().map(|t| t.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        // v4 manifest: 210 types + 2 auxiliary + 10 aliases, plus the 4
        // `Tagged*` forms of the `TypeTag` types (ToolCall, RawToolCall,
        // Source, TypedToolResult).
        assert_eq!(m.types.len(), 226);
        let tagged: Vec<&str> = names
            .iter()
            .copied()
            .filter(|n| {
                [
                    "TaggedToolCall",
                    "TaggedRawToolCall",
                    "TaggedSource",
                    "TaggedTypedToolResult",
                ]
                .contains(n)
            })
            .collect();
        assert_eq!(tagged.len(), 4);
    }

    #[test]
    fn duplicate_rust_name_uses_ts_rename() {
        let m = real();
        assert_eq!(
            get(&m, "ReasoningPart").doc_source,
            "aimux-core/src/result.rs"
        );
        assert_eq!(
            get(&m, "LanguageModelReasoningPart").doc_source,
            "aimux-core/src/language_model_message.rs"
        );
    }

    #[test]
    fn variants_keep_declaration_order() {
        let m = real();
        // decision_model.rs: `Text, Object, Array` (not alphabetical).
        let WireBody::Union { repr, variants } = &get(&m, "DecisionInput").body else {
            panic!()
        };
        assert_eq!(repr, &UnionRepr::Untagged);
        let order: Vec<&str> = variants.iter().map(|v| v.rust.as_str()).collect();
        assert_eq!(order, ["Text", "Object", "Array"], "not sorted");
    }

    #[test]
    fn wire_names_are_used_as_is() {
        let m = real();
        let WireBody::Record { fields } = &get(&m, "FunctionTool").body else {
            panic!()
        };
        assert!(
            fields
                .iter()
                .any(|f| f.wire == "inputSchema" && f.rust == "input_schema")
        );
        let WireBody::Record { fields } = &get(&m, "ChatCompletionMessage").body else {
            panic!()
        };
        assert!(fields.iter().any(|f| f.wire == "reasoning_content"));
    }

    #[test]
    fn presence_dimensions() {
        let m = real();
        let WireBody::Record { fields } = &get(&m, "DecisionBooleanCriteria").body else {
            panic!()
        };
        assert_eq!(
            fields[0].presence,
            Presence {
                omittable: true,
                nullable: true,
                tri_state: true,
                skip_if_empty: true
            }
        );
        assert_eq!(fields[0].ty, WireTy::Named("DecisionInput".into()));
        let WireBody::Record { fields } = &get(&m, "ChatCompletionMessage").body else {
            panic!()
        };
        let content = fields.iter().find(|f| f.wire == "content").unwrap();
        assert_eq!(
            content.presence,
            Presence {
                nullable: true,
                ..Presence::default()
            },
            "Option without skip_serializing_if: null, never absent"
        );
    }

    #[test]
    fn string_enum_and_proxy_alias() {
        let m = real();
        assert!(matches!(
            &get(&m, "FinishReasonUnified").body,
            WireBody::StringEnum { values, .. } if values.contains(&"tool-calls".to_string())
        ));
        assert_eq!(
            get(&m, "Size").body,
            WireBody::Alias(WireTy::Prim("String".into()))
        );
    }

    #[test]
    fn generic_parameters_use_concrete() {
        let m = real();
        let WireBody::Union { variants, .. } = &get(&m, "StreamPart").body else {
            panic!()
        };
        let text = serde_json::to_string(variants).unwrap();
        assert!(text.contains("RawToolCall"), "{text}");
        assert!(!text.contains("\"named\":\"C\""), "{text}");
    }

    #[test]
    fn flatten_is_expanded() {
        let inner = def(
            "Inner",
            record(vec![field("b", "b", TypeRef::Prim("u32".into()))]),
        );
        let mut flat = field("ignored", "inner", rf("InnerAlias"));
        flat.flatten = true;
        let outer = def(
            "Outer",
            record(vec![field("a", "a", TypeRef::Prim("String".into())), flat]),
        );
        let mut aliases = BTreeMap::new();
        aliases.insert(
            "InnerAlias".to_string(),
            AliasDef {
                source: "x.rs".into(),
                ty: rf("Inner"),
            },
        );
        let m = lower(&Model {
            types: vec![inner, outer],
            auxiliary_types: vec![],
            aliases,
        })
        .unwrap();
        let WireBody::Record { fields } = &get(&m, "Outer").body else {
            panic!()
        };
        let wires: Vec<&str> = fields.iter().map(|f| f.wire.as_str()).collect();
        assert_eq!(wires, ["a", "b"]);
    }

    /// A tri-state field without `skip_serializing_if` cannot round-trip
    /// through `TriState[T]` + `omitzero` (emit/go.rs): generation error.
    #[test]
    fn tri_state_without_skip_is_an_error() {
        let mut f = field("body", "body", TypeRef::Option(Box::new(TypeRef::Json)));
        f.default = true;
        f.serde_with = Some("deserialize_with=present".into());
        f.null = crate::extract::NullHandling {
            input: "distinct",
            output: "asValue",
        };
        f.line = 9;
        let model = |f: FieldDef| Model {
            types: vec![def("R", record(vec![f]))],
            auxiliary_types: vec![],
            aliases: BTreeMap::new(),
        };
        let err = lower(&model(f.clone())).unwrap_err();
        assert_eq!(
            err,
            [
                "x.rs:9: R.body: tri-state field (null input `distinct`) without `skip_serializing_if`: absent and null cannot both round-trip"
            ]
        );
        // With `skip_serializing_if = "Option::is_none"` it lowers.
        f.omit_when_empty = true;
        f.omitted_when = Some("Option::is_none".into());
        let m = lower(&model(f)).unwrap();
        let WireBody::Record { fields } = &get(&m, "R").body else {
            panic!()
        };
        assert!(fields[0].presence.tri_state && fields[0].presence.skip_if_empty);
    }

    #[test]
    fn flatten_of_non_struct_is_an_error() {
        let mut flat = field("x", "x", TypeRef::Prim("u32".into()));
        flat.flatten = true;
        flat.line = 7;
        let err = lower(&Model {
            types: vec![def("Outer", record(vec![flat]))],
            auxiliary_types: vec![],
            aliases: BTreeMap::new(),
        })
        .unwrap_err();
        assert_eq!(err, ["x.rs:7: Outer.x: `flatten` target is not a struct"]);
    }

    #[test]
    fn all_unit_internal_enum_stays_a_union() {
        let shape = Shape::Enum {
            representation: Representation::Internal { tag: "t".into() },
            variants: vec![VariantDef {
                rust: "A".into(),
                wire: Some("a".into()),
                aliases: vec![],
                other: false,
                shape: Shape::Unit,
            }],
        };
        let m = lower(&Model {
            types: vec![def("E", shape)],
            auxiliary_types: vec![],
            aliases: BTreeMap::new(),
        })
        .unwrap();
        assert!(matches!(get(&m, "E").body, WireBody::Union { .. }));
    }

    #[test]
    /// `DecisionQuestion::Score.criteria: Vec<Option<DecisionInput>>`: an inner
    /// `Option` lowers to `WireTy::Nullable`.
    fn nested_option_is_nullable() {
        let m = real();
        let text = serde_json::to_string(&get(&m, "DecisionQuestion").body).unwrap();
        assert!(text.contains("nullable"), "{text}");
    }
}
