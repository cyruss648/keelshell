# 2026-10-06 API inference options — author candidate

Candidate is isolated from main at `e247f2f96618847b8a946e620e4e723f79096fd6`. No commit, push, model call, supplier CLI launch, GUI launch or installation is performed by this author. Root integration and the required new independent review remain separate. Design and usage: [ADR 0054](../../adr/0054-ai-inference-options.md), [API inference](../../product/AI_INFERENCE.md).

## Behavior under test

- Existing model-scoped capability validation; strict protocol mapping for optional effort and independently composed Messages effort + adaptive/disabled/legacy manual thinking; old single-field choices retain their semantics; default omission.
- Exact decimal thousandth storage, explicit zero, non-finite/exponent/overprecision rejection, declared sampling support, mutual exclusion and reasoning/sampling incompatibility.
- Profile storage roundtrip and missing-field migration; unknown secret fields rejected; local CLI and legacy projection cannot silently use/drop explicit API controls.
- Actual loopback HTTP receives the exact Ask preview for three protocols. Fixed connectivity tests use the same fields; a 400 rejection preserves the approved body.
- Real GPUI controls declare support, emit exact Apply metadata, show model-specific preview, preserve invalid drafts across profile/model/locale changes, revoke old review/cancel requests, clear destination declarations and refuse inactive known-secret metadata, including typed effort and persisted thinking-mode strings. Messages independent controls emit and reload composed metadata, preserve invalid budgets on effort edits and clear both fields on destination changes. Chinese/English constrained forms have a scrolling natural-height body and fixed Apply.

## Preserved failures

The first all-targets compilation failed because the new component returned `Div` while its observed test-support wrapper had a distinct type. The first focused test compilation failed because a glob import brought a test macro into recursive expansion. Two following attempts corrected the concrete test UI click method and tuple index type. These four compiler failures are retained with exit 101 and original logs/receipts; no tests or native flow are claimed from them. Final code uses explicit test imports, GPUI `click` and a `usize` ID.

The first full gate then stopped on 24 `clippy::unwrap_used` violations in the new tests; diagnostic error handling replaced those unwraps without relaxing the lint. The following gate found a test-only equality assertion comparing a reply type without `PartialEq`; a `matches!` assertion now checks the expected 400 error. Both failed full gates preserve exact 266-input before/after snapshots, original logs and terminal receipts. The extended gate then caught a missing test-only `ProviderClient` import; its failed exit and stable 267-input snapshot are retained before correcting the import. The next combined HTTP test failed because its synchronous HTTP client was dropped inside a Tokio async test; a synchronous test now owns the runtime and enters it only for async connectivity calls. The production adapters were unchanged by this fixture correction. Its original panic log, exit and stable 267-input snapshot remain preserved.

The initial cache-copy command referenced `/usr/bin/cp`, absent on this host, and failed before copying. `/bin/cp -cR` then completed an independent APFS target clone from main; writable targets are not shared. This setup correction is distinct from product verification.

## Original frozen author results (blocked by independent P2)

