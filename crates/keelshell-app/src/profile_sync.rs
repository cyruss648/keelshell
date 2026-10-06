//! Reviewable encrypted profile synchronization; blocking work never runs in GPUI.

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod review_tests;
#[cfg(test)]
mod tests;
mod view;
use crate::i18n::{Message, t};
use gpui_kit::{component::input::InputState, *};
use keelshell_core::{
    AppState, ProfileSyncChoice, ProfileSyncError, ProfileSyncPreview, ProfileSyncReview,
    ProfileSyncService, StateStore,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::runtime::Runtime;
use uuid::Uuid;
use zeroize::Zeroizing;

pub enum ProfileSyncEvent {
    Changed {
        state: Box<AppState>,
        message: Message,
    },
    Close,
}
pub struct ProfileSyncPanel {
    store: Arc<StateStore>,
    runtime: Arc<Runtime>,
    directory: Entity<InputState>,
    password: Entity<InputState>,
    forget_confirmation: bool,
    configured: bool,
    enabled: bool,
    pending: bool,
    review: Option<ProfileSyncReview>,
    choices: BTreeMap<Uuid, ProfileSyncChoice>,
    preview: Option<ProfileSyncPreview>,
    effects_acknowledged: bool,
    impact_page: usize,
    page: usize,
    cancellation: Option<Arc<AtomicBool>>,
    close_after_work: bool,
    status: Message,
    _job: Option<Task<()>>,
}
impl EventEmitter<ProfileSyncEvent> for ProfileSyncPanel {}
enum Action {
    Inspect,
    Apply,
    Resume,
    DiscardPending,
    Disable,
    Forget,
}
enum Report {
    Review(ProfileSyncReview),
    State(AppState, bool),
}
impl ProfileSyncPanel {
    pub fn new(
        store: Arc<StateStore>,
        state: &AppState,
        runtime: Arc<Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let directory = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t(
                cx,
                "用户选定的共享或挂载目录（绝对路径）",
                "User-selected shared or mounted directory (absolute path)",
            ))
        });
        if let Some(local) = &state.profile_sync {
            directory.update(cx, |input, cx| {
                input.set_value(local.directory().to_string_lossy(), window, cx)
            });
        }
        let password = cx.new(|cx| {
            InputState::new(window, cx).masked(true).placeholder(t(
                cx,
                "同步密码（仅本次操作）",
                "Sync password (this operation only)",
            ))
        });
        Self {
            store,
            runtime,
            directory,
            password,
            forget_confirmation: false,
            configured: state.profile_sync.is_some(),
            enabled: state.profile_sync.as_ref().is_some_and(|s| s.enabled()),
            pending: state
                .profile_sync
                .as_ref()
                .is_some_and(|s| s.publication_pending()),
            review: None,
            choices: BTreeMap::new(),
            preview: None,
            effects_acknowledged: false,
            impact_page: 0,
            page: 0,
            cancellation: None,
            close_after_work: false,
            status: Message::new(
                "默认关闭。选择两台设备共同访问的目录并输入同一同步密码，先读取差异，再显式批准。",
                "Disabled by default. Choose a directory both devices can access and use the same sync password. Pull differences, then approve explicitly.",
            ),
            _job: None,
        }
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.password.read(cx).focus_handle(cx).focus(window, cx);
    }
    fn busy(&self) -> bool {
        self.cancellation.is_some()
    }
    /// Test-only observation of actual background admission and completion.
    #[cfg(test)]
    pub(crate) fn background_work_pending_for_test(&self) -> bool {
        self.busy()
    }
    /// Test-only state summary: never include a password or perform storage I/O.
    #[cfg(test)]
    pub(crate) fn diagnostics_for_test(&self, cx: &App) -> String {
        format!(
            "busy={} review_rows={:?} choices={} preview={} effects_acknowledged={} password_bytes={} review_snapshot={:?} configured={} enabled={} pending={} status={:?}",
            self.busy(),
            self.review.as_ref().map(|review| review.rows().len()),
            self.choices.len(),
            self.preview.is_some(),
            self.effects_acknowledged,
            self.password.read(cx).value().len(),
            self.review
                .as_ref()
                .map(|review| review.local_state().snapshot),
            self.configured,
            self.enabled,
            self.pending,
            self.status.render(cx),
        )
    }
    fn clear_password(&self, window: &mut Window, cx: &mut App) {
        self.password
            .update(cx, |p, cx| p.set_value("", window, cx));
    }
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_password(window, cx);
        self.review = None;
        self.preview = None;
        self.effects_acknowledged = false;
        self.choices.clear();
        if self.busy() {
            self.close_after_work = true;
            self.cancel(cx);
        } else {
            cx.emit(ProfileSyncEvent::Close);
        }
    }
    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = &self.cancellation {
            c.store(true, Ordering::Release);
            self.status = Message::new(
                "等待后台取消。已开始的本地保存可能完成；未发布的批准保留为待处理记录。",
                "Waiting for cancellation. A local save already admitted may complete; an unpublished approval remains pending.",
            );
            cx.notify();
        }
    }
    fn discard_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.clear_password(window, cx);
        self.review = None;
        self.preview = None;
        self.effects_acknowledged = false;
        self.choices.clear();
        self.status = Message::new(
            "审核已取消，未保存或发布。",
            "Review cancelled; nothing saved or published.",
        );
        cx.notify();
    }
    fn choose(&mut self, id: Uuid, choice: ProfileSyncChoice, cx: &mut Context<Self>) {
        if !self.busy()
            && self
                .review
                .as_ref()
                .is_some_and(|r| r.rows().iter().any(|r| r.id == id))
        {
            self.choices.insert(id, choice);
            self.refresh_preview(cx);
            cx.notify();
        }
    }
    fn refresh_preview(&mut self, cx: &mut Context<Self>) {
        self.preview = None;
        self.effects_acknowledged = false;
        self.impact_page = 0;
        let Some(review) = &self.review else {
            return;
        };
        if review.rows().len() != self.choices.len() {
            return;
        }
        match review.preview(&self.choices) {
            Ok(preview) => {
                self.effects_acknowledged = preview.route_changes.is_empty()
                    && review
                        .rows()
                        .iter()
                        .all(|row| row.local_placement.is_none());
                self.preview = Some(preview);
            }
            Err(error) => self.status = failure(error),
        }
        cx.notify();
    }
    fn request_forget(&mut self, cx: &mut Context<Self>) {
        if self.busy() || self.pending || self.review.is_some() {
            return;
        }
        self.forget_confirmation = true;
        self.status = Message::new(
            "确认解除本机与同步空间的关联？本地连接保留，共享密文不删除，但本机的版本回放保护将丢失。再次确认后可选择新目录重新建立空间。",
            "Forget this device's channel pairing? Local profiles remain and shared ciphertext is not deleted, but this device's rollback protection will be lost. Confirm again to select a new directory/channel.",
        );
        cx.notify();
    }

    fn submit(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if matches!(action, Action::Forget) && !self.forget_confirmation {
            return;
        }
        self.forget_confirmation = false;
        if matches!(action, Action::Apply)
            && self
                .review
                .as_ref()
                .is_none_or(|r| r.rows().len() != self.choices.len())
        {
            return;
        }
        if matches!(action, Action::Apply) && (self.preview.is_none() || !self.effects_acknowledged)
        {
            return;
        }
        let password = Zeroizing::new(self.password.read(cx).value().to_string());
        self.clear_password(window, cx);
        if matches!(action, Action::Inspect | Action::Apply | Action::Resume)
            && (password.is_empty() || password.len() > 4096)
        {
            self.status = Message::new(
                "请输入有效的同步密码，再执行本次操作。",
                "Enter a valid sync password for this operation.",
            );
            self.focus(window, cx);
            cx.notify();
            return;
        }
        let directory = PathBuf::from(self.directory.read(cx).value().to_string());
        let review = if matches!(action, Action::Apply) {
            self.review.take()
        } else {
            None
        };
        let choices = self.choices.clone();
        let store = self.store.clone();
        let cancellation = Arc::new(AtomicBool::new(false));
        self.cancellation = Some(cancellation.clone());
        self.status = Message::new(
            "正在后台验证并处理加密同步…",
            "Authenticating and processing encrypted sync in the background…",
        );
        let task = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move {
                tokio::task::spawn_blocking(move || {
                    let service = ProfileSyncService::new(store.clone());
                    let result = match action {
                        Action::Inspect => service
                            .inspect(directory, password, &cancellation)
                            .map(Report::Review),
                        Action::Apply => match review {
                            Some(review) => service
                                .apply(review, choices, password, &cancellation)
                                .map(|outcome| Report::State(outcome.state, outcome.published)),
                            None => Err(ProfileSyncError::Incomplete),
                        },
                        Action::Resume => service
                            .resume(password, &cancellation)
                            .map(|outcome| Report::State(outcome.state, outcome.published)),
                        Action::Forget => service
                            .forget(&cancellation)
                            .map(|s| Report::State(s, false)),
                        Action::Disable => service
                            .disable(&cancellation)
                            .map(|s| Report::State(s, false)),
                        Action::DiscardPending => service
                            .discard_pending(&cancellation)
                            .map(|s| Report::State(s, false)),
                    };
                    match result {
                        Err(ProfileSyncError::Pending) => match store.load() {
                            Ok(state)
                                if state
                                    .profile_sync
                                    .as_ref()
                                    .is_some_and(|s| s.publication_pending()) =>
                            {
                                Ok(Report::State(state, false))
                            }
                            _ => Err(ProfileSyncError::Pending),
                        },
                        result => result,
                    }
                })
                .await
            },
        );
        self._job = Some(cx.spawn_in(window, async move |this, cx| {
            let result = match task.await {
                Ok(Ok(r)) => r,
                _ => Err(ProfileSyncError::Invalid),
            };
            let _ = this.update_in(cx, |panel, window, cx| panel.complete(result, window, cx));
        }));
        cx.notify();
    }
    fn complete(
        &mut self,
        result: Result<Report, ProfileSyncError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancellation = None;
        match result {
            Ok(Report::Review(review)) => {
                cx.emit(ProfileSyncEvent::Changed {
                    state: Box::new(review.local_state().clone()),
                    message: Message::new(
                        "已读取保存的配置以审核同步差异。",
                        "Saved configuration loaded for sync review.",
                    ),
                });
                self.status = Message::new(
                    format!(
                        "已验证 {} 项差异。逐项选择本地或共享版本；再次输入密码并批准后才会保存、启用及发布。",
                        review.rows().len()
                    ),
                    format!(
                        "Verified {} differences. Select local or shared for every row; re-enter the password and approve to save, enable and publish.",
                        review.rows().len()
                    ),
                );
                self.choices.clear();
                self.page = 0;
                self.review = Some(review);
                self.refresh_preview(cx);
            }
            Ok(Report::State(state, published)) => {
                self.enabled = state.profile_sync.as_ref().is_some_and(|s| s.enabled());
                self.pending = state
                    .profile_sync
                    .as_ref()
                    .is_some_and(|s| s.publication_pending());
                self.configured = state.profile_sync.is_some();
                self.review = None;
                self.preview = None;
                self.effects_acknowledged = false;
                self.choices.clear();
                self.status = if published {
                    Message::new(
                        "本地保存与加密共享发布均已完成。其它设备需要读取并审核。",
                        "Local save and encrypted shared publication completed. Other devices must pull and review.",
                    )
                } else if self.pending {
                    Message::new(
                        "本地审核结果已保存，共享发布待完成。可再次输入密码继续；若共享版本已变化，可放弃待发布记录并重新审核，本地选择保留。",
                        "Local approval saved; shared publication is pending. Re-enter the password to resume, or discard the unpublished receipt and review again if the shared version changed. Local choices remain.",
                    )
                } else {
                    Message::new(
                        "设置已保存；连接与当前 SSH 会话保持。",
                        "Settings saved; profiles and active SSH sessions remain.",
                    )
                };
                cx.emit(ProfileSyncEvent::Changed {
                    state: Box::new(state),
                    message: self.status.clone(),
                });
            }
            Err(error) => {
                self.review = None;
                self.preview = None;
                self.effects_acknowledged = false;
                self.choices.clear();
                self.status = failure(error);
            }
        }
        self.clear_password(window, cx);
        if self.close_after_work {
            self.close_after_work = false;
            cx.emit(ProfileSyncEvent::Close);
        }
        cx.notify();
    }
}
impl Drop for ProfileSyncPanel {
    fn drop(&mut self) {
        if let Some(c) = &self.cancellation {
            c.store(true, Ordering::Release);
        }
    }
}
fn failure(error: ProfileSyncError) -> Message {
    match error {
        ProfileSyncError::Storage(keelshell_core::Error::TooLarge) => Message::new(
            "同步快照或待发布本机收据超过容量限制，未截断数据。请减少同步记录、标签或其它本机历史后重新审核；2,000 条是记录上限，不保证任意大小的配置都可同步。",
            "The sync snapshot or pending local receipt exceeds the size limit; data was not truncated. Reduce profiles, labels or other local history and review again. 2,000 is a count ceiling, not a guarantee for arbitrarily large profiles.",
        ),
        ProfileSyncError::Stale => Message::new(
            "审核后本地或共享内容变化，未覆盖；请重新读取。",
            "Local or shared content changed after review; nothing overwritten. Pull again.",
        ),
        ProfileSyncError::Replay => Message::new(
            "拒绝旧快照、删除墓碑丢失或记录重放。保留当前配置，请恢复共享目录的新版本。",
            "Refused rollback, missing tombstones or record replay. Current configuration retained; restore the newer shared snapshot.",
        ),
        ProfileSyncError::Channel => Message::new(
            "目录或同步空间与已保存身份不一致，未读取其它空间。",
            "Directory or channel differs from the saved identity; another space was not accepted.",
        ),
        ProfileSyncError::Pending => Message::new(
            "已有批准的待发布记录，请继续完成或明确放弃该记录。已发布的记录需继续确认。",
            "An approved publication is pending. Resume or explicitly discard its receipt; an already published receipt must be acknowledged by resuming.",
        ),
        ProfileSyncError::Cancelled => Message::new(
            "在保存前取消，未发布。",
            "Cancelled before saving; nothing published.",
        ),
        ProfileSyncError::Busy => Message::new(
            "共享目录正在被另一个客户端使用，请稍后重试。",
            "Another client is using the shared directory. Retry later.",
        ),
        ProfileSyncError::Storage(keelshell_core::Error::Validation(_)) => Message::new(
            "所选连接组合或路线无效，请检查跳板及各项选择并重新审核。",
            "The chosen profile set or route is invalid. Check jump profiles and choices, then review again.",
        ),
        ProfileSyncError::Incomplete => Message::new(
            "请逐项选择所有差异。",
            "Choose a resolution for every difference.",
        ),
        _ => Message::new(
            "同步验证或存储失败。请检查目录、密码与快照完整性；未自动重置数据。",
            "Sync authentication or storage failed. Check directory, password and snapshot integrity; data was not reset automatically.",
        ),
    }
}
