# ADR 0050：审核绑定的 AI 请求头与显式代理

- 日期：2026-10-05
- 状态：A、B、C 分别被新的独立 P1 反例否定；D 草稿秘密修复作者完整门禁与MAC15双程序开发编译/只读检查通过并冻结，fresh 非作者复核、主树门禁与实际原生验收待完成
- 范围：命名 API 配置的发现模型、固定连接测试、审核后 Ask

## 决策

`RequestOptions` 是不可序列化、所有权明确的秘密快照。`ProviderConfig` 捕获选项，`PreparedRequest` 再捕获完整 provider；发送客户端逐项比较选项，在任何网络请求之前拒绝不同的头或路由。临时值使用 zeroizing 所有权，HeaderValue 标记为 sensitive。Debug、typed errors、审核摘要与模型目录不返回秘密值。

最多 32 条请求头，名称不超过 128 字节、值不超过 8 KiB，名称和值合计不超过 64 KiB。名称忽略 ASCII 大小写唯一；所有值非空且不含控制字符。认证、固定协议版本、Host、Content-Length、Content-Type、Connection、Transfer-Encoding、Upgrade、TE、Trailer、Cookie、压缩、Expect、转发与代理路由头由传输层保留；无认证模式也不能覆盖这些头。当前内置认证头仍由协议独立生成。

默认 Direct 显式忽略环境代理。显式路由为 HTTP、HTTPS、SOCKS5 或 SOCKS5h origin，拒绝 URL userinfo、query、fragment、非根 path 与零端口；通过一个 `Proxy::all` 覆盖所有 HTTP(S) 请求，不设置直连排除。HTTP(S) 代理认证为 Basic；SOCKS5 为 username/password，最多各 255 UTF-8 字节；SOCKS5 使用本地 DNS，SOCKS5h 使用代理 DNS。用户名中的冒号拒绝，两个字段都不接受控制字符。代理故障不降级，重定向与重试关闭，HTTPS 始终使用验证证书的 TLS。

解析发生在用户明确点击发现、测试或预览时；发送消费已预览的快照，不重新读取环境变量。API 密钥、每个请求头值、代理用户名、密码以及 Basic 编码加入脱敏集合。模型 ID 或分页游标含秘密时拒绝目录；错误响应体丢弃。Anthropic 每页复用同一选项、总 deadline 与累计响应字节预算。上下文的已知秘密集合最多 4096 项、每项 1 MiB、合计 8 MiB；超界拒绝，不漏掉多配置秘密继续发送。

## 元数据与凭据

`AiSecretRef::Ephemeral { id }` 仅保存不含值的身份；重启后没有值就拒绝请求。Environment 仅保存显式变量名，在请求动作读取；代理环境变量的值采用严格 `{"username":"…","password":"…"}` 对象。SecretStore 仅保存 UUID，必须由用户显式解锁。

原 API key v1/v2 payload 与 kind code 3 保持兼容。新增 `AiRequestSecret` kind code 4，payload 绑定版本、reference UUID、profile UUID、精确 endpoint、protocol、API 后端与用途。请求头用途额外绑定规范化名称；代理用途额外绑定精确代理 URL。角色、引用或目的地不符不能解锁。保存创建新密文 UUID，不覆盖旧项；取消后已经写入的孤立密文仍由既有维护流程清理。

进程凭据集合保留独立 API key map，再增加按 profile 和用途绑定的 request slots。换 endpoint/protocol/backend 清理旧目的地秘密；请求头改名/换来源清理该槽，代理换路由/来源清理代理槽。修改已解锁值解除旧 store 关联；元数据只保留新的 Ephemeral 引用。字段同步先清理内存签名，再处理排队 UI 清空事件，防止旧值复活。

## UI 与生命周期

配置表单提供请求头行增删、临时/环境/凭据库来源、加密保存/解锁/清除，以及直连/显式代理、地址、可选认证。输入值使用密码控件；凭据库主密码/确认/提交/取消只显示在对应请求头或代理行，与 API 认证模式无关，取消或完成恢复对应秘密字段焦点。审核显示头名称与来源、路由与是否认证，不显示值。中文与英文使用同一逻辑，颜色跟随主题，表单正文滚动、操作 footer 固定。

无效文本按 profile 保留，切换配置、语言、主题或旧保存回执均不能把最后有效元数据冒充当前草稿；应用和请求都检查草稿。任何头、路由、来源或秘密变化推进 revision，取消旧 operation/vault prompt。Vault 回传仍核对 prompt ID、取消标记、revision、完整 profile 和用途，迟到值 drop，不关联新草稿。完整凭据集合改变也撤销助手审核，包括非当前 profile 的秘密变化。

