# Connection library transactions — 2026-10-05

Base: `3a2df35f88466c650e86056c20bf67f23f92a7f4`. Implementation is isolated in
the managed `feature/library-bulk-recycle` worktree. See
[ADR 0045](../../adr/0045-reviewed-connection-library-transactions.md).

## Implemented behavior

- Explicit profile selection and reviewed folder moves, tag add/remove/replace,
  uniform favorites and soft deletion; search-hidden selections remain listed.
- Atomic trash/restore/purge of a selected jump chain. Retained active dependents
  block trash; retained active or trashed dependents block permanent purge.
- Single, selected and entire-trash permanent cleanup all require exact review
  and explicit permanent confirmation. Batch trash has an explicit, reviewed
  restore/undo proposal; single restoration remains available.
- Drafts survive domain/save failures and language changes. Confirmation binds
  metadata, selection and active SSH identities. StateStore performs one worker
  save with its existing revision, lock and atomic replacement.
- Vault entries and host trust remain available; purging profile metadata never
  erases secrets, closes an established session or sends terminal commands.

## Local gates

- `cargo test -p keelshell-core --locked`: **321 ordinary and 4 documentation tests
  passed**, including nine new public batch-operation integration tests.
- The first passing default-stack connection-library run passed **23 GPUI tests**,
  including nine new bulk scenarios. The final undo-scope refinement added one
  GPUI regression; final `cargo test -p keelshell-app --locked` passed **341 tests**
  on the final production source, including all ten new bulk scenarios and one
  review-binding unit test. No `RUST_MIN_STACK` is set for these passes.
- Final `cargo fmt --all -- --check`, strict workspace/all-target Clippy and
  `scripts/check.py --policy-only` passed. Registry requirements and Cargo.lock
  were unchanged.
- The complete `python3 scripts/check.py` passed **1008 ordinary + 8 documentation
  tests**, six script tests, the default CLI process controller and its extra
  2 MiB controller. Its application run contained 340 tests, before the last
  undo-scope refinement. Final source then passed the complete 341-test
  application run, strict Clippy, format and policy checks above; the unchanged
  core and transport gates are not relabeled as a rerun of final workspace.
- Final `cargo build -p keelshell-app --locked` passed. `file` and `otool -L`
  confirmed a native arm64 Mach-O executable with system framework/library
  dependencies. This is compilation/structure evidence, with no native launch,
  installed application replacement or final Release claim.

The controlled SSH GPUI scenario uses a bounded loopback TCP SSH server owned by
this test, not an emulated route: the established session appears in review,
removing its active identity invalidates confirmation, a fresh trash confirmation
persists metadata, the original session remains open and remote terminal input
count stays zero. Domain route tests independently cover whole jump chains and
retained dependencies; the loopback scenario does not claim a native SSH jump UI
acceptance.

Logs remain under ignored `work/library-bulk-evidence/`: `core-full.log`,
`app-library-eighth.log`, `undo-scope-final.log`, `app-full-final.log`,
`clippy-final.log`, `fmt-final.log`, `policy-final.log`, `gate-final.log`,
`native-build-final.log` and `native-macho.log`.

## Preserved failures and diagnosis

Initial GUI-test compilation used a nonexistent ScopedWindow bounds method and
an unqualified `#[test]` shadowed by the imported GPUI test macro; corrected to
the element snapshot and qualified Rust test attribute. The first runnable
permanent-cleanup test overflowed the default test-thread stack. A 16 MiB
diagnostic-only run passed, identifying stack pressure rather than an operation
loop; it is not acceptance evidence. The review view is split into editor,
target and footer helpers before rerunning default-stack gates. Follow-up
compilation errors during that split (method visibility and copied UUID
dereference) and a test index type mismatch were retained and corrected.

ARM64 disassembly also located stack pressure in the pre-split
`connection_table` frame: approximately `0x121cc0` bytes. Splitting rows, folder
tree, header and import toolbar behind `AnyElement` boundaries reduced that
frame to approximately `0x12fc0`, with the row builder `0x92410` and new modal
`0xd7c0`. `pre-split-stack-observation.txt` preserves the exact observed prefix
and source-state limitation; `split-view-stack-frame.log` contains final excerpts.
The default-stack old wide and compact regressions then passed. One compact
layout regression initially clipped the row by 0.5 pixels; showing bulk actions
only while profiles are selected restored its space without weakening bounds
assertions. Strict Clippy also found duplicated focus branches and a modulo lint
in the extracted builder; both were corrected.

No product or test stack limit is increased for final acceptance. No failed
evidence is removed or converted into a pass.

## Unverified boundaries

The GPUI tests use actual production callbacks/layout with isolated state and
bounded owned SSH fixtures. They do not prove GPU screenshots, customer SSH,
Windows/Linux native windows, OS signals, signed/notarized packages or deployed
releases. Independent review and integrated native exercise are delegated to
the root task after this slice freezes; this record does not claim their result.

## 整合负责人核对的独立复审

冻结作者提交 `eabd37f1c91041ea0ac0f452878cf0f5487ab435` 经独立代理只读复审，无剩余可复现 P1/P2。437 个跟踪文件前后 SHA-256 一致，HEAD 与干净状态不变。根任务核对了 39 项证据清单，完整 diff 的 SHA-256 为 `43ec90116d19426264743cacea3dcc0aa01a42725fddf71f1690bcd6e346e60b`，独立报告为 `ec4b6335eb942809acbc2f251f69a2118e2b9550de4b65cfbc928c0a847c46db`。

- 独立 core：321 普通测试与 4 文档测试通过。
- 独立连接库 GPUI：24 项通过，含真实 TCP SSH 会话身份失效与软删除后会话保持；没有设置 `RUST_MIN_STACK`。
- 独立 strict workspace/all-target Clippy、fmt 与 `x.y` 策略通过。
- 源码副本的 5 个补充探针通过：900×580 双语/明暗永久清理审核页脚、精确焦点、主题/语言实际保存后的审核保留，以及原始滚轮将第二行选择按钮完全滚入视口后点击成功。额外显式 2 MiB 线程只重复工具栏/审核两个布局场景。

早期探针的未注册观察点、过滚与按钮部分可见时点击失败保留在 ignored `work/library-bulk-review-20261005/`，包含失败源码、最终探针与 SHA 清单；不把探针错误归为产品缺陷，也不删除失败记录。作者证据已逐份 SHA 核对复制到主工作区。此处尚不声称根任务最终整仓门禁、合并后的新程序原生、其他平台原生或新提交 CI 已通过。
