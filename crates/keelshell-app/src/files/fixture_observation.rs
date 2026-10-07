//! Bounded test-only admission history; observations never own a permit or worker.
use super::FixtureGroup;
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Write},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

const EVENT_LIMIT: usize = 256;
const REQUEST_LIMIT: usize = 64;
const GROUP_LIMIT: usize = 8;

/// Numeric identity only; carrying it cannot keep a fixture alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct GroupId(u64);

impl GroupId {
    /// Private failure controls do not participate in process admission.
    pub(super) const UNOBSERVED: Self = Self(0);
}

/// Fixed lifecycle labels contain no transport, command or filesystem values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Kind {
    RequestCreated,
    Acquired,
    GroupBound,
    SameAppReused,
    RequestScopeDropped,
    AdmissionTimeout,
    AdmissionClosed,
    GroupLastOwnerDropped,
    CleanupStarted,
    QueueRegistered,
    QueueReferencesPending,
    QueueCloseStarted,
    QueueCloseFailed,
    QueueSchedulerJoined,
    PermitReleased,
    AdmissionFailedClosed,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Owned,
    Cleanup,
    QueueReferences,
    QueueScheduler,
    FailedClosed,
}

#[derive(Clone, Serialize)]
struct Event {
    sequence: u64,
    elapsed_ms: u64,
    kind: Kind,
    label: &'static str,
    request: Option<u64>,
    group: Option<GroupId>,
    queue_index: Option<usize>,
    count: usize,
}

#[derive(Clone, Serialize)]
struct PendingRequest {
    label: &'static str,
    created_ms: u64,
}

struct Group {
    label: &'static str,
    weak: Option<Weak<FixtureGroup>>,
    phase: Phase,
    queues: usize,
    queue_index: Option<usize>,
    sampled_queue_strong_count: usize,
}

#[derive(Serialize)]
struct GroupSnapshot {
    label: &'static str,
    weak_bound: bool,
    live_group_strong_count: usize,
    phase: Phase,
    queues: usize,
    queue_index: Option<usize>,
    sampled_queue_strong_count: usize,
}

#[derive(Default)]
struct State {
    sequence: u64,
    events_dropped: u64,
    metadata_dropped: u64,
    events: VecDeque<Event>,
    requests: BTreeMap<u64, PendingRequest>,
    groups: BTreeMap<GroupId, Group>,
}

impl State {
    fn push(&mut self, mut event: Event) {
        self.sequence = self.sequence.saturating_add(1);
        event.sequence = self.sequence;
        if self.events.len() == EVENT_LIMIT {
            self.events.pop_front();
            self.events_dropped = self.events_dropped.saturating_add(1);
        }
        self.events.push_back(event);
    }
}

#[derive(Serialize)]
struct Snapshot {
    schema: u8,
    available: bool,
    complete_metadata: bool,
    elapsed_ms: u64,
    records_lost: u64,
    events_dropped: u64,
    metadata_dropped: u64,
    requests: BTreeMap<u64, PendingRequest>,
    groups: BTreeMap<GroupId, GroupSnapshot>,
    events: VecDeque<Event>,
}

struct Observer {
    started: Instant,
    next: AtomicU64,
    lost: AtomicU64,
    state: Mutex<State>,
}

