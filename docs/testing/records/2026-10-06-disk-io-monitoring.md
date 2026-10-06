# 逐设备磁盘 I/O 作者验证 — 2026-10-06

作者冻结时的状态：独立候选已实现，完整工程门禁实际通过1252普通/8doc/6Python和两实际CLI控制器；定向 35 项监控检查、2 项 owned TCP/SSH 检查和最终 1 项真实 Linux 样本解析通过。非作者评审、主线整合与原生桌面尚未完成。本记录由作者编写，不是自评独立 PASS。

基线为公开 `24a19b954b2634c3f54d9554e5ce9b04a0062e9b` 的隔离 feature 分支。未改之前冻结的参数候选，未提交推送/标签/安装。依赖、Cargo.lock、工具链及生产 batch/workflow/files/SFTP/MCP 权限不变。新资源读取仍由已认证 SSH 的固定脚本执行，blocking I/O 在原后台 worker 内。

## 源码与运行

原基线 566 个 tracked/nonignored 文件有 bytes/SHA 输入映射。target 在根确认无 Cargo lock holder 后 CoW 复制，复制前后无 holder；之后独立目录可写，不共享可写缓存。每次 author runner 保存所有输入正文、before/after bytes/SHA、stdout/stderr 和实际 exit；新 session/process group 有 1800 秒总预算，超时才 SIGTERM5秒/SIGKILL5秒。这里的 wait 和组不存在观察不证明未观察的逃逸后代。

下列运行各有 569 文件输入前后相等，空 private TMP 已移除，直接 leader 实际回收且原 numeric process group 后读不存在：

| 运行 | 命令/范围 | 实际结果 |
| --- | --- | --- |
| compile-v1 | app+session locked no-run | 101；GPUI Observed 元素返回 Div 的三处类型错误，未运行测试 |
| focused-v2 | 两包 monitor filter | 101；测试中引用错误不能装箱离开作用域，glob test 宏递归；未运行测试 |
| focused-v3 | 同 filter | 101；5 通过/2 失败；新设备末行及原端口按钮不在可见范围 |
| focused-v4 | 同 filter，实际滚动 | 101；6 通过/1 失败；原 footer 未登记观察标记 |
| focused-v5 | 同 filter，登记 footer | 101；6 通过/1 失败；大幅内层 wheel 传播到外层，使下一个主题设备区移出可见范围 |
| focused-v6 | 同 filter，实际 padding wheel/小步内层滚动 | 101；缺少 InputEvent trait，未运行测试 |
| focused-v7 | 最终 filter | 0 / 38.184 秒；app7+session28，共35通过 |
| ssh-disk-v1 | session disk_monitor integration | 0 / 41.129 秒；2通过/1 opt-in ignored |
| genuine-linux-parser-v1 | 明确提供两份真实 Linux 样本的 ignored 单项 | 0 / 0.231 秒；1通过 |

所有失败和当时源码正文仍在 ignored 本轮证据中，没有删除原断言、延长期限、跳过可见性检查或把失败替换成后来通过。

首次完整 `scripts/check.py` 实际1 / 45.698秒，572输入前后相等。x.y、fmt及6项Python通过，strict Clippy拒绝一处 filter_map(bool.then)；未进入workspace/doc/controller。改为等价filter/map，原失败收据与输入正文保持，第二完整门禁实际1 / 31.426秒，572输入相等，另外拒绝新文件测试模块后的生产impl（items_after_test_module），仍未进入整仓测试；只把测试模块移至文件末尾，原失败保持，第三完整门禁已实际通过，详见下文。

## 已证明的定向行为

领域单元覆盖完整 11/15/17 布局、512 字节单位、完成次数/时间、可下降 gauge、所有累计字段回退、boot/btime/uptime、新设备和全部身份分量变化、消失、父盘与分区分开、不支持布局、数值/名字/重复行/启动标识错误以及 256/257 行、128/129 名字字节和 256 KiB 预算边界。合法 idle 保持零但无请求均值，不把缺失或错误转为零。

自有 loopback TCP/SSH 服务使用固定测试 host key 和测试口令，只返回受控 proc 字节；真实加密协议执行生产 LinuxMonitor。两项 integration 验证精确相同固定命令、5 次采样中的差值/消失/boot变化/缺少boot仍保留内存，以及非法 UTF-8 和 2 MiB+1 的精确 typed OutputLimit。服务不执行脚本，不能称作真正内核采样。

GPUI 使用生产 MonitorPanel 和真实 SSH fixture；已有暂停/进程身份/SIGTERM/端口/MCP缓存断言保持。选择设备不采样不探测，显式下一次刷新才新增 snapshot+ps；消失保留选择但不显示旧数值。900×580、连接管理模态关闭/AI关闭的生产 Workspace 的中文/英文×System/Light/Dark 六组合实际发送外层 padding wheel、内层逐步 wheel 和最后一行 click，终端及刷新/暂停/工具栏可见、指标在列宽内、状态底部不越窗口。Workspace 中终端传输为受控 channel，不是原生桌面或真实交互 shell；GPUI 观察不检验屏幕像素或完整辅助功能。

## 真实 Linux 采集，单独范围

