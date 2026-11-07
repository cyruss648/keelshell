//! Process-local window owners and serialization of shared configuration writes.
//!
//! A window owns its transports and consent. Only the store, runtime, updater and
//! committed preferences are shared. The mutex also guards the interval between
//! a worker committing bytes and the UI receiving its completion notification.
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use gpui_kit::*;
use keelshell_core::{AppState, SnapshotRevision, StateStore};
use uuid::Uuid;

use crate::{
    updater::UpdatePanel,
    workspace::{Workspace, window_lifecycle::CloseIntent},
};

#[derive(Default)]
struct Stores {
    windows: HashMap<PathBuf, WeakEntity<AppWindows>>,
    startup_load_errors: HashMap<PathBuf, String>,
}
impl Global for Stores {}

#[derive(Clone)]
pub(crate) struct Registration {
    pub(crate) id: Uuid,
    pub(crate) window: AnyWindowHandle,
    pub(crate) workspace: WeakEntity<Workspace>,
    pub(crate) summary: Arc<Mutex<WindowSummary>>,
}

#[derive(Clone)]
pub(crate) struct WindowSummary {
    pub(crate) id: Uuid,
    pub(crate) sequence: usize,
    pub(crate) session_count: usize,
    pub(crate) context: String,
}

pub(crate) struct AppWindows {
    pub(crate) windows: Vec<Registration>,
    pub(crate) gate: Arc<Mutex<ConfigurationGate>>,
    pub(crate) updater: Entity<UpdatePanel>,
    pub(crate) update_owner: Option<Uuid>,
    pub(crate) quitting: bool,
    pub(crate) quit_install: bool,
    pub(crate) cleaned: std::collections::HashSet<Uuid>,
    pub(crate) cleanup_attempt: Option<Uuid>,
    pub(crate) cleanup_intent: Option<CloseIntent>,
    pub(crate) cleanup_participants: std::collections::HashSet<Uuid>,
    pub(crate) cleanup_attempts: HashMap<Uuid, Uuid>,
    pub(crate) cleanup_aborted: bool,
    next_window_sequence: usize,
}
pub(crate) struct ConfigurationGate {
    state: AppState,
    epoch: u64,
    uncertain: bool,
    load_error: Option<String>,
    writer: Option<Uuid>,
    maintenance: Option<Uuid>,
}

#[derive(Clone, Copy)]
pub(crate) struct ConfigurationChanged;
impl EventEmitter<ConfigurationChanged> for AppWindows {}

/// A single off-thread store transaction. Dropping an unfinished lease makes
/// its outcome unknown; it never silently admits another write or retries.
pub(crate) struct WriteLease {
    gate: Arc<Mutex<ConfigurationGate>>,
    owner: Uuid,
    finished: bool,
}

/// A reload transaction that marks the shared gate uncertain if its worker is
/// cancelled before it publishes a loaded snapshot.
pub(crate) struct ReloadLease {
    gate: Arc<Mutex<ConfigurationGate>>,
    owner: Uuid,
    finished: bool,
}

impl ConfigurationGate {
    fn new_with_load_error(state: AppState, load_error: Option<String>) -> Self {
        Self {
            state,
            epoch: 0,
            uncertain: false,
            load_error,
            writer: None,
            maintenance: None,
        }
    }

    pub(crate) fn current(&self, revision: SnapshotRevision, epoch: u64) -> bool {
        !self.uncertain
            && self.load_error.is_none()
            && self.state.snapshot == revision
            && self.epoch == epoch
    }

    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }

    pub(crate) fn state(&self) -> &AppState {
        &self.state
    }

    pub(crate) fn available(&self) -> bool {
        self.writer.is_none() && self.maintenance.is_none()
    }

    pub(crate) fn allows_new_window(&self) -> bool {
        self.available() && !self.uncertain && self.load_error.is_none()
    }

    pub(crate) fn begin(
        gate: &Arc<Mutex<Self>>,
        owner: Uuid,
        revision: SnapshotRevision,
        epoch: u64,
    ) -> Result<WriteLease, &'static str> {
        let mut state = gate.lock().map_err(|_| "configuration gate poisoned")?;
        if !state.current(revision, epoch) {
            return Err("configuration changed; explicitly reload before reviewing again");
        }
        if !state.available() {
            return Err("another configuration write or maintenance operation is in progress");
        }
        state.writer = Some(owner);
        Ok(WriteLease {
            gate: gate.clone(),
            owner,
            finished: false,
        })
    }

    pub(crate) fn begin_maintenance(&mut self, owner: Uuid, window_count: usize) -> bool {
        if window_count != 1 || !self.available() || self.uncertain {
            return false;
        }
        self.maintenance = Some(owner);
        true
    }

    pub(crate) fn maintenance_owned(&self, owner: Uuid) -> bool {
        self.maintenance == Some(owner)
    }

    pub(crate) fn end_maintenance(&mut self, owner: Uuid) {
        if self.maintenance == Some(owner) {
            self.maintenance = None;
        }
    }

    pub(crate) fn begin_reload_lease(
        gate: &Arc<Mutex<Self>>,
        owner: Uuid,
    ) -> Result<ReloadLease, &'static str> {
        let mut state = gate.lock().map_err(|_| "configuration gate poisoned")?;
        if !state.available() {
            return Err("another configuration write or maintenance operation is in progress");
        }
        state.writer = Some(owner);
        Ok(ReloadLease {
            gate: gate.clone(),
            owner,
            finished: false,
        })
    }

    pub(crate) fn maintenance_committed(&mut self, owner: Uuid, state: &AppState) -> bool {
        if self.maintenance != Some(owner) {
            return false;
        }
        self.state = state.clone();
        self.uncertain = false;
        self.load_error = None;
        self.epoch = self.epoch.wrapping_add(1);
        true
    }
}

