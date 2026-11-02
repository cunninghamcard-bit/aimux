# 待 Trey 决定事项（DECISIONS）

> 行号来源：#200 = `b90f054f:docs/aisdk-architecture-alignment.md`；RFC-0035/0036 = b90f054f `rfc/`；RFC-0037 / RFC-0038(decision) = `3f0673f2:rfc/`；RFC-0038(size) = `origin/docs/rfc-0038-size-performance-budgets`；RFC-0039/0040/0041/0042 = 各自 `origin/docs/rfc-00xx-*` 分支（0042 @f6cbb042）；fork = `exp/restart@8341fe7b`。只读核对。
> 编号固定，新事项从末尾续编，不重排。状态：未决 = 先按"当前默认"执行。

## RFC-0042 T1–T5（来源：`rfc/0042-v4-type-boundaries.md`，上游分支 docs/rfc-0042-v4-type-boundaries@f6cbb042，第 27–35 行）

RFC-0042 在 :25 把 T1–T5 列为"本文新增、需要接受的决定"，维护者还没有接受。fork 以 RFC 为主线，所以下表的"fork 状态"指 fork 里照着做的状态，不代表上游已经接受。

| 编号 | 决定（RFC 原文概括） | RFC 行号 | fork 状态 |
|---|---|---|---|
| T1 | 公共 wire 上不引入 `ToolInput::Raw/Parsed`，用不同层 DTO 的 `String` 和 JSON value 区分 | :29 | accepted-in-fork |
| T2 | 删除 `aimux-stream` 的公共 tracker 和第二套 `ToolCallStreamPart`，在 provider-utils 里只保留一个协议内部累积器 | :30、:35 | pending-Trey（RFC :35 明确写了待决） |
| T3 | 无法唯一归属的工具增量不静默丢弃，终止模型流并报 `InvalidResponseData` | :31、:35、:123 | pending-Trey（见 D9；与 #204 `861f26e` 冲突；RFC :35 要求定下来之前阻塞实现合入） |
| T4 | 非法调用保留原文的结构化错误证据，贯穿所有 wire 和 repair 路径；不能靠 stringify(parsed input) 恢复原文 | :32 | accepted-in-fork |
| T5 | D8 以 #200 的统一 manifest 为入口，自定义 serde 要提供显式 codec 描述；不允许第二条 TS 生成链 | :33 | accepted-in-fork（另见 A3/D 系列里的 ts-rs 退役条件） |

#204 的处置：接受归属方向（T2），不接受"整套累积算法直接删掉就算完成"（RFC :30）；ambiguity-drop 和 T3 的冲突见 D9。

1. **D1 — A2：RFC-0035 §3.2 改为 camelCase 文字**
   - 问题：RFC-0035 写 snake_case，实现与 RFC-0042 为 camelCase，是否修 RFC-0035 文字。
   - 选项：a) 改 RFC-0035 示例与"snake_case"字样；b) 不改，靠 RFC-0042 覆盖。
   - 当前默认：按 RFC-0042 camelCase（代码已一致），RFC 文字暂不改。
   - 依据：RFC-0035 `rfc/0035-host-side-tool-call-repair.md:70-77`；RFC-0042:17、:147；#200:1739（过时现状 #200:1657）；代码 `aimux-core/src/parse_tool_call.rs:324-330`。
   - 附注（2026-10-10 20:41 UTC）：#226 的 Kotlin/Java DecisionModelTest（kt:71、java:70）把 provider config 写成 `base_url`，统一改成 `baseUrl`。provider config 的形状来自栈，不属于 #226 的 decision wire；PR #7 的 `provider_handle_test` 已经这样改过。

2. **D2 — A4：ToolCall/ToolResult 是否改 Rust enum**
   - 问题：内部 Static/Dynamic enum（wire 不变）是否采纳。
   - 选项：a) enum + 手写 serde，按 `dynamic` 判别、不加 tag；b) 保持 struct，仅加 `RawArguments` newtype。
   - 当前默认：b) 保持 struct。
   - 依据：RFC-0042:80（不加 status tag、newtype 可用）、:97（按 dynamic/invalid 字段编码）、:71-78（禁止 untagged）；#200:2724；代码 `aimux-core/src/tool.rs:124,143,179`；上游 A `generate-text/tool-call.ts:60-62`。

