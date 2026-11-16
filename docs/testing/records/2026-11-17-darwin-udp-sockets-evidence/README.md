# Darwin UDP 与缓存端口分页原件

最终 r3：729 工程输入，aggregate
`e94405c2858d9796d9256dfd44eb277914c18fedd14e258e8025c747528e9a0e`。
基线为 `e1f7359a975726dc6cf110fb12edcbb8a582cd95` 加本批工作差分。
该差分现已作为 `16703293012d798f885e1590f02c348c47e81a10` 普通推送；
199成员与729工程输入的精确Git绑定通过，不重放测试。

`originals.tar.gz` 的199成员与 `manifest.json` 保存：

- 自有 PID 的真实 lsof 选择实验、官方手册来源摘要与全部阶段冻结。
- 实际 Cargo 路径、三份编译库存、专项初败／修订、完整 gate 初败／修订。
- 最终普通 Rust3325/0/58 ignored、app1375/0/2、session lib354/0/4、
  14rustdoc、Python293/2 skipped及独立helper30；准确app1377／session358库存与执行相等。
- 真实Darwin OpenSSH15/0/0、临时服务／密钥的清理原件。
- Hooke静态批准及Nash独立prebuilt69/0/0的原始报告、命令和校验器诊断。
- 依赖官方请求失败／超时，正常release构建、提交前包结构与完整文件摘要，
  以及两份专用测试副本的回收。

正常包和全部二进制不进入archive；提交前manifest的commit为null，来源明确为
r3工作差分。正常release app SHA为
`c18d78d41c229205ccbcae4bbd0ca27620158fbd8559ac7d6006ecd09c30826d`，
MCP为`bb415f3ae3ae7ca62bdb78abe752b50f5a3a796dc65ec99490f54ee28cb3f693`。
未启动／安装／签名／公证，原生桌面和发行另验。

只读重算，不重放测试或读取已清理的活动路径：

```sh
python3 docs/testing/records/2026-11-17-darwin-udp-sockets-evidence/verify.py
python3 docs/testing/records/2026-11-17-darwin-udp-sockets-evidence/verify.py --source COMMIT_SHA
```

初次193成员封存因过滤器把 `package-` 前缀回执也当作目录排除，验证器
精确报告缺少 `package-precommit-r1.json`。原archive、manifest、校验器、封存器
及失败回执保留在 `verification-diagnostics/initial-seal-r1/`。
只修正目录过滤条件，199成员完整重封存后验证通过；没有重跑功能测试。
最新在线依赖仍UNKNOWN，三平台桌面及发行仍开放。
提交后正常包绑定该工程，五个应用文件与提交前逐字节相等；旧活动副本在
逐项核对Git原件后回收，当前正常包和共享target保持。
交付及新CI状态见[本批记录](../2026-11-17-darwin-udp-sockets.md)。
