# 目录取消确认点与 EOF 辅助读回隔离

日期：2026-10-06。候选基于精确 `6b4e5fcc7a3b8576f54f91e820d80f637f5efb1c`，仅修改三个测试/夹具源码文件和本记录。生产传输、未知写隔离、每个 owner 的一秒空闲期限，以及之前目录同步独立 binary 的迁移保持原正文。此候选的本机工程结果不关闭后继 CI、非作者复审或其他平台原生验收。

## 原 CI 与实际反例

[Quality 37442335391](https://github.com/cyruss648/keelshell/actions/runs/37442335391) attempt 1 已 completed/failure，绑定上述基线：

| Runner / job | 原结果 | 保留的失败 |
| --- | --- | --- |
| macOS / 112198843126 | success | 全量原始日志和清理副本保留 |
| Ubuntu / 112198842825 | failure，相关 binary 136 通过 / 1 失败 | `directory_cancellation_preserves_partial_tree_and_next_fifo_job_runs` 返回 `transfer stopped without a terminal event` |
| Windows / 112198843189 | failure，相关 binary 7 通过 / 1 失败 | `other_download_owners_eof_cannot_renew_unknown_write_or_writable_close` 返回 `Timeout("SFTP read")`；只读 CLOSE 用例通过 |

三份完整原始日志已按上游收据实际读回，SHA-256 分别为 Linux `caef35c6dd0f3a04b73dfe8da70229f8aa244f482112e9f9f8f990a66051826d`、macOS `12e7c62be2a454a1d3e276c8dd5b9262a85046b31add405337285ef3bc44afd6`、Windows `fb78932e2dd4218866f38b8622a42cda93d257847767b3593dc9965b123292dd`。不追溯解释更早 c158 提交中的 Windows 裸 `Elapsed(())`。

第一个受控 TCP 反例在第二个非零偏移 WRITE 实际进入挂起后取消，原 `Cancelled` 预期不变。生产返回 `Uncertain { bytes: 65536 }`；原 helper 的新增诊断证明它消费并忽略此事件，随后返回缺终态错误。实际 exit 101、0 通过 / 1 失败、body 0.13 秒；完整日志 SHA-256 `fb6139af18fed2d324fba90bceb5f6e5960d506c9ef304168172632707f1a124`。这确定性证明 helper 遗漏及一种取消竞态机制。原 Linux CI 没有记录事件正文，其精确触发事件仍未证明；生产保守保留未知写与隔离是此受控状态的正确结果。

第二个反例保持被测 owner 一秒期限，实际在 1.044341416 秒得到 `Uncertain { bytes: 32768 }`，14 次独立下载 / 14 次有效 EOF、隔离 ID 在释放晚写后不变。只在测量结束后，将辅助最终 READ 延迟设为 750 ms，实际得到阶段标记明确的 `Timeout("SFTP read")`。实际 exit 101、0 通过 / 1 失败；完整日志 SHA-256 `acc5915e1b9c442dcb5684027a8ac177345b92bb01896723a34b2dd080ccbd01`。此反例只运行 WRITE 阶段，CLOSE 阶段因故意失败未执行。

生产 `SftpSession::read` 使用覆盖 OPEN、内容 READ、EOF READ 和 CLOSE 的固定总期限。原函数唯一直接 `sftp.read` 位于最终内容读回，源代码传播路径能缩小原 Windows 错误的位置；在其原 50 ms READ 配置下为什么超时尚未证明，不声称原 runner 有 750 ms 延迟。此反例证明辅助总期限读回与 owner 空闲期限是独立场景。

## 测试修复与不变断言

[目录测试](../../../crates/keelshell-session/tests/fixtures/directory_transfers.rs)的终态 helper 识别 `Completed`、`Cancelled`、`Failed` 和 `Uncertain`，准确报告未知写。已知取消仍严格要求 `Cancelled`：复用 exact-path 第二 WRITE hold，先确认真实进入且未到期，请求 pause 后才释放 WRITE，等待真正 `Paused`，最后 cancel。暂停确认字节必须严格大于零且小于总量，取消字节等于暂停字节，保留的部分内容精确一致，后续 FIFO 任务完整完成。这样不依赖 100 ms 延迟或事件消费者的调度速度，也不扩大原等待上界。

新增 2 MiB / current-thread 受控场景，在第二个 WRITE 未回复时取消，要求 `Uncertain` 字节等于已确认进度；进度同时与实际远端部分内容核对。排队的冲突任务失败，直接冲突写被 `MutationQuarantined` 拒绝，不冲突的安全目录任务完整完成并精确读回。释放挂起写之后，再要求隔离 ID 不变、直接冲突仍拒绝。

[协议夹具](../../../crates/keelshell-session/tests/fixtures/sftp.rs)为真实写入完成、Handler 准备成功 STATUS 增加独立原子计数器；共享克隆观察同一计数，既有元数据时间线正文保持原样。计数器仅证明 Handler 准备响应，不能证明 STATUS 已写入传输层或取消 worker 收到 ACK。当前晚写场景的结论是晚远端变更发生后隔离仍保留；没有用此计数器关闭客户端收到晚 ACK 的证明边界。测试最后仅对自有夹具作用域显式确认风险，实际回收拥有的监听任务并检查端口拒绝连接。

[EOF 独立测试](../../../crates/keelshell-session/tests/transfer_idle_eof_independent.rs)在 owner / side 测量、晚回复与隔离观察、时间线捕获完成后，先停止夹具 cadence 延迟并复位辅助 READ 延迟，再进行固定总期限内容读回；给 setup、enqueue、owner / side、隔离、读回和关闭传播错误增加具体阶段。原 `timeout=1 秒`、被测 READ 50 ms、side 20 ms、owner / side 外层 3 秒、side 至少 5、EOF 至少 side、700 至 1500 ms 区间、未知字节 32768 / 12、两个隔离 ID 不变、冲突上传拒绝、原文件内容断言均保留。没有扩大生产或被测 owner 的超时。

## 实际验证与保留的中间结果

最终目录专项 2 通过 / 0 失败，actual 0 / 8.587514 秒，body 0.05 秒：`Paused` 与 `Cancelled` 均为 131072 字节，部分内容和后继 FIFO 检查通过；未知写已确认 65536 字节，冲突失败、安全任务完成 196613 字节，隔离 ID `[2]` 不变，Handler 准备的成功 STATUS 计数从 5 到 6。完整日志 SHA-256 `e0dcf49fa6d84626793c05bfb62929ca36ac61872f582d8353fbc15e9ac62901`。

EOF 独立专项全部 8 项通过，actual 0 / 12.771272 秒：WRITE 1.057991750 秒、CLOSE 1.055106417 秒，各 14 次独立下载 / 14 次有效 EOF，分别得到预期未知结果 32768 / 12 字节，隔离 ID 不变。真正无回复的只读 CLOSE 仍分别在 EOF 后 1003 / 1004 ms 以空闲超时失败；没有被有效 EOF 永久续期。完整日志 SHA-256 `f3822bfcdb689658f5d422aca12ab17591409c444dd9264f864009be54f38051`。

最终 `python3 -B scripts/check.py` 完整门禁 actual 0 / 397.097301 秒：1395 普通测试、8 doc、6 Python 全部通过，16 项显式 ignored 未冒充通过；`cargo fmt --all --check`、x.y 依赖策略、严格 `cargo clippy --workspace --all-targets --locked -- -D warnings`、工作区测试及独立 2 MiB 完整控制器均通过。完整日志 381044 字节，SHA-256 `ef0a4dac1f6a88181a4dfa828693c6443894cf1df0fee957b6869c7724118260`。

619 份门禁输入正文共 13769303 字节，源映射 SHA-256 `3cbeb4e0d53df597faebd6b6b009356c924a0f4426ce5c595beb9cd132762c0f`，运行前后实际相等。其中只有上述三个测试/夹具源码不同，其他 616 份输入与精确基线相等。门禁结束后仅新增本记录，新的 620 份完整正文另外冻结；它不改变已验证的 619 份输入。

所有失败和中间候选均保留完整日志、actual wait 收据、前后映射及源正文：

- 首次准备错误把自有 CoW cache copy 与 Cargo 重叠，实际编译 101，出现 `serde_core StableCrateId collision`，0 业务测试；日志 SHA-256 `760907e302d056c7ded60c33fdfdc921538379f202a24c728dbe2d9eb4c75f84`。全部拥有的句柄实际结束后，再独立完成替换 cache copy，未继续与 Cargo 并行复制。
- v1 目录专项实际 1 通过 / 1 失败。新增业务结果已取得，但错误等待未监视 disconnect 的监听任务自然退出，导致 3 秒 cleanup 失败；日志 SHA-256 `931276b451aa07d2f849d772f03f35c6211682371948d7a4358466f6bb970d8c`。最终改成 abort 并等待自有 JoinHandle，仍要求监听端口拒绝连接。
- v2 把 WRITE 观察混入既有元数据时间线。自审发现这会扩大已有时间线统计，尽管该版专项和完整门禁实际通过，仍恢复原时间线正文，改为独立计数器；未声称已证明此前时间线有生产旁路。v2 完整门禁 actual 0 / 684.693836 秒，1395 普通 / 8 doc / 6 Python，日志 SHA-256 `c5d82ed71f8587591e4f6fe3943819c76d6ef4618f7261c01979ceff919bca49`。
- v3 独立计数器版专项和完整门禁实际通过，actual 0 / 438.110146 秒，1395 普通 / 8 doc / 6 Python，日志 SHA-256 `a9b0be58d1ff9a6ef55829cfa450bf5b31ebe1fac2189b5cd24c1c602b975da4`。随后非作者初审指出已知取消仍依赖 100 ms 延迟，故等待全部实际句柄结束、冻结原输入后才改为上述受控暂停流程。v3 结果不替代最终源码的门禁。

所有最终 owned 子命令取得实际 exit 0 / reaped 收据，无 timeout、无未知身份信号、无 PG 幸存者；独立 post-wait 探针确认进程组不存在，全部私有临时根为空并移除。长命令使用独立 target / TMPDIR、有界实际等待及 leader birth / PG 身份绑定，不清理其他 owner。最终源码、测试记录、patch、所有原日志和失败/中间版本按完整 SHA-256 清单冻结，供新的非作者复审。

## 开放边界

最终候选还需新的非作者复审、主树整合验证及绑定新精确提交的 Linux / Windows 原生 CI。原 Linux 的精确事件、原 Windows 在 50 ms 配置下的超时原因仍未证明；本机受控 TCP 通过不关闭这些历史根因。服务端 Handler 准备 STATUS 不等于传输送达或取消 worker 收到 ACK。本轮没有桌面 GUI、实际外部智能体、真实 OpenSSH 互通、Linux / Windows 桌面、打包、发布或更新验收，也没有提交、推送或打标签。

## 新非作者复核与根整合进度

新的独立子 agent 在精确 6b4 隔离树完整读回作者包和原 619 Git blob，复原 patch 后 620 输入前后逐份相等；生产及其他 616 输入不变。独立目录两项、EOF 八项、WRITE hold 三项，以及格式、x.y 和严格 session Clippy 均实际退出 0。已知暂停与取消均为 131072 字节；未知写 65536、安全后继 196613、本次隔离 ID [1] 前后相等。WRITE owner 1.071242292 秒、14 次 side/EOF；CLOSE 1.000434167 秒、13 次 side/EOF；真正无回复的只读 CLOSE 在 EOF 后 1003 ms 失败。六个 owned 命令实际等待退出，进程组与私有 TMP 已回收。

作者完整门禁原日志另由非作者全文核验；本次限定复核未重复整个工作区控制器，也没有原生桌面结果。根已逐字读回 4461 份独立封存正文并将三个测试路径和本记录整合到工作副本，保留既有文档与并行候选。根完整门禁结果见下节；新精确提交的后继 CI 仍待执行，此状态不改变上面的历史失败边界。

## 主工作副本完整检查

根执行 `scripts/check.py` 实际退出 0，用时 469.778754 秒；格式、严格工作区全目标 Clippy、x.y 依赖策略、1395 项普通测试、8 项 doc-test 和 6 项 Python 测试通过，16 项显式 ignored 保持原状态。默认与额外 2 MiB 本地智能体控制器各取得 626 个阶段、38 个 TCP 返回、future 5176 字节并到达结束；该控制器使用自有夹具，不调用供应商 CLI 或模型。

检查前后 621 个完整工程输入、13,791,215 字节逐份相等，输入清单 SHA-256 `0d96fa0f6c6c7a98255a84aba769f1b5771af613af52b3ecab18134a4b1200a9`。完整日志 381,293 字节，SHA-256 `10c9a852fae355abf42c1e0ab2e4accaaed135053c9fc5dc05b80dfba8234405`。持有的 runner 已实际 wait/reap，退出 0、无超时，所属进程组没有遗留。检查后仅补充本节及状态文档，文档增补不属于原检查输入正文。

本次检查不提供新的桌面、OpenSSH、Windows/Linux 原生或发布验收；后继 GitHub Quality 必须绑定实际新提交，不能沿用旧提交或本机结果。

## 精确提交的三平台后继 CI

后继提交为 `b26c4c88fa54dbc9788899f86136fb14ef8cb37a`，[Quality 37455024526](https://github.com/cyruss648/keelshell/actions/runs/37455024526) attempt 1 已实际 completed/success，三个 job 均成功。新的独立子 agent 读取完整 API、三个原始 job 日志、完整 attempt ZIP 和两份 OpenSSH artifact；根另逐份复制并读回362份封存正文、11,618,004字节，封存SHA-256 `aee3b6a9f67f133279408b4fb85628ec16da6a9a7b06fb324a6eeb2c81a7e23d`。620个已提交 blob 的路径、mode和ID与该精确提交相等。

| 平台 / job | 普通测试 / doc / ignored | Python | 实际控制器 |
| --- | --- | --- | --- |
| Windows / 112240410393 | 1376 / 8 / 16 | 6脚本、57打包 | 默认与额外2MiB各626阶段、38 TCP；future5536字节 |
| Linux / 112240410701 | 1395 / 8 / 17 | 6脚本、57打包 | 默认626/38；额外2MiB为628/39；future5176字节 |
| macOS / 112240410807 | 1395 / 8 / 16 | 6脚本、57打包 | 默认与额外2MiB各626阶段、38 TCP；future5176字节 |

三平台格式、x.y依赖策略及严格工作区全目标Clippy通过。Linux默认TCP实际为12 connected、25 refused及一次OS104 abort；小栈为13 connected、26 refused。其余四次为12 connected、26 refused，阶段连续且到达结束；保留实际差异，不归一化为固定计数。两个目录取消场景及两项EOF独立回归在三平台均实际通过。

Windows原始日志517,969字节，SHA-256 `3e0529dd7b0c6f4064e0ae7ba16805673b603ff94955e1f67abd191240005d18`；Linux548,263字节，`2c6b8804c26aeb43b219515a4e8d910d98441810d89d401e0eabc933e484d529`；macOS517,368字节，`84d55ec0ac1a3c1ae2cbae0c66912c50ae00e3c294e775b1c832153d852f2ecf`。完整attempt ZIP530,754字节、35成员，SHA-256 `13e47c1a027ef0ea62ada565663fb6875dd4610f5f84d16566b9e4faa0026fcd`；CRC与逐成员绑定通过，三个原日志与对应聚合成员正文相等。首次CLI返回空原始body和准备错误继续保留；之后原始ANSI日志仅写入private PIPE与文件，阅读副本不替换原件。

Linux和macOS各11项隔离OpenSSH互通实际通过，用时16.119和22.088秒；两份ZIP摘要与artifact API digest相等，实际收据记录owned/observed进程退出及临时目录移除，保留原有祖先观测边界。Windows按精确workflow未执行OpenSSH。该结果证明此提交的目标平台工程检查和有限localhost互通，不提供三平台桌面、新同步/定时组合、发布或安装更新验收，也不回溯确定旧失败的精确触发原因。