3. **D3 — A5-1：录制删除 `config_snapshot()`**
   - 问题：#226 的连接/能力快照被删，仅存 providerId/provider/modelId（能力移入 input）。
   - 选项：a) 保持删除（作为对 #226 的批准修订）；b) 恢复 #226 快照。
   - 当前默认：b) 恢复 #226 快照，连同 `runtime_decision_test` 里被删的两条断言（`api_key_source == "explicit"`、用 None 重建报错）一起恢复（dr eggbot 2026-10-10 19:21 UTC 定）。
   - 依据：删除侧 #200:199、:1436、:1470；RFC-0041:35、:41；RFC-0040:197。#226 侧 RFC-0037:120-121；RFC-0038:198-199。代码 3f0673f2 `aimux-core/src/decision_model.rs:516,625` → fork `:611`、`aimux-core/src/recording.rs:162`。

4. **D4 — A5-3：provider_handle 的 env 读取**
   - 问题：库内按 `<PROVIDER>_API_KEY`/`*_BASE_URL` 隐式读 env（#226）还是仅 descriptor 声明 + 宿主允许（RFC-0040）。
   - 选项：a) 恢复 #226 行为；b) 按 RFC-0040 走 descriptor/preset。
   - 当前默认：a) 恢复 #226。
   - 依据：#226 侧 RFC-0038:24-25、:30-39；代码 3f0673f2 `aimux-providers/src/provider.rs:376,386-414,450-451`、`aimux-ffi/src/lib.rs:1482-1492`。RFC-0040 侧 :32-33、:71；两边一致处 RFC-0038:31-36 与 RFC-0040:155（base URL env 保留）。fork 现状 `aimux-ffi/src/lib.rs:1678`、`aimux-providers/src/default_providers.rs:90-104`。
   - 附：fork 中 `laya` 不在 `provider_registry.json`/`default_providers.rs`，经 ProviderHandle 不可达（RFC-0038:21,34）——不论选哪项都需修。

5. **D5 — A5-4：`retry_config()` 与 body_overrides 检查**
   - 问题：恢复 #226 的模型级重试配置与 body_overrides 显式拒绝，还是保留栈侧删除。
   - 选项：a) 恢复 #226；b) 保持删除（重试走 core `maxRetries`，body_overrides 作为未知字段被拒）。
   - 当前默认：a) 恢复 #226。
   - 依据：#226 侧 RFC-0038:25、:45-46；RFC-0037:73；代码 3f0673f2 `aimux-core/src/decision_model.rs:522,611`、`aimux-providers/src/openai/decision.rs:29-31,400`、`aimux-providers/src/runtime_decision.rs:35-37,621`。删除侧 #200:29、:1434、:159（D15）、:1458；RFC-0040:178；fork `aimux-core/src/decision_model.rs:597`、`aimux-ffi/src/lib.rs:979,1626`。
   - 注：b90f054f 栈内已无任何 `fn retry_config`，恢复需在 decision 内单独保留。

6. **D6 — RFC-0038:25「重试配置」措辞**
   - 问题：若 D5 选 b，是否把"复用…重试配置"改读为 core `maxRetries`。
   - 选项：a) 修 RFC-0038 文字；b) 不改。
   - 当前默认：b) 不改（随 D5 默认恢复 #226）。
   - 依据：RFC-0038:25；RFC-0037:73。

7. **D7 — decision-openai-full.json 图片 wire**
   - 问题：PR #7 把 `images[].data` 从 `{"Base64": "…"}` 改成纯字符串，影响 8 个绑定契约测试。
   - 选项：a) 回退为 #226 的 `{"Base64": …}`；b) 保留纯 base64（与 #200/RFC-0042 base64 适配一致）。
   - 当前默认：a) 回退 #226。
   - 依据：`contract-tests/fixtures/decision-openai-full.json`（`git diff 3f0673f2..8341fe7b`，hunk @@ -6,9 +6,7 @@）；RFC-0042:17「字节以 base64 适配」；RFC-0038:209-210（各绑定共用相同 JSON 与图片契约测试）。

8. **D8 — 阶段 3 仅末尾全绿**
   - 问题：README「每阶段全绿」与 #200 原子切换冲突。
   - 选项：a) 阶段 3 只在末尾全绿，中间只要求 `cargo check`；b) 每单元全绿。
   - 当前默认：a)。
   - 依据：#200:1509-1513；RFC-0042:230；RFC-0039:25；PHASES.md。

9. **D9 — T2/T3 与 #204 `861f26e`**
   - 问题：无法归属的工具增量：报错（T3）还是丢弃（#204）；tracker 归属（T2）。
   - 选项：a) 按 RFC-0042 返回 `InvalidResponseData`；b) 按 #204 ambiguity-drop。
   - 当前默认：a) 报错。
   - 依据：RFC-0042:30-31、:35、:123。

