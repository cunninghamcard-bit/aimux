# Host operation：技术债清理与消融实验

日期：2026-09-20。代码位于本地 `docs/host-driven-ffi-spec` 分支，尚未提交或发布。实现从 master `d66e1951613d1f95b145b65794370510fa8690bb` 开始，替代 PR #186 的跨语言回调接入方式。本文的“删除”指本地重实现清理前后，不表示已从 master 删除一套已发布 API。

## 结论

保留一个共享 Rust operation：宿主保存和执行函数，边界只传数据，Core 负责修复后的校验与共享 deadline。删掉不能贡献独立语义的类型、队列、状态和资源包装。

清理后所有针对本改动的回归通过。五项负向消融均编译成功、运行到预期断言失败；重复 repair 计时器的正向删减通过完整 operation 测试。实验检验机制的语义必要性，不是吞吐、延迟或内存 benchmark。

[当前 spec](../rfc/0035-host-driven-operations.md) 已从 433 行收敛到 190 行，与实现同步；删除尚未实现的日志框架、泛型 hook 能力和未来 coroutine API 承诺。

## 实际删减

| 删除或简化 | 最终实现与依据 |
|---|---|
| `completed` 事件及各语言分支 | 流耗尽统一 ENDED；聚合调用保留 Result，避免双重结束信号 |
| `enabled_hooks: string[]` | 当前只有 repair，使用明确的 bool；不构建 capability 注册框架 |
| Rust `WireToolCall` 与双向字段转换 | Core `RawToolCall` 直接 serde，减少字段漂移 |
| 独立 control queue | Core 顺序修复，pending slot 同时拥有请求与回复通道 |
| 额外 closing mutex 和独立 task mutex | 一个异步任务拥有者锁贯穿 close/join |
| Node/Python native 内层多余 Arc | native 对象直接拥有 Operation；C 注册表仍保留必要的 Arc |
| operation 内第二层 repair timeout wrapper | Core 已使用同一个绝对预算；删除后 16 项 operation 测试通过 |
| Node/Python 手写部分字段验证 | 统一使用 Rust reply schema；非法可选字段现在也是 ToolCallRepair，避免误报 Aborted |
| PyO3 `multiple-pymethods` 配置需求 | start 方法放回已有 Model impl，不为一个方法增加宏注册依赖 |
| Go 长期等待 context 的额外 goroutine | 使用 context.AfterFunc；关闭时停止或等待已启动的处理器 |
| 无 hook Core 路径额外创建 abort token | GenerationControl 接受可选 signal，保留旧调用的分配行为 |
| Node 临时 TextOptions 别名 | 统一公开 GenerateTextOptions，与宿主 repair 类型组合 |

没有引入 callback registry、repair 专用 TSFN、Python callable 的 native spawn_blocking 桥、通用事件总线或代码生成框架。语言自己的数据类型保留，宿主需要有类型的公开 API。

另外修正了一个已在 master 副本复现的 Go 测试债务：旧断言要求 U+FFFD 必须写成特定 JSON 转义文本；改为解析 JSON 后检查字符值，同时仍验证 JSON 有效及 NUL 被正确编码。随后 Go 全包 race 测试通过。

## 消融结果

实验副本与正常构建使用独立 target 目录。每个变体只改变一处，运行后恢复源码。Rust baseline 的 1 个单元测试和 15 个集成测试通过；Node 的暂停消费 baseline 通过。

| 单独删除的机制 | 观察到的失败 | 决策 |
|---|---|---|
| Core repair 共享 deadline | 直接 Rust 调用中未完成的 repair 突破外层等待界限 | 保留 |
| 同 lane 读取互斥 | 第二个读取没有立即返回 ReaderBusy，进入等待 | 保留 |
| OUTPUT 容量限制 | 无消费者时 producer 连续积累 10,000 个事件 | 保留 |
| 首个终态不可覆盖 | 已确定的 Timeout 被后续 cancel 改成 Aborted | 保留 |
| 独立 terminal observer | Node 用户暂停消费时，宿主 signal 保持 stuck，未收到 cancelled | 保留 |
| operation 内重复 repair 计时器 | 完整 operation 测试仍通过；负向 deadline 实验仍能检出缺陷 | 删除 |

负向变体的退出码：四个 Rust 变体为 101，Node 为 1。日志包含具体断言失败，不能把编译错误或测试未运行当作有效消融。正向删减已进入最终代码；重复 DTO 等结构性删减由同一套回归验证，未逐项声称独立因果实验。

