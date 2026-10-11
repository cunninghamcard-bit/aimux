# A5：#226 decision provider 在 fork 合并（PR #7）中的处理

> 基线：upstream master `3f0673f2`（#226，维护者自加）合入 `b90f054f`，得到 fork `exp/restart@8341fe7b`（PR #7，冲突向栈侧解决）。全程只读。
> 规则：#226 为维护者功能，行为原样保留；冲突时 #226 语义优先，除非 Trey 批准修订。
> 文档行号：RFC-0037 / RFC-0038 = `3f0673f2:rfc/0037-decision-api.md` / `rfc/0038-decision-provider-expansion.md`（master 版 0038，与 size-budgets 版同号）；RFC-0040 = `origin/docs/rfc-0040-provider-auth`；RFC-0041 = `origin/docs/rfc-0041-replay-governance`；#200 = `b90f054f:docs/aisdk-architecture-alignment.md`。

**CONFLICT 概要**：第 1、2、3 项上 #226 自带的 RFC（0037/0038）与 #200 / RFC-0040 / RFC-0041 互相矛盾，都是维护者文档。第 1、2 项 PR #7 的做法有 #200/0040/0041 的行号依据；第 3 项列为 **needs Trey**，不替 Trey 选。第 4 项 #200 有明确删除依据。

---

## 第 1 项：录制只保留 providerId/provider/modelId，删除 `config_snapshot()`

**#226 原文**
- RFC-0037:120-121「能力快照存入 provider.profile」
- RFC-0038:198-199「provider snapshot 保存连接/能力和默认协议，实际选定协议以 call options 为准」；:40「连接参数与录制中的 key 来源一致，回放不存明文凭据」

**#226 代码（3f0673f2）**
- `aimux-core/src/decision_model.rs:516` 默认 `fn config_snapshot()`，在 profile 中写 `decision_capabilities`；:625 `record_provider(&ctx.call_id, &model.config_snapshot())`
- 覆盖实现：`aimux-providers/src/openai/decision.rs:420`、`aimux-providers/src/runtime_decision.rs:661`
- 回放读取：`aimux-core/src/decision_replay.rs`（从 `provider.profile.decision_capabilities` 取能力）

**PR #7（8341fe7b）**
- `aimux-core/src/decision_model.rs:611` 改为 `ProviderRecord::from_model(provider, model_id)`；能力移入输入：`aimux-core/src/recording.rs:162` `decision_capabilities`；`decision_replay.rs` 改读 `first.input.decision_capabilities`
- 能力信息保留（从 provider.profile 移到 input）；RFC-0038:198 所说的「连接」快照被删除。

**支持删除的依据（存在）**
- #200:199「必须禁止的是下游通过公开 config、`config_snapshot()` 等接口读取配置，再自行推导凭证、实例身份或重建方式」；#200:1436 删除清单列出 `LanguageModel::config_snapshot()`；#200:1470 列出 `ProviderRecord`、`record_provider`、`api_key_source`
- RFC-0041:41「`ProviderRecord = registry row + protocol` 不再是目标；不保存 settings、key 来源」；RFC-0041:35「删除 parser/allowlist/config snapshot 重建链」
- RFC-0040:197（B02）「无旧构造类型/旧 config_snapshot 重建引用」

**#226 测试**：`aimux-providers/tests/runtime_decision_test.rs:565` `local_factories_and_wrapper_factories_preserve_credentials_for_replay`（断言 `config_snapshot().api_key_source == "explicit"`）→ PR #7 **改名并改写**为 :592 `local_runtimes_rebuild_from_the_caller_registry_without_recorded_credentials`，删掉 `config_snapshot`/`api_key_source` 断言。`openai_decision_test.rs:297` `unified_recording_replays_refusals_and_rebuilds_the_same_provider_config` 保留名称，改用 registry 重建（fork :326）。

**建议**：保留 PR #7 的做法（有 #200:199/1436、RFC-0041:41 依据）。但这改变了 #226 语义（RFC-0038:198 的连接快照），按规则写为 **修订提案，待 Trey 批准**；未批准前应视作偏离 #226。验收：同一份录制可离线 mock 回放能力与结果；录制 JSON 中不出现 base URL、key 来源。

## 第 2 项：回放只从调用方 registry 取 endpoint/key，删除 api-key 参数

**#226 原文**
- RFC-0037:124-125「不带 --mock 时可重建官方 Jev 执行请求回放」
- RFC-0038:207「record→mock replay、重建 provider→HTTP replay」

**#226 代码（3f0673f2）**
- `aimux-providers/src/replay.rs:31` `rebuild_decision_provider(&ProviderRecord, api_key)`
- `tools/aimux-replay/src/main.rs:161` `rebuild_decision_provider(&rec.provider, api_key)`

