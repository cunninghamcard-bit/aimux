//! Float-path-aware JSON equality, shared by tests/wire_diff_go.rs (U1)
//! and tests/swift_semantics.rs: numbers compare by value only at JSON
//! paths whose IR type is `f32` / `f64`.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use aimux_codegen::wire;
use serde_json::Value;

pub fn join_key(path: &str, k: &str) -> String {
    if path.is_empty() {
        k.to_string()
    } else {
        format!("{path}.{k}")
    }
}

pub type Ir = BTreeMap<String, wire::WireType>;

pub fn float_paths_in(m: &Ir, ty: &str, v: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    walk_ty(m, &wire::WireTy::Named(ty.into()), v, "", &mut out, 0);
    out
}

/// Whether serde would decode `v` as the IR type `ty` (JSON kind, integer
/// vs float, required keys, tags, untagged arms in order). Unknown keys are
/// accepted, as serde does without `deny_unknown_fields`.
pub fn decodes(m: &Ir, ty: &wire::WireTy, v: &Value, depth: usize) -> bool {
    use wire::WireTy as T;
    if depth > 64 {
        return true;
    }
    match ty {
        T::Prim(p) => match p.as_str() {
            "String" | "str" => v.is_string(),
            "char" => v.as_str().is_some_and(|s| s.chars().count() == 1),
            "bool" => v.is_boolean(),
            "u8" => v.as_u64().is_some_and(|n| n <= u64::from(u8::MAX)),
            "u16" => v.as_u64().is_some_and(|n| n <= u64::from(u16::MAX)),
            "u32" => v.as_u64().is_some_and(|n| n <= u64::from(u32::MAX)),
            "u64" | "usize" | "u128" => v.is_u64(),
            "i8" | "i16" | "i32" | "i64" | "isize" | "i128" => v.is_i64(),
            "f32" | "f64" => v.is_number(),
            _ => true,
        },
        T::Json => true,
        T::Literal(l) => v.as_str() == Some(l.as_str()),
        T::Nullable(i) => v.is_null() || decodes(m, i, v, depth + 1),
        T::Array(i) => v
            .as_array()
            .is_some_and(|a| a.iter().all(|x| decodes(m, i, x, depth + 1))),
        T::Tuple(items) => v.as_array().is_some_and(|a| {
            a.len() == items.len()
                && items
                    .iter()
                    .zip(a)
                    .all(|(i, x)| decodes(m, i, x, depth + 1))
        }),
        T::Map { value, .. } => v
            .as_object()
            .is_some_and(|o| o.values().all(|x| decodes(m, value, x, depth + 1))),
        T::Named(n) => m
            .get(n)
            .is_none_or(|t| body_decodes(m, &t.body, v, depth + 1)),
    }
}

pub fn fields_decode(m: &Ir, fields: &[wire::WireField], v: &Value, depth: usize) -> bool {
    let Some(o) = v.as_object() else {
        return false;
    };
    fields.iter().all(|f| match o.get(&f.wire) {
        None => f.presence.omittable || f.presence.nullable,
        Some(Value::Null) if f.presence.nullable => true,
        Some(x) => decodes(m, &f.ty, x, depth),
    })
}

pub fn variant_decodes(
    m: &Ir,
    b: &wire::VariantBody,
    v: &Value,
    untagged: bool,
    depth: usize,
) -> bool {
    match b {
        wire::VariantBody::Unit => !untagged || v.is_null(),
        wire::VariantBody::Fields(f) => fields_decode(m, f, v, depth),
        wire::VariantBody::Value(t) => decodes(m, t, v, depth),
        wire::VariantBody::Tuple(items) => {
            decodes(m, &wire::WireTy::Tuple(items.clone()), v, depth)
        }
    }
}

/// The variant of a union `v` is, with its payload: by tag, or (untagged)
/// the first variant in declaration order that decodes.
pub fn matched_variant<'a>(
    m: &Ir,
    repr: &wire::UnionRepr,
    variants: &'a [wire::WireVariant],
    v: &'a Value,
    path: &str,
    depth: usize,
) -> Option<(&'a wire::WireVariant, &'a Value, String)> {
    use wire::UnionRepr as R;
    let named = |tag: &str| {
        variants
            .iter()
            .find(|x| x.wire.as_deref() == Some(tag) || x.aliases.iter().any(|a| a == tag))
            .or_else(|| variants.iter().find(|x| x.other))
    };
    match repr {
        R::Internal { tag } => v
            .get(tag)
            .and_then(Value::as_str)
            .and_then(named)
            .map(|x| (x, v, path.to_string())),
        R::Adjacent { tag, content } => {
            let x = v.get(tag).and_then(Value::as_str).and_then(named)?;
            Some((
                x,
                v.get(content).unwrap_or(&Value::Null),
                join_key(path, content),
            ))
        }
        R::External => {
            if let Some(s) = v.as_str() {
                return named(s).map(|x| (x, &Value::Null, path.to_string()));
            }
            let (k, c) = v
                .as_object()
                .filter(|o| o.len() == 1)
                .and_then(|o| o.iter().next())?;
            named(k).map(|x| (x, c, join_key(path, k)))
        }
        R::Untagged => variants
            .iter()
            .find(|x| variant_decodes(m, &x.body, v, true, depth))
            .map(|x| (x, v, path.to_string())),
    }
}