impl Observer {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            next: AtomicU64::new(1),
            lost: AtomicU64::new(0),
            state: Mutex::new(State::default()),
        }
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn update(&self, change: impl FnOnce(&mut State, u64)) {
        // Contention or poisoning loses evidence, never blocks admission or Drop.
        match self.state.try_lock() {
            Ok(mut state) => change(&mut state, self.elapsed_ms()),
            Err(_) => {
                self.lost.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn request(&self, label: &'static str) -> Request<'_> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let label = fixed_label(label);
        self.update(|state, elapsed_ms| {
            if state.requests.len() < REQUEST_LIMIT {
                state.requests.insert(
                    id,
                    PendingRequest {
                        label,
                        created_ms: elapsed_ms,
                    },
                );
            } else {
                state.metadata_dropped = state.metadata_dropped.saturating_add(1);
            }
            state.push(Event {
                sequence: 0,
                elapsed_ms,
                kind: Kind::RequestCreated,
                label,
                request: Some(id),
                group: None,
                queue_index: None,
                count: 0,
            });
        });
        Request {
            observer: self,
            id,
            label,
            finished: false,
        }
    }

    fn bound(&self, id: GroupId, group: &Arc<FixtureGroup>) {
        self.update(|state, elapsed_ms| {
            if let Some(metadata) = state.groups.get_mut(&id) {
                metadata.weak = Some(Arc::downgrade(group));
            }
            let label = state
                .groups
                .get(&id)
                .map_or("fixture_group_unknown", |group| group.label);
            state.push(Event {
                sequence: 0,
                elapsed_ms,
                kind: Kind::GroupBound,
                label,
                request: None,
                group: Some(id),
                queue_index: None,
                count: 0,
            });
        });
    }

    fn group_event(&self, id: GroupId, kind: Kind, queue_index: Option<usize>, count: usize) {
        if id == GroupId::UNOBSERVED {
            return;
        }
        self.update(|state, elapsed_ms| {
            let label = state
                .groups
                .get(&id)
                .map_or("fixture_group_unknown", |group| group.label);
            if let Some(group) = state.groups.get_mut(&id) {
                match kind {
                    Kind::GroupLastOwnerDropped | Kind::CleanupStarted => {
                        group.phase = Phase::Cleanup;
                        group.queues = count;
                    }
                    Kind::QueueRegistered => group.queues = count,
                    Kind::QueueReferencesPending => {
                        group.phase = Phase::QueueReferences;
                        group.queue_index = queue_index;
                        group.sampled_queue_strong_count = count;
                    }
                    Kind::QueueCloseStarted => {
                        group.phase = Phase::QueueScheduler;
                        group.queue_index = queue_index;
                    }
                    Kind::QueueSchedulerJoined => {
                        group.phase = Phase::Cleanup;
                        group.queues = count;
                    }
                    Kind::AdmissionFailedClosed | Kind::QueueCloseFailed => {
                        group.phase = Phase::FailedClosed
                    }
                    _ => {}
                }
            }
            state.push(Event {
                sequence: 0,
                elapsed_ms,
                kind,
                label,
                request: None,
                group: Some(id),
                queue_index,
                count,
            });
            if kind == Kind::PermitReleased {
                state.groups.remove(&id);
            }
        });
    }

    fn snapshot(&self) -> Snapshot {
        let mut snapshot = Snapshot {
            schema: 1,
            available: false,
            complete_metadata: false,
            elapsed_ms: self.elapsed_ms(),
            records_lost: self.lost.load(Ordering::Relaxed),
            events_dropped: 0,
            metadata_dropped: 0,
            requests: BTreeMap::new(),
            groups: BTreeMap::new(),
            events: VecDeque::new(),
        };
        if let Ok(state) = self.state.try_lock() {
            snapshot.available = true;
            snapshot.events_dropped = state.events_dropped;
            snapshot.metadata_dropped = state.metadata_dropped;
            snapshot.requests = state.requests.clone();
            snapshot.events = state.events.clone();
            snapshot.groups = state
                .groups
                .iter()
                .map(|(id, group)| {
                    (
                        *id,
                        GroupSnapshot {
                            label: group.label,
                            weak_bound: group.weak.is_some(),
                            live_group_strong_count: group
                                .weak
                                .as_ref()
                                .map_or(0, Weak::strong_count),
                            phase: group.phase,
                            queues: group.queues,
                            queue_index: group.queue_index,
                            sampled_queue_strong_count: group.sampled_queue_strong_count,
                        },
                    )
                })
                .collect();
        }
        snapshot.records_lost = self.lost.load(Ordering::Relaxed);
        snapshot.complete_metadata =
            snapshot.available && snapshot.records_lost == 0 && snapshot.metadata_dropped == 0;
        snapshot
    }

    fn report(&self, writer: &mut impl Write) {
        // Clone bounded metadata before serializing or writing; no observer lock
        // and no fixture Arc crosses the output boundary. I/O errors are ignored.
        let snapshot = self.snapshot();
        let _ = write_snapshot(&snapshot, writer);
    }
}

fn fixed_label(label: &'static str) -> &'static str {
    if label.len() <= 256
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':'))
    {
        label
    } else {
        "fixture_label_unavailable"
    }
}

fn write_snapshot(snapshot: &Snapshot, writer: &mut impl Write) -> io::Result<()> {
    let mut line = b"FILE_FIXTURE_ADMISSION_DIAGNOSTICS ".to_vec();
    serde_json::to_writer(&mut line, snapshot).map_err(io::Error::other)?;
    line.push(b'\n');
    writer.write_all(&line)
}

fn observer() -> &'static Observer {
    static OBSERVER: OnceLock<Observer> = OnceLock::new();
    OBSERVER.get_or_init(Observer::new)
}

