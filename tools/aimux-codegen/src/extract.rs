//! Type model extracted from the Rust sources (unit 2.1; RFC-0042:200 "Rust
//! types are the protocol source"). Every `#[derive(.., TS, ..)]` item that
//! also carries `#[ts(export ..)]` becomes one [`TypeDef`]. The serde
//! attributes decide the wire shape: field/variant renames, `rename_all`,
//! enum representation (external / `tag` / `tag`+`content` / `untagged`),
//! `skip`, `default`, `skip_serializing_if`, `flatten`, proxies
//! (`from`/`into`/`try_from`).
//!
//! A field type is accepted only when it is a primitive, a known container,
//! opaque JSON, a generic parameter, another exported type, or a registered
//! codec (an explicit `#[ts(type|as = ..)]`, or an entry in [`CODECS`]).
//! Anything else fails generation with `file:line`; it never degrades to
//! `any` (RFC-0042:200).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use quote::ToTokens;
use serde::Serialize;
use syn::spanned::Spanned;
use syn::{Attribute, Fields, GenericArgument, Item, PathArguments, Type};

/// Category per source file (planner Q2). Mirrors `CATEGORY_BY_FILE` in
/// `scripts/gen_closure_inventory.py`; the coverage report compares
/// `<name> <category> <file>` lines, so the two tables cannot drift apart.
pub const CATEGORY_BY_FILE: &[(&str, &str)] = &[
    ("aimux-core/src/error.rs", "error"),
    ("aimux-core/src/recording.rs", "recording"),
    ("aimux-core/src/trace/fingerprint.rs", "recording"),
    ("aimux-core/src/trace/record.rs", "recording"),
    ("aimux-core/src/trace/store.rs", "recording"),
    ("aimux-core/src/trace/verdict.rs", "recording"),
    ("aimux-core/src/tool.rs", "tool"),
    ("aimux-core/src/parse_tool_call.rs", "tool"),
    ("aimux-core/src/content.rs", "multimodal"),
    ("aimux-core/src/embedding_model.rs", "multimodal"),
    ("aimux-core/src/files_model.rs", "multimodal"),
    ("aimux-core/src/image_model.rs", "multimodal"),
    ("aimux-core/src/speech_model.rs", "multimodal"),
    ("aimux-core/src/transcription_model.rs", "multimodal"),
    ("aimux-core/src/video_model.rs", "multimodal"),
    ("aimux-core/src/model_catalogue.rs", "provider"),
    ("aimux-core/src/model_id.rs", "provider"),
    ("aimux-core/src/message.rs", "user"),
    ("aimux-core/src/language_model_message.rs", "user"),
    ("aimux-core/src/options.rs", "user"),
];

/// Category for files not in [`CATEGORY_BY_FILE`].
pub const DEFAULT_CATEGORY: &str = "core";

/// Registered codecs for Rust types that are not exported DTOs, by the last
/// path segment. Each one names its wire form; adding a type here is the
/// explicit registration RFC-0042:200 asks for.
pub const CODECS: &[(&str, &str)] = &[];

/// A registered codec for a field that uses serde `with` /
/// `deserialize_with` / `serialize_with` without a `#[ts(type)]`. Such a
/// field is accepted only when `(owner, variant, field, serde)` matches an
/// entry here; otherwise generation fails with exactly one `file:line` error.
#[derive(Debug, Clone, Copy)]
pub struct FieldCodec {
    /// Containing type.
    pub owner: &'static str,
    /// Enum variant holding the field (`""` for a struct field).
    pub variant: &'static str,
    /// Rust field name (without `r#`).
    pub field: &'static str,
    /// The serde attribute as `<key>=<fn>`.
    pub serde: &'static str,
    /// Codec id written into the manifest.
    pub name: &'static str,
    /// Wire contract.
    pub wire: &'static str,
    /// The field's manifest type becomes [`TypeRef::TriState`]
    /// (`Option<Option<T>>` → absent | null | T).
    pub tri_state: bool,
}

/// Field codec registrations (unit 2.1).
pub const FIELD_CODECS: &[FieldCodec] = &[
    // aimux-core/src/recording.rs:73
    FieldCodec {
        owner: "Recording",
        variant: "",
        field: "schema",
        serde: "deserialize_with=deserialize_current_schema",
        name: "currentRecordingSchema",
        wire: "u32; decode accepts only RECORDING_SCHEMA and rejects any other value",
        tri_state: false,
    },
    // aimux-core/src/evaluation_model.rs:62-73
    FieldCodec {
        owner: "BooleanCriteria",
        variant: "",
        field: "true_",
        serde: "deserialize_with=present",
        name: "presence",
        wire: "absent | null | value: an absent key and an explicit null are distinct",
        tri_state: true,
    },
    FieldCodec {
        owner: "BooleanCriteria",
        variant: "",
        field: "false_",
        serde: "deserialize_with=present",
        name: "presence",
        wire: "absent | null | value: an absent key and an explicit null are distinct",
        tri_state: true,
    },
];

/// One exported type.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDef {
    pub name: String,
    /// Name in the current TypeScript export (`#[ts(rename)]`), when it differs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts_name: Option<String>,
    pub category: String,
    pub source: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub generics: Vec<String>,
    /// `#[ts(concrete(..))]` substitutions used by the current export.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub concrete: BTreeMap<String, String>,
    /// `#[serde(from|into|try_from = ..)]`: the wire goes through this type.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub proxy: BTreeMap<String, TypeRef>,
    /// `#[ts(as|type = ..)]` on the container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts_override: Option<String>,
    pub shape: Shape,
}

