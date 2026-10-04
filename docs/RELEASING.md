# GitHub 与发布流程

用户于 2026-10-03 授权创建公开仓库并推送，以及添加标签时构建、发布跨平台产物。
仓库为 [cyruss648/keelshell](https://github.com/cyruss648/keelshell)。
当前仍处于开发阶段，功能完成度见 [ROADMAP](ROADMAP.md)。公开源码和构建产物不代表全部远程 SSH 能力与原生桌面流程已验收。

## 平台与产物

| 系统 | 原生 runner | Rust target | 格式 |
|---|---|---|---|
| macOS Apple Silicon | macos-26 | aarch64-apple-darwin | ZIP 内含 KeelShell.app |
| macOS Intel | macos-26-intel | x86_64-apple-darwin | ZIP 内含 KeelShell.app |
| Windows x64 | windows-2025 | x86_64-pc-windows-msvc | ZIP 内含 EXE 与 ICO |
| Windows ARM64 | windows-11-arm | aarch64-pc-windows-msvc | ZIP 内含 EXE 与 ICO |
| Linux x64 | ubuntu-24.04 | x86_64-unknown-linux-gnu | tar.gz，usr/bin 与桌面资源 |
| Linux ARM64 | ubuntu-24.04-arm | aarch64-unknown-linux-gnu | tar.gz，usr/bin 与桌面资源 |

包名 `KeelShell-{version}-{target}.zip` 或 `.tar.gz`，每包有 `.sha256`，整体有 `SHA256SUMS`。
每个包中有 `package-manifest.json`，包含文件哈希、主程序/MCP伴随程序各自的SHA-256、版本、目标、Git commit、工作流 run ID、Rust 版本与图标来源哈希。
macOS 最低版本 15.0；Linux 以 Ubuntu 24.04 的运行库为基线，使用 Wayland/X11 与 Vulkan；不能承诺任意 Linux 发行版兼容。
Windows 使用 MSVC 与 Windows SDK，Release 着色器依赖 `fxc.exe`。应用使用 GPUI 的 asInvoker/PerMonitorV2 manifest，避免重复嵌入。

所有目标都在主程序同目录包含 `keelshell-mcp`（Windows 为 `.exe`）。打包显式接收同次构建的两个 binary；归档和完整集合校验都检查它们的目标架构、摘要及 Unix 执行位。schema 保持 1 并新增必需 `mcp_binary_sha256`，旧 helper 可按 `files` 新增 companion，新版更新预检和 helper 拒绝缺少它的包。设计与当前证据见 [ADR0041](adr/0041-mcp-companion-packaging-and-update-recovery.md) 和 [companion记录](testing/records/2026-10-04-mcp-companion-packaging.md)。

这些包目前没有 Developer ID 签名、公证或 Windows 代码签名；不是安装器。应用只在用户显式操作后下载、校验并调用更新 helper。运行中的 MCP stdio 进程需由外部智能体重启才使用新 image；Windows 占用可能触发有界重试与回滚。回滚失败会保留该次 staging/原文件备份并停止重启，不会把失败备份当成过期文件清理。临时 staging 与安装目录跨文件系统时，备份 rename 可能被拒绝；实际安装目录的三平台更新验收仍未完成。
自动测试、二进制架构与资源检查，不替代图形桌面启动、真实 SSH/SFTP 服务互操作和用户验收。

## 不发布的演练

在 GitHub Actions 的 **Release → Run workflow** 选择 `main`，或：

```sh
gh workflow run release.yml --repo cyruss648/keelshell --ref main
```

这会执行同一套六平台质量门禁、Release 编译、打包和完整集合检查。成功后可在该 run 的
`verified-release` artifact 下载产物，保留 7 天；不会创建标签或 GitHub Release。
首次配置的实际结果单独记入 [发布验收记录](testing/records/2026-10-03-release.md)。

## 标签发布

1. 完成对应版本的功能、文档、测试和原生验收，更新根 `Cargo.toml` 的 workspace package version，并更新 `Cargo.lock` 中工作区版本。将版本改动提交、推送到 `main`。
2. 在待发布提交上创建与版本完全一致的标签。例如版本为 `0.1.0-rc.1` 时：

   ```sh
   git tag -a v0.1.0-rc.1 -m 'KeelShell 0.1.0-rc.1'
   git push origin v0.1.0-rc.1
   ```

3. `v*` 标签触发 Release；预检只接受 `vX.Y.Z` 或 SemVer 预发布标签，必须与 workspace version 完全一致。构建元数据、前导零或不匹配版本都会失败。
4. 全部目标通过后，汇总检查六套包、二进制架构、manifest 和校验值。发布任务重新核验远程标签指向触发提交，确认每个包的构建提交与标签提交一致，再创建草稿、上传资产、验证 GitHub 返回的大小与 SHA-256，最后公开 Release。
5. 带 `-rc.1` 等预发布后缀的版本标记为 GitHub prerelease。任一平台失败时不发布；不要移动已经发布的标签。

本次配置演练不会自动创建 `v0.1.0`，以免把开发中的功能标作正式版本。

## 失败、权限与复跑

- 构建任务只有 `contents: read`。唯一发布任务使用仓库自带 `GITHUB_TOKEN` 的 `contents: write`；无需保存个人 PAT。
- 同一 ref 的发布任务串行执行，不取消正在进行的发布。草稿可在资产匹配时续传；已发布且完全一致的版本视为成功。
- 同名不同哈希、陌生资产或标签指向变化都会阻止继续；不会自动覆盖资产、删除 Release 或移动标签。
- 上传中途失败时保留草稿，修复实际失败原因后在 Actions 中选择 Re-run failed jobs，复用原有已经验证的构建产物。重新构建可能改变包字节与哈希，不会自动覆盖旧草稿资产；需要先人工核对旧草稿与新构建来源。先检查 run 日志与草稿，勿通过重复推送/改写标签来绕过失败。
- 构建使用锁定 Rust 工具链和 Cargo.lock，直接 registry 依赖仍用 `x.y`。Actions 固定官方最新版本的完整 commit SHA，旁注版本，避免可移动标签悄悄改变执行代码。更新时一起核验 upstream release 和 SHA。

## 本地检查

```sh
python3 -m unittest discover -s packaging -p 'test_*.py' -v
python3 scripts/check.py
python3 packaging/release.py validate-ref --tag v0.1.0
```

`packaging/package.py` 只生成 staging；`release.py` 无网络，负责归档、哈希与集合核验；
`publish.py` 才进行 GitHub 写入；`inspect_native.py` 在目标系统检查本机格式/桌面资源，不启动 GUI。
本机 Python 需 3.11+；CI 使用 3.14 系列最新 patch。图标已经校验并提交，普通构建无需重新生成或安装 Pillow。

## 官方依据（2026-10-03）

- [GitHub hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)：公开仓库可用的目标系统和架构标签。
- [工作流触发事件](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)：tag push 与 workflow_dispatch。
- [GitHub Release API](https://docs.github.com/en/rest/releases/releases)：草稿、预发布、资产摘要和发布权限。
- [GPUI Kit 安装说明](https://gpui-kit.com/docs/installation/)：原生依赖、macOS 和 Linux 平台要求。
