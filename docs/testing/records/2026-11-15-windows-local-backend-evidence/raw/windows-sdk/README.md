# Windows x64 MSVC C SDK sidecar

结论：`READY_C_SDK_PROBES_PASS_PARENT_CHECK_NOT_RUN`。SDK `10.0.22621`、MSVC CRT `14.44.35220, 14.44.35226`，复用本机 LLVM/LLD `23.1.5`。终态 `2026-11-15T12:38:50.194222+00:00`，记录起点起耗时 `542.194` 秒。父 cargo check 与 `WindowsNative` 均未运行。

父进程读取 `parent-check.json` 的 `cwd`、`env_delta` 与 `argv`。`parent-env.sh` 只有目标专用环境变量赋值，不执行任何命令；由父按既有门禁管理自己的 `CARGO_TARGET_DIR`。建议命令：

```sh
. /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/windows-sdk/parent-env.sh
cargo check -p keelshell-session --all-targets --target x86_64-pc-windows-msvc --locked --offline
```

本 sidecar **没有执行以上 Cargo 命令**，也未访问共享 target 内容。选择 GNU clang 驱动的真实 MSVC target，可避免 clang-cl 将 `/Users/...` 输入误读为 `/U`；真实失败日志的六条 jitterentropy C 编译命令保留原始编译参数，仅更换编译器、加入真实 SDK include 和独占输出路径，全部退出 0。C/C++ smoke 编译、两个原版 AWS-LC C 特性源、COFF 静态归档、真实 SDK import-lib 链接和 AMD64/PE 检查通过。DLL 仅用作链接探针，未运行。初始失败与修正各自保留回执，不覆盖。

xwin `0.6.5` 来自项目官方 GitHub Release，macOS arm64 压缩包与官方 `.sha256` 匹配；详见 `xwin-provenance.json`。较新的稳定版缺少适用的 macOS 发布资产，未编译或安装任何 Rust 工具。37 个 SDK/CRT 包、共 `314794518` 字节，全部来自 `download.visualstudio.microsoft.com`，逐个匹配实际 Microsoft product manifest 的 SHA-256；见 `sdk-payloads.json`。

来源限制：Microsoft 频道清单中的 product-manifest digest 与实际产品清单不一致。固定版本 xwin 源码 `downloads/xwin-0.6.5-manifest.rs` 149–158 行明确记录此上游问题并跳过该单一摘要；本回执记录两值及实际产品清单 SHA，不声称该摘要匹配或已验证 Microsoft 签名。SDK/CRT 包 payload 的 37 项摘要全部独立匹配。冗余直接产品清单下载超时，部分文件明确命名为 `.partial`，不能用于重建；xwin cache 内的完整产品清单可用。

`receipt.json` 汇总所有终态，`logs/*.json` 包含 argv、期限、退出码、wait/reap 与日志摘要。`sysroot-files.sha256` 绑定 `5547` 个 SDK 文件、`625293395` 字节；`local-tools.json` 记录本地工具实际二进制 SHA 与既有来源限制。工具和 SDK 均保留在本目录或只读复用现有工具；未改动产品、Cargo.lock/config、全局环境或依赖，未伪造头文件、Windows cfg 或替换加密实现。
