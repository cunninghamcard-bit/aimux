#!/usr/bin/env python3
"""Write tests/common/go_fixtures.rs and go_fixtures_empty.rs (unit 2.2 on v4).

One Rust literal per Go type that both the hand-written bindings/go
(types.go + multimodal_types.go + repair.go RawToolCall) and the generated Go declare; enums get one
fixture per variant. Values are built from generated/manifest.json field
lists; the test serializes them with serde, so serde stays the truth.
`full` fills every optional with a value; `empty` uses None / [] / "" / 0.

Usage: python3 tools/aimux-codegen/scripts/gen_go_fixtures.py GEN_TYPES_GO
(GEN_TYPES_GO: the `aimux-codegen --emit go` types.go, for the Go names).
`--check` fails if the committed files differ.
"""
import collections, json, re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CG = ROOT / "tools/aimux-codegen"
m = json.loads((CG / "generated/manifest.json").read_text())
ALL = m["types"] + m.get("auxiliaryTypes", [])
ALIASES = m.get("aliases", {})
cnt = collections.Counter(t["name"] for t in ALL)
IR = {}
BY_RUST = collections.defaultdict(list)
for t in ALL:
    ir = t["name"] if cnt[t["name"]] == 1 else (t.get("tsName") or t["name"])
    IR[ir] = t
    BY_RUST[t["name"]].append(t)

def go_names(path):
    return set(re.findall(r"^type ([A-Za-z0-9_]+)", Path(path).read_text(), re.M))

hand = go_names(ROOT / "bindings/go/types.go") | go_names(ROOT / "bindings/go/multimodal_types.go")
# repair.go needs cgo; only its RawToolCall (type + JSON codec) is compared.
hand |= {"RawToolCall"} & go_names(ROOT / "bindings/go/repair.go")
ARGS = [a for a in sys.argv[1:] if a != "--check"]
CHECK = "--check" in sys.argv[1:]
gen = go_names(ARGS[0])
COVERED = sorted(hand & gen)

KEYWORDS = {"type", "ref", "move", "match", "loop", "fn", "impl", "use", "mod", "self", "crate", "static", "const", "struct", "enum", "trait", "where", "async", "await", "dyn", "box", "in", "for", "if", "else", "let", "mut", "pub", "return", "super", "true", "false", "unsafe", "while", "as", "break", "continue", "extern", "override", "priv", "typeof", "yield", "abstract", "become", "do", "final", "macro", "virtual", "try", "gen"}

# Types whose fields are private or whose serde is hand-written: a
# constructor expression (full, empty).
SPECIAL = {
    "Timestamp": ("aimux_core::timestamp::Timestamp::from_unix_millis(1_700_000_000_000).unwrap()",) * 2,
    "NamespacedKind": ('aimux_core::types::NamespacedKind::new("x.y").unwrap()',) * 2,
    "AiMuxError": ('aimux_core::error::AiMuxError::InvalidArgument("x".to_string())',) * 2,
    "Size": ("aimux_core::shared::Size::new(2, 3)",) * 2,
    "AspectRatio": ("aimux_core::shared::AspectRatio::new(16, 9)",) * 2,
    "FileBytes": ('aimux_core::shared::FileBytes::Base64("AQI=".to_string())', "aimux_core::shared::FileBytes::Binary(vec![])"),
    "ImageFileData": ('aimux_core::image_model::ImageFileData::Base64("AQI=".to_string())', "aimux_core::image_model::ImageFileData::Binary(vec![])"),
    "VideoFileData": ('aimux_core::video_model::VideoFileData::Base64("AQI=".to_string())', "aimux_core::video_model::VideoFileData::Binary(vec![])"),
    "AudioData": ('aimux_core::speech_model::AudioData::Base64("AQI=".to_string())', "aimux_core::speech_model::AudioData::Binary(vec![])"),
    "AudioInput": ('aimux_core::transcription_model::AudioInput::Url("https://example.com/a.wav".to_string())', "aimux_core::transcription_model::AudioInput::Binary(vec![])"),
    "ImageOutputs": ('aimux_core::image_model::ImageOutputs::Base64(vec!["AQI=".to_string()])', "aimux_core::image_model::ImageOutputs::Base64(vec![])"),
    "ToolChoice": ("aimux_core::tool::ToolChoice::Auto", "aimux_core::tool::ToolChoice::Auto"),
}
# Extra top-level fixtures (one per variant) for hand-written-serde enums.
SPECIAL_TOP = {
    "AudioData": ['aimux_core::speech_model::AudioData::Base64("AQI=".to_string())', "aimux_core::speech_model::AudioData::Binary(vec![1, 2, 255])"],
    "ImageOutputs": ['aimux_core::image_model::ImageOutputs::Base64(vec!["AQI=".to_string()])', "aimux_core::image_model::ImageOutputs::Binary(vec![vec![1, 2, 255]])"],
    "ToolChoice": ["aimux_core::tool::ToolChoice::Auto", "aimux_core::tool::ToolChoice::None", "aimux_core::tool::ToolChoice::Required", 'aimux_core::tool::ToolChoice::Tool { tool_name: "x".to_string() }'],
}
# Types whose fixtures are skipped from `back` (Serialize only).
NO_DESERIALIZE = {t["name"] for t in ALL if t["source"].endswith("openai_output.rs")}

