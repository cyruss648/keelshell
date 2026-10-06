# 目录镜像独立反例与撤权修复 — 2026-10-06

状态：原作者候选被实际 P1 反例阻止；本树完成窄修及完整工程检查。
修复由发现问题的评审者实现，因此修复仍需要另一位非作者复审。
根组合检查、macOS 桌面以及 Windows/Linux 原生验收仍开放。

基线为精确 `7f50d60a59034dd145c40414c334e8bfc4e09f67`。原作者 97 份正文、
2,490,565 字节及 23 路径补丁已完整读回；原封存包保持不变。新工作树重放补丁后，
改变的每一份正文与作者源相等。根源码、Git 远端、既有应用和客户资源未改变。

## 确认的缺陷

`remove_local_reviewed` 在树重新验证后，等待最后一次远端来源 LSTAT。
原实现没有在该等待之后、设置 pending 和调用本地删除之前再次检查授权。
新实际 TCP/SSH/SFTP 用例用两个独占 gate 分别控制树验证与最终 LSTAT，在后者
已经进入后撤销授权。原实现返回 `Ok(())` 且删除已审核本地文件，用例实际退出
101，3.499798833 秒；原始日志 1,307 字节，SHA-256 为
`efc9d45cdf0d68d185a2e880baa17a861b8839ba7ea14d34ccdb5977092867c5`。
失败前已经关闭 SFTP/SSH、实际 join 监听任务，并确认原端口拒绝连接。
原源码和失败收据保留，未将失败改写为通过。

生产修复只在最终观察之后补 `self.authority()?`，并解释该等待的撤权边界。
它同时检查会话是否关闭与调用者授权，发生在 pending 和本地删除分派之前。
不改变目录范围、递归策略、期限、队列、人工审核或未知写隔离。

## 实际专项

- 14 项 TCP/SSH/SFTP：实际退出 0，3.631025209 秒，包含原 5 项及新 9 项。
  常规文件和空目录两项最终 LSTAT 撤权均返回 `Closed`、保留精确目标，且相同树可
  立即重新取得 owner，未制造未知 pending/隔离。远端复制/建目录在 canonical 等待
  期间撤权后不创建目标、不开始 atomic WRITE；远端删除在最终树验证期间撤权后，
  REMOVE gate 进入数为 0。本地复制共同 guard 在实际远端观察之后拒绝执行本地
  file/mkdir 闭包，闭包调用数均为 0；这不是完整桌面复制流程验收。
- 新反例同时验证实际非空目标快照整份计划拒绝、最后校验后目录被填充时收到已知
  RMDIR 拒绝且保留子项、本地目标替换为链接/非空目录时不跟随、不删除，以及
  同大小内容改变导致指纹改变、旧审核 token 被拒绝。未知 REMOVE 的既有隔离、
  正常双向删除、逐项读回及已知协议拒绝仍通过。
- 新 2 项 headless GPUI：实际退出 0，66.683611917 秒。实际批准第一项删除后，
  第二项 REMOVE 正在等待时取消，第一项 Completed、第二项 Unknown、后续项
  CancelledBeforeWrite 保持，后继修改仍被隔离。原认证面板 suspend 后清空审核及
  会话并更换 token，旧审核无法执行，目标保留。未打开原生窗口。

## 完整检查与证据边界

`python3 scripts/check.py` 实际退出 0，845.300271208 秒：1,506 普通测试、8 doc、
6 Python；格式、x.y 依赖策略与严格 workspace/all-target Clippy 通过。
16 项 ignored 未执行。默认与额外 2 MiB 控制器各有 626 条完整连续阶段，
这些阶段不是额外 Rust 测试数量；两控制器均正常完成。
原始完整日志 396,279 字节，SHA-256 为
`5fad5815919f7a0cb88ba4e416287fbbbe85eb8ddbc55d01ceb38a46c41262dc`。

662 份完整工程输入、14,318,718 字节在门禁前后相等，正文地图 SHA-256 为
`acbccfd343a6f1423083dac25391861afaca79828c47af8856cd76da48e0dc96`。
本记录在门禁后添加；不把新增记录计入当时 662 输入。新的格式/依赖/链接/补丁
读回另行保存。所有 owned 检查保存真实 wait、原进程组缺失和私有临时目录清理。

首轮新增 GPUI 测试因 TestAppContext API 用法错误退出 101，163.803536667 秒；
原测试正文与编译日志保留，改用 `read_with` 后通过。首轮只读摘要 helper 错把
阶段编号当作从 1 起，实际编号是 0–625；原 helper 与失败保留，新 helper 重新
读取原完整日志，未重跑或修改工程门禁结果。原源码快照是修复前归档，不声称
归档时 Cargo 子进程仍存活。

缓存仅在两个 Cargo 锁、打开描述符与消费者检查后只读 CoW 复制。原 676,770 份
缓存元数据前后相等；使用不同 inode 的独立缓存执行检查。完整原始元数据保留，
封存包采用可完整解压读回的 gzip 编码。缓存复制不等于干净构建。

原作者 macOS 包属于原未修复源码，不能作为本次修复的桌面证据。本次没有启动、
安装、签名、推送或发布应用。新非作者复核和后继根组合应使用新的源码/产物绑定。
参见[镜像设计](../../product/DIRECTORY_MIRROR.md)及
[原作者工程记录](2026-10-06-directory-mirror.md)。

The original author candidate is blocked by a demonstrated revocation defect.
The narrow repair and complete engineering gate pass here, but the repair author
is the discovering reviewer. A separate non-author review, new combined root
checks and current native binary bindings are required. Controlled TCP/SFTP and
headless GPUI results do not establish desktop or cross-platform native acceptance.
