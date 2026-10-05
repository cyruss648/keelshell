# 2026-10-05 AI request options 实施记录

- 基线：`23310ee13286adb488a451277addf49b05cd476b`
- 工作树：独立 `ai-request-options` managed worktree；不提交、推送或启动安装应用
- 状态：原 A 作者候选冻结后，独立反例确认 P1；不得整合。原字节证据保留，B 修复另记
- Toolchain：1.98.1；registry requirements 为 x.y，Cargo.lock 保留 resolved patch

## 构建隔离

经 parent 授权对闲置源 target 只读 APFS clone。第一次误用 60 秒进程总时限而中止，失败 receipt 保留，partial target 未复用。第二次 fresh clone 完成：77.261s 复制＋19.460s 完整核验；545682 文件、6559 目录，源与目标数量/size 一致，0 shared inode，0 symlink，源未修改。私有 TMP 与 evidence 目录 mode 0700，receipt / log mode 0600；clone 结束后告知 parent 源已不再读取。所有后续 cargo 使用本工作树 target。

## 已执行的 focused 验证

| 收据 | 结果 | 说明 |
| --- | --- | --- |
| layer1-test | 48 passed | 新底层落地后的原 AI lib 行为 |
| app-check-1 | failed | Zeroizing serde / Stateful Div / 测试 helper 集成错误；日志保留 |
| app-check-2 | passed | app 含 tests 编译；不是窗口验收 |
| options-http-1 | failed | 新 Basic 编码 Display 实现错误；修复后保留日志 |
| options-http-2 | failed | 新测试误用 AssistantReply private field / PartialEq；修复后保留日志 |
| options-http-3 | 6 passed | 真实 loopback direct/header、HTTP proxy、SOCKS5、无 fallback、redirect、取消、validation |
| options-http-4 | 9 passed | 增加 Anthropic 多页秘密 cursor 拒绝、SOCKS5h proxy DNS/auth、HTTPS proxy TLS handshake/failure |
| app-options-1/2 | failed | GPUI 测试通配 import 引发宏递归 / 缺少 AppContext；修复而未提高递归限制 |
| app-options-3/4 | failed | 测试 helper 在 entity update 内重入 render / 拆分 view 后方法可见性；修复 |
| app-options-5 | 5 passed, 1 failed | 空秘密槽被插入；修复为空值立即移除 |
| app-options-6 | 8 passed | app 请求选项 / 真实加密 vault / 迟到结果 / inactive-profile review 撤销 |
| focused-final-1 | failed | queued 清空与目的地防护缺失导致 3 项真实回归失败；恢复防护，不弱化断言 |
| app-options-7 | 9 passed | header/ref/destination 独立隔离、无效草稿保留与阻断 |
| options-http-final-1 | 13 passed | 12 项行为加 1 项独立子进程 fixture helper；见下方传输证据 |
| fullcheck-1 | failed | 1 项 collapsible_if 与新增测试 expect_used；修复，不放宽 Clippy |
| app-options-vault-ui-1 | failed | purpose 行 focus helper 的私有字段边界 / Focusable import；移回所属模块 |
| app-options-vault-ui-2 | 10 passed | auth=None 的 header/proxy vault prompt 实际 click/input/cancel/focus；900×580 固定 footer |
| fullcheck-2 | failed | policy/Python/fmt/strict Clippy 通过；旧 core 测试仍断言 Env API auth 不支持，更新有效引用 admission 与无效身份拒绝 |
| fullcheck-3 | passed | 完整统一门禁 238.616s、inputs_unchanged=true；未将其当成新增 UI wire 测试后的最终输入 |
| app-options-end-to-end-1 | failed | 新测试误将 TestAppContext 当作 App 读取 entity；改为 read_with |
| app-options-end-to-end-2 | 12 passed | 21.527s、inputs_unchanged=true；新增真实 UI options→discover/Test→Apply→Assistant review/send 同代理、原站零连接、回显脱敏；theme 切换保留无效草稿 |
| fullcheck-4 | passed | 167.254s、inputs_unchanged=true；Python 6 项 / policy / fmt / strict Clippy / Rust 1095 passed、11 ignored / 普通及小栈进程控制器 |

传输回环覆盖 Direct 自定义头在 discovery/Test/Ask 一致、HTTP Basic 代理三条路径与原站零连接、SOCKS5 与 SOCKS5h 的真实握手和 DNS/认证、HTTPS 代理 TLS ClientHello 与失败关闭、代理失败无直连 fallback、选项不匹配零网络、禁止 redirect、请求前与响应中取消、Anthropic 每页头一致/秘密游标拒绝、分页累计响应预算，以及 `NO_PROXY=*` 独立子进程不绕过显式路由。代理 Basic 编码、用户名、密码和请求头回显均脱敏。

原始日志与 exit 收据在 ignored evidence 目录，未含用户密钥、客户日志或真实服务配置。所有 focused 最终验证与 fullcheck 收据都记录输入 hash；通过的 focused 验证输入未变化。格式化包装器的 `inputs_unchanged=false/90` 表示预期格式写入，实际 fmt exit=0，并非检查通过。重用已有 label 被拒绝，没有覆盖历史证据。完整作者候选补丁与输入 manifest 已按最终文件冻结在 ignored evidence 中；manifest 包括修改与新增源码、lock、产品/ADR/测试记录的文件 size/hash 与基线。完整 proof manifest 包含每次运行、所有失败、clone 收据与日志 hash。候选冻结后无 cargo 或 GUI 进程，不自行提交或推送。非作者复核、main 门禁及原生验收仍待 root 记录。

最终 Rust harness 汇总为 1095 passed、0 failed、11 ignored（44 个 harness，含 doc tests）；普通/小栈的 harness=false 进程隔离控制器另外通过，不混入 1095。11 项 ignored 为两项已安装供应商 CLI 与九项临时 OpenSSH 服务测试，不视为成功验收。新增 HTTP suite 13 passed 中一项为 NO_PROXY 独立子进程 helper，其余 12 项行为；app focused 12 项包含真实 UI→HTTP 代理完整链路。

## 尚未验证

不把编译、GPUI controlled test windows、TCP fixture、HTTPS TLS 握手失败关闭或依赖 API 证据当成真实云供应商/代理证书/已安装应用验收。本轮无 macOS 实际窗口、Windows/Linux 原生运行或生产 SSH/AI 目标；这些由 root 后续整合与验收记录说明。


## 独立复核否定 A

独立五路径反例观察非活动代理的裸 Basic 编码进入另一个配置的已审核正文，五条路径均为 true；0 send / 0 CLI / 0 job。独立 negative receipt 为 exit101、53.48s、inputs_unchanged=true。A 本机门禁通过不能关闭该 P1，原冻结 A source / proof / archive 没有修改；新修复在独立 B worktree 完成并另行复核，见[修复记录](2026-10-05-ai-request-options-basic-fix.md)。
