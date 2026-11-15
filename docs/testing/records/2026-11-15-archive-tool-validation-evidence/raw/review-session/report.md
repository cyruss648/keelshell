# 非作者严格只读复核：服务端工具版本与内存流式自测后端

当前建议：**REQUEST_CHANGES**。源码与 Python 风险专项已完成；开放 P2 两项。
**Rust 最终预构建产物及完整库存尚未由父提供，本报告尚未收口。**
原生 macOS/Windows/Linux 不由本报告代签。

| 严重度 | 开放数量 |
| --- | ---: |
| P0 | 0 |
| P1 | 0 |
| P2 | 2 |
| P3 | 0 |

复核绑定 baseline `6e2b0a7ee3493ace52ba157049ab668ba464c4f0`，
冻结 `work/archive-validation-batch-20261115/frozen-inputs-r1.json`，
607 项工程输入，工程 SHA-256
`d68bec67c2cd4313383736733096f5c4ae68ccc6d786793609d31cb34aedc210`。
已独立重算全部 607 项，零 mismatch；工程摘要按
`json.dumps(files, sort_keys=True, separators=(",", ":"))` 重算一致。
清单文件自身 SHA-256 为
`0d6a2b75914fe90175066f9103f984a1669c81fda4d03b2be4faecafe62a9222`，
与工程摘要含义不同。初始 HEAD 等于 baseline。原件 `freeze-initial-r1.json`。

只读范围是 `crates/keelshell-session/` 内的
`src/archive_validation.rs`、`src/archive_validation.py`、
`src/archive_validation/tests.rs`、`src/archive_validation/test_archive_validation.py`、
`tests/archive_validation.rs`、`tests/openssh_interop.rs` 新增测试和 `src/lib.rs` 新声明。
已读用户给定 AGENTS、仓库 AGENTS、RTK、HANDOFF 当前首段、
STRATEGY、SHARED_TARGET、rust-best-practices 及相关错误处理/测试/文档/所有权指引。
ADR 0139 和工具指南用于核对功能契约；未审签 UI。
现有 SSH diagnostic/channel 所有权代码仅用于理解新 API 调用的边界，不扩大审签范围。

1. **P2：正常退出路径遗漏同组后台后代。** 位置
   `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation.py:211`
   和同文件 `:227`。直接子进程在 EOF 后被 `wait()` 回收，再因
   `returncode is not None` 跳过组级收尾。普通后代关闭标准流、保留原进程组，
   leader 正常退出时，helper 实际返回 `passed / 0` 而后代仍存活。
   重复执行可以留下不受工作预算控制的后台进程；不需要 setsid 或进程组逃逸。
   真实有界红色回归、绝对路径和修复方向见 `notice-1.md`；
   reviewer 在 `finally` 清理自己的 fixture 后代并观察到 PID 消失。

2. **P2：本地 POSIX suite 无条件进入 Windows 默认 Rust 单测。** 位置
   `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation/tests.rs:323`
   和同文件 `:329`。suite 使用 `/bin/sh`、`fork`、`killpg`，并要求实际 helper
   返回 0；生产 helper 在
   `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation.py:293`
   明确拒绝非 POSIX 平台并返回 65。源码库存和受控 admission 观测见
   `windows-suite-source-r1.json`，详细触发、影响和平台门限修复方向见 `notice-2.md`。
   此结论来自源码及受控平台条件，不冒充 Windows 原生执行。

除上述两项外，范围内功能与边界核对如下：

| 项目 | 复核结论与证据边界 |
| --- | --- |
| 真实功能 | 通过已认证 SSH 的固定 exec 运行 stdlib helper；七项固定版本参数与三项固定流样本确已实现。zip/unzip/rz/sz 功能保持 NotTested，无 capability 缓存或传输授权。 |
| 命令与输入 | 完整 helper 作为一个 POSIX shell 引号参数可见；工具路径进入 argv 而非 shell 拼接。tar 固定唯一普通成员、gzip 固定字节、Python BytesIO ZIP/tar.gz 往返；没有用户文件参数。 |
| 环境 | 子进程环境采用 PATH/LANG/LC_ALL/TZ allowlist，拒绝相对 PATH 和控制/隐藏字符，移除工具选项和加载器/Python 环境钩子。此为诊断，不是恶意 executable 的 sandbox；bootstrap Python 仍来自 SSH 命令环境。 |
| 时间与输出 | 每命令一秒、合计六秒工作预算，双向非阻塞 pipe、每命令 combined 4096 bytes、累计 16 KiB，cleanup wait 0.2 秒。Rust 外层十秒与 combined 32 KiB。独立测到精确输出界限、越界、背压、非零退出和信号时实际回收直接子进程；正常退出的同组后代遗漏见 P2。OS 创建进程及文件系统元数据调用不具硬实时保证。 |
| 取消与 SSH 所有权 | 新入口在打开前、diagnostic exec 前、pending 中及结果采用前检查取消；检查 connected 状态和总 deadline。依赖现有 PendingChannel 的独立 bounded owner 处理 drop/晚 open。取消/CLOSE 不证明远端停止，文档明确保留该边界。加密 SSH fixture 源码覆盖这些路径，但本 reviewer 尚未执行 Rust。 |
| 严格协议 | serde deny_unknown_fields、required_nullable、固定协议/七项顺序/数量/状态，路径和版本 UTF-8 字节上限、控制及 bidi 检查；拒绝额外文档、截断、非法 UTF-8、stderr 和缺少成功退出。源码测试包含逐截断前缀与重复字段；尚待最终 Rust 产物实际执行。 |
| 版本/feature 语义 | 版本是未经真实性认证的安全显示文本；版本状态与固定样本互不推导。只有 tar/gzip/python3 接纳流检查，四项其他工具强制 NotTested；不声称真实归档、Zmodem 或后续文件访问已授权。Python 3.8+ 与 POSIX 要求明确。 |
| 测试有效性 | Python suite 含真实子进程、stdout/stderr 并发与 waitpid 回收检查，也含 mock 分派/失败测试，二者未混作真实互操作。独立补充发现正常退出同组后代漏测。OpenSSH 新增测试为 ignored opt-in，只比较 owned 目录前后名字/大小，不能据此认证整个远端文件系统无写入。 |

