//! SFTP browser and bounded UTF-8 editor. Mutations are explicit reviewed actions.
use crate::i18n::{Message, t};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputState, Textarea, TextareaState},
    },
    *,
};
use keelshell_core::{
    DirectoryCompareReport, DirectoryEntryKind, DirectoryEntrySnapshot, DirectoryEntryStatus,
    DirectorySyncDeletePolicy, DirectorySyncDirection, DirectorySyncPlan, diff_utf8,
};
use keelshell_session::{
    SessionError, SshSession,
    sftp::{
        DirectoryResumePlan, DirectoryTransferPlan, FileResumePlan, RegularFileSnapshot,
        RemoteEntry, SftpSession, TransferDirection, TransferEvent, TransferQuarantineReview,
        TransferSpec,
    },
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

mod merge;
mod parallel;
mod sync;
mod transfer;
mod view;
mod worker;
use transfer::{TransferPhase, TransferStatus, TransferUpdate};
use worker::operate;

use gpui_kit::assets::IconName;
#[derive(Clone)]
enum IsolationTarget {
    Remote(String),
    Local(PathBuf),
}
#[derive(Clone)]
enum Operation {
    InspectFileQuarantine(IsolationTarget),
    InspectQuarantine(TransferSpec),
    AcknowledgeQuarantine(TransferQuarantineReview),
    List(String),
    Read(RemoteEntry),
    ReadMerge {
        path: String,
        base: Vec<u8>,
        draft: String,
    },
    ApplyPatchToDraft {
        path: String,
        draft: String,
        patch: String,
    },
    Mkdir(String),
    Rename(String, String),
    Delete(RemoteEntry),
    SetPermissions(RemoteEntry, u32),
    Upload(PathBuf, String),
    Download(String, PathBuf),
    PlanDirectory(TransferSpec),
    TransferDirectory(DirectoryTransferPlan),
    PlanResume(TransferSpec, bool),
    ResumeFile(FileResumePlan),
    ResumeDirectory(DirectoryResumePlan),
    Compare(PathBuf, String),
    PlanDirectorySync(
        PathBuf,
        String,
        DirectorySyncDirection,
        DirectorySyncDeletePolicy,
    ),
    ApplyDirectorySync(Box<DirectorySyncComparison>, sync::journal::SharedJournal),
    Save {
        path: String,
        reviewed: RegularFileSnapshot,
        content: Vec<u8>,
    },
}
enum Outcome {
    QuarantineInspected(TransferQuarantineReview),
    Listed(String, Vec<RemoteEntry>),
    Read(RegularFileSnapshot),
    Saved(RegularFileSnapshot),
    MergeLoaded {
        snapshot: RegularFileSnapshot,
        plan: keelshell_core::TextMergePlan,
        base: Vec<u8>,
        draft: String,
    },
    PatchApplied(String, String, String),
    Done(Message),
    PlannedDirectory(DirectoryTransferPlan),
    PlannedFileResume(FileResumePlan),
    PlannedDirectoryResume(DirectoryResumePlan),
    Compared(DirectoryComparison),
    DirectorySyncPlanned(DirectorySyncComparison),
}

enum WorkerMessage {
    Progress(Message),
    Transfer(TransferUpdate),
    Result(Box<Result<Outcome, FileFailure>>),
}

pub struct FilesPanel {
    session: Option<SshSession>,
    /// Changes whenever this panel is detached from its authenticated host.
    /// Recovery proposals carry this token so a stale action cannot cross a
    /// reconnect/session boundary.
    session_token: uuid::Uuid,
    suspended: bool,
    host: String,
    runtime: Arc<tokio::runtime::Runtime>,
    // The input is a navigation draft; mutations use only this canonical directory.
    directory: Option<String>,
    path: Entity<InputState>,
    name: Entity<InputState>,
    mode: Entity<InputState>,
    local: Entity<InputState>,
    editor: Entity<TextareaState>,
    confirmation_scroll: ScrollHandle,
    tools_scroll: ScrollHandle,
    transfer_list_scroll: ScrollHandle,
    entries: Vec<RemoteEntry>,
    selected: Option<RemoteEntry>,
    editing: Option<(String, Vec<u8>)>,
    editor_snapshot: Option<RegularFileSnapshot>,
    merge_review: Option<merge::MergeReview>,
    patch: Entity<TextareaState>,
    patch_visible: bool,
    text_review_scroll: ScrollHandle,
    diff_preview_scroll: ScrollHandle,
    diff_preview: Option<SharedString>,
    status: Message,
    busy: bool,
    pending: Option<(Message, Operation)>,
    operation_stop: Option<Arc<AtomicBool>>,
    operation_id: Option<uuid::Uuid>,
    transfer_pause: Option<tokio::sync::watch::Sender<bool>>,
    transfer: Option<TransferStatus>,
    recovery: Option<RecoveryCandidate>,
    isolation_target: Option<IsolationTarget>,
    resume_mode: bool,
    comparison: Option<DirectoryComparison>,
    sync_journal: Option<sync::journal::SharedJournal>,
    mirror_conflicts: Vec<keelshell_core::DirectoryMirrorConflict>,
    transfer_jobs: Vec<parallel::QueuedTransfer>,
    selected_transfer: Option<uuid::Uuid>,
    transfer_queue: Arc<tokio::sync::OnceCell<Arc<keelshell_session::sftp::TransferQueue>>>,
    queue_parallelism: Arc<std::sync::atomic::AtomicUsize>,
    // Tests retain setup admission in the actual entity, after queue ownership.
    #[cfg(test)]
    fixture_group: Option<Arc<fixture_group::FixtureGroup>>,
}

struct DirectoryComparison {
    local: PathBuf,
    remote: String,
    report: DirectoryCompareReport,
    sync_plan: Option<DirectorySyncPlan>,
}

#[derive(Clone)]
struct DirectorySyncComparison {
    local: PathBuf,
    remote: String,
    report: DirectoryCompareReport,
    plan: DirectorySyncPlan,
}

/// A failed transfer can offer a new read-only verification step, but never a
/// replay. The token binds the proposal to the live authenticated session that
/// produced the partial output.
struct RecoveryCandidate {
    session_token: uuid::Uuid,
    spec: TransferSpec,
    directory: bool,
}

fn recovery_is_available(
    candidate: Option<&RecoveryCandidate>,
    session_token: uuid::Uuid,
    suspended: bool,
    busy: bool,
    pending: bool,
    phase: Option<TransferPhase>,
) -> bool {
    !busy
        && !suspended
        && !pending
        && phase == Some(TransferPhase::Failed)
        && candidate.is_some_and(|candidate| candidate.session_token == session_token)
}

/// Guard the explicit recovery click against state that can change after the
/// card was rendered. A pending approval must remain untouched when the click
/// is rejected; the caller may still be waiting for the user to confirm it.
fn recovery_request_is_allowed(
    candidate: Option<&RecoveryCandidate>,
    session_token: uuid::Uuid,
    session_available: bool,
    suspended: bool,
    busy: bool,
    pending: bool,
    phase: Option<TransferPhase>,
) -> bool {
    session_available
        && recovery_is_available(candidate, session_token, suspended, busy, pending, phase)
}
impl Drop for FilesPanel {
    fn drop(&mut self) {
        self.cancel_transfers();
        if let Some(stop) = &self.operation_stop {
            stop.store(true, Ordering::Release);
        }
    }
}
fn field(placeholder: &str, value: &str, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| {
        let mut input = InputState::new(window, cx).placeholder(placeholder.to_owned());
        input.set_value(value.to_owned(), window, cx);
        input
    })
}

#[derive(Debug, PartialEq, Eq)]
enum PathError {
    DirectoryRequired,
    InvalidName,
}
impl PathError {
    fn message(&self) -> Message {
        match self {
            Self::DirectoryRequired => Message::new(
                "请先成功打开远程目录，再修改文件",
                "Open a remote directory successfully before changing files",
            ),
            Self::InvalidName => Message::new(
                "请输入不含路径分隔符或控制字符的单个文件名",
                "Enter one file or folder name without separators or control characters",
            ),
        }
    }
}