10. **D10 — 大文件拆分范围**
    - 问题：README 写 6 个超千行文件、29 个 `_new`；b90f054f 实测 35 个 >1000 行 `.rs`、30 个 FFI `aimux_*_new`。
    - 选项：a) 只拆本计划触及的文件；b) 全部 35 个。
    - 当前默认：a)。
    - 依据：实测命令 `git ls-files '*.rs' | xargs wc -l`、`git ls-files aimux-ffi | xargs grep -ho "aimux_[a-z_]*_new\b" | sort -u`（b90f054f）；#200 §2.1 :273。
   - 口径核实（PR #13 `bd4d1dc5`，`scripts/gen_closure_inventory.py` 可以重现）：名字正好以 `_new` 结尾的有 29 个（与 README、头文件一致；PHASES 写的 30 是我算错了，以 29 为准），算上 `_new*` 是 51 个；超过千行的 .rs 文件全部算 35 个，不算 tests 是 23 个。README 写的"6 个大文件"对不上任何口径。默认仍然是只拆这次改动会碰到的文件。

11. **D11 — manifest 记录的 AI SDK 版本**
    - 选项：a) VERSIONS.json `ai@7.0.127`/`@ai-sdk/provider@4.0.21`；b) #200 基线 7.0.122/4.0.19。
    - 当前默认：a)。
    - 依据：`fixtures/aisdk/VERSIONS.json`；#200:7；RFC-0042:222。

12. **D12 — L0 passthrough 时机**
    - 问题：RFC-0036 列在 0.6，RFC-0039 排除出当前切换；RFC-0040 要求显式批准 G2。
    - 选项：a) 按 RFC-0039，切换之后（PHASES 阶段 5）；b) 按 RFC-0036 放入 0.6。
    - 当前默认：a)。
    - 依据：RFC-0036:157；RFC-0039:24；RFC-0040:220。

13. **D13 — RFC 编号冲突**
    - 问题：两份 RFC-0038（decision-provider 在 master，size-budgets 在分支）；#200 与 RFC-0036 编号冲突。
    - 选项：a) 提请维护者重编号，fork 内以全名区分；b) fork 内自行重编号。
    - 当前默认：a)（fork 不改 RFC 文件）。
    - 依据：`origin/master:rfc/0038-decision-provider-expansion.md`、`origin/docs/rfc-0038-size-performance-budgets:rfc/0038-size-performance-budgets.md`；RFC-0039:48。

14. **D14 — 带 `body_overrides` 的 handle 对非 decision 模型的行为**
   - 问题：b90f054f 栈上没有生成类模型的 body overrides，所以 D4/D5 恢复后，带 `body_overrides` 的 handle 对 language/embedding/image 模型都返回 UnsupportedFunctionality（aimux 实现在 PR #7 `d3af3473` 报告，我还没核）。
   - 选项：a) 维持报错；b) 给生成类模型补上 body overrides；c) 只对 decision 模型接受这个字段。
   - 当前默认：a) 维持报错，不扩大范围。
   - 依据：#226 的拒绝检查在 3f0673f2 `openai/decision.rs:29-31`、`runtime_decision.rs:35-37`；#200 D15（:159）。

15. **D15 — 本地 runtime 没配 key 时的 Authorization header**
   - 问题：#226 对 ollama/llamacpp/localai/vllm/sglang/laya 没配 key 时用占位 key `"not-needed"`（3f0673f2 `aimux-providers/src/provider.rs:443-454`，回放侧见 `replay.rs:249`），fork 现在不发 Authorization header。已核实 #226 会发 `Bearer not-needed`。
   - 选项：a) 按 #226 发占位 key；b) 不发 header。
   - 当前默认：a) 按 #226 发 `Bearer not-needed`。已核实 #226 确实会发：`decision_support.rs:54` → `openai/model.rs:65-70` `build_auth_headers` 不判断 key，直接插入 header（aimux 实现核，aimux RFC 复核 model.rs）。
   - 依据：见上述代码；秘密边界 RFC-0040:71。

