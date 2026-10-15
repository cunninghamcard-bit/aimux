//! Generated "field absent" fixtures, one per emit::registry use, shared
//! by wire_diff_go.rs (Go) and swift_semantics.rs (Swift).
#![allow(dead_code)]

use aimux_codegen::manifest::Inputs;
use aimux_codegen::wire;
use serde_json::{Value, json};

fn workspace() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub type RegistryBack = fn(&Value) -> Result<Value, String>;

fn back<T: serde::Serialize + serde::de::DeserializeOwned>(v: &Value) -> Result<Value, String> {
    let t: T = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(&t).map_err(|e| e.to_string())
}

/// A minimal JSON value of `t` that serde accepts: required keys only,
/// zero values, the first enum value / internally tagged variant.
pub fn minimal(m: &wire::WireModel, t: &wire::WireTy) -> Value {
    use wire::{UnionRepr, VariantBody, WireBody, WireTy};
    match t {
        WireTy::Prim(p) if p == "String" || p == "str" || p == "char" => json!(""),
        WireTy::Prim(p) if p == "bool" => json!(false),
        WireTy::Prim(_) => json!(0),
        WireTy::Json | WireTy::Nullable(_) => Value::Null,
        WireTy::Array(_) => json!([]),
        WireTy::Map { .. } => json!({}),
        WireTy::Tuple(items) => Value::Array(items.iter().map(|i| minimal(m, i)).collect()),
        WireTy::Literal(l) => json!(l),
        WireTy::Named(n) => {
            let ty = m
                .types
                .iter()
                .find(|x| &x.name == n)
                .unwrap_or_else(|| panic!("{n}"));
            match &ty.body {
                WireBody::Record { fields } => minimal_record(m, fields),
                WireBody::Alias(a) => minimal(m, a),
                WireBody::StringEnum { values, .. } => json!(values[0]),
                WireBody::Union {
                    repr: UnionRepr::Internal { tag },
                    variants,
                } => {
                    let v = &variants[0];
                    let mut o = match &v.body {
                        VariantBody::Fields(fs) => minimal_record(m, fs),
                        VariantBody::Unit => json!({}),
                        other => panic!("{n}: no minimal value for {other:?}"),
                    };
                    o[tag.as_str()] = json!(v.wire.clone().unwrap());
                    o
                }
                other => panic!("{n}: no minimal value for {other:?}"),
            }
        }
    }
}

fn minimal_record(m: &wire::WireModel, fields: &[wire::WireField]) -> Value {
    let mut o = serde_json::Map::new();
    for f in fields.iter().filter(|f| !f.presence.omittable) {
        let v = if f.presence.nullable {
            Value::Null
        } else {
            minimal(m, &f.ty)
        };
        o.insert(f.wire.clone(), v);
    }
    Value::Object(o)
}

/// serde round trip for an emit::registry owner type. A new registry
/// owner must be added here (else the test fails, not skips).
pub fn registry_back(ty: &str) -> RegistryBack {
    match ty {
        "VideoCallOptions" => back::<aimux_core::video_model::VideoCallOptions>,
        "HttpExchange" => back::<aimux_core::recording::HttpExchange>,
        "InputRecord" => back::<aimux_core::recording::InputRecord>,
        other => panic!("registry owner {other}: add its serde type to registry_back"),
    }
}

/// One generated "field absent" fixture per emit::registry use: the
/// owner's minimal value without that key. `(label, owner, json)`.
pub fn registry_fixtures() -> Vec<(String, String, Value)> {
    let model = wire::from_inputs(&Inputs::from_workspace(&workspace())).unwrap();
    aimux_codegen::emit::registry::uses(&model)
        .into_iter()
        .map(|(e, owner, f)| {
            // Every other non-Option field is written, so only this key's
            // absence is under test (an absent plain `#[serde(default)]`
            // map is a separate Go difference: nil writes null).
            let wire::WireBody::Record { fields } =
                &model.types.iter().find(|t| t.name == owner).unwrap().body
            else {
                panic!("{owner}: registry owner is not a record")
            };
            let mut o = serde_json::Map::new();
            for g in fields
                .iter()
                .filter(|g| !g.presence.nullable && g.wire != f.wire)
            {
                o.insert(g.wire.clone(), minimal(&model, &g.ty));
            }
            let j = Value::Object(o);
            (
                format!("registry-{}-absent:{}", owner, e.path),
                owner.to_string(),
                j,
            )
        })
        .collect()
}