- Core focus: four new tests passed, exit 0, 17.608184 seconds including compilation. Log 657 bytes, SHA-256 `4d31f983b9b97697f9748289553f26da609b6d4b81d10f56bb171e4691b127cd`.
- Focused intermediate run: exit 0, 219.662760 seconds including compilation, three HTTP tests, seven GPUI tests and one matched storage test passed; the existing default local-agent controller also passed. This run preceded the final 400-rejection/Apply and declaration-handoff supplements and is not substituted for the final gate. Log 13836 bytes, SHA-256 `f0b42e37722be2d7edf64f44f93ca1db6e0fdaf5cff69bb7a6fd0f5dc91c7e01`.
- Intermediate full gate before the Messages composition extension: exit 0, 293.119994 seconds; 1186 ordinary tests passed, 11 ignored, 8 doctests and 6 Python tests passed; default and 2 MiB local-agent controllers passed. All 266 engineering inputs were identical before/after. Log 128902 bytes, SHA-256 `114b8ae60448a653279d5b5465f056ce639296688e13d1b73e4ab3fc6d9897f7`. This evidence is retained and does not substitute for the extended final source gate.
- Messages composition all-targets strict Clippy focus: exit 0, 85.650203 seconds, 267 inputs stable; followed by additional combination tests and draft-boundary fixes. This is intermediate evidence.
- Intermediate extended full gate: exit 0, 319.436123 seconds; 1189 ordinary tests, 11 ignored, 8 doctests and 6 Python tests; default and 2 MiB controllers passed. All 267 inputs were identical before/after. Log 128446 bytes, SHA-256 `f5d0e4e8e798b788407eb62656e92fb9e29f7b070d08f4a132b16aeb0769c52c`. A subsequent self-review added thinking-mode metadata admission and its independent inactive-secret Apply test.
- Original final full source gate: exit 0, 186.959320 seconds; dependency policy, formatting and strict all-targets Clippy passed; 1190 ordinary tests passed, 11 ignored, 8 doctests and 6 Python tests passed; default and 2 MiB local-agent controllers passed. All 267 Rust/Cargo/toolchain/config inputs were identical before/after. Log 128443 bytes, SHA-256 `da610962c2242155dae58c9a403bee2995337e5fb7c7af8c482136aca1b7ee22`.
- Standard macOS dual-program build: exit 0, 115.679940 seconds; `cargo build --locked -p keelshell-app -p keelshell-mcp --target aarch64-apple-darwin` produced both arm64 Mach-O executables with deployment target 15.0. The same 267 engineering inputs were unchanged before/after the build and identical to the full gate. Log 919 bytes, SHA-256 `18fe7aabc1c33ceae9b415ad8faf1d7f4fdcc4a9f47f7aad96ed71e549af86c4`. Artifact hashes are frozen in the private author evidence; neither executable was launched.

Owned command wrappers use isolated target/TMP and bounded process groups, wait/reap their direct leader, record timeout and original numeric-group status, and retain failing logs. This is not a census of all escaped descendants. Test servers use loopback, bounded waits and synthetic data; vendor applications and cloud model calls are excluded.

## Open acceptance

No final native GUI flow, actual cloud-model acceptance, Windows/Linux native desktop, new exact-commit CI, signed/package installation, Release or automatic update is established by this author. Model-support declarations can be wrong: service rejection must remain failure without fallback or automatic retry. The existing project-wide gaps remain tracked by the root plan.


## Numeric metadata admission repair

A fresh non-author review blocked the original 22-file candidate despite its passing source gate. Its actual production counterexample used an inactive temporary known secret `125`, edited temperature `0.125`, clicked the real Apply control, and read the persisted thousandths value `125` from StateStore; the same mapped final request refused it as `CredentialInContext`. The review's passing counterexample test proves that defect, not acceptance. The original 353-payload candidate, manifest and all original logs remain unchanged in private evidence.

The repair only adds selected inference-number admission to the existing shared Apply/persistence validator: temperature/Top P persisted integer thousandths and exact JSON f64 wire text, plus legal selected legacy/composed Messages budgets. It does not scan full catalog JSON, expand existing numeric-limit/schema exemptions, change the final body guard or weaken request authentication/backend contracts. A new production Workspace test file expands this repaired candidate to 23 files.

New controlled regressions edit real UI inputs, click actual Apply, retain inactive known-secret owners and check unchanged StateStore bytes; a direct event bypass tests the production persistence consumer separately. Numeric unit coverage includes inactive profiles/models, wire-only forms such as `0.0`/`1.0`, both sampling fields, both legacy/composed legal budgets, and unchanged existing capability/output-limit exemptions. Normal production Apply persists explicit zero, omission and composed effort + budget; the real mapper's exact reviewed body is checked after readback.

The first repair focus failed at compilation because a test used a nonexistent `PreparedRequest::body` method; it now reads the actual `preview_json`. The second focus compiled and passed both unit tests, then its real UI tests correctly stopped on controls outside the production modal viewport. The fixture now scrolls the actual form before native clicks; it does not bypass hit testing. The third focus found a test helper identity lifetime mismatch, corrected to a static control ID. All three failed logs, receipts, stable 267-input snapshots and original failing test-source bytes are preserved.