/// Wire shape of a type.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Shape {
    #[serde(rename_all = "camelCase")]
    Struct {
        fields: Vec<FieldDef>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        deny_unknown_fields: bool,
        /// Rust fields with `#[serde(skip)]`: never on the wire.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        skipped: Vec<String>,
    },
    /// Tuple struct with one field: the wire is the inner value.
    Newtype {
        inner: TypeRef,
    },
    Tuple {
        items: Vec<TypeRef>,
    },
    Unit,
    /// serde is implemented by hand (no `derive(Serialize/Deserialize)`):
    /// the wire is the declared `#[ts(type|as)]` codec only.
    HandWritten {
        wire: String,
    },
    /// Discriminated or untagged union.
    #[serde(rename_all = "camelCase")]
    Enum {
        representation: Representation,
        variants: Vec<VariantDef>,
    },
}

/// serde enum representation.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "style", rename_all = "camelCase")]
pub enum Representation {
    External,
    Internal { tag: String },
    Adjacent { tag: String, content: String },
    Untagged,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantDef {
    pub rust: String,
    /// Discriminator value on the wire (absent for untagged).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// `#[serde(other)]` catch-all.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub other: bool,
    pub shape: Shape,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDef {
    pub wire: String,
    pub rust: String,
    #[serde(rename = "type")]
    pub ty: TypeRef,
    /// May be absent on the wire when serialized (`skip_serializing_if`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub omit_when_empty: bool,
    /// Absent on input is accepted (`default`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flatten: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// `#[serde(with|deserialize_with|serialize_with = ..)]` custom codec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serde_with: Option<String>,
    /// Registered [`FieldCodec`] for a `serde_with` field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<FieldCodecRef>,
    /// Source line of the field (for diagnostics; not in the manifest).
    #[serde(skip)]
    pub line: usize,
}

/// The registered codec used by a field.
#[derive(Debug, Clone, Serialize)]
pub struct FieldCodecRef {
    pub name: String,
    pub wire: String,
}

/// A field type.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TypeRef {
    Prim(String),
    Json,
    Option(Box<TypeRef>),
    Array(Box<TypeRef>),
    Map {
        key: Box<TypeRef>,
        value: Box<TypeRef>,
    },
    Tuple(Vec<TypeRef>),
    Ref {
        name: String,
        args: Vec<TypeRef>,
    },
    Generic(String),
    /// Explicit codec: Rust type text plus its declared wire form.
    Codec {
        rust: String,
        wire: String,
    },
    /// Three wire states: key absent, explicit `null`, or a value of the
    /// inner type (Rust `Option<Option<T>>` with a presence codec).
    TriState(Box<TypeRef>),
}

const PRIMS: &[&str] = &[
    "String", "str", "bool", "char", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16",
    "i32", "i64", "i128", "isize", "f32", "f64",
];

/// Raw item before reference resolution.
struct Found {
    def: TypeDef,
    line: usize,
    exported: bool,
}

/// A non-generic `type X = ..;` alias referenced by DTO fields.
#[derive(Debug, Clone, Serialize)]
pub struct AliasDef {
    pub source: String,
    #[serde(rename = "type")]
    pub ty: TypeRef,
}

/// Everything extracted from the sources.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// `#[ts(export)]` types: the coverage set.
    pub types: Vec<TypeDef>,
    /// `derive(TS)` types without export that an exported type reaches
    /// (e.g. the `AiMuxError` wire proxy). Listed in the
    /// `[derive-ts-without-export]` section of closure-dtos.txt.
    pub auxiliary_types: Vec<TypeDef>,
    pub aliases: BTreeMap<String, AliasDef>,
}

