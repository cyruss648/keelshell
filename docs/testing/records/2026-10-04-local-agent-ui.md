# 命名本地 CLI 与审核式助手 — 2026-10-04

## 范围与失败保存

在 backend `6276875` + 修复 `da31d98` 上接入 core 显式 execution metadata、设置页、密钥 vault 与助手问答。后端新独立只读复审通过77普通+2文档、真实process harness、两种安装版 CLI→自有 SSE、严格Clippy/fmt/policy；15份复审证据已复制至 ignored `work/local-agent-evidence-20261004/independent-fix-review/`，逐文件SHA核对一致。原独立 review manifest SHA256为 `ecf066e741dad5e33bd3f973e110ecb30130db53edbdf5724625bcbd93a03982`。这些结果早于图形接入，不代表 GUI 已验收。

首次 app check 的 preview 枚举尚缺 destination accessor，日志 `work/local-cli-ui-second-check.log` 保留。首次 local 专项6通过/4失败：Claude/loopback base origin被原API完整路径规则拒绝，以及新布局测试未登记 observation target；产品校验按 backend 分离，测试 observation补齐，修复后10专项通过。日志为 `work/local-cli-ui-local-tests.log` 与 `work/local-cli-ui-local-tests-fixed.log`。

首次全门禁因新测试使用被项目Clippy禁止的 `expect` 停止（20处），没有放宽lint，改成测试内明确失败上下文；失败日志 `work/local-cli-ui-gate-first-20261004.log` 保留。第二轮全门禁通过927普通+8文档、真实自托管process harness、脚本6项、fmt、workspace/all-target strictClippy和x.y策略，日志 `work/local-cli-ui-gate-second-20261004.log`。

## 原生检查发现与修复

初始测试包 Mach-O SHA256 `a30fc164c814c90da6e7b8ef18c00cd440bd919ae5a06e36a0bd1b43a19210c6`：macOS真实窗口确认Codex 0.160.0与Claude Code 2.1.285检查成功；自有SSE收到零请求，scratch条目零。两种metadata应用成功且state不含fixture API密钥；连接自有echo-only SSH并主动选择125字节屏幕上下文，准确CLI审核出现。

随后发现两个真实使用缺陷：profile选择器用HTTP gate禁用了CLI，长CLI JSON在受约束的flex列中没有自然滚动范围、发送按钮不可达。修复为按backend gate选择，以及自然内容/外层滚动视口。新增真实GPUI点击profile选择/撤销HTTP审核、双语长JSONwheel滚动至可见发送按钮；完整assistant专项20项通过（包含1项既有workspace布局），日志 `work/local-cli-assistant-scroll-final-tests.log`。初始包未完成发送，不计入完整问答验收。

初始controller/app/SSH/SSE已停止；fixture根删除，scratch零，回执 `work/local-cli-native-20261004/receipt.json`；原始fixture与包生成/构建日志保留。新冻结构建的完整问答、取消、语言/主题操作与清理结果将另行追加。

## 最终工程检查与新独立复审

最终 `python3 scripts/check.py` 成功：929普通、8文档、6脚本、真实自托管process harness，fmt、strict workspace/all-target Clippy和x.y策略通过。10个普通忽略项为两项显式供应商CLI和八项外部OpenSSH；它们未混入929。47项打包回归另行全过。日志分别为 `work/local-cli-ui-gate-final-20261004.log` 和 `work/local-cli-package-tests-20261004.log`。

新独立代理只读审查core/app：20助手、20设置、6凭据、core312普通+4文档、另跑11存储集成、严格Clippy和格式通过，没有剩余可复现P1/P2。受审源码九项SHA与原生构建前源一致，记录 `work/local-cli-ui-independent-review/{REVIEW.md,source-snapshot.json}`；成功结果没有删除早期失败。

## 最终 macOS 原生问答与取消

最终开发构建 Mach-O SHA256为 `7f81bc6ff5a06e6296d765bf9938d4bb4180e55d54c5eb808418ef8788b79a94`，新包/启动/HTTP/清理收据保存在 ignored `work/local-cli-native-final-20261004/`。应用不安装至用户目录；配置与TMPDIR均为自有隔离目录，真实窗口操作使用CUA。

- 实际安装版Codex0.160.0、Claude Code2.1.285的原生检查通过，检查阶段自有SSE请求零、scratch零；两种临时fixture密钥在字段中掩码，Apply后state无密钥。
- 核对自有SSH首次指纹后认证，主动选择125字节终端屏幕；中文完整问题与准确CLI JSON（执行文件、地址、模型、工具/隔离策略、准确stdin）可滚动至完整可见发送按钮。首次自动化尝试用 `down` 没移动，核对本机自然滚动映射后 `up` 可滚至底部；未据此再次修改产品代码。
- 用户界面显式发送Codex至 `/v1/responses`；收到完整fixture答复，建议只能显式送入命令审阅区，命令未执行。随后临时切到Claude、浅色及英文，问题、SSH上下文和命令草稿保留；重新审核发送Claude至 `/v1/messages?beta=true`，完整答复与英文审核动作可用。
- 回执的Codex tools缺省、Claude tools空；请求含显式canary。Claude会先发HEAD `/api/hello`，不能把完整操作写成只发一次网络请求。收据不含密钥或上下文原文。
- 慢请求首次因自动化往返超过fixture15秒hold，取消元素已失效；该尝试不算取消证明。重新在同一CUA调用中按最新状态点击取消，服务记录真实held POST与 `cancelled_peer=true`，scratch零；迟到答复未恢复回复/命令按钮。只证明本地取消，不证明供应商服务取消。
- 正常退出重启后，浅色/英文和两种CLI配置保留；预览明确提示缺API密钥。state仍无fixture密钥，未产生额外SSE推理。重启辅助launcher的300秒观察截止异常保留在restart.log，finally已停止owned child；这不是应用崩溃证据。

最终controller、两个app PID和SSH fixture全部不存在，两个listener已关闭，fixture根已删除，scratch零；`cleanup-and-source.json`逐项记录源码匹配、二进制SHA、进程与监听核对。截图通过CUA在会话中保留，未抓取用户默认连接或账户。

## 当前待核验

该功能提交的远端Quality结果待追加。未验证Windows/Linux原生窗口/供应商CLI、真实云端账户/订阅登录、任意其他CLI版本或Agent工具编排；不能把headless进程或交叉编译视作这些验收。自定义目录/预算/环境引用和可见逐步流继续按产品计划实施。