impl WriteLease {
    pub(crate) fn cancel_before_dispatch(mut self) {
        if let Ok(mut gate) = self.gate.lock()
            && gate.writer == Some(self.owner)
        {
            gate.writer = None;
            self.finished = true;
        }
    }

    pub(crate) fn finish(mut self, result: &Result<AppState, keelshell_core::Error>) {
        if let Ok(mut gate) = self.gate.lock()
            && gate.writer == Some(self.owner)
        {
            gate.writer = None;
            match result {
                Ok(state) => {
                    gate.state = state.clone();
                    gate.epoch = gate.epoch.wrapping_add(1);
                }
                Err(keelshell_core::Error::Conflict | keelshell_core::Error::Durability(_)) => {
                    gate.uncertain = true;
                    gate.epoch = gate.epoch.wrapping_add(1);
                }
                Err(_) => {}
            }
            self.finished = true;
        }
    }
}

impl ReloadLease {
    pub(crate) fn finish(mut self, state: Option<&AppState>) {
        if let Ok(mut gate) = self.gate.lock()
            && gate.writer == Some(self.owner)
        {
            gate.writer = None;
            gate.epoch = gate.epoch.wrapping_add(1);
            if let Some(state) = state {
                gate.state = state.clone();
                gate.uncertain = false;
                gate.load_error = None;
            } else {
                gate.uncertain = true;
            }
            self.finished = true;
        }
    }
}

impl Drop for WriteLease {
    fn drop(&mut self) {
        if !self.finished
            && let Ok(mut gate) = self.gate.lock()
            && gate.writer == Some(self.owner)
        {
            gate.writer = None;
            gate.uncertain = true;
            gate.epoch = gate.epoch.wrapping_add(1);
        }
    }
}

impl Drop for ReloadLease {
    fn drop(&mut self) {
        if !self.finished
            && let Ok(mut gate) = self.gate.lock()
            && gate.writer == Some(self.owner)
        {
            gate.writer = None;
            gate.uncertain = true;
            gate.epoch = gate.epoch.wrapping_add(1);
        }
    }
}

impl AppWindows {
    pub(crate) fn for_store(
        store: &StateStore,
        state: &AppState,
        runtime: Arc<tokio::runtime::Runtime>,
        cx: &mut App,
    ) -> Entity<Self> {
        if cx.try_global::<Stores>().is_none() {
            cx.set_global(Stores::default());
        }
        let startup_load_error = cx.update_global::<Stores, _>(|stores, _| {
            stores.startup_load_errors.remove(store.path())
        });
        if let Some(shared) = cx
            .global::<Stores>()
            .windows
            .get(store.path())
            .and_then(WeakEntity::upgrade)
        {
            if let Some(error) = startup_load_error {
                shared.update(cx, |shared, _| {
                    if let Ok(mut gate) = shared.gate.lock() {
                        gate.load_error = Some(error);
                    }
                });
            }
            return shared;
        }
        let updater = cx.new(|cx| UpdatePanel::new(runtime, state.settings.updates, cx));
        let shared = cx.new(|_| Self {
            windows: Vec::new(),
            gate: Arc::new(Mutex::new(ConfigurationGate::new_with_load_error(
                state.clone(),
                startup_load_error,
            ))),
            updater,
            update_owner: None,
            quitting: false,
            quit_install: false,
            cleaned: Default::default(),
            cleanup_attempt: None,
            cleanup_intent: None,
            cleanup_participants: Default::default(),
            cleanup_attempts: Default::default(),
            cleanup_aborted: false,
            next_window_sequence: 1,
        });
        cx.update_global::<Stores, _>(|stores, _| {
            stores
                .windows
                .insert(store.path().to_path_buf(), shared.downgrade());
        });
        shared
    }

    pub(crate) fn can_open(&self) -> bool {
        !self.quitting
            && self.windows.len() < keelshell_core::MAX_LAYOUT_WINDOWS
            && self.gate.lock().is_ok_and(|gate| gate.allows_new_window())
    }

    pub(crate) fn register(&mut self, registration: Registration) {
        let sequence = self.next_window_sequence;
        self.next_window_sequence = self.next_window_sequence.saturating_add(1);
        if let Ok(mut summary) = registration.summary.lock() {
            summary.sequence = sequence;
        }
        self.windows.push(registration);
    }

