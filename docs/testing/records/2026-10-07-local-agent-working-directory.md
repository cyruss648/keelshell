# 2026-10-07 本地智能体审核式工作目录

状态：独立作者工作树候选；尚未整合、提交或通过新的非作者复核。分支父检查点为 `0cca8ac59d12afdc68512b45db8830810835c634`。本记录仅覆盖默认隔离与显式所选工作目录，不证明完整 AI Agent、订阅登录或三平台桌面目标完成。

## 实现

领域/存储提供缺省 `isolated` 和绝对路径 `selected`，旧资料不迁移即可继续使用隔离模式；API 配置独立。设置提供模式切换、完整手工路径、原生目录选择、后台检查、可滚动完整所选/规范路径。草稿、配置、revision、取消与关闭使旧检查/选择器结果失效；目录变化也使已加载本地密钥的 delivery 绑定失效。

发送审核展示完整路径、运行方和明确 SSH 选区。后台持有目录组件身份及有界 Codex 元数据，拒绝符号链接、错误类型/语法、超量与变化。Unix 在 GPUI / Tokio 初始化前的固定 bootstrap 中复核 inode、摘要和 64 KiB 严格帧，用安全 `fchdir` 进入审核目录。Windows 目录句柄拒绝删除与 reparse/device/远程路径；本轮没有 Windows native 接受结果。

所选 Unix native 文件从持有常规文件复制到私有 `0700` 树；完整摘要绑定原文件、副本及实际能力探测。Darwin Codex 只复制三项 ≤64 KiB 公共签名元数据，原 bundle 和副本执行固定 strict codesign 检查。没有复制其它资源、登录、助手程序或配置。进程封装不等于任意同用户恶意进程的系统沙箱。

所选 Codex 完整项目层保持不可信，项目说明/技能发现、hooks、MCP 和工具不启用；有效能力准入要求 19 项禁用及 skip-host-discovery。唯一已核对的 vendor disabled-host 提示只在初始化后、turn 前以 `item_0` 严格形状出现一次，既不是回答也不是成功；未知/重复/晚到提示、重复 JSON 键与其它错误拒绝。默认隔离流保持原有规则。

## 真实 installed CLI / loopback 证据

测试调用 `LocalAgentClient::ask_with_progress`、实际构建的 app bootstrap 和原生 CLI，推理端为本机自有 loopback SSE 服务，不使用供应商账户、真实 API 密钥或客户主机。测试在所选目录、祖先、技能、model-instructions、环境和项目配置中放置六个不同内容 canary，并配置经过独立 viability 启动的 hook/MCP 程序标记。

- Claude Code `2.1.285` 实际通过：4 帧，完整候选与最终成功一致，1 个 Messages 推理请求；显式问题与 SSH 选区存在，六项目录 canary 均 false、tools=[]，没有项目 hook/MCP 启动，scratch 已清理。
- Codex `0.160.0` 完整签名副本实际通过：5 帧，1 个 Responses 推理请求；上述 canary 全 false、显式选区 true、tools=null、0 项目 hook/MCP，scratch 已清理。此副本的 native 字节 SHA-256 为 `6b582e8813ce7e8ed4c52814ee5cf230dba647bf2292df747a4003f2657ef201`。
- 当前已安装 Codex 在开发中更新到 `0.160.1`；原准入正确拒绝为 `UnsupportedVersion`、0 推理请求，并清理 scratch。该失败保留。核对后显式接受 `0.160.0` 和 `0.160.1` 两个 patch，后者实际通过同一生产适配器和全部内容/工具/清理断言。当前 native 字节 SHA-256 为 `cc0a05e34876414280a79726153d0fe8d55c93f704ef6c292ec409bfe36d5b06`。

精确官方 compare 仅有版本行及 remote-stdio MCP 的 Windows 环境保留变更；下载的 35 份相关 config/CLI/skills/host-warning/session 源码逐字相同，三项公共签名元数据也实际相同。该核对只支持明确两个 patch，不猜测整个 minor。参考[官方非交互模式](https://learn.chatgpt.com/docs/non-interactive-mode)、[精确版本源码差异](https://github.com/openai/codex/compare/rust-v0.160.0...rust-v0.160.1)。

作者 ignored 收据保留在 `work/local-agent-working-directory-20261007/continuation-v1` 至 `continuation-v5`、`full-gate-v1` 至 `full-gate-v4`、`ui-layout-v1` 至 `ui-layout-v4` 和两版官方源码目录；任何静态版本/帮助命令不作为模型调用成功。

## 门禁、失败与未核验范围

作者 77 项 AI 单元测试通过。此前 20 项真实 selected-directory native 控制器完整通过，含实际相对目录读访问、目录换 inode、保留旧 inode、取消、frame 篡改、未知版本、metadata 变化和 malformed bootstrap。新版本调整后一次 Claude held-cwd 准备的 3 秒 readiness 等待失败，adapter 终态未捕获；原失败保留，随后增加提前结束的 typed 终态观察且保持原 3 秒/8 秒边界，增加观察后的 20 项同预算重跑通过；未推断该旧失败的后台终态。

首次全门禁因未完成候选的严格 Clippy 发现参数/借用/条件样式与新测试 lint 作用域，已修复且后续严格 Clippy 通过。随后完整 app 套件有 552 项通过、2 项审核预览失败：长原始 JSON 没有横向范围，固定内层预览与旧外层高度断言不一致。真实 nowrap flex 内容、双向内层滚动、观察式滚动条和固定确认区域修复后，两项中英文 GPUI 行为实际通过；没有截断内容。规范版本解析拒绝前导零，三个版本/内置能力专项通过。原失败与私有收据保留。

最终作者 `full-gate-v4` 已实际通过：格式、x.y 依赖策略、workspace all-targets Clippy `-D warnings`、1533 ordinary Rust、8 doc、6 scripts Python、57 packaging Python、20 项真实目录控制器，以及默认和 2 MiB 小栈控制器各 626 条完整有序记录。常规工作区仍有 18 项 ignored；另行明确执行其中两项所选目录供应商 CLI 测试，Claude `2.1.285` / Codex `0.160.1` 再次通过，4 / 5 帧、全部六项 canary false、明确问题与 SSH 选区 true、空工具、0 项目 hook/MCP、scratch 清理均有各自回执。

五个正式命令均由作者 wrapper 实际 wait：格式、完整质量、打包、app 构建、生产 native 分别 exit 0；完整质量耗时 546.70 秒，日志 403295 字节，SHA-256 `0e0973a532400c44f0670e8717eda07ebc111acb48cb9630e7073024d44ef07d`。进程组均观察为不存在；私有 TMP 为空后移除；681 份源码输入在该门禁前后字节和摘要完全相同。更新本记录及交接/路线图是门禁后的文档变更，需要在最终候选源码映射中另行绑定。原作者工具 handle `38634` 已结束；没有依据旧 state 重启或终止任何其它进程。

最终冻结包包含完整源码与 patch、成功和失败的原始收据、精确官方来源，以及两个 Codex patch 的最小完整签名 bundle、Claude native、已测试 launcher/canary 的字节绑定；不携带通用资源或 target 缓存。作者结果不替代新的非作者复核，主树尚未整合或提交。最终 macOS 图形界面、最小窗口、语言/主题/辅助技术、Windows/Linux native CLI、订阅登录、任意环境和受限 Agent 工作流继续开放；没有把 loopback 实验或原生 CLI 当作这些验收。
