# A2：RFC-0035 §3.2 字段命名改为 camelCase

> 基线：arcships/aimux@b90f054f（rfc-0036/provider-config）；#200 设计文档 = `docs/aisdk-architecture-alignment.md`（PR #200 分支 docs/rfc-0036-aisdk-alignment，同文件已在 b90f054f）；RFC-0042 取自 origin/docs/rfc-0042-v4-type-boundaries@f6cbb042（b90f054f 中尚无此文件）。
> 权威顺序（Trey 2026-10-10 更新）：维护者 RFC（#200 + RFC 0036–0042）为主依据；AI SDK 源码仅作交叉核验。冲突处写为“提案，待 Trey 决定；决定前按 RFC 执行”。
> AI SDK：`ai@7.0.127`、`@ai-sdk/provider@4.0.21`（= fixtures/aisdk/VERSIONS.json）；#200 文档基线为 7.0.122 / 4.0.19，两版本下文引用文件已 diff，provider 的 language-model/ 目录完全一致。路径前缀 A=ai/src，P=@ai-sdk/provider/src。

**CONFLICT（RFC 内部文本冲突，不与上游冲突）**：RFC-0035 §3.2 原文写 snake_case；后出的 #200 / RFC-0042 规定 camelCase，b90f054f 实现已是 camelCase。故本修订是**对 RFC-0035 文本的勘误提案**，不改代码。待 Trey 决定；决定前按较新的 RFC-0042 §1（camelCase）执行——实现已一致。

## 1. RFC 原文
- `rfc/0035-host-side-tool-call-repair.md:70-77`："### 3.2 宿主答复(`ToolCallRepairReply`) 内部 tag、snake_case、`deny_unknown_fields`：`{"type": "repaired", "tool_call": {…}}` …"
- 同文件 :58-63（§3.1）示例也用 `tool_call`/`tool_call_id`/`tool_name`/`input_schema`。
- RFC-0042 `rfc/0042-v4-type-boundaries.md:17`："JSON 字段 camelCase，内部 `type` 标签 kebab-case"；:147 "V4 原文与错误证据经过 camelCase 新 wire 后仍须满足 RFC-0035"
- #200 `docs/aisdk-architecture-alignment.md:1739`："JSON 字段统一 camelCase"。注意同文件 :1657 现状表仍写 "`ToolCall`、`ToolResult`、`RawToolCall`…字段 snake_case"，与 b90f054f 代码不符（已过时）。

## 2. AI SDK 交叉核验
- A `generate-text/tool-call-repair-function.ts:22-34`：`(options: { instructions; system; messages; toolCall: LanguageModelV4ToolCall; tools; inputSchema; error; abortSignal? }) => Promise<LanguageModelV4ToolCall | null>`
- P `language-model/v4/language-model-v4-tool-call.ts:6-23`：`type: 'tool-call'; toolCallId; toolName; input: string;`
- 上游无 repaired/unchanged/failed 标签（返回 call 或 null，抛错即失败），这三态是 aimux 宿主协议；字段名应随 camelCase。

## 3. b90f054f 现状
- `aimux-core/src/parse_tool_call.rs:321-330`：注释 wire 为 `{"type":"repaired","toolCall":{…}}`；`#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase", deny_unknown_fields)]`
- `aimux-core/src/parse_tool_call.rs:332-342`：`Repaired { tool_call: RawToolCall }`, `Unchanged`, `Failed { message }`
- `aimux-core/src/tool.rs:121-137` `RawToolCall` `rename_all = "camelCase"`
- RFC-0035 :120 提到的 `contract-tests/fixtures/tool-call-repair.json` 在 b90f054f **不存在**（已核实），无法引用其内容。

## 4. Fork 提案与验收
- 提案：把 RFC-0035 §3.1/§3.2 示例改为 `toolCall/toolCallId/toolName/inputSchema`，“snake_case”改为“type 标签 kebab-case、字段 camelCase”；顺带标注 #200 :1657 为过时现状。错误 payload（:61 `InvalidToolInput.tool_input`）的 wire 名由 RFC-0042 :150 管辖，另行跟随。
- 验收：
  1. `serde_json::from_str::<ToolCallRepairReply>(r#"{"type":"repaired","toolCall":{"toolCallId":"c","toolName":"w","input":"{}"}}"#)` 成功；`"tool_call"` 形态因 `deny_unknown_fields` 失败。
  2. 各绑定 repair helper（如 `bindings/python/python/aimux/_repair.py`）发出的 JSON 用 camelCase。
  3. RFC-0035 文本中 `rg -n "snake_case|tool_call\"" rfc/0035-*.md` 不再有 wire 示例命中。
