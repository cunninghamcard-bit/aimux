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
        TypeRef::Option(i) | TypeRef::Array(i) => refs_in(i, out),
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
                    struct_shape(&s.fields, c)
                });
            }
            Item::Enum(e) => {
                consider(&e.attrs, &e.ident, &e.generics, rel, found, errors, |c| {
                    enum_shape(e, c)
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
    let shape = match shape {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("{rel}:{line}: {ident} {e}"));
            return;
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
            generics: generics
                .type_params()
                .map(|p| p.ident.to_string())
                .collect(),
            concrete: c.concrete.clone(),
            proxy,
            ts_override: c.ts_override.clone(),
            shape,
        },
        line,
        exported: c.ts_export,
    });
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

fn fields_of(fields: &Fields, rename_all: Option<&str>) -> Result<Shape, String> {
    match fields {
        Fields::Unit => Ok(Shape::Unit),
        Fields::Unnamed(u) => {
            let mut items = Vec::new();
            for f in &u.unnamed {
                let m = member(&f.attrs)?;
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
            for f in &n.named {
                let m = member(&f.attrs)?;
                if m.skip {
                    continue;
                }
                let rust = f
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                let rust = rust.strip_prefix("r#").unwrap_or(&rust).to_string();
                let wire = m
                    .rename
                    .clone()
                    .unwrap_or_else(|| rename(&rust, rename_all, false));
                out.push(FieldDef {
                    wire,
                    rust,
                    ty: type_ref(&f.ty, m.ts_type.as_deref()),
                    omit_when_empty: m.skip_serializing_if,
                    default: m.default,
                    flatten: m.flatten,
                    aliases: m.aliases,
                    serde_with: m.with,
                });
            }
            Ok(Shape::Struct {
                fields: out,
                deny_unknown_fields: false,
            })
        }
    }
}

fn struct_shape(fields: &Fields, c: &Container) -> Result<Shape, String> {
    let mut shape = fields_of(fields, c.rename_all.as_deref())?;
    if let Shape::Struct {
        deny_unknown_fields,
        ..
    } = &mut shape
    {
        *deny_unknown_fields = c.deny_unknown_fields;
    }
    Ok(shape)
}

fn enum_shape(e: &syn::ItemEnum, c: &Container) -> Result<Shape, String> {
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
        let m = member(&v.attrs)?;
        if m.skip {
            continue;
        }
        let rust = v.ident.to_string();
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
            shape: fields_of(&v.fields, vc.as_deref())?,
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
            TypeRef::Prim(_) | TypeRef::Json | TypeRef::Codec { .. } | TypeRef::Generic(_) => {}
            TypeRef::Option(i) | TypeRef::Array(i) => self.check(i),
            TypeRef::Map { key, value } => {
                self.check(key);
                self.check(value);
            }
            TypeRef::Tuple(items) => items.iter().for_each(|i| self.check(i)),
            TypeRef::Ref { name, args } => {
                args.iter().for_each(|i| self.check(i));
                if self.generics.contains(name)
                    || self.exported.contains(name)
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
