# Windows Zmodem 本地后端交付 — 2026-11-15

状态：`SOURCE_READY / PARENT_VALIDATION_PENDING / WINDOWS_NATIVE_NOT_RUN`。

本作者只写以下两个 Rust 文件和本目录，没有运行 Cargo、rustc、rustfmt/fmt、
Clippy 或 Rust 测试，没有操作共享 target、提交、创建子 agent 或新 chat。

- `crates/keelshell-session/src/zmodem/local_windows.rs`
- `crates/keelshell-session/src/zmodem/local_windows/tests.rs`

现有 `zmodem/local.rs` 与 `zmodem/local/tests.rs` 未改；
交付时 `git diff --exit-code -- <这两个路径>` 实际退出 0。
Cargo、`zmodem.rs`、platform 实现、归档接入和正式产品/测试文档属于父任务写集。
工作树中其他作者的既有改动保持原样。

## 接口与平台约束

保留 `UploadPlan::prepare(Vec<PathBuf>, &AtomicBool)`、`review()`、
`confirm(self, &AtomicBool)`，以及 `Destination::prepare`、`revalidate`、
`publish(self, Vec<u8>, &AtomicBool, &AtomicBool)`。
`Destination` 继续可 Clone；`path`、`staging` 和 `Saved::{path, bytes, sha256}`
均保持 `pub(super)`。Windows `revalidate` 返回安全的 `Directory`；
当前 controller 在确认时只用 `.map(|_| ())`，不依赖 Unix `File` 返回类型。

父任务负责 Windows 路径选择：

```rust
#[cfg_attr(windows, path = "zmodem/local_windows.rs")]
mod local;
```

以及 session 的 Windows 条件 `keelshell-platform` path 依赖。
本作者没有修改这两处。测试通过本文件末尾显式
`#[path = "local_windows/tests.rs"]` 接入，首行为 `#![cfg(test)]`，
随后为 `#![cfg(windows)]`，满足 remote-only 源码扫描约束。

目录只来自 `Directory::open_chain`，不调用收紧为 private 的 `from_file`。
使用当前 `Directory` / `DirectoryStamp` / `FileInfo` / `PrivateEntry`、
`metadata`、`verify_private`、`WindowsPath`、`validate_windows_component` 安全 API。
同步 native 句柄、实际 held volume 的本地磁盘检查及不泄露 security 字节的
Debug 由 platform 边界提供；本后端没有复制 ABI 调用或用 std 句柄替换该来源。
`api-input-binding.json` 记录本作者最后核对时的 API 源码摘要，不代表 platform
或整个仓库冻结，父任务可继续推进其写集。

## 实现不变量

来源：

1. 在任何 native I/O 前解析绝对盘符路径，审查所有 literal component，另对
   协议 basename 执行严格校验；拒绝 UNC/device、ADS、dot traversal、保留名、
   隐藏方向标记及非 Unicode 路径。
2. 保存从盘符根到直接父目录的全部 `Directory`，并保存完整 `DirectoryStamp`
   链；来源文件句柄也保留至审核消费。重新打开路径时对比原链与 held 对象，
   不只保留叶目录或比较大小/时间。
3. `metadata` 负责 reparse / 单硬链接 / 删除中对象的拒绝；读取前后对比完整
   `FileInfo`，按 64 KiB 检查 stop、读取预算与精确 EOF，同时计算 SHA-256。
   回查命名来源及完整父链。
4. 保持文件数、单文件 64 MiB、总计 128 MiB 预算，以及 portable Unicode
   大小写别名和真实 identity 去重。确认重新完整读取，比较全部信息、父链、
   审核字段、字节及摘要；整批成功后才生成纯协议 `SendFile`。

目标：

1. prepare / revalidate 只读，检查目标与随机 staging 名称不存在，包括 Unicode
   别名。提前取消、已占目标、已预置 staging 或祖先替换不会开始 mutation。
2. 在调用 `create_file` 之前先置 `mutated = true`。安全 API 在 exclusive create
   时即提供 protected owner-only DACL；没有先写内容再补 ACL。
3. 从创建开始一直保留同一个 `PrivateEntry`。完整写入、flush、sync_all、私有
   DACL、完整读回及前后 `FileInfo` 核验后，回查父链和 staging 命名对象。
4. 创建句柄只共享读取；观察性 `open_file` 共享全部访问，因此允许只读回查且
   不会用新的 writable/delete 句柄替换 retained writer。只将原创建
   `PrivateEntry` 交给 `Directory::rename_no_replace`。
5. rename 后重新打开精确目标，确认 identity 等于 staging 原对象、字节数正确、
   retained 与 named 完整 `FileInfo` 相等、DACL 私有、完整内容一致且读取前后
   元数据未变。再查完整父链、唯一精确 Unicode 名称、staging 缺席和命名目标。
6. 任何创建尝试之后的取消或失败都不清空 `mutated`，不删除 staging / 目标；
   由已有 controller/reservation/quarantine 管理未知结果。rename 后取消同样
   保留目标。成功回执包含准确路径、长度及小写 SHA-256。

