//! GitHub release discovery, verification, and opt-in self-update staging.
//!
//! Network work, archive extraction, and file hashing run away from the UI
//! thread. Installation is handed to a separate invocation of the same binary
//! after the user explicitly chooses “Install and restart”. The helper copies
//! only manifest-listed files, rolls back partial changes, and leaves unsigned
//! or unsupported installations reviewable instead of replacing them blindly.

mod schedule;
mod worker;

use std::{
    collections::{BTreeMap, HashSet},
    env,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Selectable,
        button::{Button, ButtonVariants},
    },
    prelude::FluentBuilder,
    *,
};
use keelshell_core::{UpdateCheckFrequency, UpdatePreferences};
use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::runtime::Runtime;

use crate::i18n::{LocalizedTooltipExt, Message, t};

/// The public repository shown by the About panel and used for release APIs.
pub const PROJECT_URL: &str = "https://github.com/cyruss648/keelshell";
const RELEASES_API: &str = "https://api.github.com/repos/cyruss648/keelshell/releases/latest";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_RELEASE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const UPDATE_HELPER_ARG: &str = "--keelshell-apply-update";

#[derive(Debug, Clone, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseInfo {
    tag: String,
    name: String,
    body: String,
    url: String,
    published_at: Option<String>,
    archive_name: String,
    archive_url: String,
    checksum_url: String,
    archive_size: u64,
}

#[derive(Debug)]
struct StagedUpdate {
    release: ReleaseInfo,
    digest: String,
    archive: PathBuf,
    payload: PathBuf,
    cleanup: Option<StageCleanup>,
}

#[derive(Clone, PartialEq, Eq)]
struct StagedIdentity {
    release: ReleaseInfo,
    digest: String,
    archive: PathBuf,
    payload: PathBuf,
}

impl StagedIdentity {
    fn matches(&self, staged: &StagedUpdate) -> bool {
        self.release == staged.release
            && self.digest == staged.digest
            && self.archive == staged.archive
            && self.payload == staged.payload
    }
}

#[derive(Clone)]
enum CancelIntent {
    Request(Box<RequestIdentity>),
    Downloaded(Arc<StagedIdentity>),
}

/// Owns a downloaded staging root until the update is either abandoned or
/// handed to the detached helper. Keeping this guard with the ready state
/// keeps abandoned stages eligible for off-thread cleanup when the service ends.
#[derive(Debug)]
struct StageCleanup {
    root: PathBuf,
    armed: bool,
    executor: tokio::runtime::Handle,
}

impl StageCleanup {
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for StageCleanup {
    fn drop(&mut self) {
        if self.armed {
            let root = self.root.clone();
            drop(self.executor.spawn_blocking(move || {
                let _ = fs::remove_dir_all(root);
            }));
        }
    }
}

impl StagedUpdate {
    fn identity(&self) -> StagedIdentity {
        StagedIdentity {
            release: self.release.clone(),
            digest: self.digest.clone(),
            archive: self.archive.clone(),
            payload: self.payload.clone(),
        }
    }

