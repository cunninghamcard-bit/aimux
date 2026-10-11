# A7：内容/消息类型的状态建模统一为 enum（D2 扩展，Trey 2026-10-11 已决）

> 基线：fork `exp/restart@8455db9a`（下文 file:line 都在这个提交上核过）；`aimux-provider` 只在 `origin/exp/u1.6-aimux-provider-v4@b837aac3` 上（PR 分支，还没合并）。#200 = `docs/aisdk-architecture-alignment.md`（8455db9a 上同文件）。V4 = `@ai-sdk/provider@4.0.21 src/language-model/v4/`（只用来核对）。参考：rig@9e7cb60 `crates/rig-core/src/completion/message.rs:167`(UserContent)、`:187`(AssistantContent)、`message/native.rs:232`(Opaque)；async-openai@c2deb88 `types/responses/response.rs:260`(Item)、`:344/:347`(McpApprovalRequest/Response)。

## 1. 决定
- 互斥的状态用 Rust enum 表达，不再用多个 `Option<bool>` 组合。只有彼此独立的数据才用 `Option`。
- **公共 JSON 不变**（RFC-0042:80、:97；#200:2298）：
  - 不新增 tag 字段，不用 `#[serde(untagged)]`，不用 `flatten`。
  - 已经有 `type` 判别字段的类型，就复用 `type`。
  - 没有 `type` 的（ToolCall/ToolResult），按 #200:2298 用 SDK 已有的 `dynamic` 字段判别。
- 统一的实现方式："wire 镜像 struct + `#[serde(try_from = "XWire", into = "XWire")]`"，ts-rs 用 `#[ts(as = "XWire")]`。
  - XWire 就是现在的 struct，原样搬过去、改成私有。所以 serde 输出和 `bindings/node/src/types/*.ts` 都保持不变，手写绑定（5 个 Types 文件、MultimodalTypes、wrapper.py）也不用碰。
  - 改动只在 Rust 公共 API 上：字段访问换成 match 或访问器。
- **absent 和 `false` 必须分开**：例如 #200:2570、:2597 的示例里显式写了 `"providerExecuted": false`。要做到逐字节往返，enum 里必须记住"字段原来是缺省还是显式 false"（见风险 R1、D25）。

