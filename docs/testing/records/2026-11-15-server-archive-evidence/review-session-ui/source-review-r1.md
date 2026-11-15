# Session / app 非作者源码复核进度

记录日期：2026-11-15，Asia/Shanghai。基线
`9112e232b6f0e022d299cba9e8498088c77b3d3f` 加未提交冻结输入。

源码复核已完成，目前没有已确认的 P0、P1、P2、P3 发现，不需要阻塞 notice。
最终 `report-r1.md` 将在后续预构建库存核对及必要执行后签署；本文件不是全仓门禁
或原生平台验收结论。

已读取 AGENTS、`/Users/hdh/.codex/RTK.md`、HANDOFF 顶部、
`docs/testing/STRATEGY.md`、ADR 0135 和共享 target 指南。
工程源码严格只读，未运行 Cargo、格式化、commit、push 或清理共享 target；
所有 reviewer 写入均在本目录。

- r2：553 工程输入、10 脚本逐项 SHA-256 一致；session binary 的 r1→r2
  仅有两个 app 测试文件变化，session 输入一致。
- r3：553/10 再核对零 mismatch。r2→r3 只有
  `crates/keelshell-app/src/files/server_archive_fixture.rs`；
  将三个 `.checked` 和导入机械反向还原后，SHA 精确等于 r2，
  证明差分确为三个 `expect` 和 `Checked` 导入，无生产输入变化。
- 父提供的 session/app binary SHA 及现场 `--list` 输出精确匹配原清单；
  session 40 项、app 全清单 1208 项，7 项新增具体名称均存在。
- 独立执行 session 集成 40/40、app 新专项 7/7，零失败、零忽略。
  两次库存命令不计测试，父执行结果不叠加计数。未独立重编译。
- 补跑共用 app 归档生命周期回归前，摘要门拒绝了已经被父门禁重建替换的
  app artifact；wrapper 退出 1，binary 未启动，执行测试数 0。
  记录在 `app-shared-archive-refused-r1.json`，等待新的摘要后执行。
- 已读取父 OpenSSH `result.json` / `tests.log`：15/15、临时服务与目录回收。
  这是父执行证据，不算 reviewer 执行。

已核对的关键路径：

| 范围 | 证据与判断 |
| --- | --- |
| 真实连接与 stale authority | session 计划保留实际 `SshSession`；执行使用 `same_connection`；app 请求、完成回包和第二次确认均重新检查当前 binding 与实际连接。相同 host/token 不恢复授权。 |
| EXEC / stdin / EOF / 预算 | specialized exec 在 OPEN 后重验，派发前置 pending，等待明确 Success 才送精确 stdin；并发输入和输出，显式 EOF；stdout/stderr 共用 cap，要求 EOF、唯一 exit status 和 CLOSE。 |
| 期限 | planning 和 execution 各有单一固定 deadline；execution 的 admission、exec、receipt 解析、SFTP 全量读回、成员验证及 staging 检查不更新 deadline。 |
| 资源和未知结果 | 源树是读 claim，final/stage 是两个写 claim；复核至读回期间持有 scope；已派发失败、取消、超时和 future drop 不清 pending，Drop 发布两个未知写隔离。 |
| 读回 | 确认私有普通文件、准确长度、前后 attrs、句柄 attrs、EOF、CLOSE、归档 SHA、完整成员匹配、stage 不存在及父路径观察一致后才成功；没有自动 retry/remove。 |
| 两次确认 | 第一次只读准备与第二次写入分别生成独立 pending review ID；正文展示完整命令和精确 UTF-8 stdin；旧确认 listener 不能确认新的 pending。 |
| 变化与迟到回包 | 名称/路径输入事件和选择变化撤销 browser review revision；迟到只读结果不能重建授权；已派发任务继续报告实际结果。 |
| 旧 owner | 共用 `archive_previous_isolation` 对 PackServerArchive 生效；worker 启动失败、成功及已知失败恢复旧 owner，未知结果保留关闭保护。 |
| UI / 主题 / 双语 | 新动作使用 registry/Palette/组件 token，无主题 ID 分支；复用完整滚动正文及固定确认/取消；状态和提示均有中英文。 |
| fixture / OpenSSH | session fixture 正常路径执行实际注册的 helper，故障路径有明确边界；app 实际 helper 输出镜像到受控 SFTP；新 OpenSSH 测试涵盖两 codec、Unicode/shell 字符文件名、预取消、完整内容和 staging 消失。 |

core helper/协议解析/codec 深入审查由另一个 reviewer 负责，本分工只检查调用合同。
headless GPUI、受控 SSH 及父 localhost OpenSSH 不构成三平台原生桌面验收。
父 full-gate-r1 的 Clippy 失败保留；r3 的 full-gate-r2 及正常 GUI/MCP 构建
由父负责，尚不在本进度文件中宣称通过。