    fn disarm_cleanup(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup.disarm();
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PackageManifest {
    schema_version: u64,
    platform: String,
    version: String,
    target: Option<String>,
    binary_sha256: String,
    // Schema 1 remains readable by old helpers, which already install every
    // listed file. Missing companion fields get an explicit error in new apps.
    mcp_binary_sha256: Option<String>,
    icon_source_sha256: String,
    installed: bool,
    signed_by_packaging_script: bool,
    native_acceptance: String,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct UpdateFile {
    relative: PathBuf,
    digest: String,
    executable: bool,
}

#[derive(Debug, Clone)]
struct UpdatePlan {
    files: Vec<UpdateFile>,
}

/// Classifies backups as disposable only after this exact staging payload has
/// completed installation. Finalization consumes the proof once.
#[derive(Debug)]
struct CommittedUpdate {
    payload: PathBuf,
    executable: PathBuf,
}

#[derive(Debug)]
enum PanelState {
    Idle,
    Checking,
    UpToDate(ReleaseInfo),
    Available(ReleaseInfo),
    Downloading(ReleaseInfo),
    Ready(StagedUpdate),
    Installing,
    Failed,
}

/// Workspace-owned persistence and explicit restart requests from the service.
pub enum UpdatePanelEvent {
    /// Hide the panel while the background service keeps its ownership.
    Close,
    /// The user explicitly authorized installation and the helper took ownership.
    Restart,
    /// Save the current policy draft through the workspace transaction.
    Preferences {
        /// Policy; the workspace supplies authoritative check metadata.
        preferences: UpdatePreferences,
        /// Draft revision, used to preserve edits made during a save.
        revision: u64,
    },
    /// A release check completed successfully at the supplied Unix time.
    Checked(u64),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestOrigin {
    Manual,
    Automatic,
}

#[derive(Clone, PartialEq, Eq)]
struct RequestIdentity {
    id: uuid::Uuid,
    target: String,
    source: &'static str,
    version: &'static str,
    generation: u64,
    release: Option<ReleaseInfo>,
}

struct InFlight {
    identity: RequestIdentity,
    origin: RequestOrigin,
    _worker: worker::Worker,
}

/// Long-lived update service. Visibility of the About panel does not own work.
pub struct UpdatePanel {
    state: PanelState,
    retained_stage: Option<StagedUpdate>,
    current_target: Option<String>,
    status: Message,
    _job: Option<Task<()>>,
    focus: FocusHandle,
    preferences: UpdatePreferences,
    draft: UpdatePreferences,
    draft_revision: u64,
    preferences_saving: bool,
    preferences_status: Message,
    generation: u64,
    request: Option<InFlight>,
    schedule: schedule::Schedule,
    background_suspended: bool,
    _poll: Task<()>,
    #[cfg(test)]
    release_endpoint: Option<String>,
    #[cfg(test)]
    download_endpoints: Option<(String, String)>,
    // Staging guards dispatch cleanup before the last runtime owner is dropped.
    runtime: Arc<Runtime>,
}

impl EventEmitter<UpdatePanelEvent> for UpdatePanel {}

impl UpdatePanel {
    /// Start delayed scheduling without moving focus or performing immediate I/O.
    pub fn new(
        runtime: Arc<Runtime>,
        preferences: UpdatePreferences,
        cx: &mut Context<Self>,
    ) -> Self {
        let executor = cx.background_executor().clone();
        let poll = cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_secs(1)).await;
                if this.update(cx, |panel, cx| panel.poll(cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            runtime,
            state: PanelState::Idle,
            retained_stage: None,
            current_target: target_triple(),
            status: Message::new(
                format!("当前版本 {CURRENT_VERSION}；后台检查不会安装更新。"),
                format!(
                    "Current version {CURRENT_VERSION}; background checks never install updates."
                ),
            ),
            _job: None,
            focus: cx.focus_handle(),
            preferences,
            draft: preferences,
            draft_revision: 0,
            preferences_saving: false,
            preferences_status: Message::new("设置已保存", "Settings saved"),
            generation: 0,
            request: None,
            schedule: schedule::Schedule::new(
                preferences,
                Instant::now(),
                unix_seconds().unwrap_or(0),
            ),
            background_suspended: false,
            _poll: poll,
            #[cfg(test)]
            release_endpoint: None,
            #[cfg(test)]
            download_endpoints: None,
        }
    }

    pub(super) fn show(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }

    pub(super) fn action_label(&self, cx: &App) -> &'static str {
        match &self.state {
            PanelState::Ready(_) => t(cx, "更新已就绪", "Update ready"),
            PanelState::Available(_) => t(cx, "发现更新", "Update available"),
            _ => t(cx, "关于/更新", "About / updates"),
        }
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        self.poll_at(Instant::now(), cx);
    }

    fn poll_at(&mut self, now: Instant, cx: &mut Context<Self>) {
        #[cfg(test)]
        if self.release_endpoint.is_none() {
            return;
        }
        if !self.background_suspended
            && self.schedule.due(now)
            && self.request.is_none()
            && matches!(
                self.state,
                PanelState::Idle
                    | PanelState::Available(_)
                    | PanelState::UpToDate(_)
                    | PanelState::Ready(_)
                    | PanelState::Failed
            )
        {
            self.begin_check(RequestOrigin::Automatic, cx);
        }
    }

    fn cancel_inflight(&mut self) {
        // Abort the owned Tokio future before dropping its foreground receiver.
        self.request = None;
        self._job = None;
    }

    fn discard_staged(&self, staged: StagedUpdate) {
        // The guard dispatches filesystem cleanup to its original Tokio worker.
        drop(staged);
    }

    fn clear_state(&mut self) {
        if let PanelState::Ready(staged) = std::mem::replace(&mut self.state, PanelState::Idle) {
            self.discard_staged(staged);
        }
        if let Some(staged) = self.retained_stage.take() {
            self.discard_staged(staged);
        }
    }

    fn retain_ready(&mut self) {
        if let PanelState::Ready(staged) = std::mem::replace(&mut self.state, PanelState::Idle) {
            // A periodic check must not consume a verified package or authorize
            // installation. Keep it until a replacement is verified or discarded.
            self.retained_stage = Some(staged);
        }
    }

    fn restore_ready(&mut self) {
        if let Some(staged) = self.retained_stage.take() {
            self.state = PanelState::Ready(staged);
        }
    }

    fn ready_matches(&self, reviewed: &StagedIdentity) -> bool {
        self.request.is_none()
            && matches!(&self.state, PanelState::Ready(staged) if reviewed.matches(staged))
    }

    fn return_to_downloaded(&mut self, reviewed: &StagedIdentity, cx: &mut Context<Self>) {
        if self.request.is_none()
            && !matches!(self.state, PanelState::Installing)
            && self
                .retained_stage
                .as_ref()
                .is_some_and(|staged| reviewed.matches(staged))
        {
            self.restore_ready();
            cx.notify();
        }
    }

    fn cancel_reviewed(&mut self, intent: &CancelIntent, cx: &mut Context<Self>) {
        // A painted Cancel can outlive its request. It must never become
        // Discard, or act on a replacement owner without a fresh review.
        let current = match intent {
            CancelIntent::Request(identity) => self.accepts(identity),
            CancelIntent::Downloaded(reviewed) => self.ready_matches(reviewed),
        };
        if current {
            self.cancel(cx);
        }
    }

    fn accepts(&self, identity: &RequestIdentity) -> bool {
        identity.generation == self.generation
            && identity.source == RELEASES_API
            && identity.version == CURRENT_VERSION
            && self.current_target.as_deref() == Some(identity.target.as_str())
            && self
                .request
                .as_ref()
                .is_some_and(|request| request.identity == *identity)
    }

    fn identity(&self, target: String, release: Option<ReleaseInfo>) -> RequestIdentity {
        RequestIdentity {
            id: uuid::Uuid::new_v4(),
            target,
            source: RELEASES_API,
            version: CURRENT_VERSION,
            generation: self.generation,
            release,
        }
    }

    fn check(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.begin_check(RequestOrigin::Manual, cx);
    }

    fn begin_check(&mut self, origin: RequestOrigin, cx: &mut Context<Self>) {
        if matches!(self.state, PanelState::Installing)
            || self
                .request
                .as_ref()
                .is_some_and(|request| request.origin == RequestOrigin::Manual)
        {
            return;
        }
        if origin == RequestOrigin::Automatic && self.request.is_some() {
            return;
        }
        let Some(target) = self.current_target.clone() else {
            self.schedule.failed(self.preferences, Instant::now());
            self.fail(
                "当前平台没有匹配的发布产物，请从项目主页手动下载。",
                "No release asset matches this platform. Download it from the project page.",
                cx,
            );
            return;
        };
        self.cancel_inflight();
        self.retain_ready();
        let identity = self.identity(target.clone(), None);
        #[cfg(not(test))]
        let endpoint = Some(RELEASES_API.to_owned());
        #[cfg(test)]
        let endpoint = self.release_endpoint.clone();
        let (worker, completion) = worker::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            Arc::new(AtomicBool::new(false)),
            async move {
                match endpoint {
                    Some(endpoint) => fetch_latest_release_at(&target, &endpoint).await,
                    None => Err(UpdateError::Network),
                }
            },
        );
        self.request = Some(InFlight {
            identity: identity.clone(),
            origin,
            _worker: worker,
        });
        self.state = PanelState::Checking;
        self.status = Message::new("正在检查 GitHub Releases…", "Checking GitHub Releases…");
        self._job = Some(cx.spawn(async move |this, cx| {
            let result = completion.await.unwrap_or(Err(UpdateError::Network));
            let _ = this.update(cx, |panel, cx| {
                panel.finish_check(&identity, origin, result, cx)
            });
        }));
        cx.notify();
    }

    fn checked(&mut self, cx: &mut Context<Self>) {
        self.schedule.succeeded(self.preferences, Instant::now());
        if let Some(seconds) = unix_seconds() {
            self.preferences.last_successful_check = Some(seconds);
            self.draft.last_successful_check = Some(seconds);
            cx.emit(UpdatePanelEvent::Checked(seconds));
        }
    }

    fn finish_check(
        &mut self,
        identity: &RequestIdentity,
        origin: RequestOrigin,
        result: Result<ReleaseInfo, UpdateError>,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts(identity) {
            return;
        }
        self.request = None;
        match result {
            Ok(release) if is_newer(&release.tag, CURRENT_VERSION) => {
                self.checked(cx);
                if self
                    .retained_stage
                    .as_ref()
                    .is_some_and(|stage| stage.release == release)
                {
                    self.restore_ready();
                    self.status = Message::new(
                        "已校验的更新仍是最新发布版本；安装需要确认。",
                        "The verified update is still the latest release; installation needs confirmation.",
                    );
                    cx.notify();
                    return;
                }
                self.state = PanelState::Available(release.clone());
                self.status = Message::new(
                    "发现新版本，可查看变更并下载校验。",
                    "A newer version is available. Review its changes and download it.",
                );
                if origin == RequestOrigin::Automatic && self.preferences.auto_download {
                    self.begin_download(release, RequestOrigin::Automatic, cx);
                }
            }
            Ok(release) => {
                self.checked(cx);
                self.state = PanelState::UpToDate(release);
                self.restore_ready();
                self.status =
                    Message::new("当前已是最新版本。", "This installation is up to date.");
            }
            Err(UpdateError::Http(404)) => {
                self.checked(cx);
                self.state = PanelState::Idle;
                self.restore_ready();
                self.status = Message::new(
                    "项目尚无可用的稳定发布版本。",
                    "No stable release is available yet.",
                );
            }
            Err(error) => {
                self.schedule.failed(self.preferences, Instant::now());
                self.status = error.message();
                self.state = PanelState::Failed;
                self.restore_ready();
            }
        }
        cx.notify();
    }

    fn download(&mut self, release: ReleaseInfo, _window: &mut Window, cx: &mut Context<Self>) {
        self.begin_download(release, RequestOrigin::Manual, cx);
    }

    fn begin_download(
        &mut self,
        release: ReleaseInfo,
        origin: RequestOrigin,
        cx: &mut Context<Self>,
    ) {
        let current = match &self.state {
            PanelState::Available(current) | PanelState::UpToDate(current) => current,
            _ => return,
        };
        if current != &release || self.request.is_some() {
            return;
        }
        let Some(target) = self.current_target.clone() else {
            return;
        };
        let identity = self.identity(target, Some(release.clone()));
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = cancelled.clone();
        let download = release.clone();
        #[cfg(not(test))]
        let endpoints = Some((release.checksum_url.clone(), release.archive_url.clone()));
        #[cfg(test)]
        let endpoints = self.download_endpoints.clone();
        let (worker, completion) = worker::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            cancelled,
            async move {
                match endpoints {
                    Some((checksum, archive)) => {
                        download_and_stage_from(download, cancellation, &checksum, &archive).await
                    }
                    None => Err(UpdateError::Network),
                }
            },
        );
        self.request = Some(InFlight {
            identity: identity.clone(),
            origin,
            _worker: worker,
        });
        self.state = PanelState::Downloading(release);
        self.status = Message::new(
            "正在下载并校验发布产物；不会自动安装。",
            "Downloading and verifying the release asset; installation remains manual.",
        );
        self._job = Some(cx.spawn(async move |this, cx| {
            let result = completion.await.unwrap_or(Err(UpdateError::Network));
            // A disposed panel cannot receive a staging guard on the UI thread.
            let _ = this.update(cx, |panel, cx| panel.finish_download(&identity, result, cx));
        }));
        cx.notify();
    }

    fn finish_download(
        &mut self,
        identity: &RequestIdentity,
        result: Result<StagedUpdate, UpdateError>,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts(identity) {
            if let Ok(staged) = result {
                self.discard_staged(staged);
            }
            return;
        }
        self.request = None;
        match result {
            Ok(staged) if identity.release.as_ref() == Some(&staged.release) => {
                self.status = Message::new(
                    format!(
                        "{} 已下载且 SHA-256 校验通过；请确认后安装并重启。",
                        staged.release.tag
                    ),
                    format!(
                        "Downloaded and verified {}; confirm installation and restart when ready.",
                        staged.release.tag
                    ),
                );
                if let Some(previous) = self.retained_stage.take() {
                    self.discard_staged(previous);
                }
                self.state = PanelState::Ready(staged);
            }
            Ok(staged) => {
                self.discard_staged(staged);
                self.schedule.failed(self.preferences, Instant::now());
                self.state = PanelState::Failed;
                self.status = UpdateError::Invalid.message();
                self.restore_ready();
            }
            Err(error) => {
                self.schedule.failed(self.preferences, Instant::now());
                self.state = PanelState::Failed;
                self.status = error.message();
                self.restore_ready();
            }
        }
        cx.notify();
    }

    pub(super) fn suspend_background(&mut self, cx: &mut Context<Self>) {
        self.background_suspended = true;
        self.cancel_inflight();
        self.restore_ready();
        self.preferences_status = Message::new(
            "配置读取失败，后台检查已暂停；修复并保存或重新载入设置后恢复。",
            "Configuration could not be loaded; background checks are paused until settings are repaired and saved or reloaded.",
        );
        cx.notify();
    }

    pub(super) fn set_preferences(
        &mut self,
        preferences: UpdatePreferences,
        cx: &mut Context<Self>,
    ) {
        let draft_was_clean = self.draft.frequency == self.preferences.frequency
            && self.draft.auto_download == self.preferences.auto_download;
        let policy_changed = self.preferences.frequency != preferences.frequency
            || self.preferences.auto_download != preferences.auto_download;
        let was_suspended = self.background_suspended;
        self.background_suspended = false;
        if policy_changed {
            self.generation = self.generation.wrapping_add(1);
            self.cancel_inflight();
            if matches!(
                self.state,
                PanelState::Checking | PanelState::Downloading(_) | PanelState::Failed
            ) {
                self.restore_ready();
            }
            if !matches!(
                self.state,
                PanelState::Ready(_) | PanelState::Available(_) | PanelState::Installing
            ) {
                self.clear_state();
                self.status = if preferences.frequency == UpdateCheckFrequency::Disabled {
                    Message::new(
                        "后台更新检查已关闭；仍可手动检查。",
                        "Background update checks are off; manual checks remain available.",
                    )
                } else {
                    Message::new(
                        "更新策略已更改；后续检查使用新设置。",
                        "Update policy changed; subsequent checks use the saved settings.",
                    )
                };
            }
        }
        if policy_changed || was_suspended {
            self.schedule
                .reset(preferences, Instant::now(), unix_seconds().unwrap_or(0));
        }
        self.preferences = preferences;
        self.draft.last_successful_check = preferences.last_successful_check;
        if draft_was_clean {
            self.draft = preferences;
        }
        cx.notify();
    }

    #[cfg(test)]
    pub(super) fn preferences_snapshot(&self) -> UpdatePreferences {
        self.preferences
    }

    #[cfg(test)]
    pub(super) fn set_test_release_endpoint(&mut self, endpoint: String) {
        self.release_endpoint = Some(endpoint);
    }

    pub(super) fn preferences_saved(
        &mut self,
        revision: u64,
        preferences: UpdatePreferences,
        cx: &mut Context<Self>,
    ) {
        self.set_preferences(preferences, cx);
        self.preferences_saving = false;
        if self.draft_revision == revision {
            self.draft = preferences;
        }
        self.preferences_status = if self.draft.frequency != preferences.frequency
            || self.draft.auto_download != preferences.auto_download
        {
            Message::new(
                "先前设置已保存；当前编辑尚未保存。",
                "Earlier settings were saved; current edits are not saved yet.",
            )
        } else {
            Message::new(
                "更新设置已保存；安装仍需确认。",
                "Update settings saved; installation still requires confirmation.",
            )
        };
        cx.notify();
    }

    pub(super) fn preferences_failed(&mut self, cx: &mut Context<Self>) {
        self.preferences_saving = false;
        self.preferences_status = Message::new(
            "设置未保存，草稿已保留；请重试。",
            "Settings were not saved; the draft is retained. Retry when ready.",
        );
        cx.notify();
    }

    fn preferences_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let dirty = self.draft.frequency != self.preferences.frequency
            || self.draft.auto_download != self.preferences.auto_download;
        div().id("update-preferences").flex_shrink_0().min_w_0().p_2().rounded(px(6.))
            .bg(rgb(visual.canvas)).border_1().border_color(rgb(visual.border)).flex().flex_col().gap_1()
            .child(div().flex().flex_wrap().items_center().gap_1()
                .child(t(cx, "后台检查", "Background checks"))
                .children([
                    (UpdateCheckFrequency::Disabled, "updates-off", "关闭", "Off"),
                    (UpdateCheckFrequency::Daily, "updates-daily", "每天", "Daily"),
                    (UpdateCheckFrequency::Weekly, "updates-weekly", "每周", "Weekly"),
                ].into_iter().map(|(frequency, id, zh, en)| Button::new(id).ghost().compact().label(t(cx, zh, en))
                    .selected(self.draft.frequency == frequency).disabled(self.preferences_saving || matches!(self.state, PanelState::Installing))
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        panel.draft.frequency = frequency;
                        panel.draft_revision = panel.draft_revision.wrapping_add(1);
                        panel.preferences_status = Message::new("设置尚未保存", "Settings not saved yet");
                        cx.notify();
                    }))))
                .child(Button::new("updates-auto-download").ghost().compact().icon(IconName::Download)
                    .label(t(cx, "自动下载并校验", "Download and verify automatically"))
                    .selected(self.draft.auto_download)
                    .disabled(self.preferences_saving || self.draft.frequency == UpdateCheckFrequency::Disabled || matches!(self.state, PanelState::Installing))
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.draft.auto_download = !panel.draft.auto_download;
                        panel.draft_revision = panel.draft_revision.wrapping_add(1);
                        panel.preferences_status = Message::new("设置尚未保存", "Settings not saved yet");
                        cx.notify();
                    })))
                .child(Button::new("save-update-preferences").primary().compact().label(t(cx, "保存设置", "Save settings"))
                    .disabled(!dirty || self.preferences_saving || matches!(self.state, PanelState::Installing))
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.preferences_saving = true;
                        panel.preferences_status = Message::new("正在保存…", "Saving…");
                        cx.emit(UpdatePanelEvent::Preferences { preferences: panel.draft, revision: panel.draft_revision });
                        cx.notify();
                    }))))
            .child(div().text_xs().text_color(rgb(visual.muted)).child(t(cx,
                "启动后延迟检查，失败会退避；仅后台发现更新时自动下载。安装始终需要确认。",
                "Checks start after a delay and back off on failure. Background discoveries may download; installation always needs confirmation.")))
            .child(div().text_xs().text_color(rgb(visual.muted)).child(self.preferences_status.render(cx)))
            .child(div().text_xs().text_color(rgb(visual.muted)).child(
                self.preferences.last_successful_check
                    .and_then(|seconds| i64::try_from(seconds).ok())
                    .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
                    .map(|date| format!("{} {}", t(cx, "上次检查：", "Last checked:"), date.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M")))
                    .unwrap_or_else(|| t(cx, "尚未完成更新检查", "No completed update check yet").to_owned())
            ))
            .into_any_element()
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if matches!(self.state, PanelState::Installing) {
            return;
        }
        let active_request = self.request.is_some();
        self.cancel_inflight();
        if active_request {
            // Cancelling replacement work revokes that request, not the already
            // verified package retained independently from it.
            self.state = PanelState::Idle;
            self.restore_ready();
        } else {
            self.clear_state();
        }
        self.schedule.succeeded(self.preferences, Instant::now());
        self.status = if active_request && matches!(self.state, PanelState::Ready(_)) {
            Message::new(
                "本次更新请求已取消；已校验的下载仍保留。",
                "Update request cancelled; the verified download was kept.",
            )
        } else if active_request {
            Message::new(
                "更新请求已取消；没有安装。",
                "Update request cancelled; nothing was installed.",
            )
        } else {
            Message::new(
                "已丢弃下载的更新；没有安装。",
                "Downloaded update discarded; nothing was installed.",
            )
        };
        cx.notify();
    }

    fn install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut staged = match std::mem::replace(&mut self.state, PanelState::Installing) {
            PanelState::Ready(staged) => staged,
            state => {
                self.state = state;
                return;
            }
        };
        if !matches!(self.state, PanelState::Installing) {
            return;
        }
        let Some(target) = self.current_target.clone() else {
            self.fail(
                "当前平台没有匹配的发布产物，无法自动安装。",
                "No release asset matches this platform, so automatic installation is unavailable.",
                cx,
            );
            drop(staged);
            return;
        };
        let current_exe = match env::current_exe() {
            Ok(path) => path,
            Err(_) => {
                self.fail(
                    "无法定位当前程序，自动安装已停止。",
                    "The current executable could not be located; automatic installation stopped.",
                    cx,
                );
                drop(staged);
                return;
            }
        };
        let Some(install_root) = installation_root(&current_exe, &target) else {
            self.fail(
                "当前程序不是可替换的已安装版本，请从发布页手动安装。",
                "This executable is not a replaceable installed package. Install manually from the release page.",
                cx,
            );
            drop(staged);
            return;
        };
        self.status = Message::new(
            "正在启动安全安装助手；程序将关闭并在成功后重启。",
            "Starting the safe installer helper. The app will close and restart after a successful update.",
        );
        let payload = staged.payload.clone();
        let current_exe_for_helper = current_exe.clone();
        let target_for_helper = target.clone();
        match spawn_update_helper(
            payload,
            install_root,
            current_exe_for_helper,
            target_for_helper,
        ) {
            Ok(()) => {
                // The helper owns the extracted payload after this point. The
                // old process exits before any destination file is changed.
                staged.disarm_cleanup();
                cx.notify();
                let _ = window;
                cx.emit(UpdatePanelEvent::Restart);
            }
            Err(error) => {
                self.state = PanelState::Failed;
                self.status = error.message();
                drop(staged);
                cx.notify();
            }
        }
    }

    fn fail(&mut self, zh: impl Into<String>, en: impl Into<String>, cx: &mut Context<Self>) {
        self.status = Message::new(zh, en);
        self.state = PanelState::Failed;
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.state,
            PanelState::Checking | PanelState::Downloading(_)
        ) {
            self.status = Message::new(
                "后台请求仍在进行；关闭面板不会安装未校验的文件。",
                "A background request is still running; closing this panel will not install an unverified file.",
            );
        }
        cx.emit(UpdatePanelEvent::Close);
    }
}