pub fn body_decodes(m: &Ir, body: &wire::WireBody, v: &Value, depth: usize) -> bool {
    use wire::WireBody as B;
    match body {
        B::Record { fields } => fields_decode(m, fields, v, depth),
        B::Alias(t) => decodes(m, t, v, depth),
        B::StringEnum { values, open, .. } => v
            .as_str()
            .is_some_and(|s| *open || values.iter().any(|x| x == s)),
        B::Union { repr, variants } => {
            let untagged = matches!(repr, wire::UnionRepr::Untagged);
            matched_variant(m, repr, variants, v, "", depth)
                .is_some_and(|(x, p, _)| untagged || variant_decodes(m, &x.body, p, false, depth))
        }
    }
}

pub fn walk_ty(
    m: &Ir,
    ty: &wire::WireTy,
    v: &Value,
    path: &str,
    out: &mut BTreeSet<String>,
    depth: usize,
) {
    use wire::WireTy as T;
    if depth > 64 {
        return;
    }
    match ty {
        T::Prim(p) if p == "f32" || p == "f64" => {
            out.insert(path.to_string());
        }
        T::Nullable(i) => walk_ty(m, i, v, path, out, depth + 1),
        T::Array(i) => {
            for (n, x) in v.as_array().into_iter().flatten().enumerate() {
                walk_ty(m, i, x, &format!("{path}[{n}]"), out, depth + 1);
            }
        }
        T::Tuple(items) => {
            for (n, (i, x)) in items
                .iter()
                .zip(v.as_array().into_iter().flatten())
                .enumerate()
            {
                walk_ty(m, i, x, &format!("{path}[{n}]"), out, depth + 1);
            }
        }
        T::Map { value, .. } => {
            for (k, x) in v.as_object().into_iter().flatten() {
                walk_ty(m, value, x, &join_key(path, k), out, depth + 1);
            }
        }
        T::Named(n) => {
            if let Some(t) = m.get(n) {
                walk_body(m, &t.body, v, path, out, depth + 1);
            }
        }
        T::Prim(_) | T::Json | T::Literal(_) => {}
    }
}

pub fn walk_fields(
    m: &Ir,
    fields: &[wire::WireField],
    v: &Value,
    path: &str,
    out: &mut BTreeSet<String>,
    depth: usize,
) {
    for f in fields {
        if let Some(x) = v.get(&f.wire) {
            walk_ty(m, &f.ty, x, &join_key(path, &f.wire), out, depth);
        }
    }
}

pub fn walk_variant(
    m: &Ir,
    b: &wire::VariantBody,
    v: &Value,
    path: &str,
    out: &mut BTreeSet<String>,
    depth: usize,
) {
    match b {
        wire::VariantBody::Unit => {}
        wire::VariantBody::Fields(fields) => walk_fields(m, fields, v, path, out, depth),
        wire::VariantBody::Value(t) => walk_ty(m, t, v, path, out, depth),
        wire::VariantBody::Tuple(items) => {
            walk_ty(m, &wire::WireTy::Tuple(items.clone()), v, path, out, depth);
        }
    }
}

pub fn walk_body(
    m: &Ir,
    body: &wire::WireBody,
    v: &Value,
    path: &str,
    out: &mut BTreeSet<String>,
    depth: usize,
) {
    use wire::WireBody as B;
    match body {
        B::Record { fields } => walk_fields(m, fields, v, path, out, depth),
        B::Alias(t) => walk_ty(m, t, v, path, out, depth),
        B::StringEnum { .. } => {}
        B::Union { repr, variants } => {
            // Only the matched variant: a path that is an integer in one
            // variant and a float in another follows the value's variant.
            if let Some((x, payload, p)) = matched_variant(m, repr, variants, v, path, depth) {
                walk_variant(m, &x.body, payload, &p, out, depth);
            }
        }
    }
}

/// JSON equality. Numbers at a path the IR types `f32` / `f64` (see
/// [`float_paths`]) compare by value: Go's encoding/json writes an `f64`
/// zero as `0`, serde as `0.0`, and both decode to the same `f64`. Every
/// other number, integers included, must be the same JSON number exactly.
pub fn json_eq_at(a: &Value, b: &Value, path: &str, floats: &BTreeSet<String>) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            x == y || (floats.contains(path) && x.as_f64() == y.as_f64())
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .enumerate()
                    .all(|(n, (a, b))| json_eq_at(a, b, &format!("{path}[{n}]"), floats))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| {
                    y.get(k)
                        .is_some_and(|w| json_eq_at(v, w, &join_key(path, k), floats))
                })
        }
        _ => a == b,
    }
}