/// Every exported type under `src_root` (an `aimux-core/src` directory),
/// sorted by `(name, source)`. `rel_prefix` is the path prefix written into
/// `source` (e.g. `aimux-core/src`).
///
/// # Errors
///
/// One message per unreadable file, parse error, unsupported construct or
/// unregistered field type, each with `file:line`.
pub fn extract(src_root: &Path, rel_prefix: &str) -> Result<Model, Vec<String>> {
    let mut files = Vec::new();
    collect_rs(src_root, &mut files).map_err(|e| vec![e])?;
    files.sort();
    let mut found = Vec::new();
    let mut errors = Vec::new();
    let mut aliases: BTreeMap<String, (AliasDef, usize)> = BTreeMap::new();
    for path in &files {
        let rel = format!(
            "{rel_prefix}/{}",
            path.strip_prefix(src_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/")
        );
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{rel}: {e}"));
                continue;
            }
        };
        let file = match syn::parse_file(&text) {
            Ok(f) => f,
            Err(e) => {
                errors.push(format!("{rel}:{}: {e}", e.span().start().line));
                continue;
            }
        };
        walk_items(&file.items, &rel, &mut found, &mut aliases, &mut errors);
    }
    let exported: BTreeSet<String> = found
        .iter()
        .filter(|f| f.exported)
        .map(|f| f.def.name.clone())
        .collect();
    let unexported_ts: BTreeSet<String> = found
        .iter()
        .filter(|f| !f.exported)
        .map(|f| f.def.name.clone())
        .collect();
    let alias_names: BTreeSet<String> = aliases.keys().cloned().collect();
    // Auxiliary types are kept only when an exported type reaches them.
    let mut reached = BTreeSet::new();
    for f in found.iter().filter(|f| f.exported) {
        refs_in_shape(&f.def.shape, &mut reached);
        f.def.proxy.values().for_each(|t| refs_in(t, &mut reached));
    }
    // Aliases (and aliases of aliases) reached from DTOs; others are ignored.
    loop {
        let before = reached.len();
        for (name, (a, _)) in &aliases {
            if reached.contains(name) {
                let mut more = BTreeSet::new();
                refs_in(&a.ty, &mut more);
                reached.extend(more);
            }
        }
        if reached.len() == before {
            break;
        }
    }
    aliases.retain(|name, _| reached.contains(name));
    for (name, (a, line)) in &aliases {
        let mut ctx = Resolve {
            exported: &exported,
            unexported_ts: &unexported_ts,
            aliases: &alias_names,
            allow_aux: true,
            generics: BTreeSet::new(),
            at: format!("{}:{line} (alias {name})", a.source),
            errors: &mut errors,
        };
        ctx.check(&a.ty);
    }
    for f in found
        .iter()
        .filter(|f| f.exported || reached.contains(&f.def.name))
    {
        let mut ctx = Resolve {
            exported: &exported,
            unexported_ts: &unexported_ts,
            aliases: &alias_names,
            allow_aux: true,
            generics: f.def.generics.iter().cloned().collect(),
            at: format!("{}:{}", f.def.source, f.line),
            errors: &mut errors,
        };
        ctx.check_shape(&f.def.shape);
        for t in f.def.proxy.values() {
            ctx.check(t);
        }
        for (k, v) in &f.def.concrete {
            if !f.def.generics.contains(k) {
                ctx.errors.push(format!(
                    "{}: `ts(concrete({k} = ..))` names no type parameter of {}",
                    ctx.at, f.def.name
                ));
            }
            match syn::parse_str::<Type>(v) {
                Ok(ty) => {
                    ctx.generics = BTreeSet::new();
                    ctx.check(&type_ref(&ty, None));
                    ctx.generics = f.def.generics.iter().cloned().collect();
                }
                Err(_) => ctx.errors.push(format!(
                    "{}: unresolved generic parameter `{k}`: cannot parse `ts(concrete)` type `{v}`",
                    ctx.at
                )),
            }
        }
    }
    let shapes: BTreeMap<String, Shape> = found
        .iter()
        .map(|f| (f.def.name.clone(), f.def.shape.clone()))
        .collect();
    for f in &mut found {
        apply_proxy(f, &shapes, &mut errors);
    }
    let alias_types: BTreeMap<String, TypeRef> = aliases
        .iter()
        .map(|(k, (a, _))| (k.clone(), a.ty.clone()))
        .collect();
    for f in found
        .iter()
        .filter(|f| f.exported || reached.contains(&f.def.name))
    {
        check_flatten(&f.def, &f.def.shape, &shapes, &alias_types, &mut errors);
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let key = |a: &TypeDef, b: &TypeDef| (&a.name, &a.source).cmp(&(&b.name, &b.source));
    let mut types = Vec::new();
    let mut auxiliary_types = Vec::new();
    for f in found {
        if f.exported {
            types.push(f.def);
        } else if reached.contains(&f.def.name) {
            auxiliary_types.push(f.def);
        }
    }
    types.sort_by(key);
    auxiliary_types.sort_by(key);
    let aliases = aliases.into_iter().map(|(k, (a, _))| (k, a)).collect();
    Ok(Model {
        types,
        auxiliary_types,
        aliases,
    })
}

/// `#[serde(from|try_from = X, into = Y)]`: with X == Y the wire is X's
/// shape (a DTO's own shape, or a newtype over a primitive/container).
/// Different directions (or a one-way proxy) cannot be described by one
/// wire shape: an error with file:line.
fn apply_proxy(f: &mut Found, shapes: &BTreeMap<String, Shape>, errors: &mut Vec<String>) {
    if f.def.proxy.is_empty() {
        return;
    }
    let at = format!("{}:{}", f.def.source, f.line);
    let from = f
        .def
        .proxy
        .get("from")
        .or_else(|| f.def.proxy.get("try_from"));
    let into = f.def.proxy.get("into");
    let text = |t: Option<&TypeRef>| {
        t.map_or_else(
            || format!("(none: own derive of {})", f.def.name),
            |t| serde_json::to_string(t).unwrap_or_default(),
        )
    };
    let (Some(from_t), Some(into_t)) = (from, into) else {
        errors.push(format!(
            "{at}: {} proxy directions differ: from = {}, into = {}",
            f.def.name,
            text(from),
            text(into)
        ));
        return;
    };
    let (a, b) = (text(Some(from_t)), text(Some(into_t)));
    if a != b {
        errors.push(format!(
            "{at}: {} proxy directions differ: from = {a}, into = {b}",
            f.def.name
        ));
        return;
    }
    f.def.shape = match from_t {
        TypeRef::Ref { name, args } if args.is_empty() && shapes.contains_key(name) => {
            shapes[name].clone()
        }
        other => Shape::Newtype {
            inner: other.clone(),
        },
    };
}

