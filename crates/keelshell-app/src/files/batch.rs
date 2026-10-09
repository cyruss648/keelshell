//! Selection-bound batch preparation is read-only; approval dispatches exact plans.

use super::*;
use keelshell_session::sftp::{FileTransferPlan, MAX_QUEUED_TRANSFERS};
use std::collections::BTreeSet;

const MAX_BATCH_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const PREPARATION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BatchBinding {
    pub(super) session: uuid::Uuid,
    revision: uuid::Uuid,
    remote: String,
    local: PathBuf,
    local_selection: Vec<PathBuf>,
    remote_selection: Vec<String>,
}

#[derive(Clone)]
pub(super) struct BatchRequest {
    binding: BatchBinding,
    direction: TransferDirection,
    sources: Vec<BatchSource>,
}

#[derive(Clone)]
struct BatchSource {
    name: String,
    local: PathBuf,
    remote: String,
    directory: bool,
    supported: bool,
}

#[derive(Clone)]
enum PreparedTarget {
    File(FileTransferPlan),
    Directory(DirectoryTransferPlan),
}

impl PreparedTarget {
    fn bytes(&self) -> u64 {
        match self {
            Self::File(plan) => plan.bytes(),
            Self::Directory(plan) => plan.bytes(),
        }
    }
    fn operation(self) -> Operation {
        match self {
            Self::File(plan) => Operation::TransferFile(plan),
            Self::Directory(plan) => Operation::TransferDirectory(plan),
        }
    }
}

#[derive(Clone)]
struct BatchRow {
    source: BatchSource,
    prepared: Result<PreparedTarget, Message>,
}

#[derive(Clone)]
pub(super) struct BatchPlan {
    pub(super) id: uuid::Uuid,
    binding: BatchBinding,
    direction: TransferDirection,
    rows: Vec<BatchRow>,
}

impl BatchPlan {
    pub(super) fn ready_count(&self) -> usize {
        self.rows.iter().filter(|row| row.prepared.is_ok()).count()
    }
    pub(super) fn review_message(&self) -> Message {
        let ready = self.ready_count();
        let rejected = self.rows.len() - ready;
        let bytes: u64 = self
            .rows
            .iter()
            .filter_map(|row| row.prepared.as_ref().ok())
            .map(PreparedTarget::bytes)
            .sum();
        let mut zh = format!(
            "批量传输审核：{ready} 个可传输目标／{rejected} 个拒绝项，共 {bytes} 字节。确认仅加入以下明确可传输目标；拒绝项不会执行。\n原SSH连接：{}\n本地目录：{}\n远程目录：{}\n普通文件上传仅原子发布；目录按项传输，失败或取消可能留下部分目录和文件。下载不覆盖本地文件；未知写入保留隔离，不自动重试。\n",
            self.binding.session,
            self.binding.local.display(),
            self.binding.remote
        );
        let mut en = format!(
            "Batch transfer review: {ready} admissible targets / {rejected} rejected entries, {bytes} bytes. Confirm adds only the exact admissible targets below; rejected entries do not run.\nOriginal SSH connection: {}\nLocal folder: {}\nRemote folder: {}\nRegular-file uploads publish atomically. Folders transfer item by item; failure or cancellation can leave partial folders and files. Downloads never overwrite local files. Unknown writes retain isolation; no automatic retry.\n",
            self.binding.session,
            self.binding.local.display(),
            self.binding.remote
        );
        for (index, row) in self.rows.iter().enumerate() {
            let (source, destination) = match self.direction {
                TransferDirection::Upload => (
                    row.source.local.display().to_string(),
                    row.source.remote.clone(),
                ),
                TransferDirection::Download => (
                    row.source.remote.clone(),
                    row.source.local.display().to_string(),
                ),
            };
            zh.push_str(&format!(
                "\n{} · {}\n来源：{source}\n目标：{destination}\n",
                index + 1,
                row.source.name
            ));
            en.push_str(&format!(
                "\n{} · {}\nSource: {source}\nDestination: {destination}\n",
                index + 1,
                row.source.name
            ));
            match &row.prepared {
                Ok(target) => {
                    let size = target.bytes();
                    let (overwrite_zh, overwrite_en) = match target {
                        PreparedTarget::File(plan) if plan.replaces_existing() => (
                            "替换审核时已存在的远端普通文件",
                            "replace the existing reviewed remote regular file",
                        ),
                        _ => (
                            "仅新建；目标必须仍不存在",
                            "create only; destination must remain absent",
                        ),
                    };
                    zh.push_str(&format!("可传输 · {size} 字节 · {overwrite_zh}\n"));
                    en.push_str(&format!("Admissible · {size} bytes · {overwrite_en}\n"));
                    if let PreparedTarget::Directory(plan) = target {
                        zh.push_str(&format!(
                            "目录：{} 文件／{} 目录；执行前重新校验完整扫描；整棵目录非原子传输，失败或取消保留已完成项\n",
                            plan.files(),
                            plan.directories()
                        ));
                        en.push_str(&format!("Folder: {} files / {} folders; complete scan revalidated before execution; the folder transfer is not atomic and retains completed items after failure or cancellation\n", plan.files(), plan.directories()));
                    }
                }
                Err(message) => {
                    zh.push_str(&format!("拒绝 · {}\n", message.translations().0));
                    en.push_str(&format!("Rejected · {}\n", message.translations().1));
                }
            }
        }
        Message::new(zh, en)
    }
}