LocalAgent 对 API 请求头和代理仍拒绝，不继承环境代理或秘密；非默认 reasoning 仍明确拒绝，保存元数据不会被这一切片重写为“已支持”。

## 依据与验证边界

实现核对锁定 reqwest 0.13.5 的[官方 ClientBuilder 文档](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html)及[官方 Proxy 文档](https://docs.rs/reqwest/0.13.5/reqwest/struct.Proxy.html)，只开启现有 `0.13` 的 socks feature，保留精确 toolchain 1.98.1 与锁定 patch。

隔离回环与 GPUI 测试是传输/受控界面证据。HTTPS 代理回环先证明 TLS 握手和 TLS 失败关闭，不伪称已有真实代理证书验收。未读取用户凭据、启动已安装应用或请求云模型；macOS、Windows、Linux 原生窗口和真实供应商兼容性由整合后验收记录另行说明。详见[产品指南](../product/AI_REQUEST_OPTIONS.md)与[测试记录](../testing/records/2026-10-05-ai-request-options.md)。


## 独立复核 P1 与 B 修复候选

原 A 工程门禁通过不能关闭秘密安全边界。独立反例使用非活动配置的已知代理用户名/密码，发现其裸 Basic 编码仍进入另一个配置的已审核正文；Chat Completions、Responses、Messages、Codex Local、Claude Local 五条 prepare 路径均出现，未发送 HTTP 或启动 CLI/job。A 候选不整合，源码与全部证明保持原字节。

B 在独立 worktree 从相同基线应用 A 补丁，仅修复该 P1。进程的每个代理凭据槽还拥有 zeroizing 的裸 Basic 与 `Basic ` 前缀形式，随原槽替换/移除而释放，不写入元数据或 vault payload，不添加 Debug/Serialize。全配置秘密集合包括两种派生形式；API/Local preparation 使用同一集合，接受回复先通过 revision gate 再对该集合脱敏。当前 RequestOptions 也纳入前缀形式。新反例源码与断言原样重放；其它配置清除和迟到回复边界另补回归。

修复候选 B 的原反例与补充回归共 3 项通过，全工程门禁 44 个 Rust harness / 1098 passed / 11 ignored、Python 6、格式、strict Clippy、依赖策略及独立小栈控制器均通过，受检查工程输入哈希未变化。macOS app 与 MCP companion 的隔离构建通过，只读确认两者为 arm64 Mach-O，并记录部署版本和哈希；未启动程序。B 冻结交付后仍须 fresh 非作者复核、主树整合门禁和实际原生窗口验收。原 A 的 1095 项通过及其它原生范围不能迁移为 B 验收。详见[修复记录](../testing/records/2026-10-05-ai-request-options-basic-fix.md)。

## B 独立复核 P1 与 C 限定修复

B 的 Ask 派生秘密修复通过了新的独立重放，但另一配置的 Models GET 仍只使用当前配置秘密集合。独立 5801 字节原反例观察到已知非活动代理裸 Basic 被接纳为可选模型，随后通过生产 InputState/sync/Test 路径发送：真实 GET 1 / POST 1，负面断言在自有服务器 joined 后失败。B 不整合。Anthropic last_id 的相同缺口在该复核中仅为源码推断，C 使用新的有界 HTTP 计数夹具证明修复后的首 GET / 无第二 GET，不冒充旧复核已经测量分页缺陷。

C 从相同基线应用完整冻结 B 补丁，仅补这条边界。RequestOptions 增加不可序列化、不可在 Debug 中显示的 zeroizing 全已知秘密快照；这些值只参与拒绝/脱敏，不变成认证或请求头。快照按值去重、稳定排序并纳入精确 equality，绑定发现、Test 与 Ask；最多 4096 输入值、每值 1 MiB、总计 8 MiB，超限失败关闭。当前显式认证、头和代理按原审核语义发送；非当前配置的已知值默认不发送。

发现的每页 model ID 和 last_id 在解析后立即检查，所有实际请求的 endpoint、生成分页 URL 与 JSON 正文在 send 前再次检查。URL 百分号编码不能隐藏已知值；取消、单个总期限和累计响应预算保持。Test 的请求 model/endpoint、返回的 model 标签及 Ask 回复使用同一快照；ContextDraft 合并显式与选项秘密后去重，原五条 prepare、reply 与迟到回执边界保持。设置中新知道的秘密还会从已有目录移除其字面 ID，并撤销在途操作，过时 callback 仍按 revision 丢弃。

C 不添加功能、不升级依赖，不读取未显式请求的其它环境变量。原 5801 字节反例与 2998 字节五路径反例均原样保留。作者完整门禁通过 45 个 Rust harness / 1106 passed / 11 ignored（含 8 doc）、Python 6、fmt、strict Clippy、x.y 和普通/2 MiB 控制器；工程输入不变。MAC15 app 与 MCP 构建及 arm64 Mach-O / minos / SHA 只读检查通过，未启动程序。验证与未验收边界见 [C 修复记录](../testing/records/2026-10-05-ai-request-options-catalog-fix.md)。

## C 独立复核 P1 与 D 草稿秘密修复

C 的新独立复核确认重复名的非活动请求头仍在 masked 控件保留原值，但用途槽重建移除了该值。另一配置因此可以接纳它为模型并在 Test 正文投递。C 原失败源码、断言、日志与收据保留，D 通过不能追溯关闭 C；原受系统中断的补充审查探针不用于 D 验证。

D 为每个 profile 增加单独的 zeroizing 草稿秘密集合，由当前有效编辑器签名登记。它无条件纳入仍保留的请求头值、代理原始字段与有界 Basic 两种派生形式；metadata、名称、用途或引用校验失败不会把已知值遗忘。普通同步和 vault 回填都更新这一集合。它只参与全已知秘密拒绝/脱敏及 equality，不参与 `request()` 的用途、reference、endpoint、protocol、proxy URL 投递校验。

清空、替换、删除、改变用途或目的地释放对应集合；pending UI 清空仍以已经清过的内存签名为准，排队事件不能恢复原秘密。取消请求、保存回执、切换 profile/语言/主题仍保留当前草稿。Assistant 的完整 credentials equality 和 revision gate 同时覆盖草稿变更，撤销旧审核、取消在途任务并拒绝迟到回复。Settings 的原 revision/cancel/目录移除路径也消费这一集合。

`all_secrets()` 合并有效槽与草稿后按内容去重，因此应用边界按唯一秘密快照计量。`RequestOptions` 4096 项、单项 1 MiB、总计 8 MiB 的失败关闭检查保持；不会截断或抹掉超限草稿。D 增加实际 GPUI/InputState、一次性自有回环及资源边界回归，作者工程结果与后续 fresh 审查另见 [D 记录](../testing/records/2026-10-05-ai-request-options-draft-fix.md)。

## D 独立复核 P1 与 E 请求头名称修复

D 的新非作者复核确认已知保留值可以成为另一配置的合法自定义请求头名称。审核摘要隐藏了该值，但实际 HTTP 字段名仍携带它；三种协议的 Discover/Test/Ask 九个场景收到真实请求。D 不整合，原 541 proof/542 archive members 保持 FAIL。

E 在 RequestOptions 共用名称检查中把名称当作请求 metadata，按 ASCII 忽略大小写检查完整已知集合和本次才提供的 API key。名称最多 128 字节，检查不生成大秘密的小写副本，也不解码 HTTP 不解释的任意混淆形式。ContextDraft 在形成可批准正文前检查，异步共同 request 与阻塞 AiClient.send 在构造实际发送前再次检查；返回 CredentialInContext。构造选项与绑定全秘密集合的先后顺序不改变最终审核/发送的检查。合法显式头值、代理认证及 API 认证仍按原用途校验与交付。

原 3378 字节 GPUI 反例不改断言纳入正式回归。原 6958 字节 HTTP 诊断刻意要求 listener 先 accept 并收到正常方法，正确拒绝因此不能让它 exit0；E 保留原字节及原失败，单独记录一次修复后的预期拒绝出口，不伪称该诊断通过。新增正式九场景回归检查有类型拒绝、0 HTTP、自有任务 joined 与 listener 释放，并覆盖晚传 key、阻塞客户端、大小写及合法交付。详见 [E 记录](../testing/records/2026-10-05-ai-request-options-metadata-fix.md)。

同一已知秘密也不能通过配置 Apply 持久化。Apply 在事件发出前检查全 catalog，Workspace 的具体 AI 写入入口在后台 StateStore 调用前再次检查事件的不可变 catalog/credentials 快照，直接或迟到事件不能绕过。两处只检查可编辑/provider提供的文本：名称、model、endpoint、proxy URL、自定义头名、环境引用、Local executable 与 reasoning 文本；头名使用同一 ASCII guard，URL与model沿用字面/百分号编码规则。固定协议标签、生成UUID和数字不扫描，也不读取新的环境变量。4096/1MiB/8MiB快照预算保持失败关闭。拒绝消息不回显秘密；当前 masked 草稿及 pending-save 新输入仍由原 revision gate 保持。
