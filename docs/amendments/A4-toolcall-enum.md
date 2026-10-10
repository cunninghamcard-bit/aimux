# A4：tool.rs 中 ToolCall/ToolResult/RawToolCall 改为 Rust enum

> 基线：arcships/aimux@b90f054f（rfc-0036/provider-config）；#200 设计文档 = `docs/aisdk-architecture-alignment.md`（PR #200 分支 docs/rfc-0036-aisdk-alignment，同文件已在 b90f054f）；RFC-0042 取自 origin/docs/rfc-0042-v4-type-boundaries@f6cbb042（b90f054f 中尚无此文件）。
> 权威顺序（Trey 2026-10-10 更新）：维护者 RFC（#200 + RFC 0036–0042）为主依据；AI SDK 源码仅作交叉核验。冲突处写为“提案，待 Trey 决定；决定前按 RFC 执行”。
> AI SDK：`ai@7.0.127`、`@ai-sdk/provider@4.0.21`（= fixtures/aisdk/VERSIONS.json）；#200 文档基线为 7.0.122 / 4.0.19，两版本下文引用文件已 diff，provider 的 language-model/ 目录完全一致。路径前缀 A=ai/src，P=@ai-sdk/provider/src。

**CONFLICT → 降级为提案**：RFC-0042 禁止为工具 wire 新增非 SDK 状态标签、禁止 untagged 公共联合，并把 RawToolCall 的去向定为“与 V4ToolCall 合并或 newtype”。"公共 JSON 不变的内部 enum" 未被 RFC 禁止但也未被决定。**提案，待 Trey 决定；决定前按 RFC（保留 struct + dynamic/invalid 字段编码）执行。**

## 1. RFC 依据
- `rfc/0042-v4-type-boundaries.md:80`："本文不用额外 `status` tag 改造 SDK 的工具 wire…不能暴露 `decode_any_tool_call`。Rust 内部如需 `RawArguments(String)` newtype 防止误用，可以使用；它不是公共联合，也不改变 V4 String wire。"
- 同文件 :71-78（§3.2）：不用 `#[serde(untagged)] Raw(String)/Parsed(Value)`
- 同文件 :97（§3.3）："内部 `ParseOutcome::Valid/Invalid` 可保证错误随非法结果存在；外部静态/动态联合按 #200 的 dynamic/invalid 字段编码，不新增非 SDK 状态标签"
- 同文件 :43-46：V4 输出 input=String、core 输出 input=JSON，各层 DTO 分离
- #200 `docs/aisdk-architecture-alignment.md:2724`："`RawToolCall` | 与 V4ToolCall 的原始 payload 合并或使用明确 newtype"

## 2. AI SDK 交叉核验
- A `generate-text/tool-call.ts:20-29` `StaticToolCall`（`dynamic?: false; invalid?: false; error?: never`）、:35-54 `DynamicToolCall`（`dynamic: true; invalid?; error?`）、:60-62 `TypedToolCall = StaticToolCall | DynamicToolCall` —— 上游确为和类型，判别字段是 `dynamic`，不是 `type`。
- A `generate-text/tool-result.ts:10,26,40` 同样 Static/Dynamic 联合。
- P `language-model/v4/language-model-v4-tool-call.ts:6-41`、`language-model-v4-tool-result.ts:7-51`：V4 层是单一对象（`type:'tool-call'`/`'tool-result'`），非联合。

## 3. b90f054f 现状
- `aimux-core/src/tool.rs:121-137` `RawToolCall` struct（input: String）
- `aimux-core/src/tool.rs:140-172` `ToolCall` struct：`input: Value`，`dynamic/invalid/error` 为 Option 字段（invalid 时 error 不受类型约束）
- `aimux-core/src/tool.rs:176-201` `ToolResult` struct
- 三者均 `rename_all = "camelCase"`，无 `type` 字段。

## 4. 参考实现（核实）
- rig@9e7cb600 `crates/rig-core/src/completion/message.rs:424` `pub struct ToolCall`（仍为 struct）；:186-191 `#[serde(tag = "type")] enum AssistantContent { … ToolCall(ToolCall) }`；:373 `enum ToolResultContent`
- async-openai@c2deb888 `async-openai/src/types/chat/chat_.rs:448-456` `#[serde(tag = "type")] enum ChatCompletionMessageToolCalls { Function(..), Custom(..) }`；:461 `struct ChatCompletionMessageToolCall`
- 两者都是“enum 包 struct、按 wire 已有判别字段打标签”，并非把单条调用拆成状态 enum。

## 5. 提案与验收
- 提案（按 RFC 可兼容的最小形态）：
  - `ToolCall` → `enum ToolCall { Static(StaticToolCall), Dynamic(DynamicToolCall) }`，`DynamicToolCall` 内用 `Option<InvalidInfo{error}>` 使 error 只随 invalid 存在；**手写 Serialize/Deserialize** 以 `dynamic` 字段判别，输出与现 JSON 逐字节一致，不加 `type`/`status`，不用 untagged。
  - `ToolResult` 同理 Static/Dynamic。
  - `RawToolCall` **不做 enum**：按 #200 :2724 并入 V4ToolCall 或 `RawArguments(String)` newtype（RFC-0042 :80）。
  - 若 Trey 不批准：保持 struct，仅加 `RawArguments` newtype。
- 验收：
  1. RFC-0042 §3.2 测试向量（:84-91 表）在 provider DTO 与 core DTO 双向往返通过，provider 原文逐字节相等。
  2. 现有 ToolCall/ToolResult JSON 快照前后 diff 为空；`rg -n 'serde\(untagged\)|tag = "status"' aimux-core/src/tool.rs` 零命中。
  3. `{"toolCallId":"c","toolName":"x","input":{},"invalid":true}` 在缺 `dynamic:true` 时被拒或规范化为 Dynamic（需在实现时与 #200 规则对齐）。