/// Join only a canonical absolute directory and one literal name, never a draft path.
fn child_path(directory: Option<&str>, name: &str) -> Result<String, PathError> {
    let directory = directory
        .filter(|path| path.starts_with('/') && !path.chars().any(char::is_control))
        .ok_or(PathError::DirectoryRequired)?;
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.contains(['/', '\\'])
        || name.chars().any(char::is_control)
    {
        return Err(PathError::InvalidName);
    }
    Ok(format!("{}/{}", directory.trim_end_matches('/'), name))
}

/// Parse the explicit POSIX mode input used by the reviewed permissions action.
/// Three digits are accepted as shorthand for a leading zero; no symbolic mode
/// expressions or shell syntax are interpreted.
fn parse_permissions_mode(value: &str) -> Result<u32, &'static str> {
    let value = value.trim();
    if !(3..=4).contains(&value.len()) || !value.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        return Err("enter a three- or four-digit octal mode between 0000 and 7777");
    }
    u32::from_str_radix(value, 8).map_err(|_| "enter a valid octal mode")
}

#[derive(Debug)]
enum FileFailure {
    Transport(String),
    IsolationBusy,
    IsolationUnknown,
    Cancelled,
    CancelledBeforeStart,
    CancellationUnconfirmed,
    OutcomeUncertain(String),
    InvalidEditor,
    TextEdit(keelshell_core::TextEditError),
    SavedReadback(String),
    Symlink,
    Cleanup,
    WorkerStopped,
    DirectoryTransfer {
        destination: String,
        error: Box<FileFailure>,
    },
    Comparison(String),
    MirrorConflicts(Vec<keelshell_core::DirectoryMirrorConflict>),
}

/// Return a terminal phase only for an operation that actually transferred
/// bytes. Read-only plans must leave an older failed transfer card untouched.
fn terminal_transfer_phase(
    is_transfer: bool,
    result: &Result<Outcome, FileFailure>,
) -> Option<TransferPhase> {
    if !is_transfer {
        return None;
    }
    Some(match result {
        Ok(_) => TransferPhase::Completed,
        Err(
            FileFailure::CancellationUnconfirmed
            | FileFailure::OutcomeUncertain(_)
            | FileFailure::WorkerStopped,
        ) => TransferPhase::Uncertain,
        Err(error) if error.is_cancelled() => TransferPhase::Cancelled,
        Err(_) => TransferPhase::Failed,
    })
}

impl From<SessionError> for FileFailure {
    fn from(error: SessionError) -> Self {
        match error {
            SessionError::MutationBusy => Self::IsolationBusy,
            SessionError::MutationQuarantined => Self::IsolationUnknown,
            SessionError::MutationUncertain => Self::OutcomeUncertain(error.to_string()),
            _ => Self::Transport(error.to_string()),
        }
    }
}
impl std::fmt::Display for FileFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IsolationBusy=>f.write_str("file target is owned by an active application mutation"),
            Self::IsolationUnknown=>f.write_str("file target is isolated by an unknown mutation; exact risk acknowledgement required"),
            Self::Transport(detail) => f.write_str(detail),
            Self::Cancelled => {
                f.write_str("File operation cancelled; remote outcome may be unknown")
            }
            Self::OutcomeUncertain(detail) => write!(f, "Transfer outcome unknown: {detail}"),
            Self::CancellationUnconfirmed => {
                f.write_str("Cancellation requested, but no terminal transfer result was received")
            }
            Self::CancelledBeforeStart => {
                f.write_str("File operation cancelled before SFTP initialization")
            }
            Self::InvalidEditor => f.write_str("Editor accepts UTF-8 files up to 1 MiB"),
            Self::TextEdit(error) => write!(f,"{error}"),
            Self::SavedReadback(error) => write!(f,"publication acknowledged but content readback failed: {error}"),
            Self::Symlink => f.write_str("Permission changes on symbolic links are disabled"),
            Self::Cleanup => f.write_str("Operation completed but SFTP cleanup failed"),
            Self::WorkerStopped => f.write_str("File worker stopped without a result"),
            Self::DirectoryTransfer { destination, error } => {
                write!(f, "{error}; inspect partial directory {destination}")
            }
            Self::MirrorConflicts(conflicts) => write!(f,"bounded mirror refused {} conflicts", conflicts.len()),
            Self::Comparison(detail) => write!(f, "Directory comparison failed: {detail}"),
        }
    }
}
impl FileFailure {
    fn is_cancelled(&self) -> bool {
        match self {
            Self::Cancelled | Self::CancelledBeforeStart => true,
            Self::DirectoryTransfer { error, .. } => error.is_cancelled(),
            _ => false,
        }
    }

    fn message(&self) -> Message {
        match self {
            Self::IsolationBusy => Message::new(
                "目标正由其它文件操作占用；请等待完成后重新审核。",
                "Another file operation owns this target; review again after it finishes.",
            ),
            Self::IsolationUnknown => Message::new(
                "目标存在未知写入结果，已隔离。普通保存或 MCP 提案确认不会解除隔离；请只读检查并显式审核风险。",
                "This target is isolated by an unknown mutation. Ordinary save or MCP approval cannot release it; inspect read-only and explicitly review its risk.",
            ),
            Self::MirrorConflicts(conflicts) => Message::new(
                format!(
                    "镜像计划已拒绝：{} 项冲突；请查看完整只读列表。没有写入或删除。",
                    conflicts.len()
                ),
                format!(
                    "Mirror refused: {} conflicts; inspect the complete read-only list. Nothing was written or deleted.",
                    conflicts.len()
                ),
            ),
            Self::Transport(detail) => Message::new(
                format!("文件操作失败：{detail}。远端结果可能未知，请检查后再重试。"),
                format!(
                    "File operation failed: {detail}. Remote completion may be unknown; inspect before retrying."
                ),
            ),
            Self::Cancelled => Message::new(
                "操作已取消；请先检查远端结果。临时文件或未完成的本地下载可能仍存在。",
                "Operation cancelled; inspect the remote result. A staged file or partial local download may remain.",
            ),
            Self::OutcomeUncertain(detail) => Message::detail(
                "传输结果未知，目标已隔离；重连不会解除隔离。请只读检查，再显式审核未知风险",
                "Transfer outcome unknown; destination isolated across reconnects. Inspect read-only, then explicitly review the unresolved risk",
                detail,
            ),
            Self::CancellationUnconfirmed => Message::new(
                "已请求取消，但尚未收到最终传输回执；结果未知，请检查目标后再操作。",
                "Cancellation requested, but the final transfer result is unknown; inspect the destination before another operation.",
            ),
            Self::CancelledBeforeStart => Message::new(
                "已在打开 SFTP 前取消操作",
                "Operation cancelled before opening SFTP",
            ),
            Self::InvalidEditor => Message::new(
                "编辑器仅支持不超过 1 MiB 的 UTF-8 文件",
                "The editor accepts UTF-8 files up to 1 MiB",
            ),
            Self::TextEdit(error) => Message::detail(
                "文本审核未完成，草稿已保留",
                "Text review incomplete; draft retained",
                error,
            ),
            Self::SavedReadback(error) => Message::detail(
                "已收到原子发布确认，但完整读回未通过；保留旧基线和草稿，请读取远端确认，不自动重试",
                "Atomic publication acknowledged, but full readback failed; old baseline and draft retained. Inspect the remote file; no automatic retry",
                error,
            ),
            Self::Symlink => Message::new(
                "为避免跟随链接误改目标，符号链接不支持修改权限",
                "Permission changes on symbolic links are disabled to avoid following a link",
            ),
            Self::Cleanup => Message::new(
                "操作已完成，但 SFTP 清理失败；请检查后再重试",
                "Operation completed, but SFTP cleanup failed; inspect before retrying",
            ),
            Self::DirectoryTransfer { destination, error } => Message::new(
                format!(
                    "目录传输未完成：{error}。请检查目标 {destination}；可能保留部分文件，不会自动清理。"
                ),
                format!(
                    "Directory transfer incomplete: {error}. Inspect {destination}; partial files may remain and are not removed automatically."
                ),
            ),
            Self::Comparison(detail) => Message::new(
                format!("目录比较失败：{detail}"),
                format!("Directory comparison failed: {detail}"),
            ),
            Self::WorkerStopped => Message::new(
                "文件工作线程未返回结果便结束",
                "File worker stopped without a result",
            ),
        }
    }
}