/// Each selected item gets an explicit outcome; a failed item is never silently
/// omitted. The total byte budget covers the exact approved subset.
pub(super) async fn prepare(
    sftp: &SftpSession,
    request: BatchRequest,
    stop: &AtomicBool,
) -> Result<BatchPlan, FileFailure> {
    if request.sources.is_empty() || request.sources.len() > MAX_QUEUED_TRANSFERS {
        return Err(FileFailure::Comparison(
            "batch selection must contain 1–32 items".into(),
        ));
    }
    let mut rows = Vec::with_capacity(request.sources.len());
    let mut destinations = BTreeSet::new();
    let mut total = 0_u64;
    let deadline = tokio::time::Instant::now() + PREPARATION_TIMEOUT;
    for source in request.sources {
        if stop.load(Ordering::Acquire) {
            return Err(FileFailure::CancelledBeforeStart);
        }
        // Portable case folding also rejects destination aliases on common
        // case-insensitive filesystems. Paths were constructed as one child.
        let key = match request.direction {
            TransferDirection::Upload => source.remote.to_lowercase(),
            TransferDirection::Download => source.local.to_string_lossy().to_lowercase(),
        };
        let prepared = if !source.supported {
            Err(Message::new(
                "链接、特殊类型、不可移植名称或未知类型不支持批量传输。",
                "Links, special files, nonportable names and unknown types are not admitted.",
            ))
        } else if !destinations.insert(key) {
            Err(Message::new(
                "重复或大小写冲突的目标；本项不会执行。",
                "Duplicate or case-alias destination; this entry will not run.",
            ))
        } else {
            let spec = TransferSpec {
                direction: request.direction,
                local: source.local.clone(),
                remote: source.remote.clone(),
            };
            let result = tokio::time::timeout_at(deadline, async {
                if source.directory {
                    sftp.plan_directory_transfer(spec)
                        .await
                        .map(PreparedTarget::Directory)
                } else {
                    sftp.plan_file_transfer(spec)
                        .await
                        .map(PreparedTarget::File)
                }
            })
            .await;
            match result {
                Ok(Ok(target))
                    if total
                        .checked_add(target.bytes())
                        .is_some_and(|next| next <= MAX_BATCH_BYTES) =>
                {
                    total += target.bytes();
                    Ok(target)
                }
                Ok(Ok(_)) => Err(Message::new(
                    "加入本项将超过批量 16 GiB 预算。",
                    "This entry would exceed the batch's 16 GiB budget.",
                )),
                Ok(Err(error)) => Err(Message::detail(
                    "只读准备拒绝",
                    "Read-only preparation refused",
                    error,
                )),
                Err(_) => Err(Message::new(
                    "批量只读准备超过 30 秒预算；请重新准备。",
                    "The batch's 30-second read-only preparation budget expired; prepare again.",
                )),
            }
        };
        rows.push(BatchRow { source, prepared });
    }
    Ok(BatchPlan {
        id: uuid::Uuid::new_v4(),
        binding: request.binding,
        direction: request.direction,
        rows,
    })
}