The fourth repair focus passed all four matched tests, exit 0, 13.936406 seconds including compilation. Its 267 engineering inputs matched before/after. Log 1113 bytes, SHA-256 `20103eea19cb2357bcdbdac6772450845d733255baf20c4c204e05b2f15b1a92`. This is focused author evidence; fresh full-source/native-build results follow when complete. All v2 evidence is separate from the unchanged original package; author validation does not replace the required fresh independent delta review.


Fresh repaired full-source gate: exit 0, 278.475820 seconds. Dependency policy, formatting and strict all-targets Clippy passed; 1194 ordinary tests, 11 ignored, 8 doctests and 6 Python tests passed; default and extra 2 MiB local-agent controllers passed. All 267 engineering inputs were identical before/after. Log 128786 bytes, SHA-256 `a6667ce33e136130459ac37e4bced3bced4bcbb503fafd3ce1eb45295cc69aef`.

Fresh repaired macOS dual-program build: exit 0, 57.008997 seconds. The standard locked arm64 build produced both app and MCP Mach-O executables with deployment target 15.0; the same 267 inputs stayed unchanged and matched the fresh full gate. Log 435 bytes, SHA-256 `57254b8f164444672b5c47fa71e1cdd4829def013554b38492445f94103f9414`. Neither binary was launched. Fresh source, artifacts' hashes, failures, wrappers and repair-delta evidence are frozen in a separate v2 package. The original frozen package and logs were read back unchanged. This repaired candidate still requires independent delta review and root integration checks; no target-native GUI/cloud/Windows/Linux/CI/package/update acceptance is claimed.


## 修复差量的独立复审与主线整合

新的独立修复复审结论为 `PASS_LIMITED`，原数值秘密准入P2闭合，无新P1/P2。完整门禁442.416s exit0；1202普通、11ignored、8doc、6Python及格式、x.y、严格整仓all-targets Clippy通过。268工程输入前后相等。新4项GPUI探针22案例10.621s exit0，覆盖实际Apply生产者、Workspace持久化消费者、inactive profile/model、存储整数/实际wire小数碰撞、正常零值/省略/组合保存和既有数值上限豁免。原125/0.125反例均拒绝且磁盘原字节不变；最终正文仍拒绝。

独立探针首次及诊断版本失败均保留：错误假定存储整数1000与实际wire小数1.0是同一秘密碰撞。诊断后正确要求metadata拒绝自己的整数碰撞，而最终正文逐字保持1.0；没有更改实现、期限、原125反例或零写入断言。原849份BLOCKED_FOR_FIX证据保持不变。新的1381份复审payload已根完整读回保存于ignored `work/ai-inference-root-readback-20261006/v2-review-proof`，MANIFEST SHA-256 `a2447caf29cce45142c2b9b222fe7e4d5653cd711fea10f1b7d9d8bd3963333e`。

主线以1e07a28350680d3aee353e78dc5e3563ad7afa11为基线应用23文件的窄补丁。22个非assistant文件与修复v2精确相等，assistant仅在API装配及独立推理测试模块增加两个hunk，原命令目标守卫/提示、脱敏、进度、revision和晚到回复保持。作者只读私有index预检与根实际整合的assistant字节相同；保护路径、Cargo.toml及Cargo.lock未变。首次根预检误用不存在的redaction.rs保护路径，在补丁应用前退出1；实际发现redact.rs后重新核对通过，失败单独保留。

组合主线门禁、标准macOS构建/窗口、精确源码CI随后按实际追加；独立GPUI/回环HTTP、源码或构建不替代供应商/云模型、Windows/Linux桌面、发布签名、安装或完整产品验收。


## 主线组合检查与未验收的原生尝试