## 2. 清单（8455db9a）
| 类型 | file:line | 现在用来编码状态的 Option/bool | 提议的 enum | wire 判别字段 | serde 能否逐字节复现 |
|---|---|---|---|---|---|
| `ToolCall` | aimux-core/src/tool.rs:143 | `dynamic`:158、`invalid`:167、`error`:171（`provider_executed`:154 是独立数据） | `enum ToolCall{Static(StaticToolCall),Dynamic(DynamicToolCall)}`；Dynamic 里放 `validity: Valid \| Invalid{error}` | `dynamic`（不是 `type`） | 能。走 try_from 加 Wire。⚠R1：absent 和 `dynamic:false`、`invalid:false` 要分开；⚠ 现在可以出现 `invalid:true` 但没有 `dynamic:true`，而 #200:2290 规定 Static 不能带 invalid，所以这种输入要么归一成 Dynamic，要么报错（D26） |
| `ToolResult` | tool.rs:179 | `dynamic`:196；`is_error`:188、`preliminary`:192 | `Static/Dynamic`。is_error/preliminary 留作独立字段，因为 V4 tool-result.ts:28、:39 就是独立可选字段 | `dynamic` | 能，⚠R1 |
| `RawToolCall` | tool.rs:124 | `dynamic`:133（`provider_executed`:130 是独立数据） | **不做 enum**：#200:2724 定为"与 V4ToolCall 合并或用 newtype"；input 改成 `RawArguments(String)`（RFC-0042:80） | — | 能（newtype 用 `serde(transparent)`） |
| `ContentPart` | aimux-core/src/content.rs:20 | 已经是 `type` enum。`ToolCall.provider_executed`:161 和 `ToolApprovalRequest.is_automatic`:56 都是独立数据 | 不变；只把内联的 ToolCall 字段抽成复用的 struct | `type`（kebab） | 能。⚠ `File/FileBase64/FileUrl/FileReference` 是 aimux 自己的 tag，保持不动 |
| `LanguageModelMessage` / `UserPart` / `AssistantPart` / `ToolPart` | language_model_message.rs:33、:233、:242、:256 | 已经是 enum（`role`、`type`） | 不变 | `role`、`type` | 能 |
| `ToolCallPart` | language_model_message.rs:102 | `provider_executed`:108，独立数据 | 不变 | — | 能 |
| `ToolApprovalResponsePart` | language_model_message.rs:218 | `approved: bool`:220 | **保留 bool**：V4 prompt.ts:260 起 `approved` 是数据，`reason` 不论批准与否都可以带 | — | 能 |
| `ToolResultOutput`/`ToolResultContent` | language_model_message.rs:135、:181 | 已经是 `type` enum | 不变 | `type` | 能 |
| `MessageContent` / `ModelPrompt` | aimux-core/src/message.rs:28、:82 | **已有 `#[serde(untagged)]`** | 不在本 amendment 范围内，不改（要改就得改 JSON）。按审查标准(3)标为"无法满足、原样保留" | 无 | 不适用（标红） |
| `GenerateContent<C,A,R>` | aimux-core/src/result.rs:24 | 已经是 `type` enum | 不变；`ToolCall(C)`、`ToolResult(ToolResult)` 跟着 U2/U3 变 | `type` | 能 |
| `RawToolApprovalRequest` | result.rs:72 | 无 | 不变 | — | 能 |
| `ToolApprovalRequestOutput` | result.rs:84 | `is_automatic`:92，独立数据 | 不变；`tool_call` 跟着 U2 变 | — | 能 |
| `StreamPart<C,A,R>` | aimux-core/src/stream_part.rs:29 | `ToolInputStart.provider_executed`:80、`dynamic`:84 | 只把 `dynamic` 换成复用 U2 的 `ToolKind{Static{explicit_false},Dynamic}`，做法和 Wire 一样 | `type` | 能，⚠R1。enum 变体里的字段要靠自定义 `with` 模块才能保持 absent/false |
| `TextStreamPart` | stream_part.rs:176 | 同上：:236、:240 | 同上 | `type` | 同上 |
| `GenerateTextResult` | aimux-core/src/generate.rs:216 | 无状态型 Option | 不变（`tool_calls` 跟着 U2 变） | — | 能 |
| `parsed_tool_call_arguments` | aimux-core/src/openai_output.rs:1031-1033 | 参数 `invalid: Option<bool>` | 改成接收 `&ToolCall` 后 match | 内部函数 | 不涉及 wire |
| `response_tool_call_input` | aimux-core/src/response_messages.rs:25 | 参数 `invalid: Option<bool>` | 同上 | 内部函数 | 不涉及 wire |
| `LanguageModelV4ToolCall` / `ToolResult`（**PR 分支** u1.6@b837aac3） | aimux-provider/src/content.rs:84（:89、:91）、:99（:105、:107、:109） | provider_executed、dynamic、is_error、preliminary | **不做 enum**：V4 DTO 按 #200:2151-2180、V4 tool-call.ts:29、:35、tool-result.ts:28-45 原样逐字段对应，全部是 `Option<bool>` | `type`（content enum :117） | 能，不改 |

## 3. provider 执行（服务端）工具在 1.10 里怎么建模：透传，不按工具种类分变体
- #200:2151 写明"provider 输出采用完整 V4 content 与 stream part"。:2157-2177 `V4ToolCallPayload`、`V4ToolResultPayload` 用的是 `provider_executed: Optional<bool>`；:2201-2211 `LanguageModelV4Content` 只有一个 `ToolCall`、一个 `ToolResult` 变体；:2233 tool-input-start 也带 `providerExecuted?`；:2522 只按 providerExecuted 决定回放到哪条消息。
- V4：`language-model-v4-tool-call.ts:7`（`type:'tool-call'`）、:29（`providerExecuted?`）；`tool-result.ts:8`；`stream-part.ts:57`；prompt `language-model-v4-prompt.ts:195`、:216。上游没有任何按服务端工具种类划分的 part。
- #200:68 规定审批只透传 provider 执行的工具；:2009 规定 tool-approval-response 只为 provider 执行工具保留。
- 结论：1.10 里**服务端工具是透传**，复用同一个 ToolCall/ToolResult 加上 `providerExecuted` 字段；工具本身的载荷留在 `input`、`result` 这两个 JSON 值里。这和 rig 的 `Opaque`（native.rs:232）思路一致。async-openai 那种"每种 item 一个变体"（response.rs:260）是 OpenAI Responses 原生 wire 的做法，属于 L0 native（PHASES 阶段 5），不放进 V4/core。
- `providerExecuted` 和 `dynamic` 互相独立（一次调用可以既是 provider 执行又是 dynamic），所以它**不进**Static/Dynamic enum，保留 `Option<bool>`。即使以后要建模成 `Executor{Unspecified,Client,Provider}`，也必须保住三态，等价于 Option<bool>。#200 对此没有反对意见，这不算新决定。

