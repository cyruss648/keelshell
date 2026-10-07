# 2026-10-07 本地 Ask 工作目录平台修复

状态：基于主线 `0d64f09acfb7e671217a8bb1d3cb1397b421558a` 的限定修复已通过主树正式本机门禁及独立源码复核；最终非作者收据与提交级 CI 单独绑定。当前记录不证明 Windows/Linux 桌面、供应商模型、完整 MCP 业务或产品完成。

## 原始失败

[Quality 37591946965](https://github.com/cyruss648/keelshell/actions/runs/37591946965) 已完成，macOS job 成功，Windows 和 Linux job 失败。完整 job 日志、终态 JSON 和 SHA-256 保存在 ignored `work/ai-directory-ci-0d64f09-20261007/`。

Windows 严格 Clippy 报告七项条件编译/未使用错误：两个 Unix 专用 mut、版本辅助方法、目录持有字段、snapshot 方法和类型。Linux 完成首个实际 cwd 案例后，在第二个 held-cwd 请求仍运行时超出旧 3 秒 readiness 观察窗，未进入 app 同步测试；这次没有新的 Linux 同步根因证据。

## 限定修复

- Unix launcher 和能力探针使用局部可变 shadow，Windows 不产生无用 mut；仅供 Unix bootstrap 使用的 snapshot 与版本方法按实际平台限定。
- 目录句柄字段重命名为 `_file`，仍由审核 authority 持有到请求收尾。Windows no-delete share、Unix openat/fchdir 均保留，没有移除保护句柄或放宽目录准入。
- Windows 自有夹具保留普通绝对盘符路径，避免 canonicalize 产生的 verbatim namespace 被生产策略拒绝；预览解析为 JSON 后精确检查所选/规范路径，实际子 cwd 在 Windows 比较 canonical identity。
- Unix 继续验证目录替换、旧 inode 相对读取和发送前拒绝。Windows 用例断言待审及运行期间替换被 no-delete 句柄拒绝、原目录 Ask 成功，并在消费后确认句柄释放。控制器统计分别为 Unix 20 / Windows 15。
- readiness 观察采用与 Ask 相同的 8 秒预算，不续期 Ask 或 40 秒总控制器；没有更改生产请求预算。新 Linux 完整运行仍需验证，不能据此宣布旧失败已解决。

双语 README 与本地智能体指南同步已实现的目录选择和明确两个 Codex patch，保留当前原生验收范围。MCP 仍由 KeelShell 向外部智能体提供服务，应用内 Ask 独立，不新增通用第三方 MCP 客户端。

## 当前门禁与剩余范围

正式主树 wrapper 的三项命令全部实际 wait 退出 0，原始日志与前后输入保留在 ignored `work/ai-directory-platform-root-20261007/`：

| 命令 | 秒 | 日志字节 | 日志 SHA-256 |
| --- | ---: | ---: | --- |
| `python3 scripts/check.py` | 886.338 | 413431 | `adeb2154b77815a69cec7cdbf39ea60c3bce1eaec0d135582426f9e56682e72b` |
| 57 项 packaging unittest | 1.364 | 248 | `e5dcba5ef3e7fa6056ceb05441de62f9e0abbff9c74deefdd8ec299db5fbf8f7` |
| app + MCP `cargo build --locked` | 31.998 | 683 | `03ce9d4dbfe1f5208081077402752b197866577a07fefb60b15cde1ff5311736` |

完整质量包括格式、x.y 依赖策略、workspace all-targets Clippy `-D warnings`、1,612 普通 Rust / 8 doc / 6 scripts Python；22 项 ignored 未执行。Unix 自有目录控制器 20 项通过，默认和显式 2 MiB 控制器各 626 条有序记录并有最终 end。三项命令各自 733 输入前后相等，leader 已实际回收、进程组观察为不存在、私有 TMP 为空并移除；没有以中间通过输出代替退出码。结果文档随后更新，源码与其它非结果输入仍绑定原门禁映射。

新的非作者逐项检查五路径 diff、全部源哈希、原始 CI 日志及三份正式结果；Windows 预览转义/普通路径、no-delete 持有和释放测试的行为与 Unix inode 案例分别复核。当前修复未以本机测试代替 Windows/Linux 执行。

先前 scoped macOS fmt/AI Clippy/AI tests 的实际退出 0 保留在 ignored `work/ai-cwd-platform-fix-20261007/`，只绑定之前的五路径候选，不验收本记录新增 Windows 测试。三个旧私有 target 根在原 PID/PGID 不存在、限定消费者检查为空及路径/所有权核对后已清理，原 16 份收据及日志逐字节 hash 前后相同，独立清理记录仍保留；没有触碰主 target、共享 VM 或活跃工作树。

本轮没有放宽 Linux 同步审批期限、断言或加密参数，也未安装应用、发布标签或操作客户服务器。本机构建仅为开发产物，没有启动该新产物作桌面验收。精确新提交三平台 CI、完整原生 UI、订阅登录、受限 Agent 和完整外部 Codex MCP 业务继续开放。
