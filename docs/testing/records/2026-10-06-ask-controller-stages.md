# 本地 Ask 控制器分阶段观测 — 2026-10-06

状态：已通过新的非作者限定代码与行为复核，主树已整合；主树完整门禁通过，新Windows CI尚待。作者门禁与非作者运行分别记录。基线 ad0912c，仅改变自托管 `local_agent_process` 测试及本记录；生产、依赖、锁、工具链、脚本和产品功能不变。

原精确 cfac CI 的 Windows 默认控制器全部断言实际通过，future marker 到完成66.8944168秒；额外2MiB入口在45秒总期限返回Elapsed，日志区间45.3624502秒。六组预算已有预期返回、实际固定PID ACK/noACK和返回后单次连接拒绝；最后Claude/noACK returned标记之后仍有断言/scratch检查，没有后续阶段标记，具体pending与原因仍未知。本候选只提高观测能力，不宣称修复该失败。

新增记录仅包含固定stage/phase枚举、适配器、控制器mode、有限case索引、序号、单调耗时与原TCP结果/系统错误码。没有记录问题、环境、输入、答复或凭据。特别在每个适配器的configured_budget完成断言之后记录End，再记录progress Begin；进度请求/真实gate、原task等待、恶意帧、receiver关闭、原后代、abort、敏感上下文、准入及scratch检查均有有限定位点。Returned仅代表相应等待返回；ScratchCheck End只证明原文件检查通过，不宣称独立完整进程清理。

原同步TCP判断通过一个一次调用的wrapper记录前后，原断言值、调用顺序和已有abort观察循环保持，不新增连接/retry/sleep。45秒整体、2/30秒请求、5秒join、3秒gate/production cleanup及所有其它Duration表达式保持。原spawn/cancel/await/abort控制流不放宽；新增best-effort stderr写忽略I/O错误，不会引入owned Ask取消/await前的诊断panic。1024条正常记录之外最多一条limit标记；出现limit标记意味着观测不完整，不能宣称定位成功。

原总timeout的Drop/JoinHandle脱离与生产Drop backstop并不构成awaited cleanup收据，此增量不改变其机制，也不把取消请求/End文字当新的进程回收证明。测试仍只运行相同自有可执行夹具，不调用供应商CLI或模型，不验收原生GUI。

macOS 本机验证实际完成：

- 定向默认控制器退出0，命令耗时31.651778秒（含重编译），Controller End为10.814895秒；定向2MiB控制器退出0，命令10.964060秒，End为10.754356秒。
- 完整 `scripts/check.py` 退出0，547.077915秒，包含依赖x.y策略、6项Python、格式、严格整仓Clippy、42个普通harness的1171 passed / 11 ignored、4个doc harness的8 passed，以及默认和2MiB控制器各一次。自托管控制器不是普通harness计数。
- 完整门禁中的两个控制器均为626条观测、38次原有TCP调用、future 5080字节；Controller End分别为10.700848秒与11.769629秒。两个适配器的ConfiguredBudget End与Progress Begin相邻；Claude对应序号331与332。没有达到1024条记录上限。
- 静态差量核对确认50处Duration表达式的文本与顺序不变，11处原TCP调用位置对应11处wrapper调用，夹具协议实现尾段逐字节不变。静态字段白名单及固定canary检查通过，不把有限canary检查扩张为任意敏感信息扫描。
- 260个工程输入在最终定向2MiB与完整门禁前后相等；所有命令使用独立可写target和TMP。命令收据记录直接leader wait回收、原进程组无剩余、未超时；该观察不是kernel birth普查或所有后代清理证明。

最初默认定向命令启动后才保存工程输入快照，其文件明确标为during，不冒称pre-start binding；后续最终两项门禁有完整before/after绑定。私有日志检查器首轮误将合法静态枚举 `credential_reject` 当成敏感值，原检查器及失败说明已保留；修正检查器后读取同一原日志通过，没有重写产品日志或改变Rust候选。

新的非作者默认与2MiB控制器分别实际10.894秒和10.889秒exit0，各626条固定字段记录、38次原TCP和5080字节future；50处Duration、82个断言、11处TCP位置及14009字节夹具尾段保持。额外三个实际Rust探针验证容量上限、未初始化无记录、真实stderr写入错误后record仍返回；恢复260输入并通过格式、x.y、diff和严格整仓Clippy。根已逐bytes/hash读回356份原始复核材料及357个归档成员。

结果边界：本候选完成作者本机门禁与新的非作者限定复核，根主树门禁及新Windows CI另记。原Windows 45秒超时的具体pending和根因没有因此确诊，也没有原生GUI、真实供应商或模型验收。stderr观测为有限数量的同步best-effort写入，不提供每次写入的独立时间上限；写入失败或capacity标记会使观测不完整。未修改原TCP判断、绝对期限或失败后的原控制流。

## 主树整合验证

根在检查点e247f2f整合两份独立候选后，完整 `scripts/check.py` 实际384.014秒exit0：42普通harness共1179通过、11原ignored，4doc harness共8通过，6Python、格式、x.y依赖策略及严格workspace/all-targets Clippy通过。默认与2MiB控制器各626条连续记录、38次原TCP调用和5080字节future，End分别10.897503秒与10.691218秒，没有limit或总期限标记。261工程输入前后相等；原始354117字节日志SHA-256 `220aeb2e779f5e961d954ed2f58f2251615e2be27357b6202bf69ee184ec9ad1`完整保留。wrapper明确wait/reap，原numeric group不存在，专属空TMP已删除；不称完整逃逸后代普查。