/// A `flatten` field must name a struct-shaped DTO (directly or through an
/// alias); anything else (a map, an enum, an `Option`, a primitive) cannot be
/// expanded into named fields: an error with file:line.
fn check_flatten(
    def: &TypeDef,
    shape: &Shape,
    shapes: &BTreeMap<String, Shape>,
    aliases: &BTreeMap<String, TypeRef>,
    errors: &mut Vec<String>,
) {
    match shape {
        Shape::Struct { fields, .. } => {
            for f in fields.iter().filter(|f| f.flatten) {
                let mut ty = &f.ty;
                let mut hops = 0;
                while let TypeRef::Ref { name, args } = ty {
                    match aliases.get(name) {
                        Some(t) if args.is_empty() && hops < 16 => {
                            ty = t;
                            hops += 1;
                        }
                        _ => break,
                    }
                }
                let ok = matches!(ty, TypeRef::Ref { name, .. }
                    if matches!(shapes.get(name), Some(Shape::Struct { .. })));
                if !ok {
                    errors.push(format!(
                        "{}:{}: {}.{}: `flatten` of {} cannot be expanded (only a struct DTO can)",
                        def.source,
                        f.line,
                        def.name,
                        f.rust,
                        serde_json::to_string(&f.ty).unwrap_or_default()
                    ));
                }
            }
        }
        Shape::Enum { variants, .. } => {
            for v in variants {
                check_flatten(def, &v.shape, shapes, aliases, errors);
            }
        }
        _ => {}
    }
}

fn refs_in_shape(shape: &Shape, out: &mut BTreeSet<String>) {
    match shape {
        Shape::Struct { fields, .. } => fields.iter().for_each(|f| refs_in(&f.ty, out)),
        Shape::Newtype { inner } => refs_in(inner, out),
        Shape::Tuple { items } => items.iter().for_each(|t| refs_in(t, out)),
        Shape::Unit | Shape::HandWritten { .. } => {}
        Shape::Enum { variants, .. } => variants.iter().for_each(|v| refs_in_shape(&v.shape, out)),
    }
}

fn refs_in(t: &TypeRef, out: &mut BTreeSet<String>) {
    match t {
        TypeRef::Option(i) | TypeRef::Array(i) | TypeRef::TriState(i) => refs_in(i, out),
        TypeRef::Map { key, value } => {
            refs_in(key, out);
            refs_in(value, out);
        }
        TypeRef::Tuple(items) => items.iter().for_each(|i| refs_in(i, out)),
        TypeRef::Ref { name, args } => {
            out.insert(name.clone());
            args.iter().for_each(|i| refs_in(i, out));
        }
        _ => {}
    }
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "tests") {
                continue;
            }
            collect_rs(&path, out)?;
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn is_cfg_test(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| a.path().is_ident("cfg") && a.meta.to_token_stream().to_string().contains("test"))
}

fn walk_items(
    items: &[Item],
    rel: &str,
    found: &mut Vec<Found>,
    aliases: &mut BTreeMap<String, (AliasDef, usize)>,
    errors: &mut Vec<String>,
) {
    for item in items {
        match item {
            Item::Mod(m) if !is_cfg_test(&m.attrs) => {
                if let Some((_, inner)) = &m.content {
                    walk_items(inner, rel, found, aliases, errors);
                }
            }
            Item::Type(t) if t.generics.params.is_empty() && !is_cfg_test(&t.attrs) => {
                aliases.insert(
                    t.ident.to_string(),
                    (
                        AliasDef {
                            source: rel.to_string(),
                            ty: type_ref(&t.ty, None),
                        },
                        t.ident.span().start().line,
                    ),
                );
            }
            Item::Struct(s) => {
                consider(&s.attrs, &s.ident, &s.generics, rel, found, errors, |c| {
                    struct_shape(&s.ident.to_string(), &s.fields, c)
                });
            }
            Item::Enum(e) => {
                consider(&e.attrs, &e.ident, &e.generics, rel, found, errors, |c| {
                    enum_shape(&e.ident.to_string(), e, c)
                });
            }
            _ => {}
        }
    }
}

/// Container-level serde/ts attributes.
#[derive(Default)]
struct Container {
    rename_all: Option<String>,
    rename_all_fields: Option<String>,
    tag: Option<String>,
    content: Option<String>,
    untagged: bool,
    deny_unknown_fields: bool,
    proxy: BTreeMap<String, String>,
    ts_export: bool,
    ts_rename: Option<String>,
    ts_override: Option<String>,
    concrete: BTreeMap<String, String>,
}

