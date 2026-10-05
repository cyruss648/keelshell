# 文件场景传输就绪屏障 — 2026-10-05

状态：作者工程门禁、根整合工程门禁与新的非作者限定复审均通过，无剩余限定范围 P1/P2。新提交 CI 仍须单独确认，原 CI failure 保持。本次改动只进入测试夹具与 GPUI 测试，没有改生产实现、依赖、锁文件、工具链或产品期限。

## 原始失败与确定性基线

精确基线 `a19d0c17f2e0e298b97805991f01e2329d494d4f` 的 Quality run `37253864420` 在 Linux 应用测试得到 375 passed、1 failed。失败测试为 `files::transfer_tests::workspace_layout_tests::real_file_rows_and_controls_survive_transfer_comparison_editor_and_review_states`。900×580、AI 关闭的四个语言/主题 `running-transfer` 场景输出完成，随后未出现 `paused-transfer`；GPUI 条件等待在 12 秒期限失败。原始日志为 162759 字节，SHA-256 `8103c668f55b45b2584ba78cb09ee8d5262e98dce7cde979bf0784120bccea84`，保持原件并另存经核对副本。

旧夹具只对每个 WRITE 延迟 200ms。768 KiB 上传用 12 个 64 KiB WRITE，约 2.4 秒的服务端等待不能锁定四个主题/语言布局测量所需的生命周期；真实后台传输可以结束，前台异步观察器仍持有 Running 快照。

确定性基线探针保留旧测试的 5 秒 SSH/SFTP 配置、原 768 KiB 数据和完整四个运行布局测量，在测试线程而非 UI 回调中，以有界真实 SFTP 读回检查远端数据完整，并记录夹具目录句柄计数。探针观察到远端完整 786432 字节、夹具目录句柄计数为零，但前台仍 `busy=true / Running / transferred=65536`。实际点击 Pause 后为 Completed，随后原 Paused 条件等待在同一 GPUI 12 秒限制失败；测试部分耗时 10.30 秒，包含编译的命令总耗时 139.03 秒、退出 101。

该探针证明存在后台完成与前台快照积压的确定性竞态，与原 CI 最后场景吻合。原日志没有后台终态快照，故原 CI 的具体触发仍是候选归因；本记录不把另一个协议测试问题或偶尔通过归作它的根因。

## 修复契约

新屏障只用于全新审核上传：首个 WRITE 必须从 offset=0 开始并正常返回真实 ACK，精确目标 handle 的非零 offset WRITE 在修改远端文件、返回 ACK 前异步等待。它不把非零 offset 当作任意续传的“首 ACK”，不会延迟其他目标。审核草稿、原源路径与实际目标由测试继续核对。

测试同时等待真实 Running、首个已确认进度与第二 WRITE entered；四种语言/主题测量逐次要求真实 Running、Pause/Cancel 必需控件和原布局/首行边界。暂停点击后要求 Pausing，再释放待处理 WRITE。Pausing 只证明请求已发布，不能证明 worker 已接收或安全点已 ACK；最终必须收到真实 Paused，读回非空且未完成的部分内容，在四种暂停布局后再次读回且不增长，再 Continue、等待结束并逐字节核对完整 786432 字节。Completed 的标签也要求真实 Completed，而不是仅凭场景名称或文件大小。

屏障使用 watch 中持久的释放状态，避免 release-before-poll 丢失唤醒。每个持有者各自拥有 Arc 身份、entered/expired 状态，旧 handler 不会把迟到统计归给下一次持有者，也不能释放新持有者。RAII Drop 同步解除自己的屏障，不执行阻塞网络操作；handler 等待期间不持有文件系统 mutex。10 秒 fallback 返回明确失败，不自动放行写入并冒充成功。它是夹具有界失败清理，与原 5 秒传输预算分别存在；`expired=false` 不代表客户端未超时，真实 Paused 与完整终态检查才是证据。原 5 秒传输预算、8 秒进度等待、12 秒终态条件等待均未放宽。