实际运行全部通过本目录 `run_evidence.py` 包装，固定受控
`PATH=/usr/bin:/bin`、独立 fixture HOME/cwd、stdin `/dev/null`、禁用字节码。
运行和探针没有用户归档输入、真实用户文件读取探针、网络 fixture 或无限等待。
所有 reviewer 写入均位于本目录；没有运行 Cargo、编译/格式化/修改源代码、
提交、push、清理工作树或更改共享 target。

| 执行项 | 真实测试计数 | 命令退出与上限 | 原件 |
| --- | --- | --- | --- |
| 原有 Python suite | 24 run / 24 passed / 0 failed / 0 skipped | exit 0；外层 45 秒，实际 1.777 秒，waited | `python-author-suite-r1.log/.json` |
| 独立 Python 风险专项 | 11 run / 11 passed / 0 failed / 0 skipped | exit 0；外层 15 秒，实际 0.416 秒，waited；七项范围源码运行前后摘要一致 | `python-independent-risks-r1.log/.json` |
| 独立同组后代红色回归 | 1 run / 0 passed / 1 failed / 0 skipped | exit 1；外层 8 秒，实际 0.167 秒，waited；fixture 已收尾 | `python-descendant-eof-r1.log/.json` |
| Windows admission 条件观测 | 不计入框架测试数量 | 将已加载 helper 的 os.name 受控设为 nt，main 返回 65 | `windows-suite-source-r1.json` |
| reviewer Rust 专项 | **0 executed，等待父最终产物/库存** | 未调用 Cargo，也未采用共享 target 中自行发现的二进制 | 待补 |
| 原生三平台/真实 OpenSSH | **0 executed by reviewer** | 不代签父或平台验收 | 待父独立证据 |

命令（均从工作树根执行）：

```sh
python3 -B work/archive-validation-batch-20261115/review-session/run_evidence.py \
  --name python-author-suite-r1 --seconds 45 -- /usr/bin/python3 -I -S -B \
  /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation/test_archive_validation.py

python3 -B work/archive-validation-batch-20261115/review-session/run_evidence.py \
  --name python-independent-risks-r1 --seconds 15 -- /usr/bin/python3 -I -S -B \
  /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-session/independent_python_risks_r1.py

python3 -B work/archive-validation-batch-20261115/review-session/run_evidence.py \
  --name python-descendant-eof-r1 --seconds 8 -- /usr/bin/python3 -I -S -B \
  /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-session/probe_descendant_eof_r1.py
```

上述三个框架调用合计 36 项执行、35 通过、1 失败；不把 subTest 分支、
mock 平台观测或未来 Rust 包装层重复执行的同一 Python suite 叠加成独立覆盖。
源码库存为 Rust lib 新增 11 项、encrypted SSH integration 11 项，
另有新增 ignored OpenSSH 1 项；这是**源码库存，不是已编译库存或运行计数**，
详见 `source-test-inventory-r1.json`。

收口条件：父停止/reap 需要修订的旧 gate 并保留原件，修复两项 P2，
提供新冻结、修复差分/回归，以及与最终冻结绑定的稳定 Rust 测试二进制、
SHA-256、完整 `--list` 库存与构建来源。之后仅运行与本范围相关的必要风险专项，
明确披露**预构建，reviewer 无独立重编译**；父完整 gate 不由本 reviewer 重复。
在此之前保持 REQUEST_CHANGES。未验的 Windows/Linux/macOS 原生桌面、
真实 SSH 服务器环境、远端取消实际停止、任意工具真实性/沙箱和广义归档兼容性，
不能由这些受控 Python 样本或未来预构建 Rust 专项代签。