impl Drop for UpdatePanel {
    fn drop(&mut self) {
        self.request = None;
        self._job = None;
    }
}

fn unix_seconds() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

impl Render for UpdatePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let (release, can_download) = match &self.state {
            PanelState::Available(release) | PanelState::UpToDate(release) => (
                Some(release),
                !matches!(self.state, PanelState::UpToDate(_)),
            ),
            PanelState::Downloading(release) => (Some(release), false),
            PanelState::Ready(staged) => (Some(&staged.release), false),
            _ => (None, false),
        };
        // Capture the displayed release and exact stage once. Sharing the
        // snapshot avoids cloning bounded release notes for each action.
        let download_review = release.filter(|_| can_download).cloned();
        let ready_review = match &self.state {
            PanelState::Ready(staged) if self.request.is_none() => {
                Some(Arc::new(staged.identity()))
            }
            _ => None,
        };
        let retained_review = self
            .retained_stage
            .as_ref()
            .filter(|_| self.request.is_none())
            .map(|staged| Arc::new(staged.identity()));
        let cancel_intent = self
            .request
            .as_ref()
            .map(|request| CancelIntent::Request(Box::new(request.identity.clone())))
            .or_else(|| ready_review.clone().map(CancelIntent::Downloaded));
        let body = release.map(|release| {
            let published = release
                .published_at
                .as_deref()
                .unwrap_or_else(|| t(cx, "未知时间", "Unknown date"));
            div()
                .id("update-release-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .bg(rgb(visual.canvas))
                .border_1()
                .border_color(rgb(visual.border))
                .rounded(px(6.))
                .text_xs()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(visual.text))
                        .child(release.name.clone()),
                )
                .child(
                    div()
                        .text_color(rgb(visual.muted))
                        .child(format!("{} · {}", release.tag, published)),
                )
                .child(
                    div()
                        .mt_2()
                        .text_color(rgb(visual.text))
                        .child(release.body.clone()),
                )
        });
        let target = self
            .current_target
            .as_deref()
            .map(str::to_owned)
            .unwrap_or_else(|| t(cx, "不支持的平台", "Unsupported platform").to_owned());
        div()
            .id("update-panel")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(visual.surface))
            .text_color(rgb(visual.text))
            .child(
                div()
                    .flex_shrink_0()
                    .p_3()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .child(t(cx, "关于与更新", "About and updates")),
                    )
                    .child(
                        Button::new("close-update-panel")
                            .ghost()
                            .compact()
                            .label("×")
                            .accessibility_label(t(cx, "关闭关于与更新", "Close about and updates"))
                            .localized_tooltip("关闭关于与更新", "Close about and updates")
                            .on_click(cx.listener(|panel, _, _, cx| panel.close(cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().child(format!("KeelShell {CURRENT_VERSION}")))
                            .child(div().text_xs().text_color(rgb(visual.muted)).child(target)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new("open-project")
                                    .icon(IconName::ExternalLink)
                                    .ghost()
                                    .compact()
                                    .label(t(cx, "GitHub 项目主页", "GitHub project"))
                                    .on_click(|_, _, cx| cx.open_url(PROJECT_URL)),
                            )
                            .child(
                                Button::new("check-updates")
                                    .icon(IconName::RefreshCw)
                                    .primary()
                                    .compact()
                                    .label(t(cx, "检查更新", "Check for updates"))
                                    .disabled(
                                        matches!(self.state, PanelState::Installing)
                                            || self.request.as_ref().is_some_and(|request| {
                                                request.origin == RequestOrigin::Manual
                                            }),
                                    )
                                    .on_click(
                                        cx.listener(|panel, _, window, cx| panel.check(window, cx)),
                                    ),
                            ),
                    )
                    .child(self.preferences_card(cx))
                    .child(body.unwrap_or_else(|| {
                        div()
                            .flex_1()
                            .min_h_0()
                            .bg(rgb(visual.canvas))
                            .rounded(px(6.))
                            .p_3()
                            .text_color(rgb(visual.muted))
                            .id("bundled-changelog")
                            .overflow_y_scroll()
                            .child(include_str!("../../../CHANGELOG.md"))
                    }))
                    .when_some(
                        match &self.state {
                            PanelState::Ready(staged) => Some(staged.digest.clone()),
                            _ => None,
                        },
                        |body, digest| {
                            body.child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(visual.muted))
                                    .child(format!("SHA-256: {digest}")),
                            )
                        },
                    )
                    .when_some(release.map(|release| release.url.clone()), |body, url| {
                        body.child(
                            Button::new("open-release")
                                .ghost()
                                .compact()
                                .label(t(cx, "在 GitHub 查看此版本", "View this release on GitHub"))
                                .on_click(move |_, _, cx| cx.open_url(&url)),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(visual.accent))
                            .child(self.status.render(cx)),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .p_3()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .gap_2()
                    .when_some(retained_review, |row, reviewed| {
                        row.child(
                            Button::new("keep-ready-update")
                                .ghost()
                                .label(t(cx, "返回已下载版本", "Return to downloaded version"))
                                .on_click(cx.listener(move |panel, _, _, cx| {
                                    panel.return_to_downloaded(&reviewed, cx);
                                })),
                        )
                    })
                    .when_some(download_review, |row, reviewed| {
                        row.child(
                            Button::new("download-update")
                                .icon(IconName::Download)
                                .primary()
                                .label(t(cx, "下载并校验", "Download and verify"))
                                .on_click(cx.listener(move |panel, _, window, cx| {
                                    panel.download(reviewed.clone(), window, cx)
                                })),
                        )
                    })
                    .when_some(ready_review.clone(), |row, reviewed| {
                        row.child(
                            Button::new("reveal-update")
                                .icon(IconName::Check)
                                .primary()
                                .label(t(cx, "查看已校验的安装包", "Show verified package"))
                                .on_click(cx.listener(move |panel, _, _, cx| {
                                    if panel.ready_matches(&reviewed) {
                                        cx.reveal_path(&reviewed.archive)
                                    }
                                })),
                        )
                    })
                    .when_some(ready_review, |row, reviewed| {
                        row.child(
                            Button::new("install-update")
                                .icon(IconName::Check)
                                .primary()
                                .label(t(cx, "确认安装并重启", "Install and restart"))
                                .on_click(cx.listener(move |panel, _, window, cx| {
                                    if panel.ready_matches(&reviewed) {
                                        panel.install(window, cx)
                                    }
                                })),
                        )
                    })
                    .when_some(cancel_intent, |row, intent| {
                        row.child(
                            Button::new("cancel-update-request")
                                .ghost()
                                .label(if matches!(intent, CancelIntent::Request(_)) {
                                    t(cx, "取消更新请求", "Cancel update request")
                                } else {
                                    t(cx, "丢弃已下载更新", "Discard downloaded update")
                                })
                                .on_click(cx.listener(move |panel, _, _, cx| {
                                    panel.cancel_reviewed(&intent, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("cancel-update-panel")
                            .ghost()
                            .label(t(cx, "关闭", "Close"))
                            .on_click(cx.listener(|panel, _, _, cx| panel.close(cx))),
                    ),
            )
    }
}

async fn fetch_latest_release_at(target: &str, endpoint: &str) -> Result<ReleaseInfo, UpdateError> {
    let client = http_client(false)?;
    let response = client
        .get(endpoint)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|_| UpdateError::Network)?;
    if response.status() != StatusCode::OK {
        return Err(UpdateError::Http(response.status().as_u16()));
    }
    let bytes = read_bounded(response, MAX_RELEASE_BYTES).await?;
    let release: GithubRelease =
        serde_json::from_slice(&bytes).map_err(|_| UpdateError::Invalid)?;
    release_info(release, target)
}

fn release_info(release: GithubRelease, target: &str) -> Result<ReleaseInfo, UpdateError> {
    if !release.tag_name.starts_with('v') || !valid_project_url(&release.html_url) {
        return Err(UpdateError::Invalid);
    }
    let archive_name = package_name(&release.tag_name, target).ok_or(UpdateError::Invalid)?;
    let archive = unique_asset(&release.assets, &archive_name)?;
    let checksum_name = format!("{archive_name}.sha256");
    let checksum = unique_asset(&release.assets, &checksum_name)?;
    if archive.size == 0
        || archive.size > MAX_ARCHIVE_BYTES
        || !valid_download_url(&archive.browser_download_url)
        || checksum.size == 0
        || checksum.size > 1024
        || !valid_download_url(&checksum.browser_download_url)
    {
        return Err(UpdateError::Invalid);
    }
    Ok(ReleaseInfo {
        tag: release.tag_name,
        name: release.name.unwrap_or_default(),
        body: release.body.unwrap_or_default(),
        url: release.html_url,
        published_at: release.published_at,
        archive_name,
        archive_url: archive.browser_download_url.clone(),
        checksum_url: checksum.browser_download_url.clone(),
        archive_size: archive.size,
    })
}

async fn download_and_stage_from(
    release: ReleaseInfo,
    cancelled: Arc<AtomicBool>,
    checksum_url: &str,
    archive_url: &str,
) -> Result<StagedUpdate, UpdateError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(UpdateError::Cancelled);
    }
    let client = http_client(true)?;
    let checksum_response = client
        .get(checksum_url)
        .send()
        .await
        .map_err(|_| UpdateError::Network)?;
    let checksum_bytes = read_bounded(checksum_response, 1024).await?;
    let checksum = std::str::from_utf8(&checksum_bytes).map_err(|_| UpdateError::Invalid)?;
    let expected = parse_checksum(checksum, &release.archive_name).ok_or(UpdateError::Invalid)?;
    let response = client
        .get(archive_url)
        .send()
        .await
        .map_err(|_| UpdateError::Network)?;
    let bytes = read_bounded(response, release.archive_size).await?;
    if bytes.len() as u64 != release.archive_size {
        return Err(UpdateError::Invalid);
    }
    let actual = hex_digest(&Sha256::digest(&bytes));
    if expected != actual {
        return Err(UpdateError::Checksum);
    }
    if cancelled.load(Ordering::Acquire) {
        return Err(UpdateError::Cancelled);
    }
    let cleanup_executor =
        tokio::runtime::Handle::try_current().map_err(|_| UpdateError::Storage)?;
    // A private unique directory prevents other users or concurrent downloads
    // from replacing a predictable temporary path. Extracting before the
    // helper starts means the helper only copies files after the old process
    // has exited; it never needs to execute archive tooling.
    tokio::task::spawn_blocking(move || {
        let directory = env::temp_dir().join(format!(
            "keelshell-update-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let result = (|| {
            if cancelled.load(Ordering::Acquire) {
                return Err(UpdateError::Cancelled);
            }
            fs::create_dir(&directory).map_err(|_| UpdateError::Storage)?;
            let cleanup = StageCleanup {
                root: directory.clone(),
                armed: true,
                executor: cleanup_executor,
            };
            let archive = directory.join(&release.archive_name);
            fs::write(&archive, &bytes).map_err(|_| UpdateError::Storage)?;
            let payload = directory.join("payload");
            fs::create_dir(&payload).map_err(|_| UpdateError::Storage)?;
            extract_archive(&archive, &payload)?;
            let plan = validate_payload(&payload, &release)?;
            if plan.files.is_empty() {
                return Err(UpdateError::Invalid);
            }
            if cancelled.load(Ordering::Acquire) {
                return Err(UpdateError::Cancelled);
            }
            Ok(StagedUpdate {
                release,
                archive,
                payload,
                digest: actual,
                cleanup: Some(cleanup),
            })
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&directory);
        }
        result
    })
    .await
    .map_err(|_| UpdateError::Storage)?
}

/// Runs the private helper mode used by an already spawned update process.
///
/// Returning `true` tells `main` that no GPUI window should be opened. The
/// helper revalidates the extracted package, copies only manifest-listed files,
/// rolls back on a partial failure, and relaunches the executable after the
/// operation. It is intentionally a command-line entry point rather than a
/// shell script so paths are passed as native arguments on every platform.
pub fn run_update_helper() -> bool {
    let helper_path = env::current_exe().ok();
    let mut args = env::args_os().skip(1);
    let Some(mode) = args.next() else {
        return false;
    };
    if mode != UPDATE_HELPER_ARG {
        return false;
    }
    let source = args.next().map(PathBuf::from);
    let install_root = args.next().map(PathBuf::from);
    let current_exe = args.next().map(PathBuf::from);
    let target = args.next().and_then(|value| value.into_string().ok());
    if args.next().is_some() {
        return true;
    }
    let (Some(source), Some(install_root), Some(current_exe), Some(target)) =
        (source, install_root, current_exe, target)
    else {
        return true;
    };
    let relaunch = current_exe.clone();
    let result = apply_update(&source, &install_root, &current_exe, &target);
    let restart = result
        .as_ref()
        .map(|committed| committed.executable.clone())
        .unwrap_or(relaunch);
    if finish_update_staging(&source, result) {
        // Ordinary failures have restored all old images before returning.
        let _ = Command::new(restart).spawn();
    }
    if helper_path
        .as_deref()
        .and_then(Path::file_name)
        .is_some_and(|name| {
            name.to_string_lossy()
                .starts_with("keelshell-update-helper-")
        })
    {
        let _ = helper_path.as_deref().map(fs::remove_file);
    }
    true
}

fn finish_update_staging(payload: &Path, result: Result<CommittedUpdate, UpdateError>) -> bool {
    let recovery_required = match result {
        Ok(committed) => committed.payload != payload,
        Err(UpdateError::RecoveryRequired) => true,
        // Preflight failures must not erase unclassified old files even when
        // their error did not originate from the installation loop.
        Err(_) => ensure_no_update_recovery(payload).is_err(),
    };
    if recovery_required {
        // A failed rollback may leave the only old image in .backup. Retain
        // this exact owned stage and do not launch a partially restored app.
        let diagnostic = concat!(
            "自动安装回滚未完成。已保留此目录中的 .backup 原文件备份；请恢复后再重试。\n",
            "Update rollback did not complete. The old-file backups in .backup were preserved; restore them before retrying.\n"
        );
        // An existing marker may be an old diagnostic or an unfamiliar object.
        // Do not overwrite it or follow a link while preserving recovery data.
        if let Ok(mut file) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(payload.join("recovery-required.txt"))
        {
            let _ = file.write_all(diagnostic.as_bytes());
        }
        eprintln!("{diagnostic}Preserved staging: {}", payload.display());
        false
    } else {
        cleanup_staging_root(payload);
        true
    }
}

fn ensure_no_update_recovery(payload: &Path) -> Result<(), UpdateError> {
    for name in [".backup", "recovery-required.txt"] {
        match fs::symlink_metadata(payload.join(name)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            // Presence and metadata errors both mean that absence of old
            // recoverable images has not been established.
            _ => return Err(UpdateError::RecoveryRequired),
        }
    }
    Ok(())
}

fn cleanup_staging_root(payload: &Path) {
    let Some(root) = payload.parent() else {
        return;
    };
    let temp = env::temp_dir();
    let is_expected_root = payload.file_name().is_some_and(|name| name == "payload")
        && root.parent().is_some_and(|parent| parent == temp)
        && root
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("keelshell-update-"));
    if is_expected_root {
        let _ = fs::remove_dir_all(root);
    }
}

fn extract_archive(archive: &Path, destination: &Path) -> Result<(), UpdateError> {
    if archive
        .extension()
        .is_some_and(|extension| extension == "zip")
    {
        extract_zip(archive, destination)
    } else if archive
        .extension()
        .is_some_and(|extension| extension == "gz")
    {
        extract_tar_gz(archive, destination)
    } else {
        Err(UpdateError::Invalid)
    }
}

fn extract_zip(archive: &Path, destination: &Path) -> Result<(), UpdateError> {
    let file = File::open(archive).map_err(|_| UpdateError::Storage)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| UpdateError::Invalid)?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(UpdateError::TooLarge);
    }
    let mut names = HashSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| UpdateError::Invalid)?;
        let raw_name = entry.name();
        // ZIP writers commonly add a trailing slash to directory members;
        // normalize only that directory marker while still rejecting empty
        // path components inside a member name.
        let name = if entry.is_dir() {
            raw_name.trim_end_matches('/')
        } else {
            raw_name
        };
        let Some(relative) = safe_relative(name) else {
            return Err(UpdateError::Invalid);
        };
        if !names.insert(relative.clone()) {
            return Err(UpdateError::Invalid);
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(UpdateError::Invalid);
        }
        let target = destination.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|_| UpdateError::Storage)?;
            continue;
        }
        let declared = entry.size();
        if declared > MAX_FILE_BYTES || total.saturating_add(declared) > MAX_UNPACKED_BYTES {
            return Err(UpdateError::TooLarge);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| UpdateError::Storage)?;
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|_| UpdateError::Invalid)?;
        if bytes.len() as u64 != declared {
            return Err(UpdateError::Invalid);
        }
        total = total.saturating_add(declared);
        let mut output = File::create(&target).map_err(|_| UpdateError::Storage)?;
        output.write_all(&bytes).map_err(|_| UpdateError::Storage)?;
        output.sync_all().map_err(|_| UpdateError::Storage)?;
        set_extracted_mode(&target, entry.unix_mode());
    }
    Ok(())
}