def module(src):
    p = src.removeprefix("aimux-core/src/").removesuffix(".rs").removesuffix("/mod")
    return "aimux_core::" + p.replace("/", "::")

def lookup(name, src=None):
    c = BY_RUST.get(name, [])
    if len(c) == 1:
        return c[0]
    for t in c:
        if t["source"] == src:
            return t
    return c[0] if c else None

def prim(p, empty):
    if p == "String":
        return "String::new()" if empty else '"x".to_string()'
    if p == "bool":
        return "false" if empty else "true"
    if p in ("f32", "f64"):
        return "0.0" if empty else "1.5"
    if p == "char":
        return "'x'"
    return "0" if empty else "1"

def rust_ty(text):
    """Very small Rust type parser for codec `rust` texts -> TypeRef-like."""
    text = text.replace(" ", "")
    mo = re.fullmatch(r"(?:[A-Za-z_]+::)*([A-Za-z_][A-Za-z0-9_]*)(?:<(.*)>)?", text)
    if not mo:
        return {"json": True}
    name, args = mo.group(1), mo.group(2)
    parts, depth, cur = [], 0, ""
    for ch in args or "":
        if ch == "<": depth += 1
        if ch == ">": depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur); cur = ""
        else:
            cur += ch
    if cur: parts.append(cur)
    a = [rust_ty(x) for x in parts]
    if name in ("String", "str", "bool", "u8", "u16", "u32", "u64", "i32", "i64", "f32", "f64", "usize"):
        return {"prim": name}
    if name == "Value": return "json"
    if name == "Option": return {"option": a[0]}
    if name in ("Vec",): return {"array": a[0]}
    if name in ("HashMap", "BTreeMap", "Map", "IndexMap"): return {"map": {"key": a[0], "value": a[1]}}
    if name in ("Box", "Arc"): return a[0]
    return {"ref": {"name": name, "args": a}}

def expr(t, empty, depth, src, generics):
    if t == "json" or (isinstance(t, dict) and "json" in t):
        return "serde_json::json!({})" if empty else 'serde_json::json!({"k": 1})'
    k, v = next(iter(t.items()))
    if k == "prim": return prim(v, empty)
    if k == "option":
        return "None" if empty or depth > 4 else f"Some({expr(v, empty, depth + 1, src, generics)})"
    if k == "triState":
        return "None" if empty or depth > 4 else f"Some(Some({expr(v, empty, depth + 1, src, generics)}))"
    if k == "array":
        if v == {"prim": "u8"}:
            return "vec![]" if empty else "vec![1, 2, 255]"
        return "vec![]" if empty or depth > 4 else f"vec![{expr(v, empty, depth + 1, src, generics)}]"
    if k == "map":
        if empty or depth > 4: return "Default::default()"
        return f"[({expr(v['key'], False, depth + 1, src, generics)}, {expr(v['value'], empty, depth + 1, src, generics)})].into_iter().collect()"
    if k == "tuple":
        return "(" + ", ".join(expr(x, empty, depth + 1, src, generics) for x in v) + ",)"
    if k == "generic":
        g = generics.get(v)
        return expr(g, empty, depth, src, {}) if g else "serde_json::json!({})"
    if k == "codec":
        return expr(rust_ty(v["rust"]), empty, depth, src, generics)
    if k == "ref":
        return named(v["name"], v.get("args", []), empty, depth + 1, src, generics)
    raise SystemExit(f"unknown typeref {t}")

