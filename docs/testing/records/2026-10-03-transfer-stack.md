# 传输任务的小栈回归

日期：2026-10-03。范围：SFTP 上传、下载和目录队列的异步任务内存布局。

## 原始失败与定位

提交 `f23b264` 的 [Quality 运行](https://github.com/cyruss648/keelshell/actions/runs/37091532008) 在 macOS 和 Ubuntu 通过；Windows 的目录取消/FIFO 测试以 `STATUS_STACK_OVERFLOW` 退出。原始日志保存在 ignored `work/credentials-tree-quality-windows-failed.log`。

目录上传将 64 KiB 数组保存在跨 `await` 的 Future 中，外层目录执行、取消和超时状态机继续包含它。普通队列上传、下载及原子上传存在相同的内联缓冲结构。独立检查确认 SFTP relay 的复制缓冲已在堆上，目录扫描使用显式 `Vec`，没有递归函数调用。

修复前，新增尺寸回归失败；原取消/FIFO 用例在 macOS 显式 2 MiB 栈上仍通过。因此，本机没有重现 Windows 的进程中止，不能将本机通过代替 Windows 原生验证。

## 修复与回归

四处固定数组改为直接 `vec!` 堆分配，保留原有 64/32 KiB 分块大小、资源限制、读写切片和取消/清理行为。不增加线程栈，不调整超时或产品传输上限。

| Future | 修复前 | 修复后 |
| --- | ---: | ---: |
| 目录内单文件上传 | 66,432 B | 912 B |
| 目录内单文件下载 | 1,040 B | 1,040 B |
| 目录执行 | 134,304 B | 16,096 B |
| 普通队列上传 / 下载 | 未单独记录 | 各 2,648 B |
| 原子上传 / 原子写入 | 未单独记录 | 8,152 / 7,960 B |

以上为本机调试构建的实测值，不是跨平台固定 ABI。新增两项尺寸回归通过类型推断测量未轮询的 Future，叶子/普通传输上限为 16 KiB，目录执行上限为 32 KiB，为平台差异留余量并阻止重新内联完整分块。

原取消/FIFO 测试改为显式 2 MiB 线程和 current-thread Tokio runtime，真实队列 worker 同样在该线程执行，保留部分文件树、已确认字节数和后续队列任务成功的断言。测试增加 30 秒总 deadline，子线程错误和 panic 传回测试框架。

独立只读复审没有未解决问题。针对性检查通过：5 项 SFTP 单元测试、8 项目录集成测试、9 项相关单文件/原子写入/通道所有权测试，以及 session all-targets 严格 Clippy。

修复后的 `python3 scripts/check.py` 完整通过依赖版本策略、Rustfmt、全 workspace/all-targets 严格 Clippy、346 项单元/集成测试和 2 项文档测试；其中 session 共 82 项。当前公开树扫描覆盖 202 个路径、164 个文本文件，禁止的具名比较标识及失效相对 Markdown 文件链接均为零。未修改打包逻辑，沿用本轮此前 47 项打包测试的通过记录。

## 本地证据

以下日志均保留在 ignored `work/`：

- `directory-stack-future-before.log`：修复前尺寸及失败回归。
- `directory-stack-before-2mib.log`：修复前 macOS 小栈取消/FIFO 用例通过。
- `directory-stack-future-after.log`：修复后尺寸及单元测试。
- `directory-stack-transfers-after-2mib.log`：8 项目录集成测试。
- `directory-stack-file-transfers-after-2mib.log`：9 项相关 SFTP 集成测试。
- `directory-stack-clippy.log`：严格 Clippy。
- `windows-stack-final-gate.log`：本次修复后的整仓门禁。

## 原生 CI 闭环

修复提交 `9c88c586c413e533d5d6e59b064f0a68ce700999` 的 [Quality 运行 37092321464](https://github.com/cyruss648/keelshell/actions/runs/37092321464) 全部通过：`macos-26`、`ubuntu-24.04`、`windows-2025`。Windows 原生运行确认目录取消/FIFO 用例及两个 Future 尺寸回归均成功，此前的栈溢出失败已经闭环。三个 job 均执行打包回归和完整 Rust 门禁。

运行元数据保存在 ignored `work/windows-stack-quality.json`，Windows 成功日志为 `work/windows-stack-quality-windows-passed.log`；原始失败日志继续保留。本节在该代码提交通过之后补记，后续文档提交不会被表述为同一次已测试提交。

本次修复没有改变界面流程。[凭据与目录集成验收](2026-10-03-credentials-tree-integration.md) 中的原生 GUI 哈希属于内存布局修复之前的构建。CI 不证明 Windows/Linux 桌面交互或新的六目标发行包已经验收。
