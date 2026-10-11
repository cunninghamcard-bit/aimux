# A9 — 1W（wire/transport）与 1A（AI SDK 对齐）核对结果（2026-10-11）

基线：fork exp/restart@8455db9a。RFC：#200 = `docs/aisdk-architecture-alignment.md`@8455db9a（下称 ALIGN）。注意调研稿引的是 `@22580efd`，它**不是** 8455db9a 的祖先，两版该文件差 22+/16-，下表行号一律按 8455db9a。
AI SDK 按 `fixtures/aisdk/VERSIONS.json`@8455db9a 钉的版本核对：openai 4.0.83（/tmp/aio/package）、provider 4.0.21、ai 7.0.127、provider-utils 5.0.53、openai-compatible 3.0.62（后两个本次 npm pack 到 /workspace/aisdk-src）。8455db9a 上的 VERSIONS.json 只有版本号、没有 integrity（1.11 还没合入）。
open PR：只有 #24–#27（E.0/E.1/E.2/E.3a），对本文引用的行只改 `openai/model.rs:238` 一行和 tracker 一行，行号不变。R.2 分支（`exp/u1R.2-remove-retry-config`，没有 open PR）删了 `retry.rs:18-42`，合入后 retry.rs 的行号要前移约 25 行。
类别：SDK = 和 AI SDK 对齐；OWN = aimux 自己的；MNT = 维护者 RFC 自己定的。

## 1. 逐条核对

