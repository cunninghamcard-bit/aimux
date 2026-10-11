# A6：按 RFC 删除 #226 这一组（关闭 D3/D4/D5/D6/D14/D16/D17）

> 决定人：Trey，2026-10-11。代码行号均为 fork `exp/restart@8455db9a`。
> 文档：#200 = `docs/aisdk-architecture-alignment.md`（8455db9a 仓内）；RFC-0040 = `upstream/docs/rfc-0040-provider-auth@3757cfda:rfc/0040-provider-auth.md`；RFC-0041 = `upstream/docs/rfc-0041-replay-governance@84fbcf42:rfc/0041-replay-governance.md`。**注意：0040/0041 不在 8455db9a 仓内 `rfc/`，只在上游分支上**（只读取，未改上游）。

## 决定
按维护者 RFC 删除 #226 恢复的四处语义，同时删掉 D16 临时白名单。边界脚本不带任何白名单也要退出码 0。

## 删除项、RFC 依据、代码位置

### 1. 录制 `config_snapshot`（录制只记 provider 名和 model 名）
- 依据：#200:199（禁止通过 `config_snapshot()` 读配置推导凭证/重建）；#200:1436（删除清单 `LanguageModel::config_snapshot()`）；#200:1470（`ProviderRecord`、`record_provider`、`api_key_source`）；RFC-0041:35、:41（不保存 settings、key 来源）、:338；RFC-0040:197（B02）。
- 代码：`aimux-core/src/decision_model.rs:556`（trait 默认 `fn config_snapshot`）、:668（`record_provider(..., &model.config_snapshot().redacted())`）；覆盖实现 `aimux-providers/src/jev.rs:98`、`openai/decision.rs:413`、`provider_handle.rs:270-273`、`runtime_decision.rs:640`；`aimux-core/src/recording.rs:221,240`（`ProviderRecord.api_key_source`）。
- 测试：`aimux-providers/tests/decision_handle_test.rs:67,77,184`；`runtime_decision_test.rs:607,616,622`。

### 2. 模型级 `retry_config()`（只用 core `maxRetries`）
- 依据：#200:29（retry 混进模型协议 `retry_config()`）；#200:292（`max_retries` 属用户操作选项）；#200:1434（删除各模态 `retry_config()`）；#200:1025（不得重新引入 provider 默认 `max_retries`）。
- 代码：`aimux-core/src/decision_model.rs:564`（定义）、:653（调用）；`aimux-core/src/retry.rs:19`（文档引用）；`aimux-providers/src/decision_support.rs:54,78`；`jev.rs:27,38,116-117`；`openai/decision.rs:17,35,41-42,410-411,417`；`provider_handle.rs:195-201`。

### 3. `body_overrides` 拒绝逻辑（字段不存在，传入按未知字段报错）
- 依据：#200:159（D15 删除通用 `body_overrides`）；#200:340、:1458、:1577；RFC-0040:178（迁移说明列"移除的 bodyOverrides"）。
- 代码：`aimux-providers/src/provider_handle.rs:43`（字段）、:157、:166、:174-176、:185-187（两处拒绝）；`tests/decision_handle_test.rs:55`。
- 绑定（要**改成未知字段错误并改测试**，不能只删）：`bindings/node/src/lib.rs:652`、:661-662（"no longer supported" 专门报错）；`bindings/python/src/lib.rs:779`（文档）；`bindings/python/python/aimux/__init__.py:224,248`；`bindings/python/tests/test_aimux.py:62-74`（断言 `InvalidArgumentError`）。

### 4. provider_handle 隐式读 `<PROVIDER>_API_KEY`
- 依据：RFC-0040:32-33（aimux 不扫描任意进程 env；不隐式授予 env 读取）、:71（仅 descriptor 标记的 credential 字段接受 `{ "env": "NAME" }`）、:92（显式→声明 env）；#200:534（仅 descriptor 标记字段支持 env 引用）、:540（env 来源不进录制）。
- 代码：`aimux-providers/src/provider_handle.rs:3`（模块文档）、:59-63（`non_empty_env`）、:69-70（文档）、:91、:118-131（`format!("{}_API_KEY")` 回退与 `api_key_source`）、:158、:169、:192、:202、:253；openai 分支 :94 `load_api_key(..., "OPENAI_API_KEY", ...)`（见歧义 1）。
- 注：`_API_KEY` 全仓命中多（providers/bindings/docs/tools/fixtures/ffi），大多是合法的显式声明，**只删隐式读取路径**。

### 5. D16 白名单
- `scripts/check_provider_boundaries.sh:54-60`（`d16_allow`）、:104-107（调用）、:172（`PLACEHOLDER_API_KEY` 例外，见歧义 2）；规则本身 :95-102、:126、:170 保留。

## 影响面（逐项核）
- 录制/回放：`ProviderRecord` 结构变化 → `aimux-core/src/decision_replay.rs`、`aimux-providers/src/replay.rs`、`tools/aimux-replay`、`fixtures/` 下录制文件需重生成并看 diff。
- 绑定：`api_key_source`/`decision_capabilities` 出现在 flutter/go/java/kotlin/python/swift 的 decision 文件（`bindings/flutter/lib/decision.dart`、`bindings/go/decision.go`、`bindings/java/.../DecisionModel.java`、`AimuxFFI.java`、`bindings/kotlin/.../DecisionModel.kt`、`Model.kt`、`bindings/python/python/aimux/__init__.py`、`bindings/python/tests/test_decision.py`、`bindings/swift/.../DecisionModel.swift`）；node/python 的 body_overrides 见第 3 项。
- 测试：上列 Rust 测试；绑定测试对比基线（Python、Flutter、4 个 Node target 原本就红）。

## RFC 歧义（未定，交 Trey）
1. **openai 原生包默认读 `OPENAI_API_KEY`**：#200:538「原生 provider 未显式提供 key 时，仍按包规则在请求时读取默认 env」，与第 4 项"只显式传或 descriptor 声明"字面冲突。`provider_handle.rs:94` 是删还是保留（视作包规则/descriptor 默认 env）？
2. **D15 占位 key 未在本次列表**：RFC-0040:155（B2）「本地服务无假 key…不要保留 PLACEHOLDER_API_KEY」。删白名单后脚本 :170-172 会拦 `provider_handle.rs:57` 的 `PLACEHOLDER_API_KEY`，"无白名单退出码 0"实际要求 D15 也选"不发 header"。
3. **录制保留 provider 名**：#200:1470 删除清单包括整个 `ProviderRecord`/`record_provider`；RFC-0041:267 目标形如 `target: {modelId, ...}`。Trey 的"记 provider 名 + model 名"是过渡做法，最终形态按 RFC-0041 G3（:338）再改。
4. **本地 runtime 的 `*_BASE_URL` env**（`provider_handle.rs:114-117`）：RFC-0040:155、:191 保留 URL env；本次不删，但它也是隐式 env 读取，需确认不算第 4 项。
5. RFC-0038:25「重试配置」措辞（原 D6）是否修文：本次只记录随 D5 关闭，未改 RFC。

## 更新 2026-10-11：歧义 1、2 已决
1. 原生 provider 的默认 env 保留（D23，#200:538，RFC-0040:67）。R.3 只删通用隐式读取。
2. D15 改为不发 header，删除 `PLACEHOLDER_API_KEY`（D24，RFC-0040:155、:192）。R.5 可以完成。