## 4. 风险
- R1 absent 和 false：`Option<bool>` 是三态，enum 如果只有两态，`"dynamic":false` 往返时会被吞掉，JSON 就变了。做法是 Static 里记一个 `dynamic_explicit_false: bool`（属于 wire 保真用的私有信息），或者确认 fixture 里从来没有 false。仓库 JSON 里，这五个字段显式写成 false 的只有一处：bedrock cassette 里的 `"isError": false`（`aimux-providers/tests/cassettes/bedrock/test_bedrock_model_with_code_execution_tool_1.json`）。那是 provider 的 HTTP 原文，而且 isError 本来就不进 enum。不过 provider 转换代码会构造 `Some(false)`（例如 `aimux-providers/src/xai/responses/convert.rs`，这几处是否都和 dynamic/invalid 有关，还要逐一核对），所以不能假定运行时不会出现 false。默认：保真（D25）。
- R2 `invalid:true` 但没有 `dynamic:true`：现在的 JSON 能表示这种状态，enum 不能。见 D26。
- R3 ts-rs：要是直接在 enum 上 derive TS，会生成 TS 联合，绑定就变了。必须用 `#[ts(as="XWire")]`，并且 `gen_ts_types.py --check` 保持零差异。
- R4 `ToolCall` 没有 `PartialEq`（tool.rs:140），因为 error 是 AiMuxError。往返测试比 JSON（`serde_json::Value` 加上原文字节），不比 Rust 值。
- R5 泛型 `StreamPart<C,..>`/`GenerateContent<C,..>` 的 ts-rs `concrete(...)`（stream_part.rs:28）不能动。
- R6 `MessageContent`/`ModelPrompt` 原本就是 untagged，不满足审查标准(3)，只记录，不在本 amendment 里改。
- R7 绑定基线：Python、Flutter、4 个 Node job 在 8455db9a 上原本就红，只要求不新增失败。

## 5. 待决问题
- ~~D25~~ 已决：按 AI SDK 规范化，见下。
- ~~D26~~ 已决：报错，见下。

## D25 / D26 已决（Trey 2026-10-11）：规范形式与验收
依据见 DECISIONS.md D25/D26 已决条（行号都在 ai@7.0.127 / @ai-sdk/provider@4.0.21 里核过，并用 node 实跑过）。R1 原来的"Static 里加 explicit_false 标记"作废。

| 面 | 字段 | 输入 | 规范输出 | AI SDK 依据 |
|---|---|---|---|---|
| core ToolCall（step content / stream tool-call） | dynamic | absent、false → Static；true → Dynamic | Static 不写；Dynamic 写 `true` | parse-tool-call.ts:276、:280-287 |
| 同上 | invalid / error | true（必须 dynamic:true 且带 error）→ Invalid；absent、false → Valid | Invalid 写 `invalid:true`+`error`；Valid 不写 | parse-tool-call.ts:126-128 |
| core ToolResult / ToolError | dynamic | absent、false → Static；true → Dynamic | **总是写**布尔 `false`/`true` | execute-tool-call.ts:169、:199 |
| StreamPart/TextStreamPart ToolInputStart | dynamic | absent、false → Static；true → Dynamic | **总是写**布尔 | stream-language-model-call.ts:809 |
| 以上所有面 + V4 tool-call/tool-result/stream-part + prompt / response message 的 tool-call | providerExecuted | 三态 | **原样透传**，不归一（AI SDK 本身就是透传） | parse-tool-call.ts:130,273,284；to-response-messages.ts:95；convert-to-language-model-prompt.ts:342；stream-language-model-call.ts:807-808 |
| V4 DTO（provider 层）dynamic | — | 三态 | 透传，不改（V4 不进 enum） | provider tool-call.ts:35、stream-part.ts:58 |

