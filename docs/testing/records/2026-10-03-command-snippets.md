# 命令片段与建议集成验收

日期：2026-10-03。范围：离线片段管理、后台本地匹配、多行命令审核与目标隔离。

## 实现与独立复审

领域 API、片段编辑器和匹配模型分别独立实现并交叉只读审查。领域 13 项新增回归、编辑器 6 项（其中 5 项真实 GPUI）、匹配模型 10 项均通过。字段/磁盘边界见 [领域记录](2026-10-03-snippet-domain.md)，决策见 [ADR 0010](../../adr/0010-command-snippets-and-local-suggestions.md)。没有新增依赖或改变磁盘格式。

工作区 10 项真实 GPUI 回归通过，使用独立 StateStore 和确定性终端队列：

- 离线新建/编辑落盘，稳定身份，多行、Tab、中文及首尾空白原样保留。
- 删除二次确认与取消，已填正文不追溯变化。
- 异步建议点击仅填入，切换标签不能执行到新目标，回到原目标点击 Run 只发送精确命令加 CR。
- 输入修改后恢复相同文本、切换标签、来源删除、远端退出均拒绝旧票据。
- 历史哨兵和未执行草稿不进入后续设置保存。
- 模态焦点隔离，磁盘冲突保留完整可编辑草稿与外部文件。
- 双语多行框限高，普通 Enter 不向 SSH 隐式发送。
- 8 项建议上下移动至最后一项仍完全可见，Enter 只填入所选原文。
- 实际 marked range 存在时 Enter/方向键/Escape 不执行建议操作。
- 真实后台任务待返回时 Escape 能抑制迟到结果，新输入可重新搜索。

## 本轮发现与修复

审查发现单行输入会删除换行，已改 Textarea；set_value 不产生 Change，统一填入入口改为显式推进 revision。建议移到后台单 worker 缓存，避免每次重绘重新搜索。列表重算归零选择，Escape 同时覆盖在途匹配。

真实键盘测试发现 GPUI 先分发已绑定的 Input action，再处理原始 KeyDown；原 capture_key_down 因此收不到方向键，并在组件先清除 IME marked range 后错误处理 Escape。改为在命令栏捕获四种 Input action，未匹配/组合输入继续传播。最后一行曾因滚动容器上边框越界 1px，将分隔线移到滚动容器外后严格 bounds 断言通过，没有放宽测试。

编译阶段的缺模块、宏同名、API 使用及共享测试模块排序失败，以及上述真实交互失败均保留在 ignored work。关键日志为 `snippet-workspace-keyboard-tests.log`、`snippet-keyboard-scroll-diagnosis.log`、`snippet-workspace-keyboard-fixed.log`；最终 10 项通过为 `snippet-workspace-keyboard-fixed-2.log`。最初整仓门禁仅因新增 mod 排序未格式化而停止，记录在 `snippets-full-gate.log`。

## 原生 macOS

使用独立 `work/snippets-native/data`，没有覆盖安装现有程序。首轮构建已核验：未连接时打开命令工具区、新建空白片段、保存中文多行命令、切换英文重新编辑。磁盘核对确认 2 行正文、末尾换行及包含逗号的标签原样保存。随后连接仅回显、不执行系统命令的回环 SSH，真实建议行可见。

首轮构建早于键盘事件顺序修复，不能证明最终键盘逻辑。修复后重新构建并打包，在同一隔离配置中连接回环 SSH：输入片段名称后显示建议，按 Enter 原样填入两行中文命令和末尾换行，终端仍只有初始提示。点击“执行”后才出现两行回显，命令栏清空，命令工具区显示当前会话历史。回显服务不运行系统命令；监控 exec 被夹具拒绝属于预期限制。

最终 macOS debug 二进制 SHA-256：`d3edfbb3240ca4adbd2848554f9226a47bf29d375d97475fff1c29ad3a21363d`。验收使用 `work/packages/snippets-final/KeelShell.app`，构建日志为 `work/snippets-native-final-build.log`；磁盘原文与清理核对保存在 `work/snippets-native/verification.json`。截图在原生 UI 操作过程中检查，未另存仓库图片。

程序与回环服务均正常退出（exit 0）；临时 SFTP 根目录已移除，监听端口已关闭。隔离配置、构建产物和日志留在 ignored work，没有覆盖用户配置或安装目录。

## 整仓与公开提交检查

`python3 scripts/check.py` 最终通过：依赖版本策略、`cargo fmt --all --check`、严格 Clippy、385 项单元/集成测试与 2 项文档测试。日志为 `work/snippets-full-gate-2.log`。上游 `block 0.1.6` 的 future-incompatibility 提示仍存在，不属于本轮新增依赖。

公开范围检查覆盖 214 个 tracked/非 ignored 路径，其中 176 个文本文件；禁止的具名产品比较标识在路径、正文及本轮两条功能提交消息中零命中，所有相对 Markdown 文件链接目标存在。中英文 README 同步增加命令片段与本地建议能力，延续 [开源项目首页结构参考](../../research/readme-presentation.md)，保留开发预览与平台验收边界。独立复审重新核对三份官方开源 README，并确认所审文档的 52 个相对文件/标题锚点引用有效；未改打包代码。

领域提交为 `2ea9b1c`，工作区整合提交为 `02fc8fcbfeb77929e7d50906b0530dd566294361`。后者的 [Quality 运行 37094089051](https://github.com/cyruss648/keelshell/actions/runs/37094089051) 在三平台全部成功；下载各 job 原始日志后逐组计数如下：

| 平台 runner | 单元/集成测试 | 文档测试 | 打包回归 | 新增命令工作区 GPUI 回归 |
| --- | ---: | ---: | ---: | ---: |
| macOS 26 | 385 | 2 | 47 | 10 |
| Ubuntu 24.04 | 385 | 2 | 47 | 10 |
| Windows 2025 | 380 | 2 | 47 | 10 |

平台条件编译会影响测试总数；三平台均无失败。日志保存在 `work/snippets-ci-{macos,linux,windows}.log`，计数摘要为 `work/snippets-ci-results.json`。本记录的后续文档提交不改变该功能提交的代码。CI 证明编译和自动化回归通过，不代表 Windows/Linux 的实际桌面与系统输入法已完成验收，也不代表已发布或签名安装包。

## 证明边界

GPUI marked-text 回归不是操作系统候选窗口验收；回环 echo 不证明实际服务器命令结果。Windows/Linux 桌面、远端 shell 文件名补全、变量模板、批量执行和历史持久化仍未验收或未实现。片段显式明文保存；历史仍只在本次进程中。
