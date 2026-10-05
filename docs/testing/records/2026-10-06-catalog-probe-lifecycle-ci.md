# 模型目录夹具生命周期提交的源码 CI — 2026-10-06

精确提交`bd9f5efa69bba8cdd484807116da3bd9b9243a42`的[Quality37356943845](https://github.com/cyruss648/keelshell/actions/runs/37356943845)已completed/failure。三个job API和原日志实际checkout一致；本提交只含两份测试源码和四份文档。258项提交工程输入由非作者与根分别实际读取Git blob核对字节/摘要，等于最终已审门禁源码。

| 平台 | 整体job | 普通Rust通过/失败/ignored | doc通过 | 本地CLI控制器 | OpenSSH |
| --- | --- | --- | --- | --- | --- |
| macOS | failure | 部分80/0/2 | 未到达 | 默认启动后panic；没有成功marker；额外2MiB未到达 | 未执行，无产物 |
| Windows | success | 完整1139/0/11 | 8 | 默认和额外2MiB都实际成功 | 跳过 |
| Linux | success | 完整1158/0/12 | 8 | 默认和额外2MiB都实际成功 | 9项另外实际通过，不叠加普通计数 |

三平台格式、x.y与严格workspace/all-targets Clippy实际通过；打包macOS/Linux57通过，Windows53通过/4skip；脚本macOS/Linux6通过，Windows5通过/1skip。ignored供应商opt-in不计模型验收。控制器没有普通test-result摘要，不把其panic加成一条普通失败，也不因普通0failed声称macOS完整通过。

## macOS失败

默认自托管控制器在精确源码`local_agent_process.rs:476`的`configured budget request cleaned its contained descendant`断言失败，cargo101、quality step1。该断言在Ask预算结果后立即检查loopback端口连接应失败；原代码遍历timeout/cancel两条路径，但日志没有记录哪一条、端口、PID、内核birth或OS原因。因此只能确认该次断言未满足，不能据此认定具体后代存活、端口复用或某项生产缺陷。

此时目录夹具及后续harness尚未执行。Mac Preserve OpenSSH步骤虽success，原日志明确无文件可上传，最终artifact API没有macOS产物；不能把步骤状态当实际OpenSSH验证。

## 新目录回归与Windows对照

Windows/Linux原具名输出实际确认：共享总读期限、accept阶段unwind取消并join、已接受连接Drop关闭并join，以及目录隐私GET路径回归均ok。Windows另实际通过SystemRoot双路径对照，短回执明确清空路径provider-init-10106、显式路径完成真实HTTP、两个child均reap和双EOF。该实测只属于当前提交/runner，不追溯填补旧日志中未记录的原因。

Linux唯一OpenSSH产物的原清理回执列70项累计内核出生身份，observed/owned stopped及私有目录删除为true；审查者只读回runner证据，没有自己观察runner或重启服务。观察间完全脱离的后代继续在证明之外。

## 原证据与未关闭边界

完整run ZIP254510字节、34成员CRC及每个成员字节实际通过，三个单job日志与aggregate分别全文相等。macOS原日志58207字节/SHA-256 `bcac2c73997983ff44c10f7daa2c936daea180e6c0d673785fb4ec56e6fca694`；Linux239689字节/SHA-256 `d5613cfa44e6d6b173e9b04c0294bdd34367704b408fef62a738f827ac68aefc`；Windows206177字节/SHA-256 `58be3d14497b3330095cb440ed8900edfc292e80cb0b63baec74a439aedb5a2f`。

独立136payload/137归档文件经根全部逐bytes/SHA读回；manifest SHA-256 `7460f1840fe2c7bd87f8d021b2aa67daff0b9493f8b99ebbd24e893d99b0eed2`，tar927831字节/SHA-256 `4c1106fd2fbd8e493bfa0463ba9f2803d415e26c882452a35fb5f33385f65669`。根首次归档读回辅助脚本误假设目录前缀的失败保留，改按实际相对文件成员核验后通过，不修改原材料。完整公开API/原日志/源码/产物留在ignored证据目录。

本次没有重跑workflow、供应商CLI、模型、GUI、客户SSH或安装器。macOS清理失败由独立工作区诊断；当前Ask未提交工作副本的MCP启动超时是另一件事。Windows/Linux源码成功不关闭macOS、新Ask、跨平台原生、真实Codex完整MCP业务或Release/安装更新。