/// Request metadata is a borrowed observer, never a semaphore or group owner.
pub(super) struct Request<'a> {
    observer: &'a Observer,
    id: u64,
    label: &'static str,
    finished: bool,
}

impl Request<'_> {
    /// Record the actual permit return; request creation is not a FIFO enqueue event.
    pub(super) fn acquired(mut self) -> GroupId {
        let id = GroupId(self.id);
        self.finished = true;
        self.observer.update(|state, elapsed_ms| {
            state.requests.remove(&self.id);
            if state.groups.len() < GROUP_LIMIT {
                state.groups.insert(
                    id,
                    Group {
                        label: self.label,
                        weak: None,
                        phase: Phase::Owned,
                        queues: 0,
                        queue_index: None,
                        sampled_queue_strong_count: 0,
                    },
                );
            } else {
                state.metadata_dropped = state.metadata_dropped.saturating_add(1);
            }
            state.push(Event {
                sequence: 0,
                elapsed_ms,
                kind: Kind::Acquired,
                label: self.label,
                request: Some(self.id),
                group: Some(id),
                queue_index: None,
                count: 0,
            });
        });
        id
    }

    /// Keep the original failure; this only records its fixed classification.
    pub(super) fn failed(&mut self, kind: Kind) {
        self.finished = true;
        self.finish(kind);
    }

    fn finish(&self, kind: Kind) {
        self.observer.update(|state, elapsed_ms| {
            state.requests.remove(&self.id);
            state.push(Event {
                sequence: 0,
                elapsed_ms,
                kind,
                label: self.label,
                request: Some(self.id),
                group: None,
                queue_index: None,
                count: 0,
            });
        });
    }
}

impl Drop for Request<'_> {
    fn drop(&mut self) {
        if !self.finished {
            // Dropping this scope accompanies cancellation/unwind, but it does
            // not claim the separate Tokio acquire future has already dropped.
            self.finish(Kind::RequestScopeDropped);
        }
    }
}

