# 独立 Zmodem peer 原件

本目录保存本批直接 OpenSSH raw PTY、源码复核和最终门禁的限定原件。
工程基线为 `ac7ec43e6a42329417a99b5933bc171faa19caf6`；
交付工程以包含本目录和五文件增量的 Git 提交为准。
本目录不声明稳定 peer 兼容、三平台原生或 FILE-05 完成。

`originals.tar.gz` 为 623 个普通成员、2,573,153 字节，
SHA256 为 `9eca3013dbd8424c0f2fa2613a0d7aae58df24d6267e40848d8a8bb45154c888`。
完整逐件长度／SHA256 在 [manifest.json](manifest.json)，
原来源至归档成员的映射在归档 `selection.json`。
解包时使用独立临时目录；不要把原件中的旧绝对路径当成当前资源。
父级辅助脚本为最后一次封存时的快照；历史失败只按各自回执和日志判定，
不推定可变辅助脚本此前每一版逐字节相等。

## 工程与复核入口

| 成员 | 范围 |
| --- | --- |
| `parent/candidate-inputs-r7.json` | 765 工程输入；canonical JSON aggregate `7ae00377c1eb95929710c7d8170d444ea293f8c27a3c8690cb83aa79bde3ede6`，五个工程文件改变。 |
| `parent/after-final-gate-r1.json` | 完整门禁后的同输入，逐项不变。 |
| `parent/final-gate-r1.json`、`.log`、`final-gate-summary-r1.json` | `0/reaped/796.379603s`；普通 Rust 3419/0/62 ignored、14 rustdoc；Python 主 523 run/3 skipped。fmt、严格 Clippy、依赖声明政策和三个 controller gate 通过。 |
| `parent/final-gate-r1-inventory-r3/summary.json` | 94 个普通套件的实际结果与精确二进制 `--list` 逐名相等，二进制摘要前后不变；两个 harness=false controller 不执行 `--list`，由门禁实际运行。 |
| `source-review/decision.json`、`summary.json`、`review.md` | Fermat最终r7三文件限定APPROVE、P0–P3全0；41唯一测试通过，三历史P2关闭。 |
| `core-peer-review/decision.json`、`summary.json` | Peirce两core文件限定批准；106项为父预编译二进制的独立执行，非独立重编译，4,138项离线复核另计。 |
| `core-peer-review/final-evidence-addendum/` | r7实际原件的1,714项补充核验、同输入／peer绑定及明确OPEN边界；原decision／summary保持。 |
| `parent/agents-closed-r7.json` | 五位本批作者／reviewer已关闭；无新增用户chat、worktree或容器。 |

库存工装首两次分别因 `ZipWriter` 诊断、嵌套 Python 输出与 Rust 状态同一行而停止；
两份失败保持。最后按完整 Rust 名称、独立二进制清单及每套件计数交叉核对通过，
没有重放完整行为门禁，也没有放宽产品或 peer 成功断言。
完整依赖新鲜度仍UNKNOWN：r1部分registry失败、r2超时原件均保留，
本批没有改依赖声明或锁文件。

## 真实 peer 与失败入口

最终 `parent/mac-lrzsz-r7/` 为9通过／1失败，
`parent/mac-sexyz-r7/` 为8通过／2失败，各有 `tests.log`、`result.json`。
对应 `*-binding.json` 将全部765输入及peer执行前后摘要与r7冻结相核对，
外层 `*-test-r7.json` 保存有界终态和reaped。
peer fd0/fd1直接位于同一真实SSH raw PTY，stderr在自有普通文件；
没有pipe、relay、interposer或替代exec传输。

两peer的CRC16、CRC32首个完整ZDATA／offset0帧和同shell继续输入有严格OO通过。
lrzsz完整batch三份审核、回执、内容与顺序通过后strictOO1；
其单个空文件仍是saved1／missingOO1／peer exit0，严格Finished断言失败。
SexyZ两个空文件边界拒绝ZRPOS0；不能从日志单独签署完整根因归属。
lrzsz r5的8／10、r6的10／10及r7的9／10均保留，不以一次绿色推定稳定兼容。

旧r3／r5／r6的raw marker布尔值只证明字节出现，不证明CRC帧格式。
r7观察器在审核后只匹配最多96字节的首帧完整独立CRC向量，不搜索载荷，
不能扩大为全部帧或batch每文件的格式证明。

`sexyz-preparation/`、`lrzsz-preparation/`保存固定公共来源、摘要、
构建和自有命令终态；后者按准备者明确白名单选择。
未封存第三方完整源码、构建目录、可执行文件、下载归档或完整手册。
原始临时SSH私钥目录已按各轮回执删除。
后续推送、精确Git绑定与最终磁盘回收回执另记，不能由本封存推定。
