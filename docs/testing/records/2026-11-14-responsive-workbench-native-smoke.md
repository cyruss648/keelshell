# 响应式工作台 macOS 精确包与启动短验 — 2026-11-14

## 范围与来源

本记录验证当前工作树提交
`e4e9eb507ce0e736e42e2939889a555a07baa5b4` 生成的 macOS arm64 开发包。
其中生产源码来自已复核的 `a33e4c3b154867e4ef542cd9ef6a1193aee31c19`，
`e4e9eb5` 只增加文档记录。包使用独立 Cargo target 和隔离数据目录，没有
安装、替换或连接已有应用数据。

## 构建与包检查

构建命令：

```text
MACOSX_DEPLOYMENT_TARGET=15.0 \
CARGO_TARGET_DIR=/private/tmp/keelshell-native-target-e4e9eb5 \
cargo build --release --locked --offline \
  --target aarch64-apple-darwin -p keelshell-app -p keelshell-mcp
```

结果：退出 `0`，耗时约 193 秒，编译 `565 crates`。

staging 命令：

```text
KEELSHELL_BUILD_COMMIT=e4e9eb507ce0e736e42e2939889a555a07baa5b4 \
python3 packaging/package.py macos \
  --binary /private/tmp/keelshell-native-target-e4e9eb5/aarch64-apple-darwin/release/keelshell-app \
  --mcp-binary /private/tmp/keelshell-native-target-e4e9eb5/aarch64-apple-darwin/release/keelshell-mcp \
  --target aarch64-apple-darwin \
  --output /private/tmp/keelshell-native-e4e9eb5/package
python3 packaging/inspect_native.py --platform macos \
  --target aarch64-apple-darwin \
  --stage /private/tmp/keelshell-native-e4e9eb5/package
```

两个命令均退出 `0`。包 manifest 的 `build.commit` 为
`e4e9eb507ce0e736e42e2939889a555a07baa5b4`，`installed=false`，
`signed_by_packaging_script=false`。GUI 和 MCP 均为 arm64 Mach-O，两个二进制
的 `LC_BUILD_VERSION` 均为 macOS `15.0`，Info.plist 也声明最低版本 `15.0`。

本次包文件摘要：

| 文件 | SHA-256 |
| --- | --- |
| `package-manifest.json` | `f142da3571468fa67efbc7113b44ac8474b31a0b4281b3e78112acbef205abee` |
| `KeelShell.app/Contents/MacOS/keelshell-app` | `1f4da1c7d400fbdcd09eb0409963b1f9a4e7a0628db308528b416c7757110e0b` |
| `KeelShell.app/Contents/MacOS/keelshell-mcp` | `f6fb3180795f76b15bc5a6fbd5695379ec79c5516d3b1531c426a644bdb287b2` |
| `KeelShell.app/Contents/Info.plist` | `dd22add85ade50271b72d6e67859f36c19c9a64c88df25bf51d5a161a1973c70` |

## 启动短验

使用包内真实 GUI 可执行文件和隔离目录
`/private/tmp/keelshell-native-e4e9eb5/data` 启动，脚本观察 8 秒：

```text
launched_pid=27345
running_after_8s=True
exit_code=-15
log_bytes=0
```

进程在观察窗口内保持运行，没有输出崩溃或启动错误；随后只向该脚本自己
启动的 PID 发送 `SIGTERM`，未安装应用，也未覆盖用户应用目录。

随后在另一次隔离启动期间执行：

```text
osascript -e 'tell application "System Events" to tell process "keelshell-app" to {frontmost, count of windows, name of every window}'
```

命令退出 `0`，读回 `false, 0,`。这证明系统事件可以找到启动进程，但本次
脚本化查询没有读到可访问窗口，因此不把它记录为窗口布局、AX 控件或视觉
验收通过。

## 打包测试

```text
python3 -m unittest discover -s packaging -p 'test_*.py' -v
```

结果：`57 tests`，全部通过，退出 `0`。测试覆盖六目标 companion 包合同、
manifest/架构/权限、归档安全、发布幂等性和原生检查失败关闭路径；它们不代替
实际 Windows/Linux GUI。

## Podman 联调检查

本机 Podman 客户端为 `6.2.0`，已有 `podman-machine-default`，初始状态为
stopped。两次执行 `podman machine start podman-machine-default` 都返回启动成功，
但 machine 随后立即回到 stopped；紧接着执行 `podman info` 均因
`127.0.0.1:56260` connection refused 失败。没有成功读取 Podman image 列表，
也没有创建、启动或清理任何测试容器，因此本片不把 Podman 主机监控或容器真实
联调写成通过。当前 machine 已保持 stopped。

## 未覆盖边界

本次没有覆盖：

- 可见窗口的真实布局、主题切换和 900×580 调整；
- VoiceOver、NVDA、IME 和完整键盘辅助技术；
- 真实 SSH、Podman 主机监控和外部 AI provider；
- 签名、公证、安装、覆盖保护、升级和回滚；
- Windows/Linux 原生窗口；
- 多显示器、系统缩放和真实像素对比度。

结论：当前源码已获得精确 macOS arm64 包结构和启动级原生证据；窗口级
原生验收仍保持开放。临时 package、Cargo target、隔离数据和启动日志在记录
摘要完成后清理，不作为长期工作树或安装产物保留。
