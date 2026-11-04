# 2026-11-04 — 原生嵌套 Job 准备阶段诊断

## 原始事实

更新器测试修复提交 `9ab663ea1aaafdcff69a3203d38910bea681f84a`
的 Quality `41843572205`，Windows job `126621167159` 在 Python 阶段失败：
`test_actual_two_existing_nested_members_survive_barrier_and_late_createprocess_is_rejected`
的 `self.assertIsNotNone(bound)` 得到 `None`。该 job 没有进入 Cargo，
因此不能作为新更新器测试通过或失败的证据。其余 7 项进程准备控制成功。

原日志为 `work/theme-integration-20261104/ci/updater-cancel/ci-windows-first.log`。
之前 `c366871`、`9d4517e`、`7b43af2` 的 Windows 日志均实际记录过此嵌套 Job
控制成功；新失败仍保留。旧用例只在成功绑定后输出回执，
无法从新失败判断是 helper 提前结束、observer/API 失败还是准备期限耗尽。
底层原因保持 **UNKNOWN**，不归因为已确认的 runner 负载或版本回归。

## 本次诊断增量

只修改 `scripts/test_windows_native_crash.py`：

- 自有 helper 输出固定的准备阶段名称；Win32 调用失败只输出数值错误码。
  原嵌套 Job 创建/加入、子进程创建和成员核验条件保持，失败仍抛出断言。
- observer 记录读取身份、绑定原句柄、施加 barrier、前后观察与 ACK 阶段。
  捕获的错误只投影固定类别、数值 errno/winerror，随后原样重新抛出。
  不改变 `run_owned` 的退出码、失败或清理语义。
- `run_owned` 返回后，在原 `bound` 断言**之前**保存并打印有限准备回执；
  包含实际 controller receipt、文件是否已发布、是否绑定及固定阶段。
  不打印子进程原始 stdout/stderr、命令参数、环境变量或异常正文。
  可选文件记录失败只输出固定类别/数值错误码；输出管道关闭也不能跳过原断言。
- 原 8 秒总期限、身份绑定前后 LIVE、原生句柄、嵌套成员集合、拒绝迟到创建、
  无副作用 marker、父子 TERMINAL 及 cleanup 的断言均保留。

这不是已证明的根因修复。新增可观察性不能把失败重试成功自动解释为原缺陷关闭；
实际 Windows 是否通过和旧失败原因必须分别记录。

## 本机验证与输入复用

- 首批四项新增控制：固定阶段/失败 receipt 保留并排除私有流；只投影数值错误字段；
  未识别阶段和畸形/超范围数值拒绝；实际 helper 生成器的源码编译检查。
  编译检查在首个 `WindowsJob` 构造前中断，只绕过 unittest 主机 skip 来生成源码，
  不执行或模拟 Windows API，不算原生验收。
- 本文件专项 67 项，1 skipped，退出 0。
- 首次完整 Python suite 126 项，1 skipped，退出 0。
- 独立 reviewer Fermat 提出 P2：配置了不可写的可选证据目录时，
  `retain_control` 的 I/O 异常会先于原绑定断言抛出。独立普通文件目录反例实际复现
  `FileExistsError`、无准备输出、原绑定断言未执行；初审 HOLD 保留。
- 修复局部 reporter，预期 I/O 错误不能替换原断言；新增真实普通文件目录反例、
  关闭 stdout 的反例，分别核对准备输出或本地回执及原始绑定 AssertionError 保持。
  修后完整 Python suite 128 项，1 skipped，退出 0；x.y 策略、格式、
  workspace/all-targets 严格 Clippy 重新通过。
- 本次验证前后 544 个非 Markdown 输入相同；相对更新器完整门禁，
  唯一变化是上述 Python 测试文件，其余 543 个输入（包括全部 Rust/Cargo、
  生产控制器和 workflow）完全相同。没有重复执行未改动的 Rust workspace suite，
  其 2,011 passed/23 ignored、rustdoc 10 的来源仍是 `9ab663e` 的完整门禁。
- Fermat 修后非作者复核 P0/P1/P2 为 0，P2-1 关闭；13 项专项控制通过
  （6 项作者控制、7 项独立控制），覆盖普通文件目录、实际写入失败、关闭输出和
  两个记录通道同时失败，原绑定断言均保持。首次 HOLD 的材料逐字节保留，
  闭环与最终输入绑定独立记录。本次增量的真实 Windows 执行仍待完成。

证据目录 `work/theme-integration-20261104/ci/native-barrier/` 保留
原测试源码摘要、专项、分项检查日志、前后输入清单。
前序 Quality 的 macOS `126621167756`、Linux `126621167873` 最终均成功，
两者 OpenSSH 互操作步骤也成功；三份完整日志在同级 `updater-cancel/ci-*`。
Windows 原失败保持，不宣称三平台整体成功。