#[allow(clippy::too_many_arguments)]
fn consider(
    attrs: &[Attribute],
    ident: &syn::Ident,
    generics: &syn::Generics,
    rel: &str,
    found: &mut Vec<Found>,
    errors: &mut Vec<String>,
    shape: impl FnOnce(&Container) -> Result<Shape, String>,
) {
    if !derives_ts(attrs) {
        return;
    }
    let line = ident.span().start().line;
    if let Some(msg) = cfg_attr_problem(attrs) {
        errors.push(format!("{rel}:{line}: {msg}"));
    }
    let c = match container(attrs) {
        Ok(c) => c,
        Err(e) => {
            errors.push(format!("{rel}:{line}: {e}"));
            return;
        }
    };
    let shape = if derives(attrs, "Serialize") || derives(attrs, "Deserialize") {
        shape(&c)
    } else if let Some(wire) = &c.ts_override {
        Ok(Shape::HandWritten { wire: wire.clone() })
    } else {
        Err("has hand-written serde and no `#[ts(type|as)]` codec".to_string())
    };
    let type_params: Vec<String> = generics
        .type_params()
        .map(|p| p.ident.to_string())
        .collect();
    let shape = match shape {
        Ok(mut s) => {
            generic_params_in_shape(&mut s, &type_params);
            s
        }
        Err(e) => {
            match e.strip_prefix('@').and_then(|r| r.split_once('@')) {
                Some((l, msg)) => errors.push(format!("{rel}:{l}: {msg}")),
                None => errors.push(format!("{rel}:{line}: {ident} {e}")),
            }
            // Keep the name resolvable so references to this type do not
            // add a second, cascading `unregistered type` error. Generation
            // already fails on the error above.
            Shape::Unit
        }
    };
    let category = CATEGORY_BY_FILE
        .iter()
        .find(|(f, _)| *f == rel)
        .map_or(DEFAULT_CATEGORY, |(_, c)| c)
        .to_string();
    let proxy = c
        .proxy
        .iter()
        .map(|(k, v)| {
            let ty: Type = syn::parse_str(v).unwrap_or_else(|_| syn::parse_quote!(Unparsed));
            (k.clone(), type_ref(&ty, None))
        })
        .collect();
    found.push(Found {
        def: TypeDef {
            name: ident.to_string(),
            ts_name: c.ts_rename.clone(),
            category,
            source: rel.to_string(),
            generics: type_params,
            concrete: c.concrete.clone(),
            proxy,
            ts_override: c.ts_override.clone(),
            shape,
        },
        line,
        exported: c.ts_export,
    });
}

/// A bare `Ref` named like one of the item's type parameters is that
/// parameter: [`TypeRef::Generic`].
fn generic_params_in_shape(shape: &mut Shape, params: &[String]) {
    if params.is_empty() {
        return;
    }
    match shape {
        Shape::Struct { fields, .. } => fields
            .iter_mut()
            .for_each(|f| generic_params_in(&mut f.ty, params)),
        Shape::Newtype { inner } => generic_params_in(inner, params),
        Shape::Tuple { items } => items.iter_mut().for_each(|t| generic_params_in(t, params)),
        Shape::Unit | Shape::HandWritten { .. } => {}
        Shape::Enum { variants, .. } => variants
            .iter_mut()
            .for_each(|v| generic_params_in_shape(&mut v.shape, params)),
    }
}

fn generic_params_in(t: &mut TypeRef, params: &[String]) {
    match t {
        TypeRef::Option(i) | TypeRef::Array(i) | TypeRef::TriState(i) => {
            generic_params_in(i, params);
        }
        TypeRef::Map { key, value } => {
            generic_params_in(key, params);
            generic_params_in(value, params);
        }
        TypeRef::Tuple(items) => items.iter_mut().for_each(|i| generic_params_in(i, params)),
        TypeRef::Ref { name, args } if args.is_empty() && params.contains(name) => {
            *t = TypeRef::Generic(name.clone());
        }
        TypeRef::Ref { args, .. } => args.iter_mut().for_each(|i| generic_params_in(i, params)),
        _ => {}
    }
}

fn derives_ts(attrs: &[Attribute]) -> bool {
    derives(attrs, "TS")
}

fn derives(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("derive")
            && a.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|ps| {
                ps.iter()
                    .any(|p| p.segments.last().is_some_and(|s| s.ident == name))
            })
    })
}