/// Start a numeric request using only a compiled test label.
pub(super) fn request(label: &'static str) -> Request<'static> {
    observer().request(label)
}

/// Attach weak metadata after constructing the actual admitted group.
pub(super) fn bound(id: GroupId, group: &Arc<FixtureGroup>) {
    observer().bound(id, group);
}

/// Record a completed lifecycle boundary with counts, never transport values.
pub(super) fn group_event(id: GroupId, kind: Kind, queue: Option<usize>, count: usize) {
    observer().group_event(id, kind, queue, count);
}

/// Emit a bounded best-effort snapshot without changing the admission result.
pub(super) fn report() {
    observer().report(&mut io::stderr());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::test_server::Checked;

    fn group(id: GroupId) -> Arc<FixtureGroup> {
        Arc::new(FixtureGroup {
            permit: None,
            queues: Mutex::new(Vec::new()),
            observation: id,
        })
    }

    #[test]
    fn bounded_history_preserves_active_metadata_and_has_no_strong_fixture_owner() {
        let observer = Observer::new();
        let id = observer.request("owner").acquired();
        let group = group(id);
        observer.bound(id, &group);
        let weak = Arc::downgrade(&group);
        let pending = observer.request("pending");
        for _ in 0..EVENT_LIMIT + 3 {
            observer.group_event(id, Kind::SameAppReused, None, 0);
        }
        let snapshot = observer.snapshot();
        assert_eq!(snapshot.events.len(), EVENT_LIMIT);
        assert!(snapshot.events_dropped > 0 && snapshot.complete_metadata);
        assert_eq!(snapshot.requests.len(), 1);
        assert_eq!(snapshot.groups[&id].live_group_strong_count, 1);
        assert!(
            snapshot
                .events
                .iter()
                .zip(snapshot.events.iter().skip(1))
                .all(|(a, b)| a.sequence < b.sequence && a.elapsed_ms <= b.elapsed_ms)
        );
        drop(group);
        assert_eq!(weak.strong_count(), 0);
        assert_eq!(observer.snapshot().groups[&id].live_group_strong_count, 0);
        drop(pending);
        assert!(observer.snapshot().requests.is_empty());
    }

    #[test]
    fn request_scope_drop_removes_metadata_without_claiming_acquisition() {
        let observer = Observer::new();
        let request = observer.request("cancelled_scope");
        let id = request.id;
        drop(request);
        let snapshot = observer.snapshot();
        assert!(snapshot.requests.is_empty() && snapshot.groups.is_empty());
        assert_eq!(
            snapshot
                .events
                .back()
                .map(|event| (event.kind, event.request)),
            Some((Kind::RequestScopeDropped, Some(id)))
        );
    }

    #[test]
    fn diagnostic_lock_contention_does_not_consume_or_release_an_actual_private_permit() {
        let observer = Observer::new();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = semaphore
            .clone()
            .try_acquire_owned()
            .checked("private permit");
        let lock = observer.state.lock().checked("private diagnostic lock");
        let id = observer.request("blocked_observer").acquired();
        observer.group_event(id, Kind::PermitReleased, None, 0);
        assert_eq!(semaphore.available_permits(), 0);
        assert!(semaphore.clone().try_acquire_owned().is_err());
        assert!(!observer.snapshot().available);
        drop(lock);
        let snapshot = observer.snapshot();
        assert!(snapshot.available && snapshot.records_lost >= 3 && !snapshot.complete_metadata);
        drop(permit);
        assert_eq!(semaphore.available_permits(), 1);
    }

    #[test]
    fn metadata_overflow_is_explicit_and_does_not_evict_the_active_owner() {
        let observer = Observer::new();
        let owner = observer.request("owner").acquired();
        let requests = (0..REQUEST_LIMIT + 1)
            .map(|_| observer.request("pending"))
            .collect::<Vec<_>>();
        let snapshot = observer.snapshot();
        assert_eq!(snapshot.requests.len(), REQUEST_LIMIT);
        assert_eq!(snapshot.metadata_dropped, 1);
        assert!(!snapshot.complete_metadata && snapshot.groups.contains_key(&owner));
        drop(requests);
        observer.group_event(owner, Kind::PermitReleased, None, 0);
        assert!(observer.snapshot().groups.is_empty());
    }

    #[test]
    fn poisoned_diagnostic_state_cannot_change_an_actual_private_permit() {
        let observer = Observer::new();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = semaphore
            .clone()
            .try_acquire_owned()
            .checked("private permit");
        let poison = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _lock = observer.state.lock().checked("private diagnostic lock");
            panic!("poison the private observer only");
        }));
        assert!(poison.is_err());
        let id = observer.request("poisoned_observer").acquired();
        observer.group_event(id, Kind::PermitReleased, None, 0);
        let snapshot = observer.snapshot();
        assert!(!snapshot.available && !snapshot.complete_metadata && snapshot.records_lost >= 3);
        observer.report(&mut Vec::new());
        assert_eq!(semaphore.available_permits(), 0);
        drop(permit);
        assert_eq!(semaphore.available_permits(), 1);
    }

    #[test]
    fn dropped_group_queue_cleanup_remains_visible_until_actual_release_event() {
        let observer = Observer::new();
        let id = observer.request("queue_owner").acquired();
        let group = group(id);
        observer.bound(id, &group);
        drop(group);
        for (kind, queue, count) in [
            (Kind::GroupLastOwnerDropped, None, 1),
            (Kind::QueueReferencesPending, Some(1), 2),
            (Kind::QueueCloseStarted, Some(1), 0),
            (Kind::QueueSchedulerJoined, Some(1), 0),
        ] {
            observer.group_event(id, kind, queue, count);
            assert!(observer.snapshot().groups.contains_key(&id));
        }
        observer.group_event(id, Kind::PermitReleased, None, 0);
        assert!(observer.snapshot().groups.is_empty());
    }

    #[test]
    fn broken_output_does_not_panic_and_fixed_labels_cannot_dump_paths() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let observer = Observer::new();
        let mut request = observer.request("/private/not_a_test_label");
        request.failed(Kind::AdmissionTimeout);
        observer.report(&mut Broken);
        let mut output = Vec::new();
        observer.report(&mut output);
        let output = String::from_utf8(output).checked("ASCII diagnostic");
        assert!(output.starts_with("FILE_FIXTURE_ADMISSION_DIAGNOSTICS "));
        assert!(!output.contains("/private/not_a_test_label"));
        assert!(
            output.contains("fixture_label_unavailable") && output.contains("admission_timeout")
        );
        assert!(observer.snapshot().requests.is_empty());
    }
}