impl FilesPanel {
    pub(super) fn pending_batch_id(&self) -> Option<uuid::Uuid> {
        match &self.pending {
            Some((_, Operation::TransferBatch(plan))) => Some(plan.id),
            _ => None,
        }
    }
    fn batch_binding(&self) -> Option<BatchBinding> {
        Some(BatchBinding {
            session: self.session_token,
            revision: self.browser_review_revision,
            remote: self.directory.clone()?,
            local: self.local_browser.listing.as_ref()?.directory.clone(),
            local_selection: self.local_browser.selection.paths().cloned().collect(),
            remote_selection: self.remote_selection.paths().cloned().collect(),
        })
    }
    pub(super) fn request_batch(
        &mut self,
        direction: TransferDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suspended || self.operation_id.is_some() || self.pending.is_some() {
            return;
        }
        let Some(binding) = self.batch_binding() else {
            return;
        };
        let mut sources = Vec::new();
        match direction {
            TransferDirection::Upload => {
                let Some(listing) = &self.local_browser.listing else {
                    return;
                };
                for path in self.local_browser.selection.paths() {
                    let Some(entry) = listing.entries.iter().find(|entry| &entry.path == path)
                    else {
                        return;
                    };
                    let name = entry.name.to_str().unwrap_or_default();
                    let portable = browser::local_destination(&listing.directory, name).is_some();
                    let remote = format!("{}/{}", binding.remote.trim_end_matches('/'), name);
                    sources.push(BatchSource {
                        name: entry.name.to_string_lossy().into_owned(),
                        local: entry.path.clone(),
                        remote,
                        directory: entry.kind == local_catalog::LocalEntryKind::Directory,
                        supported: portable
                            && entry.path == listing.directory.join(&entry.name)
                            && matches!(
                                entry.kind,
                                local_catalog::LocalEntryKind::File
                                    | local_catalog::LocalEntryKind::Directory
                            ),
                    });
                }
            }
            TransferDirection::Download => {
                for path in self.remote_selection.paths() {
                    let Some(entry) = self.entries.iter().find(|entry| &entry.path == path) else {
                        return;
                    };
                    let destination = browser::local_destination(&binding.local, &entry.name);
                    sources.push(BatchSource {
                        name: entry.name.clone(),
                        local: destination.clone().unwrap_or_else(|| binding.local.clone()),
                        remote: entry.path.clone(),
                        directory: entry.is_directory,
                        supported: destination.is_some()
                            && entry.path
                                == format!(
                                    "{}/{}",
                                    binding.remote.trim_end_matches('/'),
                                    entry.name
                                )
                            && !entry.is_symlink
                            && entry
                                .permissions
                                .is_some_and(|mode| matches!(mode & 0o170000, 0o100000 | 0o040000)),
                    });
                }
            }
        }
        if sources.is_empty() {
            return;
        }
        self.run(
            Operation::PlanBatch(BatchRequest {
                binding,
                direction,
                sources,
            }),
            window,
            cx,
        );
    }
    pub(super) fn run_batch(
        &mut self,
        plan: BatchPlan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suspended
            || self.session.is_none()
            || self.batch_binding().as_ref() != Some(&plan.binding)
        {
            self.status = Message::new(
                "批量审核已失效；会话、目录或选择已变化，请重新准备。",
                "Batch review expired; the session, folder or selection changed. Prepare again.",
            );
            cx.notify();
            return;
        }
        let count = plan.ready_count();
        if count == 0 || !self.batch_queue_has_capacity(count) {
            self.status = Message::new(
                "批量未加入队列：没有可传输项或队列空间不足；请重新审核。",
                "Batch was not queued: no admissible items or insufficient queue capacity. Review again.",
            );
            cx.notify();
            return;
        }
        // One UI transaction admits every approved target into the bounded job
        // ledger before callbacks can change selection or session state. Each
        // background owner still rechecks its immutable source and destination.
        for row in plan.rows {
            if let Ok(target) = row.prepared {
                self.run_queued_transfer(target.operation(), window, cx);
            }
        }
        self.status = Message::new(
            format!("已审核加入 {count} 个精确目标；暂停、取消及结果见传输队列。"),
            format!(
                "Queued {count} exact reviewed targets; pause, cancel and inspect results in the transfer queue."
            ),
        );
        cx.notify();
    }
    pub(super) fn batch_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let local = self.local_browser.selection.len();
        let remote = self.remote_selection.len();
        div()
            .id("batch-transfer-controls")
            .test_support()
            .flex_shrink_0()
            .min_h(px(30.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.canvas))
            .child(
                div()
                    .text_color(rgb(visual.muted))
                    .child(match crate::i18n::language(cx) {
                        keelshell_core::Language::ZhCn => {
                            format!("所选：本地 {local} · 远程 {remote}（每侧最多32项）")
                        }
                        keelshell_core::Language::En => {
                            format!("Selected: local {local} · remote {remote} (32 per side)")
                        }
                    }),
            )
            .child(
                Button::new("clear-file-selection")
                    .ghost()
                    .compact()
                    .label(t(cx, "清空选择", "Clear selection"))
                    .disabled(local + remote == 0)
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.local_browser.selection.clear();
                        panel.local_browser.selected = None;
                        panel.remote_selection.clear();
                        panel.selected = None;
                        panel.withdraw_browser_review();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("batch-upload")
                    .ghost()
                    .compact()
                    .icon(IconName::Upload)
                    .label(t(cx, "审核批量上传", "Review batch upload"))
                    .disabled(
                        self.suspended
                            || local == 0
                            || self.local_browser.listing.is_none()
                            || self.directory.is_none()
                            || self.operation_id.is_some()
                            || self.pending.is_some(),
                    )
                    .on_click(cx.listener(|panel, _, window, cx| {
                        panel.request_batch(TransferDirection::Upload, window, cx)
                    })),
            )
            .child(
                Button::new("batch-download")
                    .ghost()
                    .compact()
                    .icon(IconName::Download)
                    .label(t(cx, "审核批量下载", "Review batch download"))
                    .disabled(
                        self.suspended
                            || remote == 0
                            || self.local_browser.listing.is_none()
                            || self.operation_id.is_some()
                            || self.pending.is_some(),
                    )
                    .on_click(cx.listener(|panel, _, window, cx| {
                        panel.request_batch(TransferDirection::Download, window, cx)
                    })),
            )
            .child(div().text_color(rgb(visual.muted)).child(t(
                cx,
                "勾选／Ctrl或⌘切换 · Shift范围 · 只准备不执行",
                "Checkbox / Ctrl or ⌘ toggle · Shift range · Prepare only",
            )))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_server::{Checked, Server};
    use super::*;