只使用本机已存在的公开 rust 镜像与一只 UUID 命名自有容器，pull=never、network none、read-only/no writable tmpfs、cap-drop ALL、pids16/memory128m。只读取已有容器清单，没有在既有容器内执行或修改内容，没有停止它们，也没有安装 SSH 服务或程序。同一容器执行精确生产 collector 两次，中间等待一秒；命令实际 exit0。

原始样本 3843/3844 字节，各有5个设备、17字段布局，remote uptime 从400115.75到400116.76；样本解析单项实际通过并存在有效 per-device observations，不要求非零活动。采集脚本 SHA-256 `774f0b20cba36d559ae84a8ecdcba4080739776013ccea3136137321206ae492`；两份原始样本 SHA-256 为 `d285197ed8b83cde45cc30981b4f5a0ef3409dad6d178fe5fd77f71d4a540fd4` 和 `3a26586f5b18e4b0ba290688dbb08e766e4e5e61c6ff8991f3ce84b4e94baaf7`，内容只留 ignored 私有证据。

首次捕获外层 wrapper 实际125，因为 Podman client 在 private TMP 留下两层空 owned 目录；原命令0与外层125分别保留。后续独立检查 exact owned name 的 container exists 返回1，确认容器已移除；只对两层无文件、非 symlink、当前 owner 的空目录 rmdir，TMP 已移除，单独 cleanup receipt 保留。它不是抹掉原失败，也不是全系统后代普查。此证据只证明 Podman VM 内核可见的采集，不证明宿主 macOS 磁盘、物理硬件、客户 SSH 或 Linux 原生 GUI。

## 最终完整门禁与来源

第三次 `python3 scripts/check.py` 实际0 / 405.348288秒，572 tracked/nonignored输入前后逐bytes/SHA相等。x.y、fmt、workspace/all-targets/locked严格Clippy、45普通harness共1252通过/0失败/14ignored、4文档harness共8通过、6Python均通过。普通默认与额外显式2MiB controller实际各626阶段、38次原TCP connect与5176字节future；它们使用自有CLI fixture，不是供应商CLI或模型验收。

最终真实Linux私有样本解析再运行：genuine-linux-parser-v2实际0 / 11.362秒/1通过，572输入相等，使用与初次采集完全相同的两份原始字节和最终delta实现。固定collector字节不变；最终解析复用首次采集的原始字节。full-gate-v3 stdout124540字节/SHA-256 `ceb400025febae7c382ed273a17099134cd21d2b583bca2abf7e5ee6f1c83853`，stderr239512字节/SHA-256 `515e6ddc7a52587b3c260b92baf2efe7a8edc60eb30860df6fc22fd87a69a004`。leader实际wait、原numeric group后读不存在、空privateTMP已删除；原所有非零记录保持。

终态后仅ADR、产品指南、本记录和ROADMAP四份文档更新结果，568份其它输入（包括所有Rust/Cargo/工具链/脚本）与最终门禁逐字节相等；不是在运行期间改源码。冻结窄补丁11路径，原566基线中561非候选输入保持；完整source地图、候选正文、Git preimages、raw日志、各次运行正文及Linux样本/清理收据一起封存供新的非作者评审。本轮未改变打包代码，没有重复57打包回归、生成新的标准Mac包或启动GUI；源码测试与Linux采集分别说明其限度。

## 开放边界

Windows/Linux精确源码CI、三平台原生桌面、默认侧栏/AI同时打开的最小窗口、睡眠恢复、真实Linux远程SSH连续采样/网络中断、外部文件系统卡顿、物理热插拔与告警/历史趋势不由本轮证据证明。各proc文件非原子快照，未观察到的同身份设备重新初始化仍可能不可区分。不执行客户SSH、云模型、供应商CLI、安装、签名、公证、发布或自动更新。作者冻结时安排新非作者从精确基线复核；该复审已完成，结果见下节。作者通过不代替该复核或主线完整门禁。

## 新非作者复审与主线整合

新的非作者完整门禁实际0 /357.821秒，1257普通/8doc/6Python，14项原ignored；格式、x.y、严格workspace/all-targets/lockedClippy通过，575完整输入前后相等。两个CLI控制器均626有序阶段/38TCP/future5176。四项新的领域/TCP反例、六种语言主题的实际生产GPUI控件与输入保留、独占离线容器两份新真实内核样本及解析均独立执行，生产代码不变。测试装配、坐标与摘要解析原失败保持；连接管理模态关闭不应写作隐藏侧栏。

根逐字读回11,578份新证据、268,306,905字节，并窄合作者11路径及非作者5测试路径，共14个不同路径。12个非共享、非路线图正文等同封存来源；workspace/tests.rs三方合并保留根工作流参数回归模块，其余既有输入保持。主线此时615输入，路线图与结果文档另行更新。随后根完整组合门禁实际通过1394普通/8doc/6Python，615输入前后相等；另11项OpenSSH、57项打包回归、新标准Mac包结构检查通过。传输桌面部分证据没有运行真实Linux监控，不关闭磁盘原生验收；见[组合主线记录](2026-10-06-remote-workspace-main-integration.md)。这些主线结果均独立执行，没有继承隔离门禁。
