# 2026-10-07 工作目录控制器平台后续修复

状态：精确主线 `3873dacb9fe6608a9aec40136bb1d82200da9a51` 的 CI 已取得两个不同失败。限定 test-only 修复的正式本机门禁实际退出 0，新非作者源码和完整证据复核为 `NO_BLOCKER`。新提交的 Windows/Linux CI、Linux 同步根因、三平台桌面和完整产品仍未完成。

## 已确认的失败

[Quality 37597872725](https://github.com/cyruss648/keelshell/actions/runs/37597872725) 的 Windows job `112714793495` 在严格 Clippy 后失败于三个 AI directory 单元测试；共同 helper 无条件 canonicalize，生成被生产准入拒绝的 verbatim disk path。Linux job `112714793913` 完成前面的目录 Ask / 取消后，在整个串行控制器40秒上限失败；日志取得 Codex 首次实际cwd约7.5秒、Codex取消累计约26.5秒、Claude首次实际cwd累计约34秒，没有进入 app 同步测试。完整 raw logs 和各终态收据保存在 ignored `work/ci-3873dac-20261007/`。这不是生产目录权限或 Linux 同步根因已解决的证据。

随后 macOS job `112714793803` 已完成成功，整个 run 完成失败。三平台完整日志均已读回；macOS 的 556,601 字节日志 SHA-256 为 `bff1349e8fea3c5c255eade0afb60be068befbc2f7b78baec6b7c10db78fd37b`，三平台终态 `receipt.json` SHA-256 为 `bbf1acec7ae6c01b46d133e3de8ac9e8d6763bde5ee284294d39ea7b0cf06401`。macOS CI 成功属于父提交的 Rust/受控互通范围，不替代本修复的新 CI 或 GUI/模型业务验收。

## 修复与预算依据

Windows 单元夹具保留普通绝对盘符路径，Unix 仍 canonicalize 以消除本机临时目录的 symlink alias；所选路径和规范路径分别准确校验。新增 Windows 反例保持 verbatim、UNC、device 和 drive-relative 路径在 filesystem worker 之前拒绝，没有放宽生产准入或访问网络路径。

原40秒总控制器不等于单次请求预算。Unix 串行16次prepare/Ask、Windows11次，每次目录验证5秒、Ask8秒、异常子进程收尾3秒；四次 malformed launcher 分别2秒。readiness8秒和fixture内3秒gate都嵌在Ask中，不重复累加。新的总watchdog为 `case_count * (5 + 8 + 3) + 4 * 2 + 40`，Unix304秒、Windows224秒；最后40秒是一次性同步fixture复制/输入准备余量的选定测试兜底，不代表已测的OS最坏时间或可中断文件系统保证。

该总watchdog用于完成全部串行行为断言。所有单次Ask、目录worker、取消收尾、gate、malformed等待、版本/权限/内容/完整性断言不变；没有增加生产请求时限、修改crypto、串行化工作区测试或把部分结果标作通过。Linux日志没有阶段SHA计时或字节数据，所以不能把哈希计算表述成已证明的原因。

## 检查与边界

正式收据位于 ignored `work/directory-controller-followup-20261007-v1/scoped-quality/`。本机门禁实际 wait 退出 0，耗时 132.945 秒，734 份完整工程输入前后相等；格式、x.y 策略、workspace all-targets 严格 Clippy、全部 AI 测试和显式 2 MiB 控制器均通过。实际完成 142 普通 AI 测试、2 doc 测试、Unix 目录控制器 20 场景，以及默认和 2 MiB 各 626 条连续阶段记录；4 个供应商 CLI 测试仍 ignored。Windows 专用新增反例及其 15 个控制器场景没有在本机执行，需新 Windows CI。

完整 raw log 为 250,010 字节，SHA-256 `5fe083456a01e0dda44b8bb647ea56a052c1f6fea97fb86c76001c28da5797e5`。生产者实际 reap leader，所属进程组消失，私有 TMP 为空后删除。新的非作者全文读回并校验 734 个封存 payload、完整日志、前后输入与生产者脚本，独立复核两个 test 路径、预算和资源终态；`independent-review.json` 的 SHA-256 为 `355c3726794bf68b095ae33ae828fcf73b684c15eb173cf234166233a280517e`，结论 `NO_BLOCKER`。该复核是源码和正式收据复核，没有宣称复核者另行重跑 Cargo。

父提交733输入的完整工作区门禁保留在[平台记录](2026-10-07-local-agent-directory-platform-fix.md)；本轮生产源码没有变化，因此不会把父app测试说成新测试重新执行。门禁之后只更新本记录、HANDOFF 与 ROADMAP 的结果状态，其他工程输入保持逐字节一致。

此修复需要自己的新提交Windows/Linux实际CI。未安装或发布应用，也未进行新的桌面或外部模型业务验收。