fn extract_tar_gz(archive: &Path, destination: &Path) -> Result<(), UpdateError> {
    let file = File::open(archive).map_err(|_| UpdateError::Storage)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let mut names = HashSet::new();
    let mut total = 0_u64;
    let entries = archive.entries().map_err(|_| UpdateError::Invalid)?;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_ARCHIVE_ENTRIES {
            return Err(UpdateError::TooLarge);
        }
        let mut entry = entry.map_err(|_| UpdateError::Invalid)?;
        let raw = entry
            .path()
            .map_err(|_| UpdateError::Invalid)?
            .to_string_lossy()
            .into_owned();
        let Some(relative) = safe_relative(&raw) else {
            return Err(UpdateError::Invalid);
        };
        if !names.insert(relative.clone()) {
            return Err(UpdateError::Invalid);
        }
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() || (!kind.is_file() && !kind.is_dir()) {
            return Err(UpdateError::Invalid);
        }
        let target = destination.join(&relative);
        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|_| UpdateError::Storage)?;
            continue;
        }
        let declared = entry.header().size().map_err(|_| UpdateError::Invalid)?;
        if declared > MAX_FILE_BYTES || total.saturating_add(declared) > MAX_UNPACKED_BYTES {
            return Err(UpdateError::TooLarge);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| UpdateError::Storage)?;
        }
        let mut output = File::create(&target).map_err(|_| UpdateError::Storage)?;
        let copied = io::copy(&mut entry, &mut output).map_err(|_| UpdateError::Invalid)?;
        if copied != declared {
            return Err(UpdateError::Invalid);
        }
        output.sync_all().map_err(|_| UpdateError::Storage)?;
        total = total.saturating_add(copied);
        set_extracted_mode(
            &target,
            Some(entry.header().mode().map_err(|_| UpdateError::Invalid)?),
        );
    }
    Ok(())
}

fn set_extracted_mode(path: &Path, mode: Option<u32>) {
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777));
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
}

fn validate_payload(payload: &Path, release: &ReleaseInfo) -> Result<UpdatePlan, UpdateError> {
    let manifest_path = payload.join("package-manifest.json");
    let metadata = fs::symlink_metadata(&manifest_path).map_err(|_| UpdateError::Invalid)?;
    if !metadata.file_type().is_file() || metadata.len() > 16 * 1024 * 1024 {
        return Err(UpdateError::Invalid);
    }
    let manifest: PackageManifest =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|_| UpdateError::Storage)?)
            .map_err(|_| UpdateError::Invalid)?;
    let binaries = validate_manifest_receipt(&manifest)?;
    let version = release.tag.strip_prefix('v').ok_or(UpdateError::Invalid)?;
    if manifest.schema_version != 1
        || manifest.platform != platform_name()
        || manifest.version != version
        || manifest.target.as_deref() != Some(release_target(&release.archive_name)?)
        || !is_newer(&release.tag, CURRENT_VERSION)
        || manifest.files.len() > MAX_ARCHIVE_ENTRIES
    {
        return Err(UpdateError::Invalid);
    }
    let mut total = 0_u64;
    let mut files = Vec::with_capacity(manifest.files.len());
    for (name, digest) in manifest.files {
        let Some(relative) = safe_relative(&name) else {
            return Err(UpdateError::Invalid);
        };
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(UpdateError::Invalid);
        }
        let path = payload.join(&relative);
        ensure_no_symlink_ancestors(payload, &path)?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| UpdateError::Invalid)?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(UpdateError::Invalid);
        }
        validate_executable_mode(&metadata, binaries.contains(&name.as_str()))?;
        total = total.saturating_add(metadata.len());
        if total > MAX_UNPACKED_BYTES || hex_file_digest(&path)? != digest.to_ascii_lowercase() {
            return Err(UpdateError::Invalid);
        }
        files.push(UpdateFile {
            relative,
            digest: digest.to_ascii_lowercase(),
            executable: binaries.contains(&name.as_str()),
        });
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(UpdatePlan { files })
}

fn release_target(archive_name: &str) -> Result<&str, UpdateError> {
    let prefix = archive_name
        .strip_prefix("KeelShell-")
        .ok_or(UpdateError::Invalid)?;
    target_triples()
        .into_iter()
        .find(|target| {
            prefix.ends_with(&format!("-{target}.zip"))
                || prefix.ends_with(&format!("-{target}.tar.gz"))
        })
        .ok_or(UpdateError::Invalid)
}

fn platform_name() -> &'static str {
    match env::consts::OS {
        "macos" => "macos",
        "linux" => "linux",
        "windows" => "windows",
        _ => "unknown",
    }
}

fn packaged_binary_path(platform: &str) -> Option<&'static str> {
    match platform {
        "macos" => Some("KeelShell.app/Contents/MacOS/keelshell-app"),
        "linux" => Some("usr/bin/keelshell-app"),
        "windows" => Some("keelshell-app.exe"),
        _ => None,
    }
}