fn lit_str(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<String> {
    Ok(meta.value()?.parse::<syn::LitStr>()?.value())
}

/// Skip whatever follows a nested meta key (`= expr` or `(..)`).
fn skip_rest(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<()> {
    if meta.input.peek(syn::Token![=]) {
        let _: syn::Expr = meta.value()?.parse()?;
    } else if meta.input.peek(syn::token::Paren) {
        let _g: proc_macro2::Group = meta.input.parse()?;
    }
    Ok(())
}

fn container(attrs: &[Attribute]) -> Result<Container, String> {
    let mut c = Container::default();
    for a in attrs {
        if a.path().is_ident("serde") {
            a.parse_nested_meta(|m| {
                let key = m
                    .path
                    .get_ident()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                match key.as_str() {
                    "rename_all" => c.rename_all = Some(lit_str(&m)?),
                    "rename_all_fields" => c.rename_all_fields = Some(lit_str(&m)?),
                    "tag" => c.tag = Some(lit_str(&m)?),
                    "content" => c.content = Some(lit_str(&m)?),
                    "untagged" => c.untagged = true,
                    "deny_unknown_fields" => c.deny_unknown_fields = true,
                    "from" | "into" | "try_from" => {
                        let v = lit_str(&m)?;
                        c.proxy.insert(key, v);
                    }
                    other => {
                        return Err(
                            m.error(format!("unsupported serde container attribute `{other}`"))
                        );
                    }
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        } else if a.path().is_ident("ts") {
            a.parse_nested_meta(|m| {
                let key = m
                    .path
                    .get_ident()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                match key.as_str() {
                    "export" => c.ts_export = true,
                    "rename" => c.ts_rename = Some(lit_str(&m)?),
                    "as" | "type" => c.ts_override = Some(lit_str(&m)?),
                    "concrete" => {
                        let g: proc_macro2::Group = m.input.parse()?;
                        let text = g.stream().to_string();
                        for pair in text.split(',') {
                            if let Some((k, v)) = pair.split_once('=') {
                                c.concrete
                                    .insert(k.trim().to_string(), v.trim().to_string());
                            }
                        }
                    }
                    _ => skip_rest(&m)?,
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(c)
}

/// Field/variant-level attributes.
#[derive(Default)]
struct Member {
    rename: Option<String>,
    aliases: Vec<String>,
    skip: bool,
    default: bool,
    skip_serializing_if: bool,
    flatten: bool,
    other: bool,
    with: Option<String>,
    ts_type: Option<String>,
    ts_optional: bool,
}

fn member(attrs: &[Attribute]) -> Result<Member, String> {
    let mut m = Member::default();
    for a in attrs {
        if a.path().is_ident("serde") {
            a.parse_nested_meta(|n| {
                let key = n
                    .path
                    .get_ident()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                match key.as_str() {
                    "rename" => m.rename = Some(lit_str(&n)?),
                    "alias" => m.aliases.push(lit_str(&n)?),
                    "skip" => m.skip = true,
                    "default" => {
                        m.default = true;
                        skip_rest(&n)?;
                    }
                    "skip_serializing_if" => {
                        m.skip_serializing_if = true;
                        skip_rest(&n)?;
                    }
                    "flatten" => m.flatten = true,
                    "other" => m.other = true,
                    "with" | "deserialize_with" | "serialize_with" => {
                        m.with = Some(format!("{key}={}", lit_str(&n)?));
                    }
                    other => {
                        return Err(n.error(format!("unsupported serde field attribute `{other}`")));
                    }
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        } else if a.path().is_ident("ts") {
            a.parse_nested_meta(|n| {
                let key = n
                    .path
                    .get_ident()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                match key.as_str() {
                    "type" | "as" => m.ts_type = Some(lit_str(&n)?),
                    "optional" => {
                        m.ts_optional = true;
                        skip_rest(&n)?;
                    }
                    _ => skip_rest(&n)?,
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(m)
}

/// Error text carrying the field's own line: `@<line>@<message>`; `consider`
/// turns it into `file:<line>: ..`.
fn at_line(line: usize, msg: &str) -> String {
    format!("@{line}@{msg}")
}

/// `#[cfg_attr(.., serde(..))]` / `#[cfg_attr(.., ts(..))]` would make the
/// wire depend on build configuration the extractor cannot see: an error.
fn no_cfg_attr(attrs: &[Attribute], line: usize) -> Result<(), String> {
    match cfg_attr_problem(attrs) {
        Some(msg) => Err(at_line(line, &msg)),
        None => Ok(()),
    }
}

fn cfg_attr_problem(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|a| {
        if !a.path().is_ident("cfg_attr") {
            return None;
        }
        let text = a.meta.to_token_stream().to_string().replace(' ', "");
        (text.contains("serde(") || text.contains("ts("))
            .then(|| format!("`cfg_attr` carrying serde/ts attributes is not supported: #[{text}]"))
    })
}

/// Check a `with`-style field: `#[ts(type)]` or a [`FIELD_CODECS`] entry.
fn field_codec(
    owner: &str,
    variant: &str,
    field: &str,
    m: &Member,
    line: usize,
) -> Result<Option<&'static FieldCodec>, String> {
    let Some(with) = &m.with else {
        return Ok(None);
    };
    if m.ts_type.is_some() {
        return Ok(None);
    }
    FIELD_CODECS
        .iter()
        .find(|c| {
            c.owner == owner && c.variant == variant && c.field == field && c.serde == with
        })
        .map(Some)
        .ok_or_else(|| {
            let path = if variant.is_empty() {
                format!("{owner}.{field}")
            } else {
                format!("{owner}::{variant}.{field}")
            };
            at_line(
                line,
                &format!("{path} uses serde `{with}` but has no `#[ts(type)]` and no FIELD_CODECS registration"),
            )
        })
}

fn fields_of(
    owner: &str,
    variant: &str,
    fields: &Fields,
    rename_all: Option<&str>,
) -> Result<Shape, String> {
    match fields {
        Fields::Unit => Ok(Shape::Unit),
        Fields::Unnamed(u) => {
            let mut items = Vec::new();
            for (i, f) in u.unnamed.iter().enumerate() {
                let line = f.ty.span().start().line;
                no_cfg_attr(&f.attrs, line)?;
                let m = member(&f.attrs)?;
                field_codec(owner, variant, &i.to_string(), &m, line)?;
                items.push(type_ref(&f.ty, m.ts_type.as_deref()));
            }
            if items.len() == 1 {
                Ok(Shape::Newtype {
                    inner: items.remove(0),
                })
            } else {
                Ok(Shape::Tuple { items })
            }
        }
        Fields::Named(n) => {
            let mut out = Vec::new();
            let mut skipped = Vec::new();
            for f in &n.named {
                let line = f.ident.as_ref().map_or(0, |i| i.span().start().line);
                no_cfg_attr(&f.attrs, line)?;
                let m = member(&f.attrs)?;
                let rust = f
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                let rust = rust.strip_prefix("r#").unwrap_or(&rust).to_string();
                if m.skip {
                    skipped.push(rust);
                    continue;
                }
                let wire = m
                    .rename
                    .clone()
                    .unwrap_or_else(|| rename(&rust, rename_all, false));
                let codec = field_codec(owner, variant, &rust, &m, line)?;
                let mut ty = type_ref(&f.ty, m.ts_type.as_deref());
                if codec.is_some_and(|c| c.tri_state) {
                    let bad = || {
                        at_line(
                            line,
                            &format!("{owner}.{rust}: tri-state codec needs Option<Option<T>>"),
                        )
                    };
                    ty = match ty {
                        TypeRef::Option(outer) => match *outer {
                            TypeRef::Option(inner) => TypeRef::TriState(inner),
                            _ => return Err(bad()),
                        },
                        _ => return Err(bad()),
                    };
                }
                out.push(FieldDef {
                    wire,
                    rust,
                    ty,
                    codec: codec.map(|c| FieldCodecRef {
                        name: c.name.into(),
                        wire: c.wire.into(),
                    }),
                    omit_when_empty: m.skip_serializing_if,
                    default: m.default,
                    flatten: m.flatten,
                    aliases: m.aliases,
                    serde_with: m.with,
                    line,
                });
            }
            Ok(Shape::Struct {
                fields: out,
                deny_unknown_fields: false,
                skipped,
            })
        }
    }
}

fn struct_shape(owner: &str, fields: &Fields, c: &Container) -> Result<Shape, String> {
    let mut shape = fields_of(owner, "", fields, c.rename_all.as_deref())?;
    if let Shape::Struct {
        deny_unknown_fields,
        ..
    } = &mut shape
    {
        *deny_unknown_fields = c.deny_unknown_fields;
    }
    Ok(shape)
}

fn enum_shape(owner: &str, e: &syn::ItemEnum, c: &Container) -> Result<Shape, String> {
    let representation = match (&c.tag, &c.content, c.untagged) {
        (_, _, true) => Representation::Untagged,
        (Some(tag), Some(content), false) => Representation::Adjacent {
            tag: tag.clone(),
            content: content.clone(),
        },
        (Some(tag), None, false) => Representation::Internal { tag: tag.clone() },
        (None, None, false) => Representation::External,
        (None, Some(_), false) => return Err("`content` without `tag`".into()),
    };
    let mut variants = Vec::new();
    for v in &e.variants {
        no_cfg_attr(&v.attrs, v.ident.span().start().line)?;
        let m = member(&v.attrs)?;
        if m.skip {
            continue;
        }
        let rust = v.ident.to_string();
        let rust_name = rust.clone();
        let untagged = matches!(representation, Representation::Untagged) || {
            v.attrs.iter().any(|a| {
                a.path().is_ident("serde")
                    && a.meta.to_token_stream().to_string().contains("untagged")
            })
        };
        let wire = if untagged {
            None
        } else {
            Some(
                m.rename
                    .clone()
                    .unwrap_or_else(|| rename(&rust, c.rename_all.as_deref(), true)),
            )
        };
        let vc = container_fields_rule(&v.attrs, c)?;
        variants.push(VariantDef {
            rust,
            wire,
            aliases: m.aliases,
            other: m.other,
            shape: fields_of(owner, &rust_name, &v.fields, vc.as_deref())?,
        });
    }
    Ok(Shape::Enum {
        representation,
        variants,
    })
}

/// `rename_all` that applies to a variant's own fields: a variant-level
/// `#[serde(rename_all)]`, else the container's `rename_all_fields`.
fn container_fields_rule(attrs: &[Attribute], c: &Container) -> Result<Option<String>, String> {
    let mut rule = c.rename_all_fields.clone();
    for a in attrs {
        if a.path().is_ident("serde") && a.meta.to_token_stream().to_string().contains("rename_all")
        {
            a.parse_nested_meta(|n| {
                if n.path.is_ident("rename_all") {
                    rule = Some(lit_str(&n)?);
                } else {
                    skip_rest(&n)?;
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(rule)
}

/// serde's `rename_all` rules. `variant` inputs are PascalCase, field inputs
/// snake_case.
#[must_use]
pub fn rename(name: &str, rule: Option<&str>, variant: bool) -> String {
    let Some(rule) = rule else {
        return name.to_string();
    };
    let words: Vec<String> = if variant {
        let mut words = Vec::new();
        let mut cur = String::new();
        for ch in name.chars() {
            if ch.is_uppercase() && !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            cur.push(ch);
        }
        if !cur.is_empty() {
            words.push(cur);
        }
        words.into_iter().map(|w| w.to_lowercase()).collect()
    } else {
        name.split('_').map(str::to_string).collect()
    };
    let cap = |w: &str| {
        let mut c = w.chars();
        c.next().map_or(String::new(), |f| {
            f.to_uppercase().collect::<String>() + c.as_str()
        })
    };
    match rule {
        "lowercase" => {
            if variant {
                name.to_lowercase()
            } else {
                name.to_string()
            }
        }
        "UPPERCASE" => name.to_uppercase(),
        "PascalCase" => words.iter().map(|w| cap(w)).collect(),
        "camelCase" => words
            .iter()
            .enumerate()
            .map(|(i, w)| if i == 0 { w.clone() } else { cap(w) })
            .collect(),
        "snake_case" => words.join("_"),
        "SCREAMING_SNAKE_CASE" => words.join("_").to_uppercase(),
        "kebab-case" => words.join("-"),
        "SCREAMING-KEBAB-CASE" => words.join("-").to_uppercase(),
        _ => name.to_string(),
    }
}

fn last_ident(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}

fn args(seg: &syn::PathSegment) -> Vec<&Type> {
    match &seg.arguments {
        PathArguments::AngleBracketed(a) => a
            .args
            .iter()
            .filter_map(|g| {
                if let GenericArgument::Type(t) = g {
                    Some(t)
                } else {
                    None
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn type_ref(ty: &Type, ts_override: Option<&str>) -> TypeRef {
    let text = ty.to_token_stream().to_string().replace(' ', "");
    if let Some(wire) = ts_override {
        return TypeRef::Codec {
            rust: text,
            wire: wire.to_string(),
        };
    }
    match ty {
        Type::Reference(r) => type_ref(&r.elem, None),
        Type::Tuple(t) => TypeRef::Tuple(t.elems.iter().map(|e| type_ref(e, None)).collect()),
        Type::Slice(s) => TypeRef::Array(Box::new(type_ref(&s.elem, None))),
        Type::Array(a) => TypeRef::Array(Box::new(type_ref(&a.elem, None))),
        Type::Path(p) if p.qself.is_none() => {
            let Some(seg) = p.path.segments.last() else {
                return TypeRef::Ref {
                    name: text,
                    args: vec![],
                };
            };
            let name = seg.ident.to_string();
            let a = args(seg);
            let joined = p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            match (name.as_str(), a.as_slice()) {
                (n, []) if PRIMS.contains(&n) => TypeRef::Prim(if n == "str" {
                    "String".into()
                } else {
                    n.into()
                }),
                ("Value", []) if joined.contains("serde_json") || joined == "Value" => {
                    TypeRef::Json
                }
                ("JsonValue", []) => TypeRef::Json,
                ("Map", [k, v]) => TypeRef::Map {
                    key: Box::new(type_ref(k, None)),
                    value: Box::new(type_ref(v, None)),
                },
                ("Option", [t]) => TypeRef::Option(Box::new(type_ref(t, None))),
                ("Vec" | "VecDeque" | "BTreeSet" | "HashSet" | "IndexSet", [t]) => {
                    TypeRef::Array(Box::new(type_ref(t, None)))
                }
                ("HashMap" | "BTreeMap" | "IndexMap", [k, v]) => TypeRef::Map {
                    key: Box::new(type_ref(k, None)),
                    value: Box::new(type_ref(v, None)),
                },
                ("Box" | "Arc" | "Rc" | "Cow", [t]) => type_ref(t, None),
                ("Cow", [_, t]) => type_ref(t, None),
                _ => TypeRef::Ref {
                    name: last_ident(&p.path),
                    args: a.iter().map(|t| type_ref(t, None)).collect(),
                },
            }
        }
        _ => TypeRef::Ref {
            name: text,
            args: vec![],
        },
    }
}

struct Resolve<'a> {
    exported: &'a BTreeSet<String>,
    unexported_ts: &'a BTreeSet<String>,
    aliases: &'a BTreeSet<String>,
    allow_aux: bool,
    generics: BTreeSet<String>,
    at: String,
    errors: &'a mut Vec<String>,
}

impl Resolve<'_> {
    fn check_shape(&mut self, shape: &Shape) {
        match shape {
            Shape::Struct { fields, .. } => {
                for f in fields {
                    self.check(&f.ty);
                }
            }
            Shape::Newtype { inner } => self.check(inner),
            Shape::Tuple { items } => items.iter().for_each(|t| self.check(t)),
            Shape::Unit | Shape::HandWritten { .. } => {}
            Shape::Enum { variants, .. } => {
                for v in variants {
                    self.check_shape(&v.shape);
                }
            }
        }
    }

    fn check(&mut self, t: &TypeRef) {
        match t {
            TypeRef::Prim(_) | TypeRef::Json | TypeRef::Codec { .. } => {}
            TypeRef::Generic(g) => {
                if !self.generics.contains(g) {
                    self.errors.push(format!(
                        "{}: unresolved generic parameter `{g}` (no `ts(concrete)` and not a type parameter)",
                        self.at
                    ));
                }
            }
            TypeRef::Option(i) | TypeRef::Array(i) | TypeRef::TriState(i) => self.check(i),
            TypeRef::Map { key, value } => {
                self.check(key);
                self.check(value);
            }
            TypeRef::Tuple(items) => items.iter().for_each(|i| self.check(i)),
            TypeRef::Ref { name, args } => {
                args.iter().for_each(|i| self.check(i));
                if self.exported.contains(name)
                    || self.aliases.contains(name)
                    || (self.allow_aux && self.unexported_ts.contains(name))
                    || CODECS.iter().any(|(n, _)| n == name)
                {
                    return;
                }
                let why = if self.unexported_ts.contains(name) {
                    "derives TS but is not `#[ts(export)]`"
                } else {
                    "is not an exported type and has no registered codec"
                };
                self.errors
                    .push(format!("{}: unregistered type `{name}` ({why})", self.at));
            }
        }
    }
}