三个低层回归覆盖首 WRITE/无关目标、重叠持有拒绝、release-before-poll、克隆 Drop、受控 unwind、重新持有及旧 handler 放行、缺少释放的有限失败。这些共享夹具测试在应用与 `ssh_loopback` 两个目标各注册一次，计数需按实际目标区分。另一个真实 GPUI/SSH/SFTP 回归覆盖提前释放后的完整上传与待处理 WRITE 的真实取消，检查部分内容和子系统与目录句柄计数归零。原 11 状态 × 2 尺寸 × 中英 × 明暗 × AI 开关的全部 176 布局场景保持，未跳过任何场景或原断言。

## 作者验证

| 门禁 | 真实结果 |
| --- | --- |
| `python3 scripts/check.py` | 退出 0；377.78 秒；依赖 x.y、6 脚本、fmt、全工作区/all-targets 严格 Clippy、1060 普通 Rust + 8 文档测试通过；11 项既有 opt-in/系统测试 ignored，未将其计为通过 |
| 应用子集 | 380 passed；包含全部文件场景与新增同步/清理回归，属于 1060 普通测试，不能重复相加 |
| 默认与显式小栈控制器 | 两个场景组均报告全部断言通过，future 为 4624 bytes；`RUST_MIN_STACK` 未设置，没有调用供应商或模型 |
| 文件详细专项 | 44 项，完整的 176 条 `FILES_LAYOUT_JSON` 与四条真实传输/清理诊断单独保留于 `final-files` 日志与收据 |
| 打包回归 | 57 项通过；这是回归测试，没有生成或运行新生产原生包 |

最终详细文件专项逐个核对场景组合和必需控件；四条传输诊断要求第二 WRITE entered、真实 Paused 部分内容、Continue 后完整 786432 字节及 夹具活动子系统与目录句柄计数为零。

日志字段 `active_handles` 取自夹具 `active_directory_handles()`；该计数只跟踪 `opendir` 目录句柄，文件 `open` 不递增它。目录计数为零和活动子系统为零只证明本场景的观测清理，不能据此宣称文件 CLOSE 回执、全部 OS 资源或未知进程已验证。基线探针的数据完整与旧前台快照也不单独证明后台任务在点击前已经结束；点击后观察到的实际终态是 Completed。


## 根整合与独立复审

根 `scripts/check.py` 退出 0，320.677 秒，1060 普通、8 文档、6 脚本、格式、严格 workspace/all-targets Clippy、x.y 和默认/显式 2 MiB 完整控制器通过，future 4624 字节。471 个根输入前后未变，TMP 为空；日志 SHA 为 `6627c8970e2efb8865635a8a7eee0287cf8f696c1d2da4f08a1e7eb1d7c021a1`，输入清单 SHA 为 `ef19cc5a9d7fcae2c31c971e16100fdf9c145c78caa59bf16d981b2ec2744b2c`。根的清单包含新 CI 状态记录，与作者 470 输入的原门禁分别绑定；本记录在根检查后加入，后续状态文档不是先前工程输入。

非作者复核实际通过 44 项文件测试及全部 176 个唯一场景、208 项 session 普通与 1 项文档、原 app/session all-targets 严格 Clippy、格式和 x.y。`ssh_loopback` 的 98 项是原 95 加新增 3；9 项 OpenSSH opt-in 仍 ignored，不算执行通过。四条真实传输的稳定 Paused 内容为 131072 字节，Continue 后完整 786432 字节；目录句柄与子系统的清理计数保持上方限定。

四项私有 Handler/gate 补证验证广播释放、旧 Arc/guard 不影响新持有者、精确目标/offset0 与实际写入/ACK 顺序、首次 poll 前释放、原 10 秒 fallback 返回 Failure 且不修改数据；私有严格 Clippy/格式也通过。首次筛选为零测试的原日志保留，明确未接受，重新构建后实际枚举四项并通过。私有补证是 Handler 层，不扩张为新 SSH/GUI 原生验收；原作者 fixture 前缀保持逐字节一致。

