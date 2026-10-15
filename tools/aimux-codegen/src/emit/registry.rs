//! The serde functions emitters must reproduce, shared by every emitter:
//! custom `skip_serializing_if` predicates and `#[serde(default = "fn")]`
//! functions, by full path (`crate::<module>::..`, as the manifest records
//! them, extract.rs `qualify`).
//!
//! A manifest field naming a function that is not here, or an entry no
//! manifest field names, is an error ([`check`], run by every emitter's
//! `check` and by `--check`).

use crate::wire::{VariantBody, WireBody, WireField, WireModel};

/// What the function does on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `skip_serializing_if`: the key is left out when the value is `value`.
    Skip,
    /// `default = "fn"`: an absent key decodes as `value`.
    Default,
}

/// One registered function. `value` is JSON text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    pub path: &'static str,
    pub value: &'static str,
}

/// Every registered function.
pub const REGISTRY: &[Entry] = &[
    // aimux-core/src/recording.rs:78 `*self == Self::LanguageModel`.
    Entry {
        kind: Kind::Skip,
        path: "crate::recording::RecordingOperation::is_language_model",
        value: "\"language_model\"",
    },
    // aimux-core/src/video_model.rs:220 `fn default_video_n() -> u32 { 1 }`.
    Entry {
        kind: Kind::Default,
        path: "crate::video_model::default_video_n",
        value: "1",
    },
    // aimux-core/src/recording.rs:346 `fn default_finalized() -> bool { true }`.
    Entry {
        kind: Kind::Default,
        path: "crate::recording::default_finalized",
        value: "true",
    },
];

/// The entry for `path` of `kind`.
#[must_use]
pub fn lookup(kind: Kind, path: &str) -> Option<&'static Entry> {
    REGISTRY.iter().find(|e| e.kind == kind && e.path == path)
}

/// `value` of a [`Kind::Skip`] entry as the wire string it skips.
#[must_use]
pub fn skip_string(e: &Entry) -> Option<String> {
    serde_json::from_str::<String>(e.value).ok()
}

/// Every record and struct-variant field, with its owner name.
#[must_use]
pub fn fields(model: &WireModel) -> Vec<(&str, &WireField)> {
    let mut out = Vec::new();
    for t in &model.types {
        match &t.body {
            WireBody::Record { fields } => out.extend(fields.iter().map(|f| (t.name.as_str(), f))),
            WireBody::Union { variants, .. } => {
                for v in variants {
                    if let VariantBody::Fields(fs) = &v.body {
                        out.extend(fs.iter().map(|f| (t.name.as_str(), f)));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Each `(entry, owner, field)` use in `model`.
#[must_use]
pub fn uses(model: &WireModel) -> Vec<(&'static Entry, &str, &WireField)> {
    let mut out = Vec::new();
    for (owner, f) in fields(model) {
        for (kind, p) in [(Kind::Skip, &f.skip_fn), (Kind::Default, &f.default_fn)] {
            if let Some(e) = p.as_deref().and_then(|p| lookup(kind, p)) {
                out.push((e, owner, f));
            }
        }
    }
    out
}

/// Every named function is registered, every entry is named, and every
/// value is valid JSON (a skip value a JSON string).
///
/// # Errors
///
/// One message per problem.
pub fn check(model: &WireModel) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    for e in REGISTRY {
        let ok = match e.kind {
            Kind::Skip => skip_string(e).is_some(),
            Kind::Default => serde_json::from_str::<serde_json::Value>(e.value).is_ok(),
        };
        if !ok {
            errs.push(format!(
                "emit::registry {}: value `{}` is not valid",
                e.path, e.value
            ));
        }
    }
    for (owner, f) in fields(model) {
        for (kind, p, what) in [
            (Kind::Skip, &f.skip_fn, "skip_serializing_if"),
            (Kind::Default, &f.default_fn, "default"),
        ] {
            if let Some(p) = p
                && lookup(kind, p).is_none()
            {
                errs.push(format!(
                    "{owner}.{}: #[serde({what} = \"{p}\")] is not in emit::registry::REGISTRY; no emitter can write it",
                    f.wire
                ));
            }
        }
    }
    let used = uses(model);
    for e in REGISTRY {
        if !used.iter().any(|(u, _, _)| std::ptr::eq(*u, e)) {
            errs.push(format!(
                "emit::registry {}: not found in the manifest (no field names it)",
                e.path
            ));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{Presence, WireBody, WireField, WireModel, WireTy, WireType};

    fn field(wire: &str, skip: Option<&str>, dflt: Option<&str>) -> WireField {
        WireField {
            wire: wire.into(),
            rust: wire.into(),
            ty: WireTy::Prim("bool".into()),
            presence: Presence {
                omittable: true,
                ..Presence::default()
            },
            aliases: vec![],
            skip_fn: skip.map(str::to_string),
            default_fn: dflt.map(str::to_string),
        }
    }

    #[test]
    fn unused_and_bogus_entries_error() {
        // Empty model: every entry is unused.
        let empty = WireModel { types: vec![] };
        let e = check(&empty).unwrap_err();
        assert_eq!(e.len(), REGISTRY.len(), "{e:?}");
        assert!(
            e.iter().all(|x| x.contains("not found in the manifest")),
            "{e:?}"
        );
        // A field naming a path that is not in REGISTRY.
        let m = WireModel {
            types: vec![WireType {
                name: "R".into(),
                doc_source: "x.rs".into(),
                body: WireBody::Record {
                    fields: vec![field("n", None, Some("crate::x::bogus"))],
                },
            }],
        };
        let e = check(&m).unwrap_err();
        assert!(e.iter().any(|x| x.contains("crate::x::bogus")), "{e:?}");
    }
}