另一次非作者组合静态复核结论为 `PASS_LIMITED_STATIC_INTEGRATION`，无新P1/P2。19项非assistant候选Rust与v2相等，14项保护路径与主线基线相等；assistant仅有已审查的API装配及测试模块两个hunk，保持命令目标、诊断、脱敏、revision及取消守卫。73份复核payload已根完整读回，MANIFEST SHA-256 `8437b1f6deb7ae201b9e4ebec8ba871041694d9b0470bf2feca8437ab9ff2793`。这项复核没有再次启动GUI或运行供应商。

根组合完整 `scripts/check.py` 实际exit0，429.149s：1207普通、11ignored、8doc、6Python及格式、x.y、严格整仓all-targets Clippy通过。默认与额外2MiB控制器各626阶段，结束分别10.668080/10.728813s。552项全部可见仓库输入在门禁前后、构建后和原生结束时相等。日志357614bytes，SHA-256 `90ca773fdf73f2896d0be347ab3e68f6c19e66eda2a76a036302b6620ae47c88`。这些数量为实际日志解析，不沿用作者或旧提交的数量。

新标准macOS双程序构建exit0，9.229s；标准app打包exit0，0.310s，两个实际arm64 Mach-O及bundle最低15.0已检查。未安装覆盖已有应用，未发布标签。本次没有重跑57项包装回归，原包装输入未变；构建/打包不能代替原生配置验收。

新包实际启动到中文/System深色AI设置；完整AX包含新增37–47控件，但截图显示推理/采样控件在表单当前可视区下方。三次不同坐标/幅度的wheel尝试未观察到像素位置变化，setValue返回 `cannotClickOffscreenElement`，温度值未建立，0 HTTP请求。不能据此确认是自动化wheel传递还是应用处理造成，也不能声称已完成原生参数配置。两张实际截图、完整AX、源码/二进制绑定及生命周期回执保存在独立ignored `work/ai-inference-native-20261006`；15份payload MANIFEST SHA-256 `6673a8140718893f18734fb5e0c242a7ba0bb0581abb26eac1957ce9101fd610`。

该controller实际exit0，app -15明确wait/reap，HTTP活动0/线程关闭、自有端口关闭、private删除；根另核对两个已记录PID/birth消失。三个已检查为空的构建/门禁TMP已删除。这是有限ownership检查，不是完整内核后代普查。没有SSH、供应商CLI、云模型、客户数据或shell执行。

显式可拖动的主题化垂直滚动条后续改进正在隔离开发，保留原23文件v2及本次原生材料不变。后续需新的非作者审查、组合检查、重新构建和新的原生运行。本段追加属于检查后的明确文档更新，不再声称当前完整文件映射等于旧552输入检查点；Rust/Cargo/脚本/包装仍保持已验证字节。

## 最终可达性修复后的根检查点

后续设置滚动条、错误反馈固定动作与长请求审核确认区完成新非作者代码/GPUI复审，无新P1/P2。最终根完整门禁实际exit0，180.646134s：1211普通、11ignored、8doc、6Python及格式、x.y、严格整仓Clippy通过，554可见工程输入直至新原生结束保持相等。新标准macOS双程序构建5.375726s与打包0.293950s均成功。

最终新包中文/System实际三协议人工发送、固定回复和持久化白名单读回通过限定范围：Chat显式温度零；Messages中等努力/自适应思考且真实拖动审核正文到末尾，Send固定完整可见；Responses中等嵌套reasoning。恰好3次自有回环POST，无凭据、SSH、CLI或云模型。原失败与中间仅一次Chat的材料保留不变。精确wire摘要、独立602payload和新原生41payload manifest、源码/二进制绑定、owned wait/reap及未验收边界见[滚动与确认记录](2026-10-06-ai-request-review-scrollbar.md)。更新的公开文档属于554输入冻结/原生结束后的增量，不混用旧哈希映射。

最终新原生证据已通过非作者限定复核，无新P1/P2；独立复算1211/8doc/11ignored/6Python、两组626记录，实际stage字节、3wire和白名单settings一致。611份复核payload根已完整保存，缺失的3份旧文档原体与Chat无独立重开截图等边界如实记在[确认区记录](2026-10-06-ai-request-review-scrollbar.md)，不扩大为完整产品或跨平台验收。
