# 保存资料同步批准的实际后台阶段观察

精确诊断提交 `dd32392` 的 Quality 37506388205 已结束：Windows/macOS 成功，Linux 保存资料批准用例在原 18 秒期限失败，547 passed、1 failed、2 ignored。目录同步用例本次通过，旧 `old!` / `new!` 失败原因仍未知。本次批准已经准入，18.073 秒后仍 busy、磁盘与视图未更新、SSH 请求为零。现有日志不能区分后台排队、服务执行和前台投递；昂贵 debug KDF 是性能候选，尚未定位根因。

本增量只为 app 测试增加每次操作独立 Arc 和编号的固定大小、单调观察：submitted、实际 spawn_blocking closure 开始、服务调用返回、实际前台接收 completion。原 Pending recovery 回读位于 service_returned 之后，因此 returned 与 foreground 之间仍可能包含该回读。观察只记录时间、阶段与非敏感结果类别，旧请求的记录不归入新请求；不包含密码、密钥、路径、原文或客户身份，也不作为 busy、完成、授权或唤醒信号。

原 18 秒批准期限、45 秒绝对计划、捕获会话／路线、命令、字节与不持久化断言以及 KDF 成本保持。新增有界反例区分尚未开始、服务未返回、已返回未前台交付，并检查 detached／旧请求不污染新操作。

作者已实际通过 pinned rustfmt、依赖策略、差异检查及 locked app/all-targets 严格 Clippy（0／80.943 秒）、两个观察器反例（0／31.844 秒，harness 0.00 秒）、原真实保存资料批准专项（0／46.815 秒，harness 46.43 秒）。674 份工程输入、14,372,785 字节在窄检查前后完整 hash 相等；三个所属进程组消失，无 timeout／信号，私有临时目录已移除。随后只补充本记录，Rust 正文未变。

真实模态日志分别记录 inspect 操作 1 的服务返回／前台接收为提交后 1.172／1.173 秒，apply 使用独立操作 2，为 2.371／2.377 秒；批准后视图和磁盘更新、busy 清除、SSH 请求仍为零，原捕获会话与45秒计划后续断言通过。这是本机受控 GPUI／TCP SSH fixture 的单线程执行，不能解释原 Linux 并行失败或替代原生桌面验收。新的非作者已独立完成下述真实 panel 反例；新的提交级 CI 仍待完成。原失败和完整准备／终结收据保存在独立 ignored 范围，不覆盖。


非作者从精确 `dd32392` 独立 managed checkout 全文消费作者固定包后，导入窄候选并增加三个实际 panel 反例：真实 blocking 队列中的请求取消；服务返回但未泵前台时仍 busy；新准入操作不受保留旧 Arc 污染；另含已开始但服务未返回的加密 inspect，以及较新存储使实际 apply 合法 Stale 且磁盘字节保持。三个 panel 用例、两个观察器用例、原保存资料批准用例共六个不同专项实际通过，不能把 recorder 单元测试当作真实 panel 证据。

独立严格 app/all-targets Clippy 为 0／6.947 秒，三个 panel 用例为 0／68.368 秒（含编译，harness 2.38 秒），两个 recorder 为 0／0.423 秒，原批准为 0／46.263 秒。675 份工程输入、14,384,621 字节在最终编译／测试／格式／差异门禁前后完整 hash 与元数据相等，所属进程组均已消失、私有 TMP 空后移除。此前 reviewer 测试夹具因 `Entity.read` 的 App/TestAppContext 类型不匹配导致 E0308／Clippy 101；失败源码和完整日志保留，修复仅将该夹具读取改为 `read_with`，作者候选未改。根已分别全文消费作者 735 份和非作者 1,998 份固定 payload，非作者结论为限定 test-only 范围 NO_BLOCKER。

第三阶段后的未知边界还包括 Pending recovery 的 `store.load`、Tokio Join、bridge 通道投递、GPUI 轮询和前台 `update_in` 接收。`service_returned=true`／`foreground_completed=false` 不能直接解释为纯 bridge 或前台延迟。原 Quality 37506388205 的 Linux 根因、新的精确提交 CI 和三平台原生验收仍开放；本次专项与缓存复用不是 clean build 或产品修复证明。

诊断分支在原 dd 基线导入这四路径后重新执行限定门禁：格式 0／1.231 秒、x.y 策略 0／0.206 秒、locked app/all-targets 严格 Clippy 0／77.000 秒、三个实际 panel 用例 0／27.222 秒（含编译）、两个 recorder 0／0.408 秒、原批准 0／46.871 秒。675 份输入、14,387,211 字节在这些门禁的完整前后映射中相等；所属进程组全部消失，无超时或信号。复用此前作者的已退出诊断 target，不属于 clean build。原批准这次 inspect 服务返回／前台接收为 1.120698／1.120772 秒，apply 为 2.265535／2.266347 秒；实际批准后 busy 清除、磁盘和视图更新、即时 SSH 为零，原45秒计划所有后续断言通过。

本次导入只有 observer、真实 panel 反例、既有 panel 的 cfg(test) 点位／注册及本记录，共四路径。另一个只读审查者确认三个 Rust 正文与固定来源字节相同、去测试增量后的整个 panel 与 dd 相等，原 saved-profile 用例、core/vault/store/runtime、Cargo.lock 和 Rust 1.98.1 均未改。门禁后只补本记录，不改变编译源码；提交推送后应以新精确 head 的 Quality 结果继续定位 Linux，不能沿用旧提交三平台结论。