只 flush 文件数据；没有调用目录 sync，也没有推断目录或 rename 的 crash durability。

## 测试源码与静态检查

`test-inventory.json` 为逐名库存：**45 项，43 项默认、2 项 opt-in；实际运行 0 项**。
所有测试返回 `TestResult`，使用真实 Windows 临时目录及平台安全 API；
不依赖外部命令、sleep、客户机器或真实 SSH。两项 symlink/reparse 用例明确
`#[ignore = "requires Windows symbolic-link privilege or Developer Mode"]`，
需要具备权限的 Windows 执行者显式运行，没有静默 skip 后当通过。

优先核对以下精确名称：

- `zmodem_upload_local_observation_keeps_complete_binary_and_empty_content`
- `zmodem_upload_local_confirmation_rejects_an_identical_replacement_inode`
- `zmodem_windows_confirmation_rejects_metadata_only_changes`
- `zmodem_windows_reviews_retain_every_original_ancestor`
- `zmodem_windows_reviews_reject_replaced_grandparent_with_original_leaf_and_source`
- `zmodem_windows_upload_rejects_total_byte_excess_before_reading_the_last_file`
- `zmodem_windows_upload_rejects_expanding_unicode_case_aliases`
- `zmodem_windows_confirmation_rejects_a_new_hardlink_after_review`
- `zmodem_windows_confirmation_rejects_reused_path_after_hardlink_rebinding`
- `zmodem_local_publication_preserves_a_target_created_after_preparation`
- `zmodem_windows_publication_preserves_a_precreated_staging_name_before_mutation`
- `zmodem_windows_exclusive_create_marks_mutated_before_a_late_collision`
- `zmodem_windows_create_sets_private_dacl_before_any_content_is_written`
- `zmodem_windows_retained_creation_handle_blocks_stage_replacement_and_other_writers`
- `zmodem_windows_publication_rejects_an_identical_staging_replacement_identity`
- `zmodem_windows_cancellation_after_create_retains_empty_private_staging_and_mutation`
- `zmodem_windows_cancellation_after_readback_retains_complete_private_staging`
- `zmodem_windows_cancellation_after_rename_retains_the_published_target_for_quarantine`
- `zmodem_windows_target_race_after_staging_preserves_both_names_for_quarantine`
- `zmodem_windows_final_readback_rejects_a_late_unicode_alias_and_retains_mutation`
- `zmodem_windows_final_readback_rejects_reappeared_staging_and_retains_both_paths`
- `zmodem_windows_publication_verifies_binary_content_identity_private_dacl_and_new_target`
- `zmodem_windows_publication_verifies_an_empty_file_without_leaving_staging`

其余库存包含大小/count/剩余预算、目录来源、缺失和改写来源、末来源改变的整批拒绝、
路径原始拼写、非 Unicode、UNC/device、目标别名、no-replace 和坏/短/长读回。
预算整批用例真实建立两个合计 128 MiB 的零填充文件及末尾 1 字节文件，会完整
读取前两项；这是普通测试的实际资源需求，不把 helper 的小预算检查冒充整批检查。

取消与竞争通过生产中的 create / write / publish / final-readback 阶段私有 helper
确定性安排，使用真实对象；没有增加全局 hook、模拟 metadata、sleep 或无界线程。
替换 staging identity 的防御用例显式先关闭原创建句柄，再建立新对象，验证旧 stamp
无法授予新对象发布权；另一个用例检查原句柄保持时 replacement/write/delete 被拒绝。

`static-check.json` 只记录读取源码、逐名库存、空白、写集和 API 使用核对，
不是 Rust 解析、类型检查或行为执行。对两个未跟踪新文件执行
`git diff --no-index --check -- /dev/null <file>` 均为 exit 1、stdout/stderr 空；
该命令的 no-index 差异状态不记成 exit 0。另由 Python 实际检查末尾空白、
冲突标记、首行 test cfg、测试唯一命名/返回类型和禁止的生产操作。

## 父任务仍须执行

1. 完成 path/dependency 接入后，对集成后的实际源码执行 Windows cross-target 编译、
   格式、Clippy、完整门禁并核对新测试的编译清单。此前 platform 单独 cross-check
   成功不是本后端的编译结果。
2. 在真实 Windows 本地磁盘运行默认 43 项及具备条件的 2 项 reparse 测试；
   同时按父计划执行 platform 原生测试。重点观察权限、share compatibility、
   timestamp/rename 行为和完整发布。
3. 保持唯一 Cargo target 的父归属，按 `docs/testing/SHARED_TARGET.md` 核对产物与
   完整测试名，不能从本静态库存推断新二进制已包含这些用例。
4. controller、UI 文件选择、真实 SSH/PTY 互操作、三平台原生桌面、完整产品和
   crash durability 各自保留独立验收边界。本作者没有代签或启动其中任何一项。

静态回执补充：`static-check-r1.json` 保留首次文本规则失败；它把纯协议
`SendFile::new` 的子串误匹配为 `File::new`。补上词边界后
`static-check.json` 的文本检查全部通过，两个 Rust 文件未变；这不是编译或行为结果。