fn packaged_mcp_path(platform: &str) -> Option<&'static str> {
    match platform {
        "macos" => Some("KeelShell.app/Contents/MacOS/keelshell-mcp"),
        "linux" => Some("usr/bin/keelshell-mcp"),
        "windows" => Some("keelshell-mcp.exe"),
        _ => None,
    }
}

fn validate_executable_mode(metadata: &fs::Metadata, executable: bool) -> Result<(), UpdateError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if executable && metadata.permissions().mode() & 0o111 == 0 {
            return Err(UpdateError::Invalid);
        }
    }
    #[cfg(not(unix))]
    let _ = (metadata, executable);
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_manifest_receipt(manifest: &PackageManifest) -> Result<[&'static str; 2], UpdateError> {
    if !is_sha256(&manifest.binary_sha256)
        || !is_sha256(&manifest.icon_source_sha256)
        || manifest.installed
        || manifest.signed_by_packaging_script
        || manifest.native_acceptance != "not performed by this script"
    {
        return Err(UpdateError::Invalid);
    }
    let binary = packaged_binary_path(&manifest.platform).ok_or(UpdateError::Invalid)?;
    let digest = manifest.files.get(binary).ok_or(UpdateError::Invalid)?;
    if !is_sha256(digest) || !digest.eq_ignore_ascii_case(&manifest.binary_sha256) {
        return Err(UpdateError::Invalid);
    }
    let companion = packaged_mcp_path(&manifest.platform).ok_or(UpdateError::Invalid)?;
    let companion_digest = manifest
        .mcp_binary_sha256
        .as_deref()
        .ok_or(UpdateError::IncompletePackage)?;
    let digest = manifest
        .files
        .get(companion)
        .ok_or(UpdateError::IncompletePackage)?;
    if !is_sha256(companion_digest)
        || !is_sha256(digest)
        || !digest.eq_ignore_ascii_case(companion_digest)
    {
        return Err(UpdateError::Invalid);
    }
    Ok([binary, companion])
}

fn hex_file_digest(path: &Path) -> Result<String, UpdateError> {
    let mut file = File::open(path).map_err(|_| UpdateError::Storage)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|_| UpdateError::Storage)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex_digest(&digest.finalize()))
}

fn safe_relative(value: &str) -> Option<PathBuf> {
    if value.is_empty() || value.contains('\\') || value.contains('\0') || value.starts_with('/') {
        return None;
    }
    let mut path = PathBuf::new();
    for part in value.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            return None;
        }
        path.push(part);
    }
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Some(path)
    } else {
        None
    }
}

fn installation_root(current_exe: &Path, target: &str) -> Option<PathBuf> {
    if !current_exe.is_absolute() {
        return None;
    }
    match target {
        target if target.contains("apple-darwin") => current_exe
            .ancestors()
            .find(|path| path.file_name().is_some_and(|name| name == "KeelShell.app"))
            .and_then(Path::parent)
            .map(Path::to_path_buf),
        target if target.contains("unknown-linux") => {
            let binary = Path::new("usr").join("bin").join("keelshell-app");
            let mut components = current_exe.components().collect::<Vec<_>>();
            let suffix = binary.components().collect::<Vec<_>>();
            if components.len() < suffix.len()
                || components.split_off(components.len() - suffix.len()) != suffix
            {
                return None;
            }
            let mut root = PathBuf::new();
            for component in components {
                root.push(component.as_os_str());
            }
            Some(root)
        }
        target if target.contains("windows-msvc") => current_exe.parent().map(Path::to_path_buf),
        _ => None,
    }
}

fn spawn_update_helper(
    payload: PathBuf,
    install_root: PathBuf,
    current_exe: PathBuf,
    target: String,
) -> Result<(), UpdateError> {
    if !payload.is_absolute() || !install_root.is_absolute() || !current_exe.is_absolute() {
        return Err(UpdateError::Install);
    }
    // Keep the helper executable outside the installation tree. Windows keeps
    // the running image locked, so launching the same path would prevent the
    // helper from replacing the old executable. A private copy also avoids
    // changing the image that is still serving the UI until the UI exits.
    let helper_name = format!(
        "keelshell-update-helper-{}-{}{}",
        std::process::id(),
        uuid::Uuid::new_v4(),
        if cfg!(windows) { ".exe" } else { "" }
    );
    let helper = env::temp_dir().join(helper_name);
    fs::copy(&current_exe, &helper).map_err(|_| UpdateError::Install)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = match fs::metadata(&current_exe) {
            Ok(metadata) => metadata.permissions().mode(),
            Err(_) => {
                let _ = fs::remove_file(&helper);
                return Err(UpdateError::Install);
            }
        };
        if fs::set_permissions(&helper, fs::Permissions::from_mode(mode & 0o777)).is_err() {
            let _ = fs::remove_file(&helper);
            return Err(UpdateError::Install);
        }
    }
    let result = Command::new(&helper)
        .arg(UPDATE_HELPER_ARG)
        .arg(payload)
        .arg(install_root)
        .arg(current_exe)
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|_| UpdateError::Install);
    if result.is_err() {
        let _ = fs::remove_file(helper);
    }
    result
}

fn apply_update(
    payload: &Path,
    install_root: &Path,
    current_exe: &Path,
    target: &str,
) -> Result<CommittedUpdate, UpdateError> {
    // Check before executable, manifest, or other ordinary preflight errors:
    // a prior failed restore can legitimately leave the executable absent.
    ensure_no_update_recovery(payload)?;
    if !payload.is_absolute() || !install_root.is_absolute() || !current_exe.is_absolute() {
        return Err(UpdateError::Install);
    }
    let payload_metadata = fs::symlink_metadata(payload).map_err(|_| UpdateError::Install)?;
    let install_metadata = fs::symlink_metadata(install_root).map_err(|_| UpdateError::Install)?;
    let executable_metadata =
        fs::symlink_metadata(current_exe).map_err(|_| UpdateError::Install)?;
    if !payload_metadata.file_type().is_dir()
        || !install_metadata.file_type().is_dir()
        || !executable_metadata.file_type().is_file()
        || !current_exe.starts_with(install_root)
    {
        return Err(UpdateError::Install);
    }
    // Helper mode does not receive the release API object. Read and validate
    // the manifest directly while retaining the target and version checks.
    let plan = validate_payload_for_helper(payload, target)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match install_plan(payload, install_root, &plan) {
            Ok(()) => {
                return Ok(CommittedUpdate {
                    payload: payload.to_path_buf(),
                    executable: current_exe.to_path_buf(),
                });
            }
            Err(UpdateError::Busy) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(error) => return Err(error),
        }
    }
}

fn validate_payload_for_helper(payload: &Path, target: &str) -> Result<UpdatePlan, UpdateError> {
    let path = payload.join("package-manifest.json");
    let metadata = fs::symlink_metadata(&path).map_err(|_| UpdateError::Invalid)?;
    if !metadata.file_type().is_file() || metadata.len() > 16 * 1024 * 1024 {
        return Err(UpdateError::Invalid);
    }
    let manifest: PackageManifest =
        serde_json::from_slice(&fs::read(&path).map_err(|_| UpdateError::Storage)?)
            .map_err(|_| UpdateError::Invalid)?;
    let binaries = validate_manifest_receipt(&manifest)?;
    let expected_target = target_triples()
        .into_iter()
        .find(|candidate| *candidate == target)
        .ok_or(UpdateError::Invalid)?;
    if manifest.schema_version != 1
        || manifest.platform != platform_name()
        || manifest.target.as_deref() != Some(expected_target)
        || !is_newer(&format!("v{}", manifest.version), CURRENT_VERSION)
        || manifest.files.len() > MAX_ARCHIVE_ENTRIES
    {
        return Err(UpdateError::Invalid);
    }
    let mut total = 0_u64;
    let mut files = Vec::with_capacity(manifest.files.len());
    for (name, digest) in manifest.files {
        let relative = safe_relative(&name).ok_or(UpdateError::Invalid)?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(UpdateError::Invalid);
        }
        let source = payload.join(&relative);
        ensure_no_symlink_ancestors(payload, &source)?;
        let metadata = fs::symlink_metadata(&source).map_err(|_| UpdateError::Invalid)?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(UpdateError::Invalid);
        }
        validate_executable_mode(&metadata, binaries.contains(&name.as_str()))?;
        total = total.saturating_add(metadata.len());
        if total > MAX_UNPACKED_BYTES || hex_file_digest(&source)? != digest.to_ascii_lowercase() {
            return Err(UpdateError::Invalid);
        }
        files.push(UpdateFile {
            relative,
            digest: digest.to_ascii_lowercase(),
            executable: binaries.contains(&name.as_str()),
        });
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(UpdatePlan { files })
}

fn install_plan(payload: &Path, install_root: &Path, plan: &UpdatePlan) -> Result<(), UpdateError> {
    let backup_root = claim_update_backup(payload, |path| fs::create_dir(path))?;
    let mut installed = Vec::new();
    let mut backups = Vec::new();
    for file in &plan.files {
        let result = (|| {
            let source = payload.join(&file.relative);
            let destination = install_root.join(&file.relative);
            if !destination.starts_with(install_root) || !source.starts_with(payload) {
                return Err(UpdateError::Install);
            }
            ensure_no_symlink_ancestors(install_root, &destination)?;
            ensure_no_symlink_ancestors(payload, &source)?;
            let metadata = fs::symlink_metadata(&source).map_err(|_| UpdateError::Invalid)?;
            if !metadata.file_type().is_file() {
                return Err(UpdateError::Invalid);
            }
            validate_executable_mode(&metadata, file.executable)?;
            let source_digest = hex_file_digest(&source)?;
            if source_digest != file.digest {
                return Err(UpdateError::Checksum);
            }
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|_| UpdateError::Storage)?;
            }
            if fs::symlink_metadata(&destination).is_ok() {
                let metadata = fs::symlink_metadata(&destination).map_err(|_| UpdateError::Busy)?;
                if !metadata.file_type().is_file() {
                    return Err(UpdateError::Install);
                }
                let backup = backup_root.join(&file.relative);
                if let Some(parent) = backup.parent() {
                    fs::create_dir_all(parent).map_err(|_| UpdateError::Storage)?;
                }
                fs::rename(&destination, &backup).map_err(|_| UpdateError::Busy)?;
                backups.push((destination.clone(), backup));
            }
            copy_new_file(&source, &destination)?;
            Ok(destination)
        })();
        match result {
            Ok(destination) => installed.push(destination),
            Err(error) => {
                rollback_install(&installed, &backups)?;
                fs::remove_dir_all(&backup_root).map_err(|_| UpdateError::RecoveryRequired)?;
                return Err(error);
            }
        }
    }
    Ok(())
}

fn claim_update_backup(
    payload: &Path,
    create_directory: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<PathBuf, UpdateError> {
    ensure_no_update_recovery(payload)?;
    let backup = payload.join(".backup");
    // Another helper can claim the namespace after the absence check. Any
    // failed atomic claim leaves its ownership ambiguous; never clean it up.
    create_directory(&backup).map_err(|_| UpdateError::RecoveryRequired)?;
    Ok(backup)
}

fn ensure_no_symlink_ancestors(root: &Path, destination: &Path) -> Result<(), UpdateError> {
    let relative = destination
        .strip_prefix(root)
        .map_err(|_| UpdateError::Install)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        if current.exists()
            && fs::symlink_metadata(&current)
                .map_err(|_| UpdateError::Busy)?
                .file_type()
                .is_symlink()
        {
            return Err(UpdateError::Install);
        }
    }
    Ok(())
}

fn copy_new_file(source: &Path, destination: &Path) -> Result<(), UpdateError> {
    let parent = destination.parent().ok_or(UpdateError::Install)?;
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(UpdateError::Install)?;
    let temporary = parent.join(format!(
        ".{name}.keelshell-update-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let result = (|| {
        let mut input = File::open(source).map_err(|_| UpdateError::Storage)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    UpdateError::Busy
                } else {
                    UpdateError::Storage
                }
            })?;
        io::copy(&mut input, &mut output).map_err(|_| UpdateError::Storage)?;
        output.sync_all().map_err(|_| UpdateError::Storage)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = input
                .metadata()
                .map(|metadata| metadata.permissions().mode())
                .map_err(|_| UpdateError::Storage)?;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(mode & 0o777))
                .map_err(|_| UpdateError::Storage)?;
        }
        // The destination was moved to the backup before this function is
        // called. Renaming a fully written and synced temporary file avoids
        // exposing a partially copied executable if the helper is stopped
        // during the copy.
        fs::rename(&temporary, destination).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                UpdateError::Busy
            } else {
                UpdateError::Storage
            }
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn rollback_install(
    installed: &[PathBuf],
    backups: &[(PathBuf, PathBuf)],
) -> Result<(), UpdateError> {
    let mut failed = false;
    for path in installed.iter().rev() {
        if let Err(error) = fs::remove_file(path) {
            failed |= error.kind() != io::ErrorKind::NotFound;
        }
    }
    for (destination, backup) in backups.iter().rev() {
        failed |= fs::rename(backup, destination).is_err();
    }
    if failed {
        Err(UpdateError::RecoveryRequired)
    } else {
        Ok(())
    }
}