    pub(crate) fn summaries(&self) -> Vec<WindowSummary> {
        self.windows
            .iter()
            .filter_map(|registration| {
                registration
                    .summary
                    .lock()
                    .ok()
                    .map(|summary| summary.clone())
            })
            .collect()
    }

    pub(crate) fn window_handle(&self, id: Uuid) -> Option<AnyWindowHandle> {
        self.windows
            .iter()
            .find(|registration| registration.id == id)
            .map(|registration| registration.window)
    }

    pub(crate) fn begin_aggregate_cleanup(&mut self, ids: &[Uuid], intent: CloseIntent) -> Uuid {
        let attempt = Uuid::new_v4();
        self.quitting = true;
        self.quit_install = intent == CloseIntent::Install;
        self.cleaned.clear();
        self.cleanup_attempt = Some(attempt);
        self.cleanup_intent = Some(intent);
        self.cleanup_participants = ids.iter().copied().collect();
        self.cleanup_aborted = false;
        for id in ids {
            self.cleanup_attempts.insert(*id, attempt);
        }
        attempt
    }

    pub(crate) fn begin_window_cleanup(&mut self, id: Uuid) -> Uuid {
        let attempt = Uuid::new_v4();
        self.cleanup_attempts.insert(id, attempt);
        attempt
    }

    pub(crate) fn cleanup_attempt_for(&self, id: Uuid) -> Option<Uuid> {
        self.cleanup_attempts.get(&id).copied()
    }

    pub(crate) fn aggregate_cleanup_aborted(&self, attempt: Option<Uuid>) -> bool {
        self.cleanup_aborted && attempt.is_none_or(|attempt| self.cleanup_attempt == Some(attempt))
    }

    pub(crate) fn abort_aggregate_cleanup(
        &mut self,
        attempt: Option<Uuid>,
        failed_id: Uuid,
    ) -> Vec<Uuid> {
        let matches_attempt = attempt.is_none_or(|attempt| self.cleanup_attempt == Some(attempt));
        if !matches_attempt || self.cleanup_aborted {
            return Vec::new();
        }
        self.quitting = false;
        self.quit_install = false;
        self.cleaned.clear();
        self.cleanup_aborted = true;
        self.cleanup_participants
            .iter()
            .copied()
            .filter(|id| *id != failed_id)
            .collect()
    }

    pub(crate) fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(ConfigurationChanged);
        cx.notify();
    }
}

pub(crate) fn is_last_window(group_window_count: usize, total_window_count: usize) -> bool {
    group_window_count == 1 && total_window_count == 1
}

pub(crate) fn total_window_count(cx: &App) -> usize {
    cx.try_global::<Stores>()
        .map(|stores| {
            stores
                .windows
                .values()
                .filter_map(WeakEntity::upgrade)
                .map(|shared| shared.read(cx).windows.len())
                .sum()
        })
        .unwrap_or_default()
}

pub(crate) fn open(
    store: Arc<StateStore>,
    state: AppState,
    load_error: Option<String>,
    runtime: Arc<tokio::runtime::Runtime>,
    restore: bool,
    cx: &mut App,
) -> gpui_kit::Result<(AnyWindowHandle, Entity<Workspace>)> {
    if let Some(load_error) = load_error.clone() {
        if cx.try_global::<Stores>().is_none() {
            cx.set_global(Stores::default());
        }
        cx.update_global::<Stores, _>(|stores, _| {
            stores
                .startup_load_errors
                .insert(store.path().to_path_buf(), load_error);
        });
    }
    let options = WindowOptions {
        app_id: Some("keelshell".into()),
        #[cfg(target_os = "linux")]
        icon: crate::linux_window_icon(),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1440.), px(900.)),
            cx,
        ))),
        window_min_size: Some(size(px(900.), px(580.))),
        titlebar: Some(TitlebarOptions {
            title: Some("KeelShell".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| {
            if restore {
                Workspace::new(store, state, load_error, runtime, window, cx)
            } else {
                Workspace::new_window(store, state, load_error, runtime, false, window, cx)
            }
        })
    })
}

/// Explicit application Quit is deferred out of the active action dispatch.
/// The selected workspace presents one aggregate review before any revocation.
pub(crate) fn request_quit(cx: &mut App) {
    let active = cx.active_window().map(|window| window.window_id());
    let registrations: Vec<_> = cx
        .try_global::<Stores>()
        .map(|stores| {
            stores
                .windows
                .values()
                .filter_map(WeakEntity::upgrade)
                .flat_map(|shared| shared.read(cx).windows.clone())
                .collect()
        })
        .unwrap_or_default();
    let owner = registrations
        .iter()
        .find(|entry| Some(entry.window.window_id()) == active)
        .or_else(|| registrations.first())
        .cloned();
    if let Some(owner) = owner {
        cx.defer(move |cx| {
            let _ = owner.window.update(cx, |_, window, cx| {
                let _ = owner.workspace.update(cx, |view, cx| {
                    view.request_window_close(CloseIntent::Quit, window, cx)
                });
            });
        });
    }
}

#[cfg(test)]
mod tests;
