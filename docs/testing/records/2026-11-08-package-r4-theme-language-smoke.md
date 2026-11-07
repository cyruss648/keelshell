# package-r4 原生主题与语言短验

日期：2026-11-08（Asia/Shanghai）  
范围：当前源码提交 `fac90fef5d4edcd4e53a48ef1dfb559e101aa169` 的精确 macOS 开发包  
结论：**LIMITED_NATIVE_OBSERVED**

## 包绑定与静态检查

本次使用的包为：

`work/native-cua-20261108/package-r4/KeelShell.app`

`package-manifest.json` 的 `build.commit` 精确绑定
`fac90fef5d4edcd4e53a48ef1dfb559e101aa169`，包声明为未安装、未签名，目标为
`aarch64-apple-darwin`。GUI 和 MCP 均为 arm64 Mach-O；两者的
`LC_BUILD_VERSION` 均为 macOS `15.0`，SDK 为 `27.2`。

SHA-256：

- manifest：`787a0cd9d680c5d103f5ca998ce8ff70b9e38acc8f4beb8ba074b92b68af42df`
- GUI：`c6afd91a3cdb219a3974c92b90f0d2310d70c16a45ce1efc068bffe8e22cc139`
- MCP：`94eaf83f8bbb23ae4db7cc190c9c06332ddbc19b4bf9246dcdf35f43b3645d8f`

## 实际桌面观察

在已解锁的 macOS 桌面启动精确包，使用隔离数据目录，未安装或替换现有应用。
观察到：

1. 默认工作台可启动，快速连接卡片、连接库空态、底部工具区和顶部工作区操作均
   可见。
2. 打开“主题与外观”后，三个主题卡片及明暗模式控件均可见。
3. 点击“曜石”后，主题选择器立即切换到暖黑/石墨表面与琥珀强调色。
4. 点击“雾紫”后，主题选择器及工作区立即切换到紫灰表面与紫色强调色。
5. 点击“完成”后，雾紫工作区仍保持主题化的工具栏、快速连接卡片、连接库空态、
   搜索栏和底部工具区。
6. 点击中英文切换后，顶部工具栏、快速连接、连接管理器入口、连接库空态和底部
   工具区的可访问名称与可见文案切换为英文。
7. 打开并关闭连接管理器后，连接管理器的空态和关闭路径可见；关闭后工作台焦点
   与主体内容恢复。

本次短验没有触发 SSH 连接、凭据写入、AI provider 请求、配置导入导出、分屏、
多窗口、IME 或 VoiceOver。它只证明上述精确包在当前 macOS 桌面上的有限主题、
语言、模态和空态可见性，不授予 Windows/Linux 原生验收、完整辅助技术验收或
真实网络联调结论。

## 清理

精确包进程已停止，隔离运行目录已回收；包本身未安装，工作区没有留下本次运行的
连接、凭据或网络资源。
