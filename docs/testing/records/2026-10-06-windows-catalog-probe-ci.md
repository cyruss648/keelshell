# 主线模型目录测试夹具 CI 失败 — 2026-10-06

提交 `8d074b014a9ef863944eadca02cbf190125443c7`（parent e821）仅修改7份验证文档；258项Rust/Cargo/锁文件/工具链工程输入与e821逐字节相等。本次 [Quality37351046635](https://github.com/cyruss648/keelshell/actions/runs/37351046635) attempt1整体failure；最终run、三job API与实际checkout均为该精确SHA，不借用前一次成功结果。

| 平台 | 普通通过/失败/ignored | 文档通过 | 控制器 | 打包通过/跳过 | 脚本通过/跳过 | 系统OpenSSH |
| --- | --- | --- | --- | --- | --- | --- |
| macOS | 1155/0/11 | 8 | 默认和额外2MiB实际成功 | 57/0 | 6/0 | 单独9项通过 |
| Linux | 1155/0/12 | 8 | 默认和额外2MiB实际成功 | 57/0 | 6/0 | 单独9项通过 |
| Windows | 539/1/2（已完成部分） | 未到达 | 默认成功；额外2MiB未到达 | 53/4 | 5/1 | 跳过 |

三个平台格式、严格workspace/all-targets Clippy及x.y版本策略均实际通过。Windows应用harness为428通过/1失败；其余尚未执行的harness不算通过。macOS/Linux两控制器future为4624字节，Windows默认为4968字节。Unix单独OpenSSH步骤不加入普通计数，不表示原生桌面验收；原runner已观察出生身份清理回执分别为59/68，没有未证祖先项，不推及观察之外的后代。

Windows失败具名为 `ai_settings::tests::reviewer_b_inactive_basic_catalog_must_not_become_test_request_model`。worker在 `private_catalog_probe.rs:29` 读取HTTP请求头失败，原错误为10035（非阻塞操作无法立即完成）；parent在line72观测通道失败。日志没有失败请求头字节或到达时间，也没有测得秘密发送，不作产品泄露或具体调度原因判断。原源5801字节、SHA-256 `9b567a035dcf736c419bce8e66d42fe33bcc445199ab5039ba26e9357d312e9c` 保留。

同一Windows原日志line589仍实际通过先前的SystemRoot对照：清空时得到provider-init-10106，显式传递后完成三次真实HTTP请求，两个child回收并有stdout/stderr EOF。因此本次是不同失败，不能重新关闭当前主线CI。此前e821通过及c7/3f失败的原材料保持。

新的测试候选复用已有HTTP socket配置，显式清除接受连接继承的非阻塞标志；header和body共享3秒总期限，写入另有3秒总期限，各次部分读写只使用剩余时间。测试专用守卫在异常/提前退出时取消、shutdown已接受连接并join，不在unwind中二次panic。窗口/runtime初始化移到accept预算之前，5秒界面观察与原秘密拒绝断言保持，并要求恰好收到一次目录请求、没有后续POST。新增滴流、accept期间unwind与已接受连接取消的3项回归；首24项专项通过后完整门禁因16个测试unwrap lint失败，原失败保留，修正报告方式后重新运行严格门禁。最终作者完整门禁1158普通+8doc+6脚本及独立24项/私有清理反例/严格检查已通过，28/29作者与67/68独立材料根全量读回；精确两份测试已导入主线258工程输入并与门禁相等，见[生命周期记录](2026-10-06-catalog-probe-owned-lifecycle.md)。新Windows CI仍待验证，不称已经解决本次Windows运行。Microsoft [accept文档](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept)说明新socket继承监听socket属性；这为候选提供源码解释，没有补采原运行的时序。

完整原日志ZIP为286895字节，SHA-256 `8c916988cafe067a491e4ef2fdc43eddaf667fbc5460d97257520c12a6d553fb`，35成员全部CRC通过；三个单job日志与ZIP聚合逐字节相等。Windows原日志121231字节，SHA-256 `2ef3aaad4cf3d061a133bb2073db5ba1f3d45d36bca1afe227234a8f5b7bdd6a`。独立材料为149 payload/150归档成员，manifest SHA-256 `f0f1cb1247f170564a2a1937534fe7eea7ec33d7fac0c5fd107a68b558bc3b38`，tar1077498字节/SHA-256 `b27fd011f002854d2c2d89bcf252305f7718d4745f6fabfdf6ef0ad3dec01f71`。失败材料保持冻结，不重跑或覆盖原workflow。

根已实际全量核对149个payload与150个归档成员的bytes/SHA；本次独立辅助核验一次exit0，无辅助失败。MCP只向外部智能体提供KeelShell能力；该测试夹具增量不改变MCP或内置Ask功能。