**PR #7（8341fe7b）**
- `aimux-providers/src/replay.rs:25-39` `rebuild_decision_provider(p: &ProviderRecord, registry: &ProviderRegistry)`：按 `provider_id` 从 registry 取 provider，再核对 `provider` 身份
- `tools/aimux-replay/src/main.rs:39` CLI 仍有 `api_key`；:167-169 Jev 用显式 key 或 `JevConfig::from_env()`（TYPESAFE_API_KEY）；:176-177、:200-212 由宿主组装 registry 后传入。即 **CLI 的 key 参数仍在，只是库函数不再接收 key**。

**支持依据（存在）**
- RFC-0041:60「provider 包/descriptor 提供已验证的离线构造配方；CLI 组装 registry，不把 registry 工厂搬进 matcher」
- RFC-0041:41「不从录制 URL/child 列表反推 provider」；#200:1607「live replay 依赖宿主目标引用，不保存或重建 ProviderRecord」
- RFC-0040:73「更换 base URL 必须新建 provider，不继承旧连接的 key」

**#226 测试**：`openai_decision_test.rs:297`（fork :326）与 :340 `images_typed_choices_and_score_descriptions_roundtrip_and_key_replay`（fork :370）把 `rebuild_decision_provider(&snapshot, Some("replacement-key"))` 改为传 registry；`runtime_decision_test.rs:51` `replay(...)` helper 同步改写。测试仍覆盖 HTTP 回放，未删除。

**建议**：保留 PR #7。与 RFC-0038:207「重建 provider→HTTP replay」不矛盾（仍能重建，只是来源换成宿主 registry）。注意：#200:1607 本身要求最终连 `ProviderRecord` 也不重建；当前 `rebuild_decision_provider` 是过渡，阶段 3 随 RFC-0041 G3（:338）删除。

## 第 3 项：provider_handle（读 env）被删 —— needs Trey

**#226 原文**
- RFC-0038:24-25「全部通过既有 `Provider::decision_model` / ProviderHandle 工厂创建。复用连接、认证、headers、重试配置」
- RFC-0038:30-37 表格：各 provider 默认 base URL 与 env（`OPENAI_API_KEY`、`OLLAMA_BASE_URL`…`LAYA_BASE_URL`、`CLOUDFLARE_API_KEY`）；:39「显式 key 或对应 `<PROVIDER>_API_KEY` 可用于有认证的部署」

**#226 代码（3f0673f2）**
- `aimux-providers/src/provider.rs:376` `pub fn provider_handle(name, api_key, options)`；#226 新增 :386-404 本地运行时分支 `std::env::var(url_env)`（:398），:405-414 openai 分支 `env_var: "OPENAI_API_KEY"`；key 缺省时 :450-451 读 `{NAME}_API_KEY`
- FFI：`aimux-ffi/src/lib.rs:1482` `aimux_provider_handle_new` 调 `provider_handle`（:1492）；#226 新增 `aimux_provider_decision_model`

**PR #7（8341fe7b）**
- `aimux-providers/src/provider.rs` 已删除；`aimux-ffi/src/lib.rs:1678` `aimux_provider_handle_new` 改走 `create_provider`（`aimux-providers/src/default_providers.rs:90`），注释仍为「api_key = null reads the provider's env var from the registry entry」
- env 行为仍部分保留：`aimux-providers/src/provider_registry.json` 中 ollama/llamacpp/localai/sglang/vllm 为 `auth: none` + `base_url_env`，cloudflare 为 `env_var: CLOUDFLARE_API_KEY`
- **发现（静态核查，未跑测试）**：`laya` 不在 `provider_registry.json`，`default_providers.rs` 中也没有 `laya`；`create_provider` 对未知名返回 `NoSuchProvider`（`default_providers.rs:94-104`）。所以 #226 的 `provider_handle("laya")` 路径（RFC-0038:21,34）在 fork 中经 ProviderHandle 不可达。fork 测试只经 `create_openai_compatible` 直接构造（`runtime_decision_test.rs:33-44`），覆盖不到这一点。

**另一方：RFC-0040**
- RFC-0040:32「aimux…不加载厂商工厂，不扫描任意进程 env」；:33「宿主组装层…不经外部请求隐式授予任意 env 读取能力」
- RFC-0040:71「配置文件仅在 descriptor 标记为 credential 的字段接受 `{ "env": "NAME" }`…宿主需提供 env allowlist 或预绑定凭证句柄」
- RFC-0040:155（B2）「本地服务无假 key；URL env 与地区参数不丢…preset 生成明确工厂，auth none、base URL env」——**RFC-0040 也要求保留 base URL env**，只是改由 descriptor/preset 声明

**判断**：两份维护者文档在「库内按 `<PROVIDER>_API_KEY` 隐式读 env」（RFC-0038:39）与「env 只在 descriptor 声明、宿主允许时读」（RFC-0040:71,32-33）上冲突。base URL env 两边一致（RFC-0038:31-36 与 RFC-0040:155）。**needs Trey，不作选择**。无论怎么定，`laya` 缺失都应修（RFC-0038 与 RFC-0040:155 都要求可达）。