async fn read_bounded(
    mut response: reqwest::Response,
    maximum: u64,
) -> Result<Vec<u8>, UpdateError> {
    if response.status() != StatusCode::OK {
        return Err(UpdateError::Http(response.status().as_u16()));
    }
    if response.content_length().is_some_and(|size| size > maximum) {
        return Err(UpdateError::TooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| UpdateError::Network)? {
        if chunk.len() as u64 > maximum.saturating_sub(bytes.len() as u64) {
            return Err(UpdateError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unique_asset<'a>(assets: &'a [GithubAsset], name: &str) -> Result<&'a GithubAsset, UpdateError> {
    let mut matching = assets.iter().filter(|asset| asset.name == name);
    let asset = matching.next().ok_or(UpdateError::AssetMissing)?;
    if matching.next().is_some() {
        return Err(UpdateError::Invalid);
    }
    Ok(asset)
}

fn http_client(download: bool) -> Result<reqwest::Client, UpdateError> {
    let redirects = if download {
        reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3 || !allowed_download_redirect(attempt.url()) {
                attempt.error("release redirect is not allowed")
            } else {
                attempt.follow()
            }
        })
    } else {
        reqwest::redirect::Policy::none()
    };
    let builder = reqwest::Client::builder();
    #[cfg(test)]
    let builder = builder.no_proxy();
    builder
        .timeout(Duration::from_secs(if download { 300 } else { 20 }))
        .connect_timeout(Duration::from_secs(8))
        .redirect(redirects)
        .retry(reqwest::retry::never())
        .user_agent(format!("KeelShell/{CURRENT_VERSION}"))
        .build()
        .map_err(|_| UpdateError::Network)
}

fn allowed_download_redirect(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none_or(|port| port == 443)
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn valid_project_url(url: &str) -> bool {
    url.starts_with("https://github.com/cyruss648/keelshell/")
}

fn valid_download_url(url: &str) -> bool {
    url.starts_with("https://github.com/cyruss648/keelshell/releases/download/")
}

fn package_name(tag: &str, target: &str) -> Option<String> {
    let version = tag.strip_prefix('v')?;
    if parse_version(version).is_none() || !target_triples().contains(&target) {
        return None;
    }
    let extension = if target.contains("unknown-linux") {
        "tar.gz"
    } else {
        "zip"
    };
    Some(format!("KeelShell-{version}-{target}.{extension}"))
}

fn target_triple() -> Option<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    match (os, arch) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin".into()),
        ("macos", "x86_64") => Some("x86_64-apple-darwin".into()),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu".into()),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu".into()),
        ("windows", "aarch64") => Some("aarch64-pc-windows-msvc".into()),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc".into()),
        _ => None,
    }
}