    #[::core::prelude::v1::test]
    fn duplicate_destinations_have_a_complete_rejection_row_and_zero_writes() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .checked("owned batch review runtime");
        let server = Server::new(&runtime);
        let session = server.connect(&runtime);
        let local = tempfile::tempdir().checked("owned metadata-only sources");
        let root = local
            .path()
            .canonicalize()
            .checked("absolute source directory");
        let source = root.join("source.txt");
        std::fs::write(&source, b"review bytes").checked("seed batch source");
        let request = BatchRequest {
            binding: BatchBinding {
                session: uuid::Uuid::new_v4(),
                revision: uuid::Uuid::new_v4(),
                remote: "/".into(),
                local: root,
                local_selection: vec![source.clone()],
                remote_selection: vec![],
            },
            direction: TransferDirection::Upload,
            sources: vec![
                BatchSource {
                    name: "first".into(),
                    local: source.clone(),
                    remote: "/target.txt".into(),
                    directory: false,
                    supported: true,
                },
                BatchSource {
                    name: "alias".into(),
                    local: source,
                    remote: "/TARGET.txt".into(),
                    directory: false,
                    supported: true,
                },
            ],
        };
        runtime.block_on(async {
            let sftp = session.sftp().await.checked("batch preparation SFTP");
            let plan = prepare(&sftp, request, &AtomicBool::new(false))
                .await
                .checked("prepare both selected targets");
            assert_eq!(plan.ready_count(), 1);
            assert_eq!(plan.rows.len(), 2);
            assert!(plan.rows[1].prepared.is_err());
            assert_eq!(server.filesystem.atomic_writes_started(), 0);
            assert!(
                sftp.inspect_entry("/target.txt")
                    .await
                    .checked("no premature target")
                    .is_none()
            );
            drop(plan);
            sftp.close().await.checked("close review SFTP");
            session.close().await.checked("close review SSH");
        });
    }
}