注意：provider 执行的 tool-result 在 AI SDK 里 `dynamic` 是透传的（stream-language-model-call.ts:774、:789），可能缺省。aimux 统一写布尔，这会和 AI SDK 在"provider 没给 dynamic"时的输出不同（AI SDK 不写，aimux 写 false）。可以接受，因为 TS 类型 `dynamic?: false` 两种都允许；如果要完全对齐，就得在 provider 执行的结果上保留三态。E.3 先按"总是写"做，差异记进样本。

D26 处理：`try_from(ToolCallWire)` 遇到 `invalid==Some(true) && dynamic!=Some(true)` 返回 Err（AI SDK 类型和生成端都不会产生这种组合，aimux 的 parse_tool_call.rs:307-309 也不会）。`invalid:true` 但缺 error 也返回 Err。

验收（替换 E.2–E.4 原来的"逐字节"要求）：
1. 规范化往返逐字节一致：样本先过一遍规范化（按上表），`serialize(deserialize(x)) == normalize(x)`，而且 `serialize(deserialize(normalize(x))) == normalize(x)`。
2. 等价样例：`{"dynamic":false}` 和缺省反序列化成同一个变体（`assert_eq!`），序列化结果也相同；ToolCall 里 `invalid:false` 和缺省同理。
3. providerExecuted：true/false/absent 三条样本往返逐字节不变。
4. D26：`{"invalid":true,"error":…}` 不带 dynamic、`{"dynamic":false,"invalid":true}` 都 try_from 返回 Err。

### 与 #200 的冲突（修订说明）
- #200:2582 的 core ToolCall 示例写了 `"dynamic": false`。AI SDK 静态调用不写 dynamic（parse-tool-call.ts:280-287，实测也是这样）。改为不写这个字段。
- #200:2575-2583 的 core 示例把 V4 里的 `"providerExecuted": false`（#200:2570）丢掉了，但 AI SDK 会原样保留（parse-tool-call.ts:284，实测 step content 输出 `"providerExecuted":false`）。core 示例应该保留 `"providerExecuted": false`。#200:2597 回填消息保留 false，这一点和 AI SDK 一致（to-response-messages.ts:95）。
- #200:2290 `dynamic?:false, invalid?:false` 和类型一致（tool-call.ts:24-25），但只说明"允许写 false"；规范输出是不写。#200:2292 TypedToolResult `dynamic` 必填，和 execute-tool-call.ts:169/199 一致，但 provider 执行的路径是透传（见上面的注意）。
- #200:2291 `invalid?:bool` 只出现在 Dynamic 里，和 D26 一致，不冲突。

## 更正 2026-10-11：服务商执行的 tool-result/tool-error 的 dynamic
上面那一节说"总是写"，作废。Trey 定了以 AI SDK 为准，AI SDK 在这条路径上会把 dynamic 原样传过去（`stream-language-model-call.ts:774,789`，`convert-language-model-content.ts:114,130,150,166`）。所以服务商执行（providerExecuted 为 true）的结果里，dynamic 和 providerExecuted 一样原样保留三态；只有客户端执行的结果才规范化成总是写布尔（`execute-tool-call.ts:169,199`）。E.3 的验收要加一条样例：服务商执行、并且不写 dynamic 的结果，来回转换后逐字节不变。

## E.3 建模定稿（2026-10-11，dr eggbot 确认，属于落实"以 AI SDK 为准"）
ToolResult 按来源分 enum：
- 客户端执行：分 Static 和 Dynamic 两个变体，`dynamic` 规范化输出为布尔（`execute-tool-call.ts:169,199`）。
- 服务商执行：单独一个变体，`dynamic` 在里面作为透传数据 `Option<bool>` 保存，原样往返（`stream-language-model-call.ts:774,789`）。
E.3 验收追加一条：服务商执行且不写 `dynamic` 的样例，往返逐字节不变。
