# 固定 AI 认证头的大小写保存语义

本增量继承基线 `23310ee13286adb488a451277addf49b05cd476b` 的完整 API 请求选项 E 候选。E 的作者与新非作者完整门禁均通过1121普通/8doc，但新非作者真实GPUI回归确认限定P2：Core接受大小写等价的固定认证名，同一自有值下，小写 `x-api-key` 发出1条Apply并进入saving，混合大小写 `X-Api-Key` 发出0条且不saving。直接共享守卫分别返回Ok与CredentialInContext，0网络。原E未整合；原P2日志、原断言和原源保存，不以整仓通过关闭缺陷。

原最小GPUI反例2957字节，SHA256 `a4b385e142eb62b610fec06337db4cc4b3db2b0e99bbbc9858c2395b314735ae`，源自首次11915字节实际观测。原完整收据保留；私有探针首版测试上下文编译失败也保留，不计行为执行。此前D请求头名称泄露、C草稿秘密及其它历史失败仍保持，见[E记录](2026-10-05-ai-request-options-metadata-fix.md)。

F仅将固定公共协议认证名的例外改为与Core一致的ASCII大小写语义。不跳过任意自定义请求头；自定义名称、provider/user元数据、网络前及实际StateStore写入的全已知秘密守卫保持。两项新增正式回归覆盖小写/混合/大写固定名字合法保存，以及同一已知值出现在有效自定义名字中仍被拒绝。没有改变用途绑定、模型审核、产品限时或依赖版本。

根因作者agent恢复遇thread容量限制，接管其已停止的独立作者工作区；修改前495输入与冻结逐SHA核验相等。E67proof/68归档及全部47源码已在根保存核验，F使用新的独立证明目录，不覆盖E。主仓库仍未导入F。

根作者限定检查：16项metadata匹配测试通过，包含真实GPUI Apply/Workspace StateStore、固定名字正例和自定义名字拒绝；首次格式检查发现一处新测试布局差异，原失败收据保留，修正后格式/x.y/diff/严格workspace all-targets Clippy通过（27.147秒，输入前后相等）。最终格式源码的16项重验单独保存。以上为限定检查；E旧完整门禁不冒充F完整门禁。

新的非作者F差量复核、主树精确整合和完整门禁、最终MAC15构建及原生配置/长文件界面矩阵尚未完成。现有三平台MCP测试同步CI不验收F；没有新GUI、供应商、Windows/Linux桌面、正式Release或安装更新证据。使用方式见[请求选项](../../product/AI_REQUEST_OPTIONS.md)。

## 非作者复核与主树整合更新

新非作者 F 差量复核 `PASS_WITH_ROOT_FULL_GATE_REQUIRED`，未发现新的已证限定 P1/P2。原始 2957 字节 GPUI 反例保持字节/断言通过；四条正式 metadata 测试（含两条新增）、四条 GPUI 回归、实际 StateStore 25 拒绝与一安全保存、三种固定认证头大小写分别真实写入/精确回读通过。四项 HTTP 测试含108快照/迟到认证拒绝、9 blocking拒绝（均零请求）、9实际合法投递及分页边界；未运行旧 C 中断或供应商探针。

496 输入与候选一致，私有覆盖已撤回，完整 patch reverse check、格式/严格 workspace all-targets Clippy/x.y/6 Python/diff 通过。656 proof、657归档成员已根逐bytes/hash核验复制。Seal SHA256 `6e02ffe63e79283ae3c19fbb14c39834c5a82f70092d1ed66817ded51c26cc62`，manifest `d461a24b7bb28510d8d2ce2d410cc721bc520c1d31394c58eccf126a2fc3b175`，10,749,233字节 archive `48b15ae73544734bb53c9328b32df749e730ada0673996bea46a635d76552442`。所有已记录出生身份及观测子进程已消失，零超时/信号；不宣称未观测完全脱离后代的清理。

主树基线 df8 与作者233基线的48候选路径原字节相等，精确导入后48文件均等于 F 冻结。中英文 README 一起补齐使用说明。主树完整门禁和新MAC15/原生验证仍未完成，见[整合记录](2026-10-05-ai-request-options-main-integration.md)。历史 E 完整通过不能替代 F 整仓通过。

## 主树完整门禁与新构建

根精确整合后完整 `scripts/check.py` exit0、422.111秒，1146普通、8文档、6Python、42普通/4文档harness；11ignored不计执行。fmt、严格workspace/all-targets Clippy、x.y及默认/2MiB控制器通过，future4624字节。完整输入前后一致，原log118,596字节，SHA256 `9b2f1a14ff3a2bb6c61942644b6e659af8d07f7769a7c8dbab0b92fb09256714`。57项打包测试另通过，未重复普通Rust统计。

显式MACOSX_DEPLOYMENT_TARGET=15.0的新app/MCP及fixture构建46.226秒通过，输入前后一致；三个实际arm64二进制最低版本分别app15.0、MCP11.0、fixture15.0，均符合bundle的15.0门槛，不能称三个minos均15。258份Rust/Cargo输入在完整门禁、构建、检查及当前字节相等。没有复用 E 作者app；当前尚未启动新GUI，不把构建或打包单元测试算原生窗口通过。详见[整合记录](2026-10-05-ai-request-options-main-integration.md)。