**#226 测试**：`aimux-ffi/tests/provider_handle_test.rs` `provider_decision_factory_keeps_model_alive_after_provider_drop`（3f0673f2 :20）未改；同文件 :124 config JSON 由 `base_url` 改为 `baseUrl`。`jev_test.rs` `non_decision_provider_returns_unsupported`（fork :389）与 `openai_decision_test.rs` `factory_is_explicit_and_preserves_the_provider_boundary`（fork :294）由 `provider_handle(...)` 改为 `create_deepseek`/`create_openai`，不再测 ProviderHandle 名字解析与 env。`exports_smoke_test.rs` 中 `provider_from_env_handle_or_clean_failure` 被删——不是 #226 的测试（#226 对该文件只改了导出计数 128→129）。

## 第 4 项：删除 `DecisionModel::retry_config()` 与 body_overrides 检查

**#226 原文**
- RFC-0038:25「复用连接、认证、headers、重试配置」
- RFC-0038:45-46「未知协议、未知本 namespace 参数、生成式 `body_overrides` 均拒绝，不静默忽略」
- RFC-0037:73「官方 429 限流和 529 过载复用 Core 的临时错误重试与 Retry-After 处理」

**#226 代码（3f0673f2）**
- `aimux-core/src/decision_model.rs:522` 默认 `fn retry_config()`；:611 `prepare_retries(options.max_retries, model.retry_config(), …)`；覆盖：`openai/decision.rs:400`、`runtime_decision.rs:621`
- body_overrides 拒绝：`aimux-providers/src/openai/decision.rs:29-31`、`aimux-providers/src/runtime_decision.rs:35-37`

**PR #7（8341fe7b）**
- `aimux-core/src/decision_model.rs:597` `prepare_retries(options.max_retries, abort_signal.clone())`；trait 中 `retry_config`、`config_snapshot`、`specification_version` 均删除
- `body_overrides` 字段在栈侧已整体删除；仅剩 FFI 拒绝说明 `aimux-ffi/src/lib.rs:979,1626` 和 Node 显式报错 `bindings/node/src/lib.rs:661-662`。检查失去对象，RFC-0038:46「拒绝、不静默忽略」改由「未知字段报错」兑现

**支持删除的依据（存在）**
- #200:29「调用职责(retry…)混进了模型协议(`retry_config()`…)」；#200:1434 删除清单列「各模态 `retry_config()`」；b90f054f 中已无任何 `fn retry_config`
- #200:159（D15）「删除通用 `body_overrides`」；#200:1458「所有通用 `body_overrides` 和合并函数」；RFC-0040:178 迁移说明列「移除的 bodyOverrides」

**#226 测试**：`openai_decision_test.rs:234` `retry_and_standard_provider_errors_use_the_shared_transport`（fork :257）与 `runtime_decision_test.rs:522` `retry_and_cancellation_use_shared_transport`（fork :549）保留，删掉 `config.retry_config.initial_delay = Duration::ZERO`；`jev_test.rs` `model()` helper 同改。重试行为仍测，但退避延迟不再置零（测试变慢、是否 flaky 未验证）。#226 没有测 decision body_overrides 拒绝（`body_overrides_test.rs` 属 RFC-0017，非 #226）。

**建议**：保留删除。重试仍是「复用 Core 重试」（RFC-0037:73 成立）；RFC-0038:25 的「重试配置」应改读为 core `maxRetries`——属 RFC-0038 文字修订，**待 Trey 确认**。验收：429/529 用例在 `maxRetries=1` 下各重试一次；带 `bodyOverrides` 的 decision 请求在 FFI/Node 均返回 InvalidArgument。

---

## PR #7 中改动的 #226 测试（`git diff 3f0673f2..8341fe7b`，#226 测试文件一个都没删）
| 文件 | 变化 |
|---|---|
| `aimux-providers/tests/runtime_decision_test.rs` | 改写：helper 改为 `create_openai_compatible`+registry；`local_factories_and_wrapper_factories_preserve_credentials_for_replay` → 改名 `local_runtimes_rebuild_from_the_caller_registry_without_recorded_credentials`，删 `config_snapshot`/`api_key_source` 断言；retry 测试去掉零延迟 |
| `aimux-providers/tests/openai_decision_test.rs` | `config()`→`settings()`/`replay_registry()`；replay 测试改传 registry；factory 测试不再经 `provider_handle`；retry 去零延迟 |
| `aimux-providers/tests/jev_test.rs` | 去 `retry_config` 零延迟；`usage.raw`/metadata 比较改包 `Value::Object`；`provider_handle("deepseek")` → `create_deepseek` |
| `aimux-ffi/tests/provider_handle_test.rs` | config JSON `base_url`→`baseUrl` |
| `aimux-ffi/tests/exports_smoke_test.rs` | 导出计数/列表随栈侧变化；删了 `provider_from_env_handle_or_clean_failure`（非 #226 测试） |
| `contract-tests/fixtures/decision-openai-full.json` | 图片 `data` 由 `{"Base64": …}` 改为纯 base64 串（wire 变化，影响 8 个绑定的契约测试，**需 Trey 确认**） |
未改：`aimux-core/tests/decision_test.rs`、各绑定 decision 测试（node/python/go/java/kotlin/swift/flutter）。
