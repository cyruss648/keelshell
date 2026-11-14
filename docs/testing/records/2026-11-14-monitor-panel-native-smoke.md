# 监控视觉切片 macOS 包检查与原生观察限制 — 2026-11-14

## 固定来源

本次构建绑定本地源码提交
`aab1718f6682a82860ded8f0510ab7ccb843a902`，使用 Rust `1.100.0` 和该提交的
Cargo.lock（GPUI Kit `0.9.0`）。后续依赖升级不能沿用本包作为新版本原生证据。
未安装应用，未覆盖已有应用数据；每次启动使用独立空数据目录。

## 构建和静态包检查

```text
MACOSX_DEPLOYMENT_TARGET=15.0 \
cargo build --release --locked --offline -p keelshell-app -p keelshell-mcp
```

构建退出 `0`，日志耗时 `2m 22s`。构建期间只修改文档，Cargo manifests、
Cargo.lock 和源码保持固定。没有切换 Cargo target 的源码归属。

随后使用 `packaging/package.py macos` 生成全新的 staging 目录，并通过
`packaging/inspect_native.py --platform macos --target aarch64-apple-darwin`；
两个命令均退出 `0`。manifest 的 `build.commit` 对应上述 SHA，
`installed=false`、`signed_by_packaging_script=false`；GUI/MCP 均为 arm64
Mach-O，`minos 15.0` 与 Info.plist 合同一致。

| 文件 | SHA-256 |
| --- | --- |
| package-manifest.json | `c7df8806469c255ef62a12f827d419b05102727adb033bc1d95083da8173a8c4` |
| keelshell-app | `44493e28095537455f29e9d10c364149cf807d8d840be2b0326c9c9e8ff3c844` |
| keelshell-mcp | `d50bc3cbbb335397b8a30bcdf12471ff75e8d4a7ad402801802e0546f284db73` |

## 运行观察和失败证据

权限预检读回 `AXIsProcessTrusted=true` 和屏幕捕获预检 `true`。这些结果只说明
进程具有对应权限，不能证明桌面已解锁。

首轮隔离启动回执记录 PID `20397`，CoreGraphics 读回一个 `1440×933` 窗口
（包括系统标题栏），AX Windows 数量为 `1`；尝试设置 AX 窗口尺寸返回
`-25205`（属性不支持），尺寸没有改变。观察结束后，对该次自行启动的进程
发送 SIGTERM 并实际 `wait`，退出 `-15`，运行日志 `0` 字节。

第二轮为 Frost/Light/中文、Graphite/Dark/英文、Lavender/Light/中文分别准备
隔离配置，再各启动一次。三个启动回执的 PID 分别为 `26287`、`26341`、
`26366`，均已 SIGTERM 并 `wait`，退出 `-15`，运行日志均为 `0` 字节。
窗口边缘拖动前的前台应用归属检查未通过，三个回执的
`pointer_drag_posted=false`；没有发送该拖动动作，也没有取得紧凑尺寸。

三个不同配置的截图 SHA-256 却完全相同：
`02a33ae3d8d45c37e55274e722cfbb10d930f8a78354e0ed00b24e01d68e3e99`。
图像还保留相同的中文深色界面和配置变化提示，与主题/语言输入不相符。
这些截图不能证明三个配置实际呈现，也不能作为当前源码像素验收证据。

随后使用桌面专用通道 `@oai/sky` 的 `list_apps`，实际返回：

```text
The Mac is locked and automatic unlock could not unlock it.
Ask the user to unlock the Mac manually before continuing.
```

原生交互随即停止。截图来源和新鲜度不足以核验本轮视觉状态；不据此推断
主题实现失效，也不为了让验收通过而修改产品逻辑或放宽断言。

## 结论与交接

静态 macOS arm64 包检查为 `PASS`；桌面主题、窗口几何和控件操作验收为 `HOLD`。
AX 窗口数、截图命令退出 `0`、权限预检和进程存活均不能关闭该边界。

原始构建、包检查、运行回执与未通过的图像保存于忽略目录
`work/monitor-panel-20261114/native/`。本轮审查 worktree 和独立 target 已清理；
临时 staging 应用及隔离配置在保留 manifest、日志和失败回执后清理。
没有创建容器，没有停止或清理其他任务的进程、工作树或应用。

待桌面通道实际确认解锁后，重新取得当前最终源码/依赖的精确包，验证窗口归属、
主题和语言真实变化，再验默认与 900×580 内容区。完整 AX 控件树、VoiceOver、
IME、真实远端监控、Podman、Windows/Linux 桌面、签名安装仍需单独验收。
