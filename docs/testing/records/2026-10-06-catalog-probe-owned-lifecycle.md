# 模型目录夹具的有界 I/O 与回收 — 2026-10-06

本增量仅修改两份应用测试源码：`ai_settings/tests.rs` 中说明，以及独立隐私反例 `private_catalog_probe.rs` 的夹具。生产、依赖、锁文件、工具链均不变；258工程输入中256项与主线8d逐字节相等。原Windows主线CI仍failure，见[原始记录](2026-10-06-windows-catalog-probe-ci.md)，本机门禁不关闭其结果。

接受连接后复用已有公共测试配置，显式恢复阻塞模式。header和body共同使用3秒绝对期限，每次部分读取/Interrupted仅使用剩余时间；响应写入另有3秒绝对期限。保留3秒accept、5秒界面等待与5秒观测，以及原16KiB头/正文上限。窗口和runtime初始化先于启动accept预算。目录隐私断言保持，并加强为真实GET恰好一次、没有后续POST，不能以没有网络请求作为隐私通过。

测试专用守卫用取消标记与同一短锁登记当前已接受socket；取消先置标记，再shutdown该连接、take并join worker。登记时同锁复查取消，避免接受与登记之间遗漏清理。正常观测错误、worker错误、界面断言unwind和future释放都经过Drop；Drop不二次panic。该守卫不进入产品运行路径。正式新增三条回归覆盖部分滴流不会延长总期限、accept时unwind回收、已接受空闲连接关闭与worker引用释放；已有跨平台强制非阻塞socket的公共配置回归保留。

作者专项初始21项通过108.962秒；补齐两个源码清理缺口后的24项通过15.270秒，二者均为各自版本，不能替代最终源码证明。首完整门禁32.641秒因16个测试unwrap lint失败，完整原日志与源快照保留；修正为项目已有错误报告方式，没有lint allow或降低断言。

最终冻结源码完整门禁431.334412秒actualexit0：1158普通Rust、8文档、6脚本，格式、严格workspace/all-targets/locked Clippy、x.y版本策略、默认和额外2MiB CLI控制器全部通过（future4624字节）。11项ignored保持，系统OpenSSH和供应商opt-in不伪称本次执行。258工程输入前后逐bytes/SHA相等；包含最终24项AI设置测试。四个作者wrapper leader实际回收、原进程组未观察到残留，四个private TMP实际为空后删除；不声明观察之外的出生身份或后代。

新的非作者在独立工作区与独立cache上执行最终24项，实际全部通过；目录反例记录GET1、admittedfalse、postedsecretfalse、workerjoinedtrue。追加私有全serve反例验证头等待约1400ms及正文两块仍在约2995ms共享期限失败；接收断开、真实workerpanic及已接受连接unwind均关闭peer、释放owner并join。恢复两源及258工程输入后格式、diff、x.y及严格整工作区all-targets Clippy实际通过。独立封包67payload/68归档文件已根全量逐bytes/SHA读回，manifest SHA-256 `24e1c32b9e44484720eb401446b14ba574db2830f9baf5bf85d889e83710715a`，tar400180字节/SHA-256 `e9020a681ac3293e1859de7fd980c6bb4e67b2bba811366747fefa377dfcf931`。首次只读策略文件搜索rg退出2已保留，6项正式命令均exit0；不作为Windows或原生桌面验收。

作者封包28payload/29归档文件，manifest SHA-256 `fd460e3b98d714c0795dbed19f28d91bc87d5a407f40d3cd7ac2647625787ada`；tar141758字节、SHA-256 `5049cee4a37a97cdd4514643090c4bdf070aae1bcea9d61322fe82b20284f269`，根已全量核验。原始失败、首次lint失败、初步源码审查、候选快照及限定未验证范围都保持。打包与生产字节不变，本次未重复包装/GUI/供应商/云模型/客户SSH；新Windows CI继续单独核验。

后续已精确提交并推送bd9f5e；[新源码CI](2026-10-06-catalog-probe-lifecycle-ci.md)的Windows/Linux完整通过，实际运行目录三新增和隐私回归。macOS在更早的本地CLI预算控制器失败，目录harness未到达，所以整体CI仍failure。新CI原136/137封包及258提交blob经根核验；这不关闭macOS失败、Ask整合或原生验收。
