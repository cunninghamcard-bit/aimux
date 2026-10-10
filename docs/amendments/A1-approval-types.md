# A1：保留工具审批类型（仅透传）

> 基线：arcships/aimux@b90f054f（rfc-0036/provider-config）；#200 设计文档 = `docs/aisdk-architecture-alignment.md`（PR #200 分支 docs/rfc-0036-aisdk-alignment，同文件已在 b90f054f）；RFC-0042 取自 origin/docs/rfc-0042-v4-type-boundaries@f6cbb042（b90f054f 中尚无此文件）。
> 权威顺序（Trey 2026-10-10 更新）：维护者 RFC（#200 + RFC 0036–0042）为主依据；AI SDK 源码仅作交叉核验。冲突处写为“提案，待 Trey 决定；决定前按 RFC 执行”。
> AI SDK：`ai@7.0.127`、`@ai-sdk/provider@4.0.21`（= fixtures/aisdk/VERSIONS.json）；#200 文档基线为 7.0.122 / 4.0.19，两版本下文引用文件已 diff，provider 的 language-model/ 目录完全一致。路径前缀 A=ai/src，P=@ai-sdk/provider/src。

**结论：无冲突。保留，仅透传 provider 执行工具的审批。**

## 1. RFC 依据
- `docs/aisdk-architecture-alignment.md:68`（#200 §0.4 Q1）："消息协议**类型按 AI SDK 完整对齐**(…tool-approval、tool-error、`tool-output-denied` 等 variant 与 wire 全部就位…);**运行时本期保持单步**(…审批 variant 仅透传 provider 执行工具的审批)。**决定**:…不引入多步引擎、宿主工具执行与审批运行时"
- `docs/aisdk-architecture-alignment.md:1952-1953`：tool-approval-request `approvalId, toolCallId, reason?, isAutomatic?, signature?, inputSchemaInput?`；tool-approval-response `approvalId, approved, reason?, providerExecuted?`
- `docs/aisdk-architecture-alignment.md:2009`："tool-approval-response 仅为 provider 执行工具保留到 V4；宿主工具的审批由 core 处理"
- RFC-0042 `rfc/0042-v4-type-boundaries.md:19`："单步 core、不执行宿主工具、不引入宿主审批运行时…完整协议类型不等于实现相应运行能力"

## 2. AI SDK 交叉核验
- P `language-model/v4/language-model-v4-tool-approval-request.ts:9-16,21`：`type: 'tool-approval-request'; approvalId: string; … toolCallId: string;`（另有 `providerMetadata?` :26）
- P `language-model/v4/language-model-v4-prompt.ts:259-282`：`LanguageModelV4ToolApprovalResponsePart { type: 'tool-approval-response'; approvalId; approved: boolean; reason?; providerOptions? }`；在 tool 消息内容联合中 :49
- P `language-model/v4/language-model-v4-content.ts:17`、`language-model-v4-stream-part.ts:72`：approval request 属于 V4 content / stream part
- A `prompt/content-part.ts:281-302`：用户层 schema（request 含 `reason/isAutomatic/signature/inputSchemaInput`，response 含 `approvalId/approved/reason`）
- 一致：RFC 与上游均有审批类型，字段为 camelCase。

## 3. b90f054f 现状
- `aimux-core/src/result.rs:69-78` `RawToolApprovalRequest { approval_id, tool_call_id, provider_metadata? }`，`rename_all = "camelCase"`（V4 输出层）
- `aimux-core/src/result.rs:81-96` `ToolApprovalRequestOutput { approval_id, tool_call, reason?, is_automatic?, signature? }`（core 输出层）
- `aimux-core/src/language_model_message.rs:215-227` `ToolApprovalResponsePart { approval_id, approved, reason?, provider_options? }`；:258 作为 `ToolApprovalResponse` variant
- 透传路径：`aimux-providers/src/openai/responses/provider_events.rs:201`、`aimux-providers/src/openai/responses/convert.rs:433`、`aimux-providers/src/google/model.rs:1023`、`aimux-core/src/generate.rs:776-784,1224-1226`
- 小差：`ToolApprovalRequestOutput` 缺 `inputSchemaInput`（A content-part.ts:289；#200 :1952 列出）。

## 4. Fork 决定与验收
- 决定：保留上述 3 个类型及 variant；不加宿主审批运行时/多步循环。补 `inputSchemaInput?` 属于 #200 :1952 已列字段，跟随 RFC 实施，非本修订新增。
- 验收：
  1. `rg -n "struct (RawToolApprovalRequest|ToolApprovalRequestOutput|ToolApprovalResponsePart)" aimux-core/src` 三项仍在。
  2. JSON 往返测试：`{"type":"tool-approval-response","approvalId":"a","approved":false,"reason":"x"}` 与 `{"type":"tool-approval-request","approvalId":"a","toolCallId":"c"}` 序列化逐字段一致。
  3. `rg -n "execute|approve" aimux-core/src/generate.rs` 不出现宿主审批执行逻辑（steps 仍为 1）。