作者 79 份与非作者 160 份证明由根逐条 bytes/SHA 核验复制。独立 manifest SHA 为 `c80f1eb93538685c8c74d685536bcf1cd470d3ae61ace31b453b5e6ea2592166`，最终报告 SHA 为 `a82800827792a69091c62847d818593e15996ef0bafa5344aad08360a8cc4525`。复核确认作者最终 471 输入仅在原 470 门禁后新增本记录，也确认公共文档对目录句柄/后台终态的 MD-only 修正；原作者记录、patch 与证明不回写。九组自有检查进程已退出，TMP 为空。新提交 CI 必须另行验证，见[原整合 CI](2026-10-05-integration-ci.md)。

## 失败保留

诊断源码、原始失败 stdout/stderr、执行收据和前后输入 hash 均保留于 ignored `work/files-scene-readiness-20261005/`：

- 原 Linux CI 失败及精确 5 秒基线探针的原 Paused 超时。
- 新 cleanup 回归编写时 conditional move 的 E0382 编译失败；改为明确 Option 所有权，未放宽 lint。
- cleanup 回归的本地源 basename 与期待远端目标不匹配，真实读回 NoSuchFile；修正夹具目标并新增准确审核目标断言，未修改产品。
- 诊断输出添加时漏插两个变量的 E0425；保留输入补齐声明后重跑。
- 首次完整门禁的四处 `unwrap_used` 严格 Clippy 失败；改用 Result 与 `?`，保持 `-D warnings`。全局计数差值同时改为各持有者所属状态，最终输入另行冻结。

低层 unwind 回归会在 stderr 打印受控 panic，`catch_unwind` 收集后测试实际为 ok；它不是未报告的产品或测试失败。

## 冻结与边界

最终两个 Rust 输入的 SHA-256：

| 文件 | SHA-256 |
| --- | --- |
| `crates/keelshell-app/src/files/workspace_layout_tests.rs` | `4c905552c391bce52a3a09a4fc0a25f43754c11fd06c0abb3c4271bf65463ae2` |
| `crates/keelshell-session/tests/fixtures/sftp.rs` | `32d8f7fe0796d98e8189b2e733b9f71013ac33c350d0351adac9a48d81c55e19` |

完整门禁对原 470 个工程输入逐一冻结并核对前后 SHA，均不变；此测试记录在门禁成功后据实际输出生成，再与全部最终输入和包含新增文件的 patch 一并冻结。原始失败、每次失败 patch、stdout/stderr、命令/退出码/环境收据与前后清单保存在 ignored `work/files-scene-readiness-20261005/`；`manifest.json` 覆盖完整证据，`final.patch` 包括新增测试记录。证据文件保持本地私有，没有把执行环境绝对路径写入本记录。

作者使用工作树自有 `target`，以 APFS clone 复制只读缓存，目标目录不是 symlink，与源缓存 inode 不同；没有写入共享 target。TMP 独立限定在本工作树的 ignored 子目录，结束后清除所属临时内容并记录进程/临时目录检查。缓存和证据暂留给独立复核，后续统一清理由根任务完成。


本轮没有启动 GUI、供应商 CLI、模型或云服务，没有读取用户配置，没有安装应用或生成新的原生包。打包回归只是工程测试；既有生产程序包不会因测试夹具修复迁移成为本轮原生证据。没有新增 Linux/Windows 桌面原生接受度或新 Linux CI 通过的结论。本轮证明的是 fixture 同步与受控 GPUI/真实 loopback SSH/SFTP 行为；完整 Workspace、原生像素、键盘、IME、读屏及正式 Release 均按既有记录的范围另行验收。
