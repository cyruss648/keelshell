# 自动更新策略候选 — 2026-10-08

## 当前状态

独立 worktree 基于精确 `a4c15c04772b11a70a592ae7c95132b4bea4d818`，原功能范围保持24路径；新增测试由最初24项扩展为26项，已导入主工作副本与工具栏组合。作者的原完整门禁通过属于取消语义修复前源码；最终取消修复后的格式、x.y策略、workspace全target严格Clippy和41项updater／5项workspace受控测试另已实际通过。新的非作者独立消费候选源码与证据后，组合审查又发现旧绘制更新动作身份竞态；根以六项旧源码真实失败／同测试修后通过及最终九项回归修复。准确主树800输入完整门禁实际通过1,731普通／10doc／6Python，标准macOS包和限定原生设置／SSH交互已完成，详见[组合记录](2026-10-08-update-toolbar-main-combination.md)。没有实际安装或发布。

## 实现与测试范围

- `Settings.updates`：关闭／每天／每周，默认每天且自动下载关闭；严格解码、旧状态迁移和最近成功检查时间。
- 长期更新服务：60秒延迟启动、单调调度、15分钟至6小时失败退避、人工检查优先、实际任务取消和精确结果ownership。关闭面板只隐藏服务。
- 后台可选下载：固定项目来源、完整Release／平台绑定、已有归档／校验限制，不自动安装；Ready继续查新，旧包独立保留到新包验证，活动请求取消恢复旧包，无请求时明确“丢弃已下载更新”。
- 乐观保存与重载：保留SSH实体和未发送命令建议；检查metadata合并到真实StateStore，获胜并发设置不被旧策略覆盖；保存中再次编辑的草稿保持未保存提示。

26个新增测试为Core2个policy单元和7个真实StateStore集成、调度3个时钟测试、更新服务9个及Workspace5个受控GPUI。updater41的总数还包含29个既有helper／安全测试，不能将其都称为新增。自有loopback listener、held reply gate、独立临时目录和有界wait覆盖实际网络future取消、迟到阻塞暂存回收、整包下载／解包／摘要、Ready周期检查、替换校验失败保留旧包和活动取消／明确弃包。Workspace包含真实按钮与实际保存、并发冲突、隐藏中检查完成并持久化、重开同service、语言／主题／最低受控窗口及远程pane和草稿身份保留。

## 执行记录与原失败

所有运行使用仓库精确Rust1.98.1、arm64 macOS、jobs1、私有绝对TMPDIR和独占共享target。owned runner为每次运行保留775项输入及完整源码、原始输出、leader的实际wait结果、前后输入相等校验、进程组和TMP收尾。下表证据目录相对ignored `work/automatic-update-policy-author-20261008-v1/`；每项有 `RESULT.json`、`inputs-before.json`、`inputs-after.json`、`sources/` 和 `canonical-check.log`。

| 运行目录 | 实际结果 | 意义 |
| --- | --- | --- |
| `format-initial` | 1 | 初始偏好card缺失闭合分隔符；原700B输出保留，已修正 |
| `focused-core` | 0；449通过，0失败／ignored | Core实际policy／存储9项新增行为通过；该领域源码后续未变 |
| `focused-updater` | 101 | test glob引入GPUI test macro递归，未进入行为测试；原1141B编译失败保留，改显式导入 |
| `focused-updater-v2` | 101 | 缺少Selectable trait和TestAppContext字段访问错误；原3770B输出保留，窄修后验证 |
| `focused-updater-v3` | 0；41通过 | 取消修复前服务／worker／Ready／替换失败等受控证据 |
| `focused-workspace` | 0；5通过 | 关闭隐藏、真实持久化及保存／重载受控证据 |
| `canonical` | 1 | x.y／6Python／fmt通过后，严格Clippy发现14处test-only expect；原7899B保留，改为已有Checked检查，无lint放宽或断言／期限削弱 |
| `canonical-v2` | 0；1703普通Rust／10rustdoc／6Python通过，22ignored未执行 | 完整作者门禁1022.020秒；775输入／15,587,349B前后相同。此轮在新增held-cancel反例和取消修复之前，不能算作最终epoch的full PASS |
| `cancel-counterexample` | 101；0通过／1失败 | 保留旧生产cancel，仅运行同一最终Ready case：held周期请求时真实点击Cancel，实际在“取消新检查必须保留先前已校验包”断言panic。775输入前后一致、16.489秒；未将失败改称通过 |
| `format-final` | 0 | 修复后的最终Rust源码格式通过 |
| `focused-updater-final` | 0；41通过，0失败／ignored | 同一held-cancel反例在修后通过；实际archive路径／digest保留，继续周期查同Release，再通过明确弃包按钮回收；10.151秒 |
| `focused-workspace-final` | 0；5通过，0失败／ignored | 修后长期服务与保存／隐藏行为通过，0.697秒 |
| `policy-final` | 0 | 所有直接registry版本x.y，无新依赖 |
| `strict-final` | 0 | `cargo clippy --workspace --all-targets --locked -- -D warnings`，7.552秒 |

最后五项每次均775输入／15,590,381B前后一致，leader实际reap、无timeout／survivor，所属进程组已消失、空TMP已移除；完成后作者明确释放共享target，不保留Cargo消费者。最终文档随后按真实结果更新，最终完整组合由主树重新运行，不能从这些旧snapshot推导新full或新提交CI。

关键raw摘要：修前full `a2dd42995dfe793ba56262ae36287add6f78d2a44d3e9933c6807805b2371a63`（424,615B）；取消旧反例 `bc5f9cc78b56e50f54bdc1b98232173e4e4d524487b3f3fe5f7f7493dec356b8`（1773B）；修后updater `edbab2858816730fb132583d0f07d53a85910ad2a8410ea1f1a60d98859d7f89`（4724B）；修后workspace `e47a45ed60d167c1aa1837719c13a7b4e8a8ddf11fb480fe903a4ae9fc1c03d7`（1404B）；最终Clippy `cd0da305f1215b8f332755039bfc118d0b49247c0b5d566faa2ccac28ed24727`（434B）。最初 `work/automatic-update-policy-offline-20261008-v1` 保留原始UNRUN状态，不覆写成测试通过。

## 独立复核与未关闭验收

新的非作者分别复核源码、原失败、各轮完整raw和输入绑定，确认新增replacement／hidden held request实际覆盖、较新草稿未保存提示，以及取消反例旧源实际失败／修源实际通过和明确弃包的真实按钮回收。`AU-UX-01` 与 `AU-CANCEL-02` 已闭合，候选源码与专项验证结论为 `NO_BLOCKER`；记录在ignored `work/automatic-update-final-independent-review-20261008-v1/`。当时gate到文档更新之间仅五份文档改变，全部Rust、manifest、lockfile和toolchain输入仍匹配最终严格Clippy快照；70项Core Rust／manifest输入保持此前449测试源码。此历史候选证据不替代后来主树完整门禁；最终组合、新身份反例与限定原生结果由组合记录单独追踪。

实际GitHub下载、签名／公证、覆盖真实安装目录的更新／回滚、完整原生键盘／辅助技术以及Windows／Linux桌面未验证。受控GPUI、loopback、编译和打包不能代替这些目标。阻塞阶段测试额外持有runtime并主动放行gate，仅证明放行后的暂存回收，不证明长时间阻塞解包时整个原生应用退出有界性；该原生生命周期验收和OS中断暂存清理保持开放。没有客户数据、供应商模型／账户或现有应用覆盖。