| 条目 | 结论 | 正确证据 / 说明 | RFC 依据 | 类别 |
|---|---|---|---|---|
| W1 Fetch 边界 | 对 | fetch.rs:160-171、:180-198 | ALIGN:394-421、:275-284 | MNT |
| W2 encode 纯函数 | **部分已有** | OpenAI chat 已经有共用的 body 构建：`build_request_body_with_chat_options`（openai/convert.rs:1094），generate 和 stream 都调它（model.rs:182、:339），之后才走 `post_json_to_api`（:185、:345）。其他协议族没逐个核 | ALIGN:112；RFC-0036:41、:48（rig 的说法 RFC 没写） | OWN |
| W3 decode 共用 | 部分核实 | finish 和 usage 在两条路径共用：`parse_finish_reason`（model.rs:271、:642），`convert_usage`（:278、:537）。内容解析是不是共用没核。调研引的行号都对：transform_stream.rs:60，response_handler.rs:51、105、222、242，三处 finish 映射 | — | OWN |
| W4 鉴权在 fetch 装饰器 | 对，**但描述有误** | azure/mod.rs:250-273 不是"构造时写入"，而是 `BearerTokenFetch` 这个 Fetch 装饰器在请求时才写（`BearerTokenFetch` 在 :244，`impl Fetch` 在 :250-273，没有 authorization 时才补）。sigv4：`SigV4Fetch` 在 sigv4_fetch.rs:292，实现 Fetch 在 :333 | ALIGN:460-470 | MNT |
| W5 SSE 不进 Fetch | 对 | ALIGN:247-273（约束 2） | 同左 | MNT |
| W6 preset 数据化 | **已基本完成**（调研稿没提） | openai_compatible/config.rs:1-8 写明"SDK 用函数的地方这里都是数据"；`ConvertUsage` enum 在 :57（OpenAICompatible/Alibaba/MoonshotAI）；`BaseUrl::Preset` 引用 `PresetDescriptor`（:34-41） | ALIGN:156（D12，运行时 descriptor 表）、:765、:771 | MNT |
| W7 crate 拆分没开始 | 对 | 8455db9a 根目录只有 core/provider-utils/providers/stream/ffi。u1.6 分支另有 `aimux-provider` | ALIGN:247-273 | MNT |
| A1 请求构建 | 对 | factory_test 引的行号对（:1-8、:482、:518）。SDK 4.0.83 openai-chat-language-model.ts:393、:499 | — | SDK |
| A2 compat 字段顺序 | 部分；**RFC 行号错** | 顺序规定在 ALIGN:789-800，不是 803-811（那几行讲 body_overrides）。SDK openai-compatible@3.0.62 chat model 的 args 在 :270-325，顺序和 ALIGN 一致。aimux 实现的顺序没核 | ALIGN:789-800、:217 | SDK |
| A3 headers/key | 对 | headers.rs:64、api_key.rs:17；factory_test :575、:702、:746、:10-12。provider-utils@5.0.53 有 combine-headers.ts 和 load-api-key.ts | ALIGN:458 | SDK |
| A4 重试默认值 | 对 | retry.rs:12-16；ai@7.0.127 retry-with-exponential-backoff.ts:66-68（和 7.0.122 逐字节相同） | ALIGN:1012 | SDK |
| A5 jitter | 对；**RFC 行号错** | retry.rs:274-280，jitter 只用在指数退避上，而且只会把延迟变短（`.min(exponential)`）。规定在 ALIGN:1136（以及 :1012），不是 1137（1137 讲 helper 超时）。**Retry-After 已核**：error.rs:585-615 和 SDK :20-57 的语义一致（ms 优先、浮点秒、HTTP-date、`0<=ms`、`<60s 或 <指数退避`）；测试 retry.rs:513-534 | ALIGN:1136 | jitter = MNT；Retry-After = SDK |
| A6 重试所在层 | 对 | retry 在 aimux-core。ALIGN:282（调研写的 281 是 providers 那一行，差了一行） | ALIGN:282 | MNT |
| A7 APICallError | **"is_retryable 没测"不对** | openai 自己的测试确实没断言（openai_upstream_e2e_test.rs:176-198），但 deepseek_chat_test.rs:1447/1461/1472、groq_test.rs:1839、bedrock_upstream_e2e_test.rs:241 都断言了。**":175 注释写的是 should parse tool results" 不对**：:174 是 "OpenRouter resource exhausted error"，"should parse tool results" 在 :146，是给 `chat_tool_call` 的，没有写错。error.rs:34、:75、:527-533 对；SDK api-call-error.ts:17、:28-32，post-to-api.ts:128、:154（5.0.53） | — | SDK |
| A8 流事件顺序 | 缺口成立；**"成本最低"的前提不对** | chat-stream-basic.json 的 `result.parts` 是 **ai 层 streamText** 的输出（start/start-step/text-*/finish-step/finish，`sdk.operation=streamText`），不是 provider 层的 V4 parts（SDK :543 是 stream-start）。只有 text-start/delta/end 的 id `"0"` 和文本能直接比 | — | SDK |
| A9 finish 映射 | **有差异**（调研说已对齐，不对） | openai/convert.rs:1214-1221 把 `function_call` 映射成 Other；SDK map-openai-finish-reason.ts:13-15 映射成 tool-calls。compat 的 convert.rs:683 已经处理了；responses :2261 对 | — | SDK |
| A10 非流式工具调用 | 对 | SDK :430 | — | SDK |
| A11 流式工具分片 | **不是"没测"** | tracker 测试：streaming_tool_call_tracker_test.rs:227、:314（多个调用同时进行）、:618（交错）、:783。但没有和 AI SDK fixture 做对照 | RFC-0042:118-124（T3，见 S.2） | SDK |
| A12 usage 总数 | 对 | SDK convert-openai-chat-usage.ts（4.0.83）:26-45 | — | SDK |
| A13 usage 细分 | **"没找到代码、没有测试"不对** | 实现在 openai/model.rs:57-90；compat 测试 openai_compatible_upstream_e2e_test.rs:104、:108 断言了 cacheRead/reasoning。**偏离**：原生 OpenAI 的 `convert_usage` 优先读顶层 `cached_tokens`（Moonshot 格式，model.rs:69-76），SDK openai 只读 `prompt_tokens_details.cached_tokens`（:28） | A.5 | 细分 = SDK；顶层 cached_tokens：D28 已决（Trey 2026-10-11）按 SDK 删掉，只读 nested |
| A14 providerOptions 命名空间 | 对 | factory_test:513 | ALIGN:779 | SDK |
| A15 fallback 规则 | 没法核（没有 fixture）；**RFC 行号错** | 规则在 ALIGN:775-785，不是 786-797 | ALIGN:775-785 | SDK/MNT |
| A16 其他厂商 | 没法核对 SDK（没有 pin 源码，也没有 fixture） | e2e 测试有：azure 16、bedrock 14、vertex 8、xai 8 个 fn，deepseek_chat_test、groq_test 也有。ALIGN:1152（调研写的 1150 是录制重建那一行） | ALIGN:1152 | — |
| A17 版本不一致 | 对，范围要补 | 调研引了 provider-utils@5.0.51，pin 是 5.0.53；还漏了 openai-compatible 3.0.62、anthropic 4.0.71、google 4.0.87。本文用 pin 版本复核过 A1/A4/A7/A8/A9/A12 的 SDK 行号，都对得上（只有 A9 要看 :13-15） | ALIGN:1152 | OWN |

另外顺带核过 rig@9e7cb600：driver.rs:123-129、wire.rs:704、:734 对得上。rig 只作参考，不算 RFC 依据。

## 2. 调研稿的错误汇总
1. ALIGN 基线是 22580efd，不是 8455db9a 的祖先，行号会偏：A2（应为 789-800）、A5（应为 1136）、A6（应为 282）、A15（应为 775-785）、A16（应为 1152）。
2. A9 说已对齐，实际 OpenAI chat 的 `function_call` 映射有差异。
3. A13 说没有代码、没有测试，实际两样都有，还有一个 OWN 偏离。
4. A11 说没有测试，实际有单元测试。
5. A7 说 is_retryable 没测，实际只是 openai 没测；"注释不符"的说法不对。
6. A8：fixture 是 ai 层的，不能直接拿来和 provider 层的流对比。
7. W4：azure 是请求时由装饰器写入，不是构造时写。
8. W6：调研没提已经完成的部分；W2 也没提 OpenAI chat 已经有共用的 body 构建。