def named(name, args, empty, depth, src, generics, variant=None):
    if name in SPECIAL:
        return SPECIAL[name][1 if empty else 0]
    if name in ALIASES and name not in BY_RUST:
        return expr(ALIASES[name]["type"], empty, depth, src, generics)
    d = lookup(name, src)
    if d is None:
        return "Default::default()"
    g = dict(zip(d.get("generics", []), args))
    for k, c in d.get("concrete", {}).items():
        g.setdefault(k, rust_ty(c))
    path = f"{module(d['source'])}::{d['name']}"
    sh = d["shape"]
    if d.get("proxy") or sh["kind"] == "handWritten":
        return "Default::default()"
    if depth > 9:
        return "Default::default()"
    if sh["kind"] == "struct":
        return path + " {\n" + fields(sh, empty, depth, d["source"], g) + "}"
    if sh["kind"] == "newtype":
        return f"{path}({expr(sh['inner'], empty, depth, d['source'], g)})"
    if sh["kind"] == "unit":
        return path
    if sh["kind"] == "enum":
        vs = sh["variants"]
        v = vs[variant] if variant is not None else next((x for x in vs if not x.get("other")), vs[0])
        vp = f"{path}::{v['rust']}"
        vsh = v["shape"]
        if vsh["kind"] == "unit": return vp
        if vsh["kind"] == "newtype": return f"{vp}({expr(vsh['inner'], empty, depth, d['source'], g)})"
        if vsh["kind"] == "tuple": return vp + "(" + ", ".join(expr(x, empty, depth, d["source"], g) for x in vsh["items"]) + ")"
        if vsh["kind"] == "struct": return vp + " {\n" + fields(vsh, empty, depth, d["source"], g) + "}"
    return "Default::default()"

def fields(sh, empty, depth, src, g):
    out = ""
    for f in sh["fields"]:
        r = f["rust"]
        r = f"r#{r}" if r in KEYWORDS else r
        out += f"    {r}: {expr(f['type'], empty, depth, src, g)},\n"
    for s in sh.get("skipped", []):
        out += f"    {s}: Default::default(),\n"
    return out

def qualify(text):
    """Qualify bare DTO names in a `ts(concrete)` type text."""
    def one(mo):
        n = mo.group(0)
        d = lookup(n)
        return f"{module(d['source'])}::{n}" if d else n
    return re.sub(r"\b[A-Z][A-Za-z0-9]*\b", one, re.sub(r"\s", "", text))

def fixtures(empty):
    rows = []
    for go in COVERED:
        d = IR[go]
        sh = d["shape"]
        path = f"{module(d['source'])}::{d['name']}"
        conc = [d["concrete"][g] for g in d.get("generics", [])] if d.get("generics") else []
        args = [rust_ty(c) for c in conc]
        tyargs = ("<" + ", ".join(qualify(c) for c in conc) + ">") if conc else ""
        back = "None" if d["name"] in NO_DESERIALIZE else f"Some(back::<{path}{tyargs}>)"
        if d["name"] in SPECIAL_TOP and not empty:
            for i, e in enumerate(SPECIAL_TOP[d["name"]]):
                rows.append((f"cov-{go}-{i}", go, path, e, back))
        elif sh["kind"] == "enum" and not empty:
            for i, v in enumerate(sh["variants"]):
                rows.append((f"cov-{go}-{v['rust']}", go, path + tyargs, named(d["name"], args, empty, 0, d["source"], {}, i), back))
        else:
            label = ("empty-" if empty else "cov-") + go
            rows.append((label, go, path + tyargs, named(d["name"], args, empty, 0, d["source"], {}), back))
    return rows

def render(rows, fn, doc):
    s = f"""//! {doc}
//! Generated by tools/aimux-codegen/scripts/gen_go_fixtures.py from
//! generated/manifest.json (v4); edit the script, not this file.

#![allow(clippy::too_many_lines, clippy::unreadable_literal, clippy::default_trait_access)]

use serde_json::Value;

use super::{{Back, back}};

fn v<T: serde::Serialize>(x: &T) -> Value {{
    serde_json::to_value(x).expect("serde serializes the aimux-core value")
}}

/// `(label, Go type name, serde output, serde decode of the Go output)`.
pub fn {fn}() -> Vec<(&'static str, &'static str, Value, Option<Back>)> {{
    vec![
"""
    for label, go, path, e, back in rows:
        s += f'        (\n            "{label}",\n            "{go}",\n            {{\n                let x: {path} = {e};\n                v(&x)\n            }},\n            {back},\n        ),\n'
    s += "    ]\n}\n"
    return s

def fmt(text):
    return subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"], input=text,
                          capture_output=True, text=True, check=True).stdout


OUTPUTS = {
    CG / "tests/common/go_fixtures.rs": fmt(render(fixtures(False), "coverage_fixtures",
        "Fixtures for every hand-written Go type that has a generated counterpart: one real aimux-core value per type (per variant for enums), every optional filled.")),
    CG / "tests/common/go_fixtures_empty.rs": fmt(render(fixtures(True), "empty_fixtures",
        "Empty-value fixtures, one per covered type: Options None, arrays/maps/strings empty.")),
}
if CHECK:
    stale = [str(p) for p, t in OUTPUTS.items() if not p.exists() or p.read_text() != t]
    if stale:
        sys.exit("stale (rerun gen_go_fixtures.py): " + ", ".join(stale))
else:
    for p, t in OUTPUTS.items():
        p.write_text(t)
print(len(COVERED), "covered types")
