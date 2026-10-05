# 2026-10-05 AI 全已知秘密目录与入网边界 C 修复

- 基线：23310ee13286adb488a451277addf49b05cd476b；应用冻结 B 完整组合补丁，B 原作者与复核树不修改。
- 范围：只修全已知秘密缺失的 Discover/Test/paging 入网边界，保留 A/B 的传输、UI/vault、Basic 派生值、五路径 Ask/reply 生命周期实现，不增加产品功能或升级依赖。
- 当前状态：C 原作者候选及证明保持冻结；新独立复核随后发现保留的重复名 inactive header 被全已知秘密集合遗忘的 P1，C 不整合。原通过不关闭新失败，D 修复与验证见[草稿秘密记录](2026-10-05-ai-request-options-draft-fix.md)。
- 未提交、推送、启动 GUI/vendor/云模型，不修改 shared HANDOFF / ROADMAP / README。

## 原反例与失败保留

原 5801 字节 private_catalog_probe.rs SHA256 `9b567a035dcf736c419bce8e66d42fe33bcc445199ab5039ba26e9357d312e9c`，源码与业务断言原样接入本树 include!，无外部编译路径。先在冻结 B 生产源码重放：真实 GET 1 / POST 1、catalog admitted true、posted model contains known secret true、owned server joined 后 exit101。C 修复后原驱动可自然完成，真实 GET 1 / POST 0、两项 false、owned server joined，通过原业务断言，未改成“需要第二 HTTP 才可 join”的假通过。原 2998 字节五路径 probe SHA `020049030c4a611a5d83146231b71de7ad49bf5c4d920575235c692949b3f732` 保持。

B 作者 35 proof / 36 archive 成员（内含 A 的全部失败与原归档）及 B fresh review 118 proof / 119 full archive 成员逐 bytes/hash 复制自 root 保存的冻结证明；review-B full manifest SHA `8170ffb79f2d53925254fe987452f4b02a87b281c25c1eb08de80f8a7997bb6c`。不删除或覆盖旧失败。

## 独立构建与当前收据

managed worktree、0700 evidence/TMP、0600 日志/收据；parent 授权只读静止 B target 的独立 APFS clone。复制 114.873s + 核验 38.740s，源/目标 572250 regular files / 6671 dirs / 115954698943 bytes，所有 listing/type/size 对应，0 shared inode / 0 symlink / 0 other，source unchanged。复制后停止 B 读取；所有源码、probe、runner和证明自包含，root/reviewer target 未使用。

| 收据 | 结果 | 范围 |
| --- | --- | --- |
| catalog-c-negative | exit0 / 0 tests | 首次 exact filter 少模块限定，201.218s；不计验证，原收据保留 |
| catalog-c-negative-exact-2 | exit101 | 0.786s / inputs unchanged；原反例实际 GET1POST1，负面断言失败 |
| known-secrets-http-c-1 | exit101 / 4 passed 1 failed | 新夹具首次漏 Anthropic assistant role，InvalidResponse；只修夹具，失败保留 |
| known-secrets-http-c-2 | passed / 5 tests | 2.600s / inputs unchanged；真实三协议、cursor计数、model/encoded endpoint零网络、snapshot mismatch、私密/资源限额 |
| catalog-c-positive-1 | passed / 20 tests | 13.604s / inputs unchanged；原目录probe原样、900×580三协议生产InputState及原设置回归 |
| c-fullcheck-1 | passed | 324.683s / inputs unchanged；45 Rust harness / 1106 passed / 0 failed / 11 ignored，Python6、fmt、strict workspace Clippy、x.y、普通及2MiB控制器分别通过 |
| c-macos-build-1 | passed | 46.934s / inputs unchanged；独立target，MACOSX_DEPLOYMENT_TARGET=15.0，app与MCP，不启动程序 |
| c-native-inspection-1 | passed | 0.174s / inputs unchanged；只读 file/vtool/SHA，两个arm64 Mach-O，不启动程序 |
| catalog-c-final-exact | passed / 1 original probe | 55.223s / final工程输入unchanged；GET1POST0，目录/POST秘密两项false，owned server joined |
| five-path-c-final-exact | passed / 1 original probe | 21.865s / final工程输入unchanged；五条preview观察全部false，0send/CLI/job |

格式写入 fmt-c-1 / fmt-c-3 command exit0、wrapper90 / input change 为有意格式变更，不计检查通过；fmt-c-2 inputs unchanged / exit0。最终1106含8doc tests；11ignored为2vendor CLI+9OpenSSH，ordinary/harness-false controller与显式2MiB controller另记，不重复累计 focused 的重叠选择。原五路径、目录反例、新known secret change目录清除/stale callback均在最终门禁通过。既有block0.1.6 future-incompatibility提示保留。

只读构建产物：app 181217440 bytes / SHA `fc30e9d633447f9d52f7ddd2ae5b06f329806f2964a22e0da17148806addc23a` / arm64 Mach-O / minos15.0；MCP 16659536 bytes / SHA `73e87b2980e2617ce01cd7d48193d5a87c4d2fc467ce3e7199d1fcfc16398a20` / arm64 Mach-O / minos11.0，SDK27.0。二进制只记录hash/大小，不放入源码或proof archive。

交付是相同基线的完整组合patch、逐文件SHA/大小/status/baseline与全部工程输入manifest、原B/review-B完整归档、C所有失败/零测试收据/通过日志与proof manifest/archive/seal。原两个probe fullcheck后仍与原字节相同，reverse apply检查、逐源/归档readback核验后停止写入。无作者自有cargo/clone/GUI进程；C target/TMP保持独立，root保存证明并完成需用的clone后可清理，源码与冻结证明保留供fresh review。

## 设计与验收边界

全已知值保存为 RequestOptions 私有 zeroizing 边界快照，稳定 equality 绑定原审核及客户端，不参与头/认证传送。所有 profile 的当前已知 API key、请求头、代理原值和 Basic 两种派生形式都可参与；未读取未知/未显式请求的 inactive 环境变量。每页目录和 last_id 解析后检查，每次 send 前检查 endpoint（含百分号编码）、分页 URL 与正文；单个总期限/累计字节预算/取消机制不变。新获知的秘密删除已有目录中的字面 ID，并撤销旧操作及 stale callback。

原复核只实际测量 Chat/authNone/Ephemeral inactive bare Basic 的发现→Test 泄漏；分页在原复核仅源码推断。新的 bounded HTTP cursor 测试实际观察首 GET 数量1、无第二 GET，不能追写旧复核为已测第二缺陷。三协议新 HTTP 夹具验证当前明确头/认证仍发送，非活动已知秘密不在请求中；固定 Test 返回 model 标签和 Ask 回复脱敏。

所有证据是源码、受控 GPUI 与回环，不等于 macOS 实际窗口、Windows/Linux 原生、供应商/云模型、有效 HTTPS proxy 证书成功、安装或完整产品验收。最终 C 仍需全新非作者复核和 root 整合/原生验收。