16. **D16 — `check_provider_boundaries.sh` 拦下恢复的 #226 代码**
   - 问题：栈上的边界脚本（CI 在 `ci.yml:402` 跑）禁止 `fn config_snapshot`、`fn retry_config`、`RetryConfig`、`api_key_source`、providers 里的 `aimux_core::retry`。D3/D4/D5 恢复 #226 后，脚本在 `e48974d9` 上退出码 1（`provider_handle.rs:139` 读 `options.max_retries` 也被拦）。栈的边界规则和"#226 优先"直接冲突。来源：aimux 审查复审 PR #7。aimux 实现和 aimux 审查都在 detached worktree 里复跑过：`8341fe7b` 上退出码为 0，所以违规全部来自恢复 #226 的那几个提交。
   - 选项：a) 只对 decision 相关文件加临时白名单；b) 删掉这几条边界规则；c) 改回删除 #226 这几处（等于 D3/D4/D5 选删除）。
   - 当前默认：a) 只在 decision 相关文件加白名单（含 `provider_handle.rs` 的 `PLACEHOLDER_API_KEY`，即 D15 的 not-needed），每条注明"D16 待 Trey 定"（dr eggbot 2026-10-10 20:08 UTC 定）。
   - 依据：跟 D3/D4/D5 是同一组冲突：边界规则对应 #200 D15（:159）和 RFC-0040；#226 行为对应 RFC-0037/0038。D16 的结论应和 D3/D4/D5 一起定。
   - 实际范围（aimux 审查 20:58 UTC 在 `eed376d0` 上核）：`d16_allow` 除了 decision 文件，还包括 `aimux-core/src/recording.rs`、`aimux-core/src/retry.rs` 和 5 个测试文件。D3 的快照在 recording.rs，D5 的 RetryConfig 在 retry.rs，这两个文件躲不开。这些文件里那 4 个名字现在不会被拦。Trey 定 D3/D5 时，白名单要跟着一起收紧或删掉。

17. **D17 — 直接构造的 decision 模型和 `body_overrides`**
   - 问题：#226 在 `openai/decision.rs:29-31`、`runtime_decision.rs:35-37`（3f0673f2）拒绝 `body_overrides`。栈上直接构造用的配置类型已经没有这个字段（aimux 审查在 `d3af3473` 上核过；具体 file:line 待 aimux 实现补上），所以原位检查无从恢复，只剩 `provider_handle.rs:153-166` 那一份。
   - 选项：a) 不加回字段：直接构造的模型根本传不进 `body_overrides`，等于结构上就拒绝了；b) 把字段加回栈的配置类型，再恢复原位检查。
   - 当前默认：a) 不改栈结构。PR 描述写明拒绝点在 handle，加一个测试证明直接构造的路径上没有这个字段可传。
   - 依据：#200 D15（:159）主张配置用纯数据、不加覆盖字段；#226 的行为在上述两处。

18. **D18 — 空字符串工具参数：invalid（RFC）还是 `{}`（AI SDK）**
   - 问题：RFC-0042 §3.2 的向量第 6 行规定空字符串参数算 invalid，"不补成 `{}`"。现在 `parse_tool_call.rs:236-237` 把空串和纯空白当成 `{}`，AI SDK 也是这样（aimux 实现核对）。
   - 选项：a) 按 RFC 改成 invalid；b) 跟 AI SDK 一样保持 `{}`，同时修订 RFC-0042 的这一行。
   - 当前默认：a) 按 RFC。向量现在就入库，标 `#[ignore]`，等第 3 阶段 3.3 启用时再改解析器，在那之前行为不变（dr eggbot 21:52 UTC 定）。
   - 依据：RFC-0042:84-91（§3.2 向量表）；代码 `parse_tool_call.rs:236-237`。

19. **D19 — 重复 key 的工具参数**
   - 问题：`{"a":1,"a":2}` 期望得到什么。现在按 serde_json 得到 `{"a":2}`，JS 的 `JSON.parse` 也是后一个生效。
   - 选项：a) 后一个生效；b) 报 invalid。
   - 当前默认：a) 后一个生效，Rust 和 AI SDK 一致。如果 RFC-0042 §3.2 写了别的期望，以 RFC 为准，并且改记到 D18 那一类冲突里。
   - 依据：RFC-0042:84-91；aimux 实现用 probe 核过。

20. **D20 — 手写绑定类型里的 `inputSchemaInput`：已确认没有缺口（22:32 UTC 结项）**
   - 结论：Kotlin :1087、Swift :900、Dart :1900、Python :370-376、Java :4497 这几处都是输出或 stream 类型，按修订后的 1.2 本来就不该有这个字段。手写的 prompt part 都已经有它：kt:485、java:1510、swift:651、dart:832、py:662（aimux 实现核对，PR #12 `054b4bc0`）。不用 Trey 定。