## 3. RFC 和 AI SDK 不一致的地方（等 Trey 定，定之前按 RFC）
- M1 jitter：RFC（ALIGN:1136）保留 jitter，SDK 没有 jitter。按 RFC，不用开新决定，记在 §5 允许差异表里就行。
- M2 重试所在层：RFC 放在 `aimux` 层（ALIGN:282），SDK 也在 ai 包，两边一致，只是现状在 core。阶段 3 拆 crate 时处理。
- M3 原生 OpenAI 读顶层 `cached_tokens`：RFC 没写，SDK 没有这个行为（`convert-openai-chat-usage.ts:28`，4.0.83）。D28 已决（Trey 2026-10-11）：按 AI SDK 实际行为，只读 `prompt_tokens_details.cached_tokens`，单元 A.5。
- 待查：A2 aimux 实现的顺序、W2/W3 其他协议族、A15、A16（要先 pin 并 pack 相应的 SDK 包）。

## W.0：W1/W4/W5/W6 保持现状（行号按 `exp/e6-enum-family-audit@ba369292`）

| 条目 | 现状 file:line | 为什么不改 |
|---|---|---|
| W1 Fetch 边界 | `aimux-provider-utils/src/fetch.rs:160-171`：`trait Fetch` 只有 `fetch(FetchRequest) -> FetchResponse`，一次请求一次响应，不跟随重定向（重定向由 http.rs 的 helper 逐跳决定）；`:180` `FetchFunction = Arc<dyn Fetch>`；默认传输在请求时解析（`:182` 起，`default_fetch` `:184`） | 已经是 ALIGN:394-421、:275-284 要的边界：传输只管一跳 HTTP，编码/解码、重试、重定向都在它外面。再拆没有收益 |
| W4 鉴权放在 fetch 装饰器 | Azure `BearerTokenFetch`（`aimux-providers/src/azure/mod.rs:244`，`impl Fetch` `:250-273`）：请求时才 resolve token，只有没有 `authorization` 时才补；SigV4 `SigV4Fetch`（`aimux-provider-utils/src/sigv4_fetch.rs:292`，`impl Fetch` `:333`）对最终请求签名 | 符合 ALIGN:460-470：凭据在请求时、在合并完 per-call headers 之后才写，encode 函数不需要知道鉴权（OpenAI chat 见下面“W.1 的鉴权位置”）。A9 原表里"构造时写入"的描述是错的，代码本来就是对的 |
| W5 SSE 不进 Fetch | SSE 解析在 `aimux-provider-utils/src/response_handler.rs:378` `create_event_source_response_handler`，作用在 Fetch 返回的响应 body 上；`Fetch` trait（fetch.rs:164）不认识 SSE | 就是 ALIGN:247-273 约束 2：传输只交字节，流式协议在 helper 层解析。不改 |
| W6 preset 数据化 | `aimux-providers/src/openai_compatible/config.rs:1-8`（SDK 用函数的地方这里都是数据）；`BaseUrl::Preset { descriptor: &'static PresetDescriptor }` `:36-37`；`ConvertUsage` enum `:57` | ALIGN:156（D12）、:765、:771 要的"descriptor 是数据"已经做到；W.* 不再动 |

W.1 的鉴权位置（签名以 #39 `exp/w1-openai-chat-encode@c4e80246` 为准）：纯函数 `encode_chat(url, headers, model_id, &CallOptions, stream, parse_chat_options) -> Result<EncodedChatRequest, AiMuxError>`（`aimux-providers/src/openai/model.rs:202-209`）不碰鉴权：它收到的 `headers` 已经解析好，自己不读凭据、不加 `authorization`。`authorization` 只在 `encode_request`（`model.rs:160`）里加：它调 `OpenAIModelConfig::request_headers`（`aimux-providers/src/openai/config.rs:139`）解析凭据。原生 OpenAI 的 `Bearer <key>` 来自 provider headers：`create_openai` 用 `ProviderHeaders::bearer(...)` 构造（`openai/mod.rs:249`），请求时由 `self.headers.resolve()`（`config.rs:143`）解析出来；`:153-162` 只是 `token_provider` 分支，只有合并后的 headers 里没有 `authorization` 时才补 `Bearer <token>`。然后把结果交给 `encode_chat`（`model.rs:169`）。

W7（crate 拆分）属于阶段 3，不在 1W。W2/W3 只做 OpenAI chat（W.1、W.2），其他协议族列为待查。