async fn cancellation(stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn transfer_progress_message(
    direction: TransferDirection,
    transferred: u64,
    total: Option<u64>,
) -> Message {
    let verb_zh = match direction {
        TransferDirection::Upload => "上传中",
        TransferDirection::Download => "下载中",
    };
    let verb_en = match direction {
        TransferDirection::Upload => "Uploading",
        TransferDirection::Download => "Downloading",
    };
    let progress_zh = total.map_or_else(
        || format!("{transferred} 字节"),
        |total| format!("{transferred} / {total} 字节"),
    );
    let progress_en = total.map_or_else(
        || format!("{transferred} bytes"),
        |total| format!("{transferred} / {total} bytes"),
    );
    Message::new(
        format!("{verb_zh}：{progress_zh}"),
        format!("{verb_en}: {progress_en}"),
    )
}

fn resume_progress_message(
    transferred: u64,
    total: Option<u64>,
    existing: u64,
    directory: bool,
) -> Message {
    let total = total.map_or_else(|| "—".to_owned(), |total| total.to_string());
    if directory {
        return Message::new(
            format!(
                "目录续传已确认 {transferred} / {total} 字节（含已复核前缀）；审核时原有 {existing} 字节"
            ),
            format!(
                "Folder continuation: {transferred} / {total} confirmed bytes (includes rechecked prefixes); {existing} existing bytes at review"
            ),
        );
    }
    let added = transferred.saturating_sub(existing);
    Message::new(
        format!("续传进度（含已验证部分）：{transferred} / {total} 字节；本次新增 {added} 字节"),
        format!(
            "Continuation progress (includes verified content): {transferred} / {total} bytes; {added} new bytes"
        ),
    )
}

fn send_worker_progress(progress: &mpsc::SyncSender<WorkerMessage>, message: Message) {
    // Progress is advisory and may be coalesced when the GPUI frame is busy;
    // the terminal result always uses the blocking sender below.
    let _ = progress.try_send(WorkerMessage::Progress(message));
}

impl FilesPanel {
    pub fn new(
        session: SshSession,
        host: String,
        runtime: Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        #[cfg(test)]
        let fixture_group = fixture_group::for_app(cx);
        let mut panel = Self {
            session: Some(session),
            session_token: uuid::Uuid::new_v4(),
            suspended: false,
            host,
            runtime,
            directory: None,
            path: field(t(cx, "远程目录", "Remote directory"), ".", window, cx),
            name: field(
                t(cx, "文件名 / 新建目录", "New name / folder"),
                "",
                window,
                cx,
            ),
            mode: field(
                t(cx, "权限（八进制）", "Permissions (octal)"),
                "",
                window,
                cx,
            ),
            local: field(t(cx, "本地传输路径", "Local transfer path"), "", window, cx),
            editor: cx.new(|cx| TextareaState::new(window, cx).rows(8)),
            confirmation_scroll: ScrollHandle::new(),
            tools_scroll: ScrollHandle::new(),
            transfer_list_scroll: ScrollHandle::new(),
            entries: Vec::new(),
            selected: None,
            editing: None,
            editor_snapshot: None,
            merge_review: None,
            patch: cx.new(|cx| TextareaState::new(window, cx)),
            patch_visible: false,
            text_review_scroll: ScrollHandle::new(),
            diff_preview_scroll: ScrollHandle::new(),
            diff_preview: None,
            status: Message::new("正在打开 SFTP…", "Opening SFTP…"),
            busy: false,
            pending: None,
            operation_stop: None,
            operation_id: None,
            transfer_pause: None,
            transfer: None,
            recovery: None,
            isolation_target: None,
            resume_mode: false,
            comparison: None,
            sync_journal: None,
            mirror_conflicts: Vec::new(),
            transfer_jobs: Vec::new(),
            selected_transfer: None,
            transfer_queue: Arc::new(tokio::sync::OnceCell::new()),
            queue_parallelism: Arc::new(std::sync::atomic::AtomicUsize::new(2)),
            #[cfg(test)]
            fixture_group,
        };
        panel.run(Operation::List(".".into()), window, cx);
        panel
    }
    /// Preserve local drafts and completed records while retiring remote capabilities.
    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        if self.suspended {
            return;
        }
        self.suspended = true;
        self.session = None;
        self.session_token = uuid::Uuid::new_v4();
        self.recovery = None;
        self.pending = None;
        self.cancel_transfers();
        self.cancel_active(cx);
        cx.notify();
    }

    /// Explicitly selected, loaded directory of this live file panel; never a shell cwd.
    pub(crate) fn completion_directory(&self) -> Option<&str> {
        (!self.suspended)
            .then_some(self.directory.as_deref())
            .flatten()
    }

    /// Whether dropping this snapshot would lose an unsaved editor draft.
    pub fn has_unsaved_draft(&self, cx: &App) -> bool {
        let content = self.editor.read(cx).value();
        self.editing
            .as_ref()
            .map_or(!content.is_empty(), |(_, original)| {
                content.as_bytes() != original
            })
    }

    /// Immutable in-memory version for explicit archive-discard consent.
    pub fn draft_snapshot(&self, cx: &App) -> SharedString {
        self.editor.read(cx).value()
    }

    #[cfg(test)]
    pub(crate) fn seed_draft_for_test(
        &mut self,
        path: &str,
        original: &str,
        draft: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editing = Some((path.to_owned(), original.as_bytes().to_vec()));
        self.editor.update(cx, |editor, cx| {
            editor.set_value(draft.to_owned(), window, cx)
        });
    }

    /// Retain live transport/editor state while updating only translated hints.
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (input, zh, en) in [
            (&self.path, "远程目录", "Remote directory"),
            (&self.name, "文件名 / 新建目录", "New name / folder"),
            (&self.mode, "权限（八进制）", "Permissions (octal)"),
            (&self.local, "本地传输路径", "Local transfer path"),
        ] {
            let placeholder = t(cx, zh, en);
            input.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
        cx.notify();
    }
    fn run(&mut self, operation: Operation, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone().filter(|_| !self.suspended) else {
            return;
        };
        if self.operation_id.is_some() || (self.busy && !operation.can_run_beside_transfers()) {
            self.status = Message::new(
                "请等待或取消当前文件操作",
                "Wait for or cancel the active file operation",
            );
            cx.notify();
            return;
        }
        if operation.transfer_status().is_some() {
            self.run_queued_transfer(operation, window, cx);
            return;
        }
        if let Some(target) = operation.isolation_target() {
            self.isolation_target = Some(target);
        }
        let recovery_spec = operation.recovery_spec();
        let is_transfer = recovery_spec.is_some();
        if let Some((spec, directory)) = recovery_spec {
            self.recovery = Some(RecoveryCandidate {
                session_token: self.session_token,
                spec,
                directory,
            });
        } else if !matches!(
            &operation,
            Operation::PlanResume(..)
                | Operation::InspectFileQuarantine(_)
                | Operation::InspectQuarantine(_)
                | Operation::AcknowledgeQuarantine(_)
        ) {
            // A browse/edit/mutation action is a new workflow; do not retain
            // a hidden recovery proposal after the user leaves its card.
            self.recovery = None;
        }
        if !matches!(
            &operation,
            Operation::Compare(..) | Operation::PlanDirectorySync(..)
        ) {
            self.comparison = None;
        }
        if let Operation::List(path) = &operation
            && (path.trim().is_empty() || path.chars().any(char::is_control))
        {
            self.status = Message::new(
                "请输入不含控制字符的有效远程目录",
                "Enter a nonempty remote directory without control characters",
            );
            cx.notify();
            return;
        }
        if matches!(
            &operation,
            Operation::Compare(..) | Operation::PlanDirectorySync(..)
        ) {
            self.mirror_conflicts.clear();
        }
        if let Operation::ApplyDirectorySync(_, journal) = &operation {
            self.sync_journal = Some(journal.clone());
        }
        self.busy = true;
        self.pending = None;
        let operation_id = uuid::Uuid::new_v4();
        self.operation_id = Some(operation_id);
        if let Some(status) = operation.transfer_status() {
            self.transfer = Some(status);
        } else if !matches!(
            &operation,
            Operation::PlanResume(..)
                | Operation::InspectFileQuarantine(_)
                | Operation::InspectQuarantine(_)
                | Operation::AcknowledgeQuarantine(_)
        ) {
            // Keep a failed transfer card visible while a recovery plan is
            // being checked; browsing or unrelated operations retire it.
            self.transfer = None;
        }
        self.status = match &operation {
            Operation::PlanDirectory(_) => Message::new(
                "正在扫描目录，完成后需确认…",
                "Scanning directory; confirmation is required before transfer…",
            ),
            Operation::TransferDirectory(_) => Message::new(
                "正在复核已审核的目录…",
                "Rechecking the reviewed directory…",
            ),
            Operation::PlanResume(..) => Message::new(
                "正在只读校验源与已有部分内容，完成后需审核确认…",
                "Verifying the source and existing content without writing; review is required…",
            ),
            Operation::ResumeFile(_) | Operation::ResumeDirectory(_) => Message::new(
                "正在复核已审核的续传内容…",
                "Rechecking the reviewed continuation…",
            ),
            Operation::Compare(_, _) => Message::new(
                "正在读取两侧目录快照（只读）…",
                "Reading both directory snapshots (read-only)…",
            ),
            Operation::PlanDirectorySync(..) => Message::new(
                "正在读取两侧文件内容并生成审核计划（只读）…",
                "Reading both sides and generating a reviewable plan (read-only)…",
            ),
            Operation::ApplyDirectorySync(..) => Message::new(
                "正在复核审核快照并同步目录…",
                "Rechecking reviewed snapshots and synchronizing directories…",
            ),
            _ => Message::new("正在处理…", "Working…"),
        };
        let editor_before = self.editor.read(cx).value().to_string();
        let review_only = matches!(
            &operation,
            Operation::InspectFileQuarantine(_)
                | Operation::InspectQuarantine(_)
                | Operation::PlanResume(..)
                | Operation::PlanDirectory(_)
                | Operation::PlanDirectorySync(..)
        );
        let navigation_before = self.path.read(cx).value().to_string();
        let runtime = self.runtime.clone();
        let stop = Arc::new(AtomicBool::new(false));
        self.operation_stop = Some(stop.clone());
        let worker_stop = stop.clone();
        let (pause, pause_receiver) = tokio::sync::watch::channel(false);
        self.transfer_pause = self.transfer.as_ref().map(|_| pause);
        let (sender, receiver) = mpsc::sync_channel(16);
        #[cfg(test)]
        let fixture_group = self.fixture_group.clone();
        if let Err(error) =
            crate::terminal::spawn_transport_worker("keelshell-sftp", stop, move || {
                #[cfg(test)]
                let _fixture_group = fixture_group;
                let result = runtime.block_on(operate(
                    session,
                    operation,
                    worker_stop,
                    pause_receiver,
                    &sender,
                ));
                let completion = result.as_ref().map(|_| ()).map_err(ToString::to_string);
                let _ = sender.send(WorkerMessage::Result(Box::new(result)));
                completion
            })
        {
            // A failed foreground start cannot release running queue jobs' guard.
            self.busy = self.has_active_transfers();
            self.operation_stop = None;
            self.operation_id = None;
            self.transfer_pause = None;
            if is_transfer {
                self.recovery = None;
            }
            if is_transfer && let Some(transfer) = &mut self.transfer {
                transfer.phase = TransferPhase::Failed;
            }
            self.status =
                Message::detail("无法启动文件工作线程", "Unable to start file worker", error);
            cx.notify();
            return;
        }
        let executor = cx.background_executor().clone();
        cx.spawn_in(window,async move |this,cx| {
            let result = loop {
                match receiver.try_recv() {
                    Ok(WorkerMessage::Result(result)) => break *result,
                    Ok(WorkerMessage::Progress(message)) => {
                        if this
                            .update_in(cx, |view, _, cx| {
                            if view.operation_id != Some(operation_id) { return; }
                            if !view.suspended && !view.transfer.as_ref().is_some_and(|state| matches!(state.phase,TransferPhase::Pausing|TransferPhase::Paused|TransferPhase::Resuming|TransferPhase::Cancelling)) {
                                view.status = message;
                            }
                            cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                    Ok(WorkerMessage::Transfer(event)) => {
                        if this.update_in(cx, |view, _, cx| {
                            if view.operation_id != Some(operation_id) { return; }
                            if let Some(transfer) = &mut view.transfer {
                                transfer.update(event);
                                if transfer.phase != TransferPhase::Cancelling {
                                    match event {
                                        TransferUpdate::Paused(..) => view.status = Message::new("传输已暂停；继续将使用原源路径、目标和 SSH 会话。", "Transfer paused; continuing preserves its source, destination and SSH session."),
                                        TransferUpdate::Resumed(..) => view.status = Message::new("正在继续原传输…", "Continuing the original transfer…"),
                                        _ => {}
                                    }
                                }
                            }
                            cx.notify();
                        }).is_err() { return; }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        break Err(FileFailure::WorkerStopped)
                    }
                    Err(mpsc::TryRecvError::Empty) => {},
                }
                executor.timer(Duration::from_millis(16)).await;
                if this.update_in(cx,|_,_,_|()).is_err() { return; }
            };
            let _ = this.update_in(cx,|view,window,cx| {
                if view.operation_id != Some(operation_id) { return; }
                view.busy = view.has_active_transfers();
                view.operation_stop = None;
                view.operation_id = None;
                view.transfer_pause = None;
                if let Some(phase) = terminal_transfer_phase(is_transfer, &result)
                    && let Some(transfer) = &mut view.transfer
                {
                    transfer.phase = phase;
                }
                if view.suspended {
                    // A late worker may acknowledge a write, but may not replace the
                    // archived editor, its original baseline, or an approved plan.
                    view.status = match &result {
                        Ok(_) => Message::new("旧会话任务已完成；保留归档内容。", "Previous session task completed; archived contents retained."),
                        Err(error) => Message::detail("旧会话任务结束", "Previous session task ended", error),
                    };
                    cx.notify();
                    return;
                }
                if let Err(FileFailure::MirrorConflicts(conflicts)) = &result { view.mirror_conflicts = conflicts.clone(); }
                let succeeded = result.is_ok();
                match result {
                    Ok(Outcome::Listed(path,entries)) => {
                        if view.path.read(cx).value().as_ref() == navigation_before.as_str() {
                            view.path.update(cx,|input,cx|input.set_value(path.clone(),window,cx));
                        }
                        view.directory = Some(path);
                        view.status = Message::new(format!("共 {} 项",entries.len()),format!("{} entries",entries.len()));
                        view.entries = entries;
                        view.selected = None;
                    }
                    Ok(Outcome::Read(snapshot)) => {
                        let path = snapshot.entry.path.clone();
                        let content = snapshot.content.clone();
                        if view.editor.read(cx).value().as_ref() == editor_before.as_str() {
                            view.editor.update(cx,|input,cx|input.set_value(String::from_utf8_lossy(&content).into_owned(),window,cx));
                            view.status = Message::new(format!("正在编辑 {path} · {} 字节",content.len()),format!("Editing {path} · {} bytes",content.len()));
                            view.editing = Some((path,content));
                            view.editor_snapshot = Some(snapshot);
                            view.merge_review = None;
                            view.diff_preview = None;
                        } else {
                            view.status = Message::new("加载期间检测到新编辑，已保留当前内容；请在准备好后重新打开文件。", "Your edits changed while loading; preserved the editor. Open the file again when ready.");
                        }
                    }
                    Ok(Outcome::Saved(snapshot)) => {
                        let path = snapshot.entry.path.clone();
                        let content = snapshot.content.clone();
                        if view.editing.as_ref().is_some_and(|(current,_)| current == &path) {
                            let dirty = view.editor.read(cx).value().as_bytes() != content;
                            view.editing = Some((path,content));
                            view.editor_snapshot = Some(snapshot);
                            view.merge_review = None;
                            view.diff_preview = None;
                            view.status = if dirty { Message::new("已保存审核版本；后续编辑尚未保存", "Saved the reviewed version; newer edits remain unsaved") } else { Message::new("远程文件已原子保存并完整读回", "Remote file atomically saved and fully read back") };
                        }
                    }
                    Ok(Outcome::MergeLoaded {snapshot, plan, base, draft}) => {
                        view.receive_merge(snapshot, plan, base, draft, window, cx);
                    }
                    Ok(Outcome::PatchApplied(path, original, result)) => {
                        if view.editing.as_ref().is_some_and(|(current,_)| current == &path)
                            && view.editor.read(cx).value().as_ref() == original.as_str() {
                            view.editor.update(cx, |input,cx|input.set_value(result,window,cx));
                            view.merge_review = None;
                            view.diff_preview = None;
                            view.status = Message::new("差异已精确应用到草稿；远端未写入，请审核最终全文。", "Patch applied exactly to the draft; remote unchanged. Review the complete result.");
                        } else { view.status = Message::new("解析期间草稿已变化，保留当前草稿；请重新预览差异。", "Draft changed during patch parsing; preserved it. Preview the patch again."); }
                    }
                    Ok(Outcome::Done(message)) => view.status = message,
                    Ok(Outcome::QuarantineInspected(review)) => {
                        view.status = Message::new("只读检查完成；请审核解除隔离的未知风险", "Read-only inspection complete; review the risk before releasing isolation");
                        view.confirmation_scroll.set_offset(point(px(0.), px(0.)));
                        view.pending = Some((parallel::quarantine_review_message(&review), Operation::AcknowledgeQuarantine(review)));
                    }
                    Ok(Outcome::PlannedDirectory(plan)) => {
                        view.status = Message::new("扫描完成，请审核目录传输", "Scan complete; review the directory transfer");
                        view.confirmation_scroll.set_offset(point(px(0.), px(0.)));
                        view.pending = Some((directory_review_message(&plan), Operation::TransferDirectory(plan)));
                    }
                    Ok(Outcome::PlannedFileResume(plan)) => {
                        view.status = Message::new("部分内容校验通过，请审核续传", "Existing content verified; review continuation");
                        view.confirmation_scroll.set_offset(point(px(0.), px(0.)));
                        view.pending = Some((resume_review_message(plan.direction(), plan.local_path(), plan.remote_path(), plan.bytes(), plan.existing_bytes(), None), Operation::ResumeFile(plan)));
                    }
                    Ok(Outcome::PlannedDirectoryResume(plan)) => {
                        view.status = Message::new("部分目录校验通过，请审核续传", "Existing tree verified; review continuation");
                        view.confirmation_scroll.set_offset(point(px(0.), px(0.)));
                        view.pending = Some((resume_review_message(plan.direction(), plan.local_path(), plan.remote_path(), plan.bytes(), plan.existing_bytes(), Some((plan.files(),plan.directories()))), Operation::ResumeDirectory(plan)));
                    }
                    Ok(Outcome::Compared(comparison)) => {
                        let review_count = comparison.report.review_count();
                        view.comparison = Some(DirectoryComparison {
                            local: comparison.local,
                            remote: comparison.remote,
                            report: comparison.report,
                            sync_plan: None,
                        });
                        view.status = if review_count == 0 {
                            Message::new("两侧目录已按可用元数据匹配", "The directories match on available metadata")
                        } else {
                            Message::new(
                                format!("目录比较完成：{review_count} 项需要审核"),
                                format!("Directory comparison complete: {review_count} entries need review"),
                            )
                        };
                    }
                    Ok(Outcome::DirectorySyncPlanned(comparison)) => {
                        let operation_count = comparison.plan.operation_count();
                        view.comparison = Some(DirectoryComparison {
                            local: comparison.local,
                            remote: comparison.remote,
                            report: comparison.report,
                            sync_plan: Some(comparison.plan),
                        });
                        view.status = Message::new(
                            format!("内容校验完成：已生成 {operation_count} 项审核计划；当前仅预览，不会写入"),
                            format!("Content verification complete: {operation_count} reviewed operations; preview only, nothing was written"),
                        );
                    }
                    Err(error) => view.status = if review_only {
                        Message::detail("审核未完成，尚未开始传输", "Review incomplete; transfer not started", error)
                    } else { error.message() },
                }
                if is_transfer && succeeded {
                    // A completed transfer has no recovery action. Failed or
                    // cancelled transfers retain the proposal for an explicit
                    // user-triggered verification step.
                    view.recovery = None;
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn remote_child(&mut self, name: &str) -> Option<String> {
        match child_path(self.directory.as_deref(), name) {
            Ok(path) => Some(path),
            Err(error) => {
                self.status = error.message();
                None
            }
        }
    }
    fn new_remote_path(&mut self, cx: &Context<Self>) -> Option<String> {
        let name = self.name.read(cx).value().trim().to_owned();
        self.remote_child(&name)
    }
    fn request_read(&mut self, entry: RemoteEntry, window: &mut Window, cx: &mut Context<Self>) {
        let dirty = self
            .editing
            .as_ref()
            .is_some_and(|(_, original)| self.editor.read(cx).value().as_bytes() != original);
        if dirty {
            self.confirm(
                Message::new(
                    format!("放弃未保存的编辑并打开 {}？", entry.path),
                    format!("Discard unsaved edits and open {}?", entry.path),
                ),
                Operation::Read(entry),
                cx,
            );
        } else {
            self.run(Operation::Read(entry), window, cx);
        }
    }
    fn confirm(&mut self, message: Message, operation: Operation, cx: &mut Context<Self>) {
        if self.suspended {
            return;
        }
        if self.operation_id.is_some() || (self.busy && !operation.can_run_beside_transfers()) {
            self.status = Message::new(
                "请等待或取消当前文件操作",
                "Wait for or cancel the active file operation",
            );
        } else {
            self.confirmation_scroll.set_offset(point(px(0.), px(0.)));
            self.pending = Some((message, operation));
        }
        cx.notify();
    }
    fn execute_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended {
            return;
        }
        if self.operation_id.is_some()
            || (self.busy
                && self
                    .pending
                    .as_ref()
                    .is_none_or(|(_, operation)| !operation.can_run_beside_transfers()))
        {
            return;
        }
        if let Some((_, Operation::Save { content, .. })) = &self.pending
            && self.editor.read(cx).value().as_bytes() != content
        {
            self.pending = None;
            self.status = Message::new(
                "审核后内容已修改，请重新审核再保存。",
                "Editor changed after review. Review the new contents before saving.",
            );
            cx.notify();
            return;
        }
        if let Some((_, operation)) = self.pending.take() {
            self.run(operation, window, cx);
        }
    }

    fn request_sync_review(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.suspended || self.pending.is_some() {
            return;
        }
        let Some(comparison) = &self.comparison else {
            return;
        };
        let Some(plan) = &comparison.sync_plan else {
            return;
        };
        let review = DirectorySyncComparison {
            local: comparison.local.clone(),
            remote: comparison.remote.clone(),
            report: comparison.report.clone(),
            plan: plan.clone(),
        };
        match sync::review_message(&review) {
            Ok(message) => {
                let journal = Arc::new(std::sync::Mutex::new(sync::journal::Journal::new(
                    &review.plan,
                )));
                self.confirm(
                    message,
                    Operation::ApplyDirectorySync(Box::new(review), journal),
                    cx,
                )
            }
            Err(error) => {
                self.status = error.message();
                cx.notify();
            }
        }
    }

    /// Build a bounded, read-only unified diff for the current remote-file draft.
    /// The preview never sends data or mutates the remote file; saving still requires
    /// the existing explicit review confirmation.
    fn toggle_diff_preview(&mut self, cx: &mut Context<Self>) {
        if self.diff_preview.is_some() {
            self.diff_preview = None;
            self.status = Message::new("已隐藏差异预览", "Diff preview hidden");
            cx.notify();
            return;
        }
        let Some((path, original)) = self.editing.as_ref() else {
            self.status = Message::new("请先打开远程文件", "Open a remote file first");
            cx.notify();
            return;
        };
        let draft = self.editor.read(cx).value();
        self.diff_preview_scroll.set_offset(point(px(0.), px(0.)));
        match diff_utf8(original, draft.as_bytes()) {
            Ok(diff) => {
                match diff.render(&format!("{path} (remote)"), &format!("{path} (draft)")) {
                    Ok(rendered) if rendered.is_empty() && original == draft.as_bytes() => {
                        self.diff_preview = Some("（当前草稿与读取的基线字节相同）\n(The draft is byte-identical to the captured baseline.)".into());
                        self.status = Message::new(
                            "当前草稿与读取的基线字节相同",
                            "The draft is byte-identical to the captured baseline",
                        );
                    }
                    Ok(rendered) if rendered.is_empty() => {
                        self.diff_preview = Some("（逻辑行相同，字节仍不同；此预览将 CRLF/LF 视为相同。请在保存审核中核对最终全文。）\n(Logical lines match, but bytes still differ; this preview treats CRLF/LF equally. Review the complete final text before saving.)".into());
                        self.status = Message::new(
                            "逻辑行相同，字节仍不同；请核对最终全文",
                            "Logical lines match, but bytes still differ; review the complete final text",
                        );
                    }
                    Ok(rendered) if rendered.len() <= 128 * 1024 => {
                        self.diff_preview = Some(format!("逻辑行预览（CRLF/LF 视为相同）；不是可直接应用的精确字节补丁。\nLogical-line preview (CRLF/LF treated equally); not an exact byte patch.\n{rendered}").into());
                        self.status = Message::new(
                            "差异已生成，请核对后再保存",
                            "Diff generated; review it before saving",
                        );
                    }
                    Ok(_) => {
                        self.status = Message::new(
                            "差异预览超过 128 KiB，请缩小编辑范围后再查看",
                            "Diff preview exceeds 128 KiB; narrow the edit before viewing",
                        );
                    }
                    Err(error) => {
                        self.status = Message::new(
                            format!("无法生成差异：{error}"),
                            format!("Unable to generate diff: {error}"),
                        );
                    }
                }
            }
            Err(error) => {
                self.status = Message::new(
                    format!("无法生成差异：{error}"),
                    format!("Unable to generate diff: {error}"),
                );
            }
        }
        cx.notify();
    }

    /// Start a fresh, read-only continuation plan after explicit user action.
    /// This never reuses a stale resume plan and cannot cross a session token.
    pub(super) fn can_offer_recovery(&self) -> bool {
        recovery_is_available(
            self.recovery.as_ref(),
            self.session_token,
            self.suspended,
            self.busy,
            self.pending.is_some(),
            self.transfer.as_ref().map(|transfer| transfer.phase),
        )
    }

    pub(super) fn request_recovery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(candidate) = self.recovery.as_ref() else {
            return;
        };
        let session_valid = !self.suspended
            && self.session.is_some()
            && candidate.session_token == self.session_token;
        let phase = self.transfer.as_ref().map(|transfer| transfer.phase);
        let allowed = recovery_request_is_allowed(
            Some(candidate),
            self.session_token,
            self.session.is_some(),
            self.suspended,
            self.busy,
            self.pending.is_some(),
            phase,
        );
        if !allowed {
            if !session_valid || phase != Some(TransferPhase::Failed) {
                self.recovery = None;
            }
            self.status = Message::new(
                if self.busy {
                    "当前文件操作仍在进行，请等待完成后再恢复"
                } else if self.pending.is_some() {
                    "当前操作仍待审核，请先处理审核栏"
                } else {
                    "恢复建议已失效，请在当前 SSH 会话中重新选择源和目标"
                },
                if self.busy {
                    "A file operation is still running; wait before recovery"
                } else if self.pending.is_some() {
                    "An operation is awaiting review; handle the review bar first"
                } else {
                    "The recovery proposal is stale; choose the source and destination again in the current SSH session"
                },
            );
            cx.notify();
            return;
        }
        let spec = candidate.spec.clone();
        let directory = candidate.directory;
        self.status = Message::new(
            "正在准备新的续传校验；确认后才会写入目标",
            "Preparing a new continuation check; nothing will be written until you confirm",
        );
        self.run(Operation::PlanResume(spec, directory), window, cx);
    }
}

fn directory_review_message(plan: &DirectoryTransferPlan) -> Message {
    let (source, target) = match plan.direction() {
        TransferDirection::Upload => (
            plan.local_path().display().to_string(),
            plan.remote_path().to_owned(),
        ),
        TransferDirection::Download => (
            plan.remote_path().to_owned(),
            plan.local_path().display().to_string(),
        ),
    };
    Message::new(
        format!(
            "源：{source}\n目标：{target}\n{} 个文件 · {} 个目录 · {} 字节。仅新建目标，不跟随符号链接。\n上限：32 层 / 10000 项 / 16 GiB / 15 分钟。取消或失败会保留部分目录；确认传输？",
            plan.files(),
            plan.directories(),
            plan.bytes()
        ),
        format!(
            "Source: {source}\nDestination: {target}\n{} files · {} directories · {} bytes. New destination only; symlinks are refused.\nLimits: 32 levels / 10000 entries / 16 GiB / 15 minutes. Failure or cancellation leaves a partial tree. Transfer?",
            plan.files(),
            plan.directories(),
            plan.bytes()
        ),
    )
}

fn resume_review_message(
    direction: TransferDirection,
    local: &std::path::Path,
    remote: &str,
    total: u64,
    existing: u64,
    tree: Option<(usize, usize)>,
) -> Message {
    let (source, target) = match direction {
        TransferDirection::Upload => (local.display().to_string(), remote.to_owned()),
        TransferDirection::Download => (remote.to_owned(), local.display().to_string()),
    };
    let counts_en = tree.map_or_else(String::new, |(files, dirs)| {
        format!("{files} files / {dirs} directories · ")
    });
    let counts_zh = tree.map_or_else(String::new, |(files, dirs)| {
        format!("{files} 个文件 / {dirs} 个目录 · ")
    });
    Message::new(
        format!(
            "续传源：{source}\n续传目标：{target}\n{counts_zh}已校验部分 {existing} / {total} 字节；待补充 {} 字节。\n仅在已有内容与源一致时追加；执行前会重新校验，不覆盖不同内容。取消或失败可保留部分目标。确认续传？",
            total.saturating_sub(existing)
        ),
        format!(
            "Resume source: {source}\nResume destination: {target}\n{counts_en}Verified existing content: {existing} / {total} bytes; {} bytes remain.\nAppend only when existing content matches the source; rechecked before writing. Different content is never overwritten. Cancellation or failure may leave partial output. Continue?",
            total.saturating_sub(existing)
        ),
    )
}

impl Operation {
    fn isolation_target(&self) -> Option<IsolationTarget> {
        match self {
            Self::Mkdir(path) | Self::Save { path, .. } => {
                Some(IsolationTarget::Remote(path.clone()))
            }
            Self::Rename(from, _) => Some(IsolationTarget::Remote(from.clone())),
            Self::Delete(entry) | Self::SetPermissions(entry, _) => {
                Some(IsolationTarget::Remote(entry.path.clone()))
            }
            Self::ApplyDirectorySync(review, _) => Some(
                if review.plan.direction() == DirectorySyncDirection::LeftToRight {
                    IsolationTarget::Remote(review.remote.clone())
                } else {
                    IsolationTarget::Local(review.local.clone())
                },
            ),
            _ => None,
        }
    }
    fn can_run_beside_transfers(&self) -> bool {
        self.recovery_spec().is_some()
            || matches!(
                self,
                Self::InspectFileQuarantine(_)
                    | Self::InspectQuarantine(_)
                    | Self::AcknowledgeQuarantine(_)
                    | Self::List(_)
                    | Self::Read(_)
                    | Self::ReadMerge { .. }
                    | Self::ApplyPatchToDraft { .. }
                    | Self::PlanDirectory(_)
                    | Self::PlanResume(..)
                    | Self::Compare(..)
                    | Self::PlanDirectorySync(..)
            )
    }
    /// Derive a fresh continuation request only from an operation that already
    /// performed transfer I/O. Review-only plans intentionally return `None`.
    fn recovery_spec(&self) -> Option<(TransferSpec, bool)> {
        match self {
            Self::Upload(local, remote) => Some((TransferSpec::upload(local, remote), false)),
            Self::Download(remote, local) => Some((TransferSpec::download(remote, local), false)),
            Self::TransferDirectory(plan) => Some((
                TransferSpec {
                    local: plan.local_path().to_owned(),
                    remote: plan.remote_path().to_owned(),
                    direction: plan.direction(),
                },
                true,
            )),
            Self::ResumeFile(plan) => Some((
                TransferSpec {
                    local: plan.local_path().to_owned(),
                    remote: plan.remote_path().to_owned(),
                    direction: plan.direction(),
                },
                false,
            )),
            Self::ResumeDirectory(plan) => Some((
                TransferSpec {
                    local: plan.local_path().to_owned(),
                    remote: plan.remote_path().to_owned(),
                    direction: plan.direction(),
                },
                true,
            )),
            Self::InspectFileQuarantine(_)
            | Self::InspectQuarantine(_)
            | Self::AcknowledgeQuarantine(_)
            | Self::List(_)
            | Self::Read(_)
            | Self::ReadMerge { .. }
            | Self::ApplyPatchToDraft { .. }
            | Self::Mkdir(_)
            | Self::Rename(_, _)
            | Self::Delete(_)
            | Self::SetPermissions(_, _)
            | Self::PlanDirectory(_)
            | Self::PlanResume(..)
            | Self::Compare(..)
            | Self::ApplyDirectorySync(..)
            | Self::PlanDirectorySync(..)
            | Self::Save { .. } => None,
        }
    }

    fn transfer_status(&self) -> Option<TransferStatus> {
        let (spec, directory, continuation) = match self {
            Self::Upload(local, remote) => (TransferSpec::upload(local, remote), false, false),
            Self::Download(remote, local) => (TransferSpec::download(remote, local), false, false),
            Self::TransferDirectory(plan) => (
                TransferSpec {
                    local: plan.local_path().to_owned(),
                    remote: plan.remote_path().to_owned(),
                    direction: plan.direction(),
                },
                true,
                false,
            ),
            Self::ResumeFile(plan) => (
                TransferSpec {
                    local: plan.local_path().to_owned(),
                    remote: plan.remote_path().to_owned(),
                    direction: plan.direction(),
                },
                false,
                true,
            ),
            Self::ResumeDirectory(plan) => (
                TransferSpec {
                    local: plan.local_path().to_owned(),
                    remote: plan.remote_path().to_owned(),
                    direction: plan.direction(),
                },
                true,
                true,
            ),
            _ => return None,
        };
        let mut status = TransferStatus::new(spec, directory, continuation);
        status.reviewed_existing = match self {
            Self::ResumeFile(plan) => plan.existing_bytes(),
            Self::ResumeDirectory(plan) => plan.existing_bytes(),
            _ => 0,
        };
        Some(status)
    }
}

fn size_label(size: Option<u64>) -> String {
    let Some(size) = size else {
        return "—".into();
    };
    if size < 1024 {
        return format!("{size} B");
    }
    if size < 1024 * 1024 {
        return format!("{:.1} KiB", size as f64 / 1024.);
    }
    if size < 1024 * 1024 * 1024 {
        return format!("{:.1} MiB", size as f64 / (1024. * 1024.));
    }
    format!("{:.1} GiB", size as f64 / (1024. * 1024. * 1024.))
}
fn permissions_label(entry: &RemoteEntry) -> String {
    let Some(mode) = entry.permissions else {
        return "—".into();
    };
    let mut chars = vec![if entry.is_symlink {
        'l'
    } else if entry.is_directory {
        'd'
    } else {
        '-'
    }];
    for (index, bit) in [
        0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001,
    ]
    .into_iter()
    .enumerate()
    {
        chars.push(if mode & bit != 0 {
            ['r', 'w', 'x'][index % 3]
        } else {
            '-'
        });
    }
    for (index, flag, marked, unmarked) in [
        (3, 0o4000, 's', 'S'),
        (6, 0o2000, 's', 'S'),
        (9, 0o1000, 't', 'T'),
    ] {
        if mode & flag != 0 {
            chars[index] = if chars[index] == 'x' {
                marked
            } else {
                unmarked
            };
        }
    }
    chars.into_iter().collect()
}
fn modified_label(timestamp: Option<u32>) -> String {
    timestamp
        .and_then(|value| chrono::DateTime::from_timestamp(i64::from(value), 0))
        .map(|value| value.format("%Y/%m/%d %H:%M").to_string())
        .unwrap_or_else(|| "—".into())
}
fn type_label(entry: &RemoteEntry, cx: &App) -> &'static str {
    if entry.is_symlink {
        t(cx, "符号链接", "Link")
    } else if entry.is_directory {
        t(cx, "文件夹", "Folder")
    } else {
        t(cx, "文件", "File")
    }
}
fn table_cell(text: impl Into<SharedString>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .px_2()
        .overflow_hidden()
        .child(text.into())
}

fn confirmation_bar(
    cx: &App,
    message: String,
    scroll: &ScrollHandle,
    confirm: Button,
    cancel: Button,
) -> impl IntoElement {
    use gpui_kit::component::scroll::{ScrollableElement, ScrollbarAxis};
    let visual = crate::design::palette(cx);
    // Each original line keeps its intrinsic width and height. A single wrapped
    // text child has no horizontal extent for long paths, even when overflow is
    // enabled; nonshrinking rows make both axes inspectable without truncation.
    let lines = message.split('\n').enumerate().map(|(index, line)| {
        let line = SharedString::from(line.to_owned());
        div()
            .id(("file-confirmation-line", index))
            .flex_shrink_0()
            .min_h(px(18.))
            .line_height(px(18.))
            .whitespace_nowrap()
            .role(accesskit::Role::Label)
            .aria_label(line.clone())
            .child(line)
            .test_support()
    });
    div()
        .id("file-confirmation-bar")
        .w_full()
        .min_w_0()
        .px_2()
        .py_1()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_2()
        .bg(rgb(visual.danger_surface))
        .border_t_1()
        .border_color(rgb(visual.danger_border))
        .child(
            div()
                .id("file-confirmation-message")
                .flex_1()
                .min_w_0()
                .max_h(px(48.))
                .overflow_y_scroll()
                .overflow_x_scroll()
                .track_scroll(scroll)
                .relative()
                .flex()
                .flex_col()
                .items_start()
                .font_family("monospace")
                .text_xs()
                .pb_2()
                .children(lines)
                .scrollbar(scroll, ScrollbarAxis::Both)
                .test_support(),
        )
        // Review actions remain outside the two-axis scrolling region.
        .child(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap_2()
                .child(confirm)
                .child(cancel),
        )
        .test_support()
}

#[cfg(test)]
mod tests {
    use super::{
        FileFailure, Operation, Outcome, RecoveryCandidate, TransferDirection, TransferPhase,
        child_path, modified_label, parse_permissions_mode, permissions_label,
        recovery_is_available, recovery_request_is_allowed, size_label, terminal_transfer_phase,
        transfer_progress_message,
    };
    use keelshell_session::sftp::RemoteEntry;

    #[test]
    fn mutation_requires_a_loaded_absolute_directory() {
        for directory in [
            None,
            Some(""),
            Some("."),
            Some("draft/relative"),
            Some("/bad\npath"),
        ] {
            assert!(child_path(directory, "notes.txt").is_err());
        }
        assert_eq!(
            child_path(Some("/home/operator"), "notes.txt"),
            Ok("/home/operator/notes.txt".to_owned())
        );
    }

    #[test]
    fn remote_child_keeps_root_and_unicode_names_literal() {
        assert_eq!(
            child_path(Some("/"), "说明.txt"),
            Ok("/说明.txt".to_owned())
        );
        assert_eq!(
            child_path(Some("/var/log/"), "app log.txt"),
            Ok("/var/log/app log.txt".to_owned())
        );
        assert_eq!(
            child_path(Some("/tmp"), "$(command); file"),
            Ok("/tmp/$(command); file".to_owned())
        );
    }

    #[test]
    fn remote_child_rejects_traversal_and_display_control_characters() {
        for name in [
            "",
            ".",
            "..",
            "../victim",
            "one/two",
            "one\\two",
            "bad\0name",
            "bad\nname",
            "\u{1b}[2J",
        ] {
            assert!(
                child_path(Some("/home/operator"), name).is_err(),
                "{name:?}"
            );
        }
    }

    #[test]
    fn permission_input_is_strict_octal_without_symbolic_or_shell_syntax() {
        assert_eq!(parse_permissions_mode("0640"), Ok(0o640));
        assert_eq!(parse_permissions_mode("640"), Ok(0o640));
        for value in ["", "64", "06400", "0780", "u+rw", "$(id)", " 0640x"] {
            assert!(parse_permissions_mode(value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn recovery_candidate_is_derived_only_from_started_transfer_operations() {
        let spec =
            keelshell_session::sftp::TransferSpec::upload("/tmp/source.txt", "/srv/source.txt");
        let candidate = Operation::Upload(spec.local.clone(), spec.remote.clone()).recovery_spec();
        assert!(candidate.is_some());
        let Some((recovery, directory)) = candidate else {
            return;
        };
        assert_eq!(recovery, spec);
        assert!(!directory);

        assert!(Operation::PlanResume(spec, false).recovery_spec().is_none());
        assert!(Operation::List("/srv".into()).recovery_spec().is_none());
    }

    #[test]
    fn recovery_candidate_requires_failed_transfer_and_current_session() {
        let token = uuid::Uuid::new_v4();
        let candidate = RecoveryCandidate {
            session_token: token,
            spec: keelshell_session::sftp::TransferSpec::download(
                "/srv/source.txt",
                "/tmp/source.txt",
            ),
            directory: false,
        };
        assert!(recovery_is_available(
            Some(&candidate),
            token,
            false,
            false,
            false,
            Some(TransferPhase::Failed),
        ));
        for (suspended, busy, pending, phase) in [
            (true, false, false, Some(TransferPhase::Failed)),
            (false, true, false, Some(TransferPhase::Failed)),
            (false, false, true, Some(TransferPhase::Failed)),
            (false, false, false, Some(TransferPhase::Cancelled)),
            (false, false, false, Some(TransferPhase::Completed)),
        ] {
            assert!(!recovery_is_available(
                Some(&candidate),
                token,
                suspended,
                busy,
                pending,
                phase,
            ));
        }
        assert!(!recovery_is_available(
            Some(&candidate),
            uuid::Uuid::new_v4(),
            false,
            false,
            false,
            Some(TransferPhase::Failed),
        ));
    }

    #[test]
    fn read_only_resume_plan_never_rewrites_failed_transfer_phase() {
        let success: Result<Outcome, FileFailure> =
            Ok(Outcome::Done(crate::i18n::Message::empty()));
        let failed: Result<Outcome, FileFailure> = Err(FileFailure::WorkerStopped);
        let cancelled: Result<Outcome, FileFailure> = Err(FileFailure::Cancelled);
        for result in [&success, &failed, &cancelled] {
            assert_eq!(terminal_transfer_phase(false, result), None);
        }
        assert_eq!(
            terminal_transfer_phase(true, &success),
            Some(TransferPhase::Completed)
        );
        assert_eq!(
            terminal_transfer_phase(true, &failed),
            Some(TransferPhase::Uncertain)
        );
        let rejected = Err(FileFailure::Transport("server rejected the request".into()));
        assert_eq!(terminal_transfer_phase(false, &rejected), None);
        assert_eq!(
            terminal_transfer_phase(true, &rejected),
            Some(TransferPhase::Failed)
        );
        assert_eq!(
            terminal_transfer_phase(true, &cancelled),
            Some(TransferPhase::Cancelled)
        );
    }

    #[test]
    fn recovery_click_guard_rejects_changed_state_without_requiring_a_new_candidate() {
        let token = uuid::Uuid::new_v4();
        let candidate = RecoveryCandidate {
            session_token: token,
            spec: keelshell_session::sftp::TransferSpec::download(
                "/srv/source.txt",
                "/tmp/source.txt",
            ),
            directory: false,
        };
        assert!(recovery_request_is_allowed(
            Some(&candidate),
            token,
            true,
            false,
            false,
            false,
            Some(TransferPhase::Failed),
        ));
        for (session_available, suspended, busy, pending, phase) in [
            (false, false, false, false, Some(TransferPhase::Failed)),
            (true, true, false, false, Some(TransferPhase::Failed)),
            (true, false, true, false, Some(TransferPhase::Failed)),
            (true, false, false, true, Some(TransferPhase::Failed)),
            (true, false, false, false, Some(TransferPhase::Completed)),
        ] {
            assert!(!recovery_request_is_allowed(
                Some(&candidate),
                token,
                session_available,
                suspended,
                busy,
                pending,
                phase,
            ));
        }
    }

    fn entry(mode: Option<u32>, directory: bool, symlink: bool) -> RemoteEntry {
        RemoteEntry {
            name: "sample".into(),
            path: "/sample".into(),
            size: None,
            is_directory: directory,
            is_symlink: symlink,
            permissions: mode,
            modified: None,
        }
    }

    #[test]
    fn permission_columns_preserve_special_bits_and_entry_types() {
        assert_eq!(
            permissions_label(&entry(Some(0o100644), false, false)),
            "-rw-r--r--"
        );
        assert_eq!(
            permissions_label(&entry(Some(0o41777), true, false)),
            "drwxrwxrwt"
        );
        assert_eq!(
            permissions_label(&entry(Some(0o104640), false, false)),
            "-rwSr-----"
        );
        assert_eq!(
            permissions_label(&entry(Some(0o120777), false, true)),
            "lrwxrwxrwx"
        );
        assert_eq!(permissions_label(&entry(None, false, false)), "—");
    }

    #[test]
    fn metadata_columns_distinguish_missing_zero_and_utc_time() {
        assert_eq!(size_label(None), "—");
        assert_eq!(size_label(Some(0)), "0 B");
        assert_eq!(size_label(Some(1536)), "1.5 KiB");
        assert_eq!(modified_label(None), "—");
        assert_eq!(modified_label(Some(0)), "1970/01/01 00:00");
        assert_eq!(modified_label(Some(946_684_800)), "2000/01/01 00:00");
        assert_eq!(modified_label(Some(1_709_164_800)), "2024/02/29 00:00");
    }

    #[gpui_kit::test]
    fn transfer_progress_messages_localize_known_and_unknown_totals(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|cx| {
            let upload = transfer_progress_message(TransferDirection::Upload, 1024, Some(4096));
            assert_eq!(upload.render(cx), "上传中：1024 / 4096 字节");
            crate::i18n::set_language(keelshell_core::Language::En, cx);
            assert_eq!(upload.render(cx), "Uploading: 1024 / 4096 bytes");
            let download = transfer_progress_message(TransferDirection::Download, 512, None);
            assert_eq!(download.render(cx), "Downloading: 512 bytes");
            crate::i18n::set_language(keelshell_core::Language::ZhCn, cx);
            assert_eq!(download.render(cx), "下载中：512 字节");
        });
    }

    #[gpui_kit::test]
    fn existing_file_error_retranslates_without_rewriting_remote_detail(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        let error = FileFailure::Transport("/srv/服务.conf: permission denied".into()).message();
        cx.update(|cx| {
            assert!(error.render(cx).starts_with("文件操作失败："));
            crate::i18n::set_language(keelshell_core::Language::En, cx);
            assert!(error.render(cx).starts_with("File operation failed:"));
            assert!(
                error
                    .render(cx)
                    .contains("/srv/服务.conf: permission denied")
            );
            crate::i18n::set_language(keelshell_core::Language::ZhCn, cx);
            assert!(
                error
                    .render(cx)
                    .contains("/srv/服务.conf: permission denied")
            );
        });
    }
}

#[cfg(test)]
pub(crate) mod fixture_group;
#[cfg(test)]
#[path = "files/layout_tests.rs"]
mod layout_tests;
#[cfg(test)]
pub(crate) mod test_server;
#[cfg(test)]
mod transfer_tests;

#[cfg(test)]
#[expect(
    dead_code,
    reason = "The protocol fixture exposes controls used by several independent suites"
)]
#[path = "../../keelshell-session/tests/fixtures/sftp.rs"]
pub(crate) mod sftp_test_filesystem;
