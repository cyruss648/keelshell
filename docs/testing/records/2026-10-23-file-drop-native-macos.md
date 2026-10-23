# macOS 原生文件管理器拖放验收记录 — 2026-10-23

## 范围

本次尝试验证 FILE-02 的 macOS 原生桌面闭环：从 Finder 将本地文件或目录拖入 KeelShell 的远程文件表，进入审核，取消或确认后分别检查远端写入和逐字节读回。验收目标仅覆盖 macOS Finder；不能据此推导 Windows Explorer、Linux 文件管理器、辅助技术或完整跨平台桌面通过。

## 隔离构建

在仓库 `/Users/hdh/work/develop/projects/keelshell` 的干净 `main`（`e2233c81b90a181e6c5636d199f10b84227fd73e`）上完成：

```text
cargo build -p keelshell-app -p keelshell-mcp --locked
python3 packaging/package.py macos \
  --binary target/debug/keelshell-app \
  --mcp-binary target/debug/keelshell-mcp \
  --output work/native-file-drop-20261023/package
```

构建和隔离打包成功。隔离应用包位于 `work/native-file-drop-20261023/package`，配置和日志位于 `work/native-file-drop-20261023/data` 与 `work/native-file-drop-20261023/logs/app.log`。本次没有安装、签名、替换已有应用、创建标签或发布 Release。

## 原生桌面结果

未执行 Finder 拖放。原生 UI 工具在尝试获取桌面状态时返回：

```text
Native apps: Error: The Mac is locked and automatic unlock could not unlock it.
Ask the user to unlock the Mac manually before continuing.
```

因此以下证据均为 `UNRUN`：拖放事件、远程文件表高亮、审核界面、取消后的零写入、确认后的远端读回、中文浅色与英文深色场景、截图和 AX 信息。隔离应用进程已终止，没有留下运行中的测试应用。

## 后续准入

Mac 解锁后，使用同一隔离包重新执行：

1. 普通文件和目录各一次 Finder 拖入。
2. 中文浅色与英文深色各观察一次审核内容和目标路径。
3. 取消审核并确认远端没有新文件和原子写入。
4. 再次拖入并确认，逐字节读回文件和目录内容。
5. 在忙碌、目录变化、会话切换和已有待审操作期间重复拖放，确认旧事件不能越权。

在这些步骤完成前，FILE-02 的 macOS 原生 Finder 验收保持 `OPEN`；受控 GPUI 拖放和本次隔离包构建结果不能替代原生桌面证据。