fn target_triples() -> [&'static str; 6] {
    [
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-gnu",
        "aarch64-pc-windows-msvc",
        "x86_64-pc-windows-msvc",
    ]
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let values: Vec<_> = value.split('.').collect();
    if values.len() != 3
        || values.iter().any(|part| {
            part.is_empty()
                || (part.len() > 1 && part.starts_with('0'))
                || !part.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return None;
    }
    Some((
        values[0].parse().ok()?,
        values[1].parse().ok()?,
        values[2].parse().ok()?,
    ))
}

fn is_newer(remote: &str, current: &str) -> bool {
    remote
        .strip_prefix('v')
        .and_then(parse_version)
        .zip(parse_version(current))
        .is_some_and(|(remote, current)| remote > current)
}

fn parse_checksum(value: &str, file_name: &str) -> Option<String> {
    let mut lines = value.lines().filter(|line| !line.trim().is_empty());
    let line = lines.next()?.trim();
    if lines.next().is_some() {
        return None;
    }
    let mut fields = line.split_whitespace();
    let checksum = fields.next()?.to_ascii_lowercase();
    let name = fields.next()?.trim_start_matches('*');
    if fields.next().is_some()
        || checksum.len() != 64
        || !checksum.chars().all(|c| c.is_ascii_hexdigit())
        || name != file_name
    {
        return None;
    }
    Some(checksum)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateError {
    Network,
    Http(u16),
    TooLarge,
    Invalid,
    IncompletePackage,
    AssetMissing,
    Checksum,
    Storage,
    Busy,
    Install,
    RecoveryRequired,
    Cancelled,
}

impl UpdateError {
    fn message(self) -> Message {
        match self {
            Self::Cancelled => Message::new(
                "更新请求已取消，未安装。",
                "Update request cancelled; nothing was installed.",
            ),
            Self::Network => Message::new(
                "网络请求失败或超时，请稍后重试。",
                "The network request failed or timed out. Try again later.",
            ),
            Self::Http(status) => Message::detail(
                "发布服务返回 HTTP 状态",
                "Release service returned HTTP status",
                status,
            ),
            Self::AssetMissing => Message::new(
                "此版本缺少当前平台安装包或校验文件，请查看 GitHub 发布页。",
                "The release has no matching platform asset or checksum. See the GitHub release page.",
            ),
            Self::TooLarge => Message::new(
                "响应超过大小上限，已停止。",
                "The response exceeded its size limit and was stopped.",
            ),
            Self::Invalid => Message::new(
                "发布元数据或校验文件无效，已停止。",
                "Release metadata or checksum data was invalid. The operation stopped.",
            ),
            Self::IncompletePackage => Message::new(
                "安装包缺少必需的 MCP 伴随程序或校验信息，已停止更新。请从发布页获取完整的新版本安装包。",
                "The package lacks the required MCP companion or its checksum. Updating stopped. Get a complete newer package from the release page.",
            ),
            Self::Checksum => Message::new(
                "SHA-256 校验不匹配，未保存安装包。",
                "SHA-256 mismatch. The package was not saved.",
            ),
            Self::Storage => Message::new(
                "无法保存已校验安装包，请检查本机存储空间。",
                "Unable to save the verified package. Check local storage.",
            ),
            Self::Busy => Message::new(
                "当前安装目录正在使用中，自动安装已回滚；请稍后重试。",
                "The installation directory is busy. The update was rolled back; try again later.",
            ),
            Self::Install => Message::new(
                "自动安装失败，原安装未被替换。请从发布页手动安装。",
                "Automatic installation failed and the existing installation was preserved. Install manually from the release page.",
            ),
            Self::RecoveryRequired => Message::new(
                "自动安装回滚未完成，已保留更新暂存目录和原文件备份并停止重启。请先恢复备份再重试。",
                "Update rollback did not complete. Staging and old-file backups were preserved, and restart stopped. Restore the backups before retrying.",
            ),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    mod automatic;

    fn set_test_executable(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable mode");
        }
        #[cfg(not(unix))]
        let _ = path;
    }

    fn companion_payload() -> (tempfile::TempDir, PathBuf, String, serde_json::Value) {
        let directory = tempfile::tempdir().expect("temporary update directory");
        let payload = directory.path().join("payload");
        let binary_name = packaged_binary_path(platform_name()).expect("platform app path");
        let companion_name = packaged_mcp_path(platform_name()).expect("platform MCP path");
        for (name, bytes) in [
            (binary_name, b"new application".as_slice()),
            (companion_name, b"new companion".as_slice()),
        ] {
            let path = payload.join(name);
            fs::create_dir_all(path.parent().expect("executable parent"))
                .expect("executable parent");
            fs::write(&path, bytes).expect("executable");
            set_test_executable(&path);
        }
        let binary_digest =
            hex_file_digest(&payload.join(binary_name)).expect("application digest");
        let companion_digest =
            hex_file_digest(&payload.join(companion_name)).expect("companion digest");
        let target = target_triple().expect("release target");
        let manifest = serde_json::json!({
            "schema_version": 1, "platform": platform_name(), "version": "99.0.0", "target": target,
            "binary_sha256": binary_digest, "mcp_binary_sha256": companion_digest,
            "icon_source_sha256": "0".repeat(64), "installed": false, "signed_by_packaging_script": false,
            "native_acceptance": "not performed by this script",
            "files": {binary_name: binary_digest, companion_name: companion_digest},
        });
        write_test_manifest(&payload, &manifest);
        (directory, payload, target, manifest)
    }

    fn write_test_manifest(payload: &Path, manifest: &serde_json::Value) {
        fs::write(
            payload.join("package-manifest.json"),
            serde_json::to_vec(manifest).expect("manifest JSON"),
        )
        .expect("manifest");
    }

    fn owned_companion_payload() -> (tempfile::TempDir, PathBuf, String) {
        let (_original, source, target, _manifest) = companion_payload();
        let directory = tempfile::Builder::new()
            .prefix("keelshell-update-repeat-entry-test-")
            .tempdir()
            .expect("owned staging");
        let payload = directory.path().join("payload");
        fs::rename(source, &payload).expect("move isolated payload into owned stage");
        (directory, payload, target)
    }

    fn assert_repeated_recovery_entry_preserves_backup(scenario: &str) {
        let (directory, payload, target) = owned_companion_payload();
        let install = tempfile::tempdir().expect("isolated installation");
        let binary_name = packaged_binary_path(platform_name()).expect("binary");
        let binary = install.path().join(binary_name);
        fs::create_dir_all(binary.parent().expect("parent")).expect("binary parent");
        fs::write(&binary, b"old application").expect("old application");
        let backup = payload.join(".backup").join(binary_name);
        fs::create_dir_all(backup.parent().expect("parent")).expect("backup parent");
        fs::write(&backup, b"only recoverable old image").expect("old backup");
        let first_result = apply_update(&payload, install.path(), &binary, &target);
        assert_eq!(
            first_result.as_ref().err().copied(),
            Some(UpdateError::RecoveryRequired)
        );
        assert!(!finish_update_staging(&payload, first_result));
        match scenario {
            "invalid-manifest" => fs::write(payload.join("package-manifest.json"), b"invalid")
                .expect("invalid manifest"),
            "missing-manifest" => {
                fs::remove_file(payload.join("package-manifest.json")).expect("remove manifest")
            }
            "missing-executable" => fs::remove_file(&binary).expect("remove executable"),
            _ => panic!("unknown recovery fixture scenario"),
        }
        let result = apply_update(&payload, install.path(), &binary, &target);
        let error = result.as_ref().err().copied();
        let restart = finish_update_staging(&payload, result);
        eprintln!(
            "repeated recovery {scenario}: error={:?}, restart={restart}, backup={}",
            error,
            backup.exists()
        );
        assert!(!restart, "recovery entry must not permit restart");
        assert_eq!(error, Some(UpdateError::RecoveryRequired));
        assert_eq!(
            fs::read(backup).expect("unique old image retained"),
            b"only recoverable old image"
        );
        assert!(directory.path().exists());
    }

    #[::core::prelude::v1::test]
    fn repeated_recovery_entry_preserves_backup_when_executable_is_missing() {
        assert_repeated_recovery_entry_preserves_backup("missing-executable");
    }

    #[::core::prelude::v1::test]
    fn repeated_recovery_entry_preserves_backup_when_manifest_is_invalid() {
        assert_repeated_recovery_entry_preserves_backup("invalid-manifest");
    }

    #[::core::prelude::v1::test]
    fn repeated_recovery_entry_preserves_backup_when_manifest_is_missing() {
        assert_repeated_recovery_entry_preserves_backup("missing-manifest");
    }

    #[::core::prelude::v1::test]
    fn ordinary_error_finalization_preserves_unclassified_backup() {
        let (directory, payload, _target) = owned_companion_payload();
        let backup = payload.join(".backup/old-image");
        fs::create_dir(backup.parent().expect("parent")).expect("backup directory");
        fs::write(&backup, b"only old image").expect("backup");
        assert!(!finish_update_staging(&payload, Err(UpdateError::Install)));
        assert_eq!(fs::read(backup).expect("retained image"), b"only old image");
        assert!(directory.path().exists());
    }

    #[::core::prelude::v1::test]
    fn committed_result_cannot_classify_another_stagings_backup() {
        let (_committed_stage, payload, target) = owned_companion_payload();
        let install = tempfile::tempdir().expect("isolated installation");
        let binary = install
            .path()
            .join(packaged_binary_path(platform_name()).expect("binary"));
        fs::create_dir_all(binary.parent().expect("parent")).expect("binary parent");
        fs::write(&binary, b"old application").expect("old application");
        let committed = apply_update(&payload, install.path(), &binary, &target)
            .expect("committed isolated update");
        let (other_directory, other_payload, _target) = owned_companion_payload();
        let backup = other_payload.join(".backup/old-image");
        fs::create_dir(backup.parent().expect("parent")).expect("other backup");
        fs::write(&backup, b"only other old image").expect("other image");
        assert!(!finish_update_staging(&other_payload, Ok(committed)));
        assert_eq!(
            fs::read(backup).expect("other old image retained"),
            b"only other old image"
        );
        assert!(other_directory.path().exists());
    }

    #[::core::prelude::v1::test]
    fn backup_claim_race_keeps_the_other_attempts_old_image() {
        let (directory, payload, _target) = owned_companion_payload();
        let result = claim_update_backup(&payload, |backup| {
            // Insert a real competing claim after the absence check and before
            // this attempt's atomic mkdir, without relying on scheduler timing.
            fs::create_dir(backup)?;
            fs::write(backup.join("old-image"), b"only competing old image")?;
            fs::create_dir(backup)
        });
        assert_eq!(result.err(), Some(UpdateError::RecoveryRequired));
        assert!(!finish_update_staging(&payload, Err(UpdateError::Storage)));
        assert_eq!(
            fs::read(payload.join(".backup/old-image")).expect("competing old image retained"),
            b"only competing old image"
        );
        assert!(directory.path().exists());
    }

    #[::core::prelude::v1::test]
    fn recovery_marker_alone_blocks_entry_and_ordinary_error_finalization() {
        let (directory, payload, target) = owned_companion_payload();
        let marker = payload.join("recovery-required.txt");
        fs::write(&marker, b"preserved recovery instructions").expect("marker");
        let install = tempfile::tempdir().expect("isolated installation");
        let binary = install
            .path()
            .join(packaged_binary_path(platform_name()).expect("binary"));
        assert_eq!(
            apply_update(&payload, install.path(), &binary, &target).err(),
            Some(UpdateError::RecoveryRequired)
        );
        assert!(!finish_update_staging(&payload, Err(UpdateError::Invalid)));
        assert_eq!(
            fs::read(marker).expect("unchanged marker"),
            b"preserved recovery instructions"
        );
        assert!(directory.path().exists());
    }

    #[cfg(unix)]
    #[::core::prelude::v1::test]
    fn backup_metadata_error_is_not_treated_as_absence() {
        let directory = tempfile::Builder::new()
            .prefix("keelshell-update-metadata-test-")
            .tempdir()
            .expect("owned staging");
        let payload = directory.path().join("payload");
        std::os::unix::fs::symlink("payload", &payload).expect("self-referential payload link");
        let error = fs::symlink_metadata(payload.join(".backup")).expect_err("metadata loop");
        assert_ne!(error.kind(), io::ErrorKind::NotFound);
        let install = tempfile::tempdir().expect("isolated installation");
        let binary = install.path().join("missing-executable");
        let target = target_triple().expect("target");
        assert_eq!(
            apply_update(&payload, install.path(), &binary, &target).err(),
            Some(UpdateError::RecoveryRequired)
        );
        assert!(!finish_update_staging(&payload, Err(UpdateError::Install)));
        assert!(directory.path().exists());
        assert!(
            fs::symlink_metadata(payload)
                .expect("preserved payload link")
                .is_symlink()
        );
    }

    #[cfg(unix)]
    #[::core::prelude::v1::test]
    fn recovery_marker_link_does_not_modify_its_external_target() {
        let (directory, payload, target) = owned_companion_payload();
        let outside = tempfile::tempdir().expect("isolated outside marker target");
        let target_file = outside.path().join("instructions");
        fs::write(&target_file, b"keep external instructions").expect("target");
        std::os::unix::fs::symlink(&target_file, payload.join("recovery-required.txt"))
            .expect("marker link");
        let install = tempfile::tempdir().expect("isolated installation");
        let binary = install.path().join("missing-executable");
        let result = apply_update(&payload, install.path(), &binary, &target);
        assert_eq!(
            result.as_ref().err().copied(),
            Some(UpdateError::RecoveryRequired)
        );
        assert!(!finish_update_staging(&payload, result));
        assert_eq!(
            fs::read(target_file).expect("outside instructions preserved"),
            b"keep external instructions"
        );
        assert!(directory.path().exists());
    }

    fn test_release(target: &str) -> ReleaseInfo {
        ReleaseInfo {
            tag: "v99.0.0".into(),
            name: String::new(),
            body: String::new(),
            url: String::new(),
            published_at: None,
            archive_name: package_name("v99.0.0", target).expect("archive name"),
            archive_url: String::new(),
            checksum_url: String::new(),
            archive_size: 0,
        }
    }

    fn assert_both_validators_reject(payload: &Path, target: &str, error: UpdateError) {
        assert_eq!(
            validate_payload(payload, &test_release(target)).err(),
            Some(error)
        );
        assert_eq!(
            validate_payload_for_helper(payload, target).err(),
            Some(error)
        );
    }

    #[::core::prelude::v1::test]
    fn package_names_bind_version_and_target() {
        assert_eq!(
            package_name("v1.2.3", "x86_64-unknown-linux-gnu").as_deref(),
            Some("KeelShell-1.2.3-x86_64-unknown-linux-gnu.tar.gz")
        );
        assert!(package_name("v1.2", "x86_64-unknown-linux-gnu").is_none());
        assert!(package_name("v1.2.3", "unknown").is_none());
    }

    #[::core::prelude::v1::test]
    fn update_comparison_rejects_invalid_or_older_versions() {
        assert!(is_newer("v1.2.4", "1.2.3"));
        assert!(!is_newer("v1.2.3", "1.2.3"));
        assert!(!is_newer("v1.2.2", "1.2.3"));
        assert!(!is_newer("release", "1.2.3"));
    }

    #[::core::prelude::v1::test]
    fn checksum_parser_requires_exact_filename_and_line() {
        let digest = "a".repeat(64);
        assert_eq!(
            parse_checksum(&format!("{digest}  file.zip\n"), "file.zip"),
            Some(digest.clone())
        );
        assert_eq!(
            parse_checksum(&format!("{digest} *file.zip\n"), "file.zip"),
            Some(digest)
        );
        assert!(parse_checksum(&format!("{}  other.zip", "b".repeat(64)), "file.zip").is_none());
        assert!(
            parse_checksum(&format!("{}  file.zip\nextra", "c".repeat(64)), "file.zip").is_none()
        );
    }

    #[::core::prelude::v1::test]
    fn release_urls_are_pinned_to_github_repository() {
        assert!(valid_project_url(
            "https://github.com/cyruss648/keelshell/releases/tag/v1.0.0"
        ));
        assert!(valid_download_url(
            "https://github.com/cyruss648/keelshell/releases/download/v1.0.0/file.zip"
        ));
        assert!(!valid_download_url("https://example.test/file.zip"));
    }

    #[::core::prelude::v1::test]
    fn archive_target_and_relative_paths_are_strict() {
        assert_eq!(
            release_target("KeelShell-1.2.3-x86_64-unknown-linux-gnu.tar.gz").ok(),
            Some("x86_64-unknown-linux-gnu")
        );
        assert_eq!(
            release_target("KeelShell-1.2.3-aarch64-apple-darwin.zip").ok(),
            Some("aarch64-apple-darwin")
        );
        for value in [
            "../escape",
            "/absolute",
            "a/../b",
            "a\\b",
            "C:drive",
            "./name",
        ] {
            assert!(safe_relative(value).is_none(), "{value:?}");
        }
        assert_eq!(
            safe_relative("KeelShell.app/Contents/MacOS/keelshell-app"),
            Some(PathBuf::from("KeelShell.app/Contents/MacOS/keelshell-app"))
        );
    }

    #[::core::prelude::v1::test]
    fn zip_directory_markers_are_normalized_without_relaxing_paths() {
        let directory = tempfile::tempdir().expect("temporary archive directory");
        let archive_path = directory.path().join("package.zip");
        let destination = directory.path().join("payload");
        std::fs::create_dir(&destination).expect("payload");
        let file = File::create(&archive_path).expect("archive");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .add_directory("KeelShell.app/", options)
            .expect("directory");
        writer
            .start_file("KeelShell.app/Contents/file", options)
            .expect("file");
        writer.write_all(b"content").expect("file content");
        writer.finish().expect("archive finish");
        extract_archive(&archive_path, &destination).expect("extract");
        assert_eq!(
            std::fs::read(destination.join("KeelShell.app/Contents/file")).expect("extracted"),
            b"content"
        );
    }

    #[::core::prelude::v1::test]
    fn installation_roots_follow_packaged_layouts() {
        #[cfg(not(windows))]
        {
            assert_eq!(
                installation_root(
                    Path::new("/opt/keelshell/usr/bin/keelshell-app"),
                    "x86_64-unknown-linux-gnu"
                ),
                Some(PathBuf::from("/opt/keelshell"))
            );
            assert_eq!(
                installation_root(
                    Path::new("/Applications/KeelShell.app/Contents/MacOS/keelshell-app"),
                    "aarch64-apple-darwin"
                ),
                Some(PathBuf::from("/Applications"))
            );
        }
        #[cfg(windows)]
        assert_eq!(
            installation_root(
                Path::new(r"C:\Program Files\KeelShell\keelshell-app.exe"),
                "x86_64-pc-windows-msvc"
            ),
            Some(PathBuf::from(r"C:\Program Files\KeelShell"))
        );
    }

    #[::core::prelude::v1::test]
    fn helper_manifest_checks_digest_and_target() {
        let directory = tempfile::tempdir().expect("temporary update directory");
        let payload = directory.path().join("payload");
        std::fs::create_dir_all(&payload).expect("payload");
        let binary_name = packaged_binary_path(platform_name()).expect("current platform");
        let binary = payload.join(binary_name);
        std::fs::create_dir_all(binary.parent().expect("binary parent")).expect("binary parent");
        std::fs::write(&binary, b"verified binary").expect("binary");
        set_test_executable(&binary);
        let digest = hex_file_digest(&binary).expect("digest");
        let companion_name = packaged_mcp_path(platform_name()).expect("current platform MCP");
        let companion = payload.join(companion_name);
        std::fs::write(&companion, b"verified companion").expect("companion");
        set_test_executable(&companion);
        let companion_digest = hex_file_digest(&companion).expect("companion digest");
        let target = target_triple().expect("current release target");
        let manifest = serde_json::json!({
            "schema_version": 1,
            "platform": platform_name(),
            "version": "99.0.0",
            "target": target,
            "binary_sha256": digest,
            "mcp_binary_sha256": companion_digest,
            "icon_source_sha256": "0".repeat(64),
            "installed": false,
            "signed_by_packaging_script": false,
            "native_acceptance": "not performed by this script",
            "files": {binary_name: digest, companion_name: companion_digest},
        });
        std::fs::write(
            payload.join("package-manifest.json"),
            serde_json::to_vec(&manifest).expect("manifest JSON"),
        )
        .expect("manifest");
        let plan = validate_payload_for_helper(&payload, &target).expect("valid manifest");
        assert_eq!(plan.files.len(), 2);
        std::fs::write(&binary, b"tampered").expect("tamper");
        assert!(matches!(
            validate_payload_for_helper(&payload, &target),
            Err(UpdateError::Invalid)
        ));
    }

    #[::core::prelude::v1::test]
    fn helper_manifest_rejects_missing_platform_binary_or_receipt_boundaries() {
        let directory = tempfile::tempdir().expect("temporary update directory");
        let payload = directory.path().join("payload");
        std::fs::create_dir_all(&payload).expect("payload");
        let target = target_triple().expect("current release target");
        let manifest = serde_json::json!({
            "schema_version": 1,
            "platform": platform_name(),
            "version": "99.0.0",
            "target": target,
            "binary_sha256": "0".repeat(64),
            "icon_source_sha256": "0".repeat(64),
            "installed": false,
            "signed_by_packaging_script": false,
            "native_acceptance": "not performed by this script",
            "files": {"package-manifest.json": "0".repeat(64)},
        });
        std::fs::write(
            payload.join("package-manifest.json"),
            serde_json::to_vec(&manifest).expect("manifest JSON"),
        )
        .expect("manifest");
        assert!(matches!(
            validate_payload_for_helper(&payload, &target),
            Err(UpdateError::Invalid)
        ));
    }

    #[::core::prelude::v1::test]
    fn install_plan_replaces_file_and_retains_backup_until_cleanup() {
        let directory = tempfile::tempdir().expect("temporary update directory");
        let payload = directory.path().join("payload");
        let install = directory.path().join("install");
        std::fs::create_dir_all(&payload).expect("payload");
        std::fs::create_dir_all(&install).expect("install");
        let source = payload.join("keelshell-app.exe");
        let destination = install.join("keelshell-app.exe");
        std::fs::write(&source, b"new").expect("source");
        std::fs::write(&destination, b"old").expect("destination");
        let plan = UpdatePlan {
            files: vec![UpdateFile {
                relative: PathBuf::from("keelshell-app.exe"),
                digest: hex_file_digest(&source).expect("source digest"),
                executable: false,
            }],
        };
        install_plan(&payload, &install, &plan).expect("install");
        assert_eq!(std::fs::read(&destination).expect("installed file"), b"new");
        assert_eq!(
            std::fs::read(payload.join(".backup/keelshell-app.exe")).expect("backup"),
            b"old"
        );
    }

    #[::core::prelude::v1::test]
    fn companion_receipt_is_required_by_download_and_helper() {
        let (_directory, payload, target, original) = companion_payload();
        let companion_name = packaged_mcp_path(platform_name()).expect("companion path");
        for missing_field in [true, false] {
            let mut manifest = original.clone();
            if missing_field {
                manifest
                    .as_object_mut()
                    .expect("manifest object")
                    .remove("mcp_binary_sha256");
            } else {
                manifest["files"]
                    .as_object_mut()
                    .expect("files object")
                    .remove(companion_name);
            }
            write_test_manifest(&payload, &manifest);
            assert_both_validators_reject(&payload, &target, UpdateError::IncompletePackage);
        }
        let mut manifest = original;
        manifest["mcp_binary_sha256"] = serde_json::json!("0".repeat(64));
        write_test_manifest(&payload, &manifest);
        assert_both_validators_reject(&payload, &target, UpdateError::Invalid);
    }

    #[::core::prelude::v1::test]
    fn companion_bytes_and_regular_file_are_required_before_installation() {
        let (_directory, payload, target, _manifest) = companion_payload();
        let companion = payload.join(packaged_mcp_path(platform_name()).expect("companion path"));
        fs::write(&companion, b"changed companion").expect("tamper companion");
        assert_both_validators_reject(&payload, &target, UpdateError::Invalid);
        fs::remove_file(&companion).expect("remove companion");
        assert_both_validators_reject(&payload, &target, UpdateError::Invalid);
        fs::create_dir(&companion).expect("directory instead of executable");
        assert_both_validators_reject(&payload, &target, UpdateError::Invalid);
    }

    #[cfg(unix)]
    #[::core::prelude::v1::test]
    fn both_executable_modes_are_required_by_download_helper_and_install() {
        use std::os::unix::fs::PermissionsExt;
        let (directory, payload, target, _manifest) = companion_payload();
        let plan = validate_payload_for_helper(&payload, &target).expect("validated plan");
        let install = directory.path().join("install");
        fs::create_dir(&install).expect("install directory");
        for name in [
            packaged_binary_path(platform_name()).expect("binary"),
            packaged_mcp_path(platform_name()).expect("companion"),
        ] {
            let path = payload.join(name);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("nonexecutable");
            assert_both_validators_reject(&payload, &target, UpdateError::Invalid);
            assert!(matches!(
                install_plan(&payload, &install, &plan),
                Err(UpdateError::Invalid)
            ));
            assert!(
                !install
                    .join(packaged_binary_path(platform_name()).expect("binary"))
                    .exists()
            );
            assert!(
                !install
                    .join(packaged_mcp_path(platform_name()).expect("companion"))
                    .exists()
            );
            set_test_executable(&path);
        }
    }

    #[cfg(unix)]
    #[::core::prelude::v1::test]
    fn companion_symlink_is_rejected_by_both_validators() {
        let (directory, payload, target, _manifest) = companion_payload();
        let companion = payload.join(packaged_mcp_path(platform_name()).expect("companion"));
        let outside = directory.path().join("outside");
        fs::rename(&companion, &outside).expect("move companion");
        std::os::unix::fs::symlink(&outside, &companion).expect("companion symlink");
        assert_both_validators_reject(&payload, &target, UpdateError::Install);
    }

    #[::core::prelude::v1::test]
    fn validated_update_installs_app_and_companion_in_isolated_directory() {
        let (directory, payload, target) = owned_companion_payload();
        let installation = tempfile::tempdir().expect("isolated installation");
        let install = installation.path();
        let binary_name = packaged_binary_path(platform_name()).expect("binary");
        let companion_name = packaged_mcp_path(platform_name()).expect("companion");
        for (name, bytes) in [
            (binary_name, b"old application".as_slice()),
            (companion_name, b"old companion".as_slice()),
        ] {
            let path = install.join(name);
            fs::create_dir_all(path.parent().expect("parent")).expect("parent");
            fs::write(path, bytes).expect("old executable");
        }
        let unlisted = install.join("personal-file");
        fs::write(&unlisted, b"preserve").expect("unlisted file");
        let binary = install.join(binary_name);
        let committed = apply_update(&payload, install, &binary, &target).expect("apply update");
        assert_eq!(committed.executable, binary);
        assert_eq!(
            fs::read(install.join(binary_name)).expect("updated app"),
            b"new application"
        );
        assert_eq!(
            fs::read(install.join(companion_name)).expect("updated MCP"),
            b"new companion"
        );
        assert_eq!(
            fs::read(payload.join(".backup").join(binary_name)).expect("old app backup"),
            b"old application"
        );
        assert_eq!(
            fs::read(payload.join(".backup").join(companion_name)).expect("old MCP backup"),
            b"old companion"
        );
        assert_eq!(fs::read(unlisted).expect("unlisted file"), b"preserve");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(install.join(companion_name))
                    .expect("installed mode")
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
        }
        assert!(finish_update_staging(&payload, Ok(committed)));
        assert!(!directory.path().exists(), "committed backup cleanup");
        assert_eq!(
            fs::read(binary).expect("installed app remains"),
            b"new application"
        );
        assert_eq!(
            fs::read(install.join(companion_name)).expect("installed MCP remains"),
            b"new companion"
        );
    }

    #[::core::prelude::v1::test]
    fn late_failure_restores_both_images_or_removes_new_companion() {
        for old_companion in [true, false] {
            let (directory, payload, target, mut manifest) = companion_payload();
            let failure_name = "zz-late-failure";
            let failure = payload.join(failure_name);
            fs::write(&failure, b"verified resource").expect("resource");
            manifest["files"][failure_name] =
                serde_json::json!(hex_file_digest(&failure).expect("resource digest"));
            write_test_manifest(&payload, &manifest);
            let plan = validate_payload_for_helper(&payload, &target).expect("valid plan");
            fs::write(&failure, b"changed after validation").expect("late corruption");
            let install = directory.path().join("install");
            let binary_name = packaged_binary_path(platform_name()).expect("binary");
            let companion_name = packaged_mcp_path(platform_name()).expect("companion");
            let binary = install.join(binary_name);
            let companion = install.join(companion_name);
            fs::create_dir_all(binary.parent().expect("parent")).expect("parent");
            fs::write(&binary, b"old application").expect("old app");
            if old_companion {
                fs::write(&companion, b"old companion").expect("old MCP");
            }
            assert_eq!(
                install_plan(&payload, &install, &plan).err(),
                Some(UpdateError::Checksum)
            );
            assert_eq!(fs::read(binary).expect("restored app"), b"old application");
            if old_companion {
                assert_eq!(fs::read(companion).expect("restored MCP"), b"old companion");
            } else {
                assert!(!companion.exists());
            }
            assert!(!payload.join(".backup").exists());
            assert!(!install.join(failure_name).exists());
        }
    }

    #[::core::prelude::v1::test]
    fn failed_rollback_retains_old_backup_and_prevents_next_attempt() {
        let (directory, payload, target, _manifest) = companion_payload();
        let install = directory.path().join("install");
        let binary_name = packaged_binary_path(platform_name()).expect("binary");
        let binary = install.join(binary_name);
        fs::create_dir_all(&binary).expect("blocking directory");
        let backup = payload.join(".backup").join(binary_name);
        fs::create_dir_all(backup.parent().expect("backup parent")).expect("backup parent");
        fs::write(&backup, b"only old application image").expect("old backup");
        assert_eq!(
            rollback_install(&[], &[(binary, backup.clone())]).err(),
            Some(UpdateError::RecoveryRequired)
        );
        let plan = validate_payload_for_helper(&payload, &target).expect("valid plan");
        assert_eq!(
            install_plan(&payload, &install, &plan).err(),
            Some(UpdateError::RecoveryRequired)
        );
        assert_eq!(
            fs::read(backup).expect("preserved old backup"),
            b"only old application image"
        );
        assert!(payload.exists());
    }

    #[::core::prelude::v1::test]
    fn helper_finalization_keeps_failed_recovery_and_blocks_restart() {
        let directory = tempfile::Builder::new()
            .prefix("keelshell-update-test-")
            .tempdir()
            .expect("owned staging");
        let payload = directory.path().join("payload");
        let backup = payload.join(".backup/keelshell-app");
        fs::create_dir_all(backup.parent().expect("parent")).expect("backup parent");
        fs::write(&backup, b"only old image").expect("backup");
        assert!(!finish_update_staging(
            &payload,
            Err(UpdateError::RecoveryRequired)
        ));
        assert_eq!(
            fs::read(&backup).expect("retained backup"),
            b"only old image"
        );
        let diagnostic =
            fs::read_to_string(payload.join("recovery-required.txt")).expect("recovery diagnostic");
        assert!(
            diagnostic.contains("回滚未完成") && diagnostic.contains("rollback did not complete")
        );
        let restored = tempfile::tempdir().expect("isolated restored installation");
        let restored_image = restored.path().join("old-image");
        fs::rename(&backup, &restored_image).expect("explicit old-image restoration");
        fs::remove_dir(payload.join(".backup")).expect("remove restored backup directory");
        fs::remove_file(payload.join("recovery-required.txt")).expect("clear recovery marker");
        assert!(finish_update_staging(&payload, Err(UpdateError::Install)));
        assert!(!directory.path().exists());
        assert_eq!(
            fs::read(restored_image).expect("restored image"),
            b"only old image"
        );
    }

    #[::core::prelude::v1::test]
    fn staging_cleanup_is_owned_until_helper_handoff() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("cleanup runtime");
        let directory = tempfile::tempdir().expect("temporary staging parent");
        let root = directory.path().join("staging");
        std::fs::create_dir(&root).expect("staging root");
        {
            let _cleanup = StageCleanup {
                root: root.clone(),
                armed: true,
                executor: runtime.handle().clone(),
            };
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while root.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !root.exists(),
            "owned background cleanup completes within its bound"
        );

        let root = directory.path().join("handed-off");
        std::fs::create_dir(&root).expect("handed-off root");
        let cleanup = StageCleanup {
            root: root.clone(),
            armed: true,
            executor: runtime.handle().clone(),
        };
        cleanup.disarm();
        assert!(root.exists());
    }

    #[::core::prelude::v1::test]
    fn helper_cleanup_removes_only_our_temp_staging_root() {
        let root = env::temp_dir().join(format!("keelshell-update-test-{}", uuid::Uuid::new_v4()));
        let payload = root.join("payload");
        std::fs::create_dir_all(&payload).expect("payload");
        cleanup_staging_root(&payload);
        assert!(!root.exists());

        let unrelated =
            env::temp_dir().join(format!("keelshell-not-update-{}", uuid::Uuid::new_v4()));
        let unrelated_payload = unrelated.join("payload");
        std::fs::create_dir_all(&unrelated_payload).expect("unrelated payload");
        cleanup_staging_root(&unrelated_payload);
        assert!(unrelated.exists());
        std::fs::remove_dir_all(unrelated).expect("unrelated cleanup");
    }
}