可复现脚本：`scripts/ablate_host_operations.py`。先构建 Node native 扩展并安装 npm 依赖，再从仓库根目录执行：

```sh
python3 scripts/ablate_host_operations.py --output /tmp/aimux-ablation --node
```

脚本复制当前 tracked/untracked 源码，不修改工作区；`--cargo /path/to/cargo` 可选择工具链包装器。输出目录包含每项日志、results.json 和实验源码副本。退出成功表示 baseline 通过且负向变体出现预期失败。

## 验证范围

环境：macOS arm64，Rust 1.98.1，Python 3.9，JDK 17 / Gradle 8.8，Dart 3.4.4。native 链接使用本机可工作的 macOS 15.4 SDK。

| 层 | 最终验证 |
|---|---|
| Core | 347 单元测试；4 个 stream timeout matrix + 1 个 recording e2e |
| operation | 1 个背压单元测试 + 15 个生命周期、协议、OpenAI 输出集成测试 |
| C ABI | 33 单元测试、18 export/ownership smoke tests；124 个导出与头文件匹配 |
| Node | native build、TypeScript 编译；operation/wrapper/error 共 26 项 |
| Python | native build；operation/wrapper/error 共 25 项 |
| Go | `go test -race -count=1 ./...` 全包通过，含新增的 3 个 operation 测试 |
| Java | HostOperationTest + TypedModelTest，共 12 项 |
| Kotlin | HostOperationTest + TypedModelTest，共 5 项 |
| Dart | 实际 native 调用的 3 项测试：同 isolate 嵌套、永不完成 Future 的超时、repair 中取消订阅；analyze 无问题 |
| Swift | 全模块 swiftc typecheck/编译；3 项真实 native smoke：同步线程及嵌套、MainActor 异步、超时取消 |

SwiftPM 在本机因缺少 BuildServerProtocol 动态符号启动失败，因此 Swift 使用直接 swiftc 编译运行，未声称 SwiftPM 测试套件通过。可复现 smoke 位于 `contract-tests/host-operation-smoke.swift`：

```sh
swiftc -module-name Aimux -I bindings/swift/Sources/CAimuxFFI \
  -L target/debug -laimux_ffi \
  -Xlinker -rpath -Xlinker "$PWD/target/debug" \
  bindings/swift/Sources/Aimux/*.swift \
  bindings/swift/Tests/AimuxTests/MockHTTPServer.swift \
  contract-tests/host-operation-smoke.swift -o /tmp/aimux-host-smoke
/tmp/aimux-host-smoke contract-tests/host-operation.json
```

本机没有 Flutter SDK，Dart 测试在包含同一份 lib/test 源码的纯 Dart harness 运行；不是 Flutter/iOS/Android 打包验证。macOS Dart hardened runtime 忽略 DYLD_LIBRARY_PATH，测试将 native dylib 放在临时 Dart SDK 可搜索位置。Java/Kotlin 测试用临时 Gradle init 脚本将 JNA 指向 target/debug，没有修改发布构建配置。

常规测试入口：

```sh
cargo test -p aimux-core -p aimux-operation -p aimux-ffi --lib
cargo test -p aimux-operation -p aimux-ffi --test operations --test exports_smoke_test
cargo test -p aimux-core --test stream_timeout_matrix_test --test recording_e2e_test
# bindings/node
npm run build:typed
npx ava __test__/operation.test.ts __test__/wrapper.test.ts __test__/error.test.ts
# bindings/python（已安装 native 扩展）
pytest -q tests/test_operation.py tests/test_wrapper.py tests/test_error.py
# bindings/go（配置 native library 路径后）
go test -race -count=1 ./...
# bindings/java 与 bindings/kotlin（配置 JNA 路径后）
gradle test --tests '*HostOperationTest' --tests '*TypedModelTest'
```

## 明确保留的边界

- 64 是 operation 队列的事件数上界，单事件字节数没有上限。Core 原有 stream pump 仍可能无界，Kotlin 原有 Sequence 仍先聚合，未证明端到端内存上界。
- 取消宿主用户代码是合作式的。无限同步阻塞或吞掉取消的 coroutine 不能被框架强制停止；Rust 已结束不代表宿主所有用户代码都退出。
- close 的一秒合作式等待后 abort/join 依赖 runtime 可调度，不保证抢占阻塞 runtime 的自定义同步代码。
- 本轮没有做真实 provider 联网测试、性能基准或 Windows/Linux/mobile 构建；发布前仍需平台 CI。没有把无 hook 的既有所有入口统一切换到新 transport。

这些边界已写入 spec，避免把局部改进包装成全链路保证。
