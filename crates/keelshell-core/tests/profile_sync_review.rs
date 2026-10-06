//! Fresh reviewer public-API checks on isolated stores and real shared-file operations.
use keelshell_core::{
    AuthMethod, Connection, Error, ProfileSyncChoice, ProfileSyncError, ProfileSyncReview,
    ProfileSyncService, StateStore,
};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    path::PathBuf,
    sync::{Arc, Barrier, atomic::AtomicBool, mpsc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;
use zeroize::Zeroizing;

const PASSWORD: &str = "review-only-shared-directory-password";
const TARGET: Uuid = Uuid::from_u128(71_000);
const JUMP: Uuid = Uuid::from_u128(71_001);
const FILE: &str = "keelshell-profiles.ksync";

struct Pair {
    _temporary: tempfile::TempDir,
    directory: PathBuf,
    a: Arc<StateStore>,
    b: Arc<StateStore>,
}
impl Pair {
    fn new() -> Self {
        let temporary = tempfile::tempdir().checked();
        let directory = temporary.path().join("shared");
        std::fs::create_dir(&directory).checked();
        let a = Arc::new(StateStore::new(temporary.path().join("a/state.json")));
        let b = Arc::new(StateStore::new(temporary.path().join("b/state.json")));
        let mut state = a.load().checked();
        let mut target = Connection::new("Target", "target.fixture.invalid", "review");
        target.id = TARGET;
        target.auth = AuthMethod::PrivateKey {
            path: "/synthetic/fixture-key".into(),
        };
        target.credential_ref = Some(Uuid::from_u128(71_002));
        state.connections.push(target);
        a.save(&state).checked();
        b.save(&b.load().checked()).checked();
        Self {
            _temporary: temporary,
            directory,
            a,
            b,
        }
    }
    fn service(store: &Arc<StateStore>) -> ProfileSyncService {
        ProfileSyncService::new(store.clone())
    }
    fn inspect(&self, store: &Arc<StateStore>) -> ProfileSyncReview {
        Self::service(store)
            .inspect(self.directory.clone(), password(), &AtomicBool::new(false))
            .checked()
    }
    fn sync(&self, store: &Arc<StateStore>, choice: ProfileSyncChoice) {
        let review = self.inspect(store);
        let choices = choices(&review, choice);
        let result = Self::service(store)
            .apply(review, choices, password(), &AtomicBool::new(false))
            .checked();
        assert!(result.published);
    }
    fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.directory.join(FILE)).checked()
    }
    fn establish(&self) {
        self.sync(&self.a, ProfileSyncChoice::Local);
        self.sync(&self.b, ProfileSyncChoice::Remote);
    }
}
fn password() -> Zeroizing<String> {
    Zeroizing::new(PASSWORD.into())
}
fn choices(
    review: &ProfileSyncReview,
    value: ProfileSyncChoice,
) -> BTreeMap<Uuid, ProfileSyncChoice> {
    review.rows().iter().map(|row| (row.id, value)).collect()
}
fn seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .checked()
        .as_secs()
}

#[test]
fn upstream_endpoint_change_resets_unchanged_downstream_local_authentication() {
    let pair = Pair::new();
    let mut a = pair.a.load().checked();
    let mut jump = Connection::new("Jump", "jump-a.fixture.invalid", "review");
    jump.id = JUMP;
    a.connections.push(jump);
    a.connections[0].jump_host = Some(JUMP);
    pair.a.save(&a).checked();
    pair.establish();
    let mut b = pair.b.load().checked();
    let downstream = b.connections.iter_mut().find(|p| p.id == TARGET).checked();
    downstream.auth = AuthMethod::PrivateKey {
        path: "/synthetic/device-b-key".into(),
    };
    downstream.credential_ref = Some(Uuid::from_u128(71_003));
    pair.b.save(&b).checked();
    let old_route = b.connection_route(TARGET).checked().identity();
    let mut a = pair.a.load().checked();
    a.connections
        .iter_mut()
        .find(|p| p.id == JUMP)
        .checked()
        .host = "jump-b.fixture.invalid".into();
    pair.a.save(&a).checked();
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    let review = pair.inspect(&pair.b);
    assert!(review.rows().iter().all(|r| r.id != TARGET));
    let choices = choices(&review, ProfileSyncChoice::Remote);
    let outcome = Pair::service(&pair.b)
        .apply(review, choices, password(), &AtomicBool::new(false))
        .checked();
    assert!(outcome.published);
    assert_ne!(
        outcome.state.connection_route(TARGET).checked().identity(),
        old_route
    );
    let downstream = outcome
        .state
        .connections
        .iter()
        .find(|p| p.id == TARGET)
        .checked();
    assert_eq!(
        downstream.auth,
        AuthMethod::Agent,
        "local authentication must remain bound to the complete reviewed route"
    );
    assert_eq!(downstream.credential_ref, None);
}

#[test]
fn remote_deletion_uses_local_receipt_time_and_keeps_local_recycle_metadata() {
    let pair = Pair::new();
    pair.establish();
    let mut a = pair.a.load().checked();
    a.soft_delete_connection(TARGET, 23).checked();
    pair.a.save(&a).checked();
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    let before = seconds();
    pair.sync(&pair.b, ProfileSyncChoice::Remote);
    let after = seconds();
    let b = pair.b.load().checked();
    assert!(b.connections.is_empty());
    assert_eq!(b.deleted_connections[0].connection.id, TARGET);
    assert!(
        (before..=after).contains(&b.deleted_connections[0].deleted_at),
        "received deletion must not be presented as epoch time or the peer's unsynchronized clock"
    );
    assert!(pair.inspect(&pair.b).rows().is_empty());
}

#[test]
fn accepting_shared_legacy_group_keeps_local_folder_and_does_not_create_echo_edits() {
    let pair = Pair::new();
    let mut a = pair.a.load().checked();
    a.connections[0].group = "Peer legacy label".into();
    pair.a.save(&a).checked();
    let mut b = pair.b.load().checked();
    let mut target = Connection::new("Local", "target.fixture.invalid", "review");
    target.id = TARGET;
    b.connections.push(target);
    let folder = b.create_folder("Device B folder", None).checked();
    b.move_connection(TARGET, Some(folder)).checked();
    pair.b.save(&b).checked();
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    pair.sync(&pair.b, ProfileSyncChoice::Remote);
    let saved = pair.b.load().checked();
    assert_eq!(saved.folder_id_of(TARGET), Some(folder));
    assert_eq!(saved.connections[0].group, "Device B folder");
    assert_eq!(saved.connections[0].name, "Target");
    assert!(
        pair.inspect(&pair.b).rows().is_empty(),
        "local tree placement must not manufacture a synchronized edit"
    );
    let mut saved = saved;
    saved
        .rename_folder(folder, "Renamed local folder")
        .checked();
    pair.b.save(&saved).checked();
    assert!(pair.inspect(&pair.b).rows().is_empty());
    let mut a = pair.a.load().checked();
    a.connections[0].group = "Updated peer label".into();
    a.connections[0].name = "Peer renamed profile".into();
    pair.a.save(&a).checked();
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    pair.sync(&pair.b, ProfileSyncChoice::Remote);
    let saved = pair.b.load().checked();
    assert_eq!(saved.folder_id_of(TARGET), Some(folder));
    assert_eq!(saved.connections[0].group, "Renamed local folder");
    assert_eq!(saved.connections[0].name, "Peer renamed profile");
    assert!(pair.inspect(&pair.b).rows().is_empty());
}

#[test]
fn real_transport_lock_refuses_apply_without_local_or_shared_replacement() {
    let pair = Pair::new();
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(pair.directory.join("keelshell-profiles.ksync.lock"))
        .checked();
    lock.try_lock().checked();
    let local = std::fs::read(pair.b.path()).checked();
    let peer = pair.bytes();
    let review = pair.inspect(&pair.b);
    let choices = choices(&review, ProfileSyncChoice::Remote);
    assert!(matches!(
        Pair::service(&pair.b).apply(review, choices, password(), &AtomicBool::new(false)),
        Err(ProfileSyncError::Busy)
    ));
    assert_eq!(std::fs::read(pair.b.path()).checked(), local);
    assert_eq!(pair.bytes(), peer);
    drop(lock);
    pair.sync(&pair.b, ProfileSyncChoice::Remote);
}

#[test]
fn simultaneous_reviewed_publishers_have_one_winner_and_preserve_the_loser() {
    let pair = Pair::new();
    pair.establish();
    for (store, name) in [(&pair.a, "Device A edit"), (&pair.b, "Device B edit")] {
        let mut state = store.load().checked();
        state.connections[0].name = name.into();
        store.save(&state).checked();
    }
    let generation = pair.b.load().checked().profile_sync.checked().generation();
    let (a_review, b_review) = (pair.inspect(&pair.a), pair.inspect(&pair.b));
    let originals = [
        std::fs::read(pair.a.path()).checked(),
        std::fs::read(pair.b.path()).checked(),
    ];
    let barrier = Arc::new(Barrier::new(2));
    let (tx, rx) = mpsc::channel();
    let mut workers = Vec::new();
    for (index, store, review) in [(0, pair.a.clone(), a_review), (1, pair.b.clone(), b_review)] {
        let barrier = barrier.clone();
        let tx = tx.clone();
        workers.push(std::thread::spawn(move || {
            let choices = choices(&review, ProfileSyncChoice::Local);
            barrier.wait();
            let result =
                Pair::service(&store).apply(review, choices, password(), &AtomicBool::new(false));
            tx.send((index, result)).checked();
        }));
    }
    drop(tx);
    let mut wins = 0;
    for _ in 0..2 {
        let (index, result) = rx.recv_timeout(Duration::from_secs(30)).checked();
        match result {
            Ok(outcome) => {
                assert!(outcome.published);
                assert_eq!(
                    outcome.state.profile_sync.checked().generation(),
                    generation + 1
                );
                wins += 1;
            }
            Err(ProfileSyncError::Busy | ProfileSyncError::Stale) => {
                let store = if index == 0 { &pair.a } else { &pair.b };
                assert_eq!(std::fs::read(store.path()).checked(), originals[index]);
            }
            Err(error) => panic!("unexpected simultaneous publisher result: {error:?}"),
        }
    }
    for worker in workers {
        worker.join().checked();
    }
    assert_eq!(wins, 1);
}

#[test]
fn absent_optional_ledger_migrates_and_unknown_ledger_fields_stay_rejected() {
    let pair = Pair::new();
    let original = pair.a.load().checked();
    let old = serde_json::to_vec(&original).checked();
    assert!(
        !String::from_utf8(old.clone())
            .checked()
            .contains("profile_sync")
    );
    std::fs::write(pair.a.path(), &old).checked();
    let loaded = pair.a.load().checked();
    assert!(loaded.profile_sync.is_none());
    assert_eq!(loaded.connections, original.connections);
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    let mut wire = serde_json::to_value(pair.a.load().checked()).checked();
    wire["profile_sync"]["unexpected_policy"] = serde_json::json!(true);
    std::fs::write(pair.a.path(), serde_json::to_vec(&wire).checked()).checked();
    assert!(matches!(pair.a.load(), Err(Error::Json(_))));
}

trait Checked<T> {
    fn checked(self) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => panic!("review fixture failed: {e:?}"),
        }
    }
}
impl<T> Checked<T> for Option<T> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Some(v) => v,
            None => panic!("review fixture missing value"),
        }
    }
}

#[test]
fn pure_preview_discloses_downstream_route_changes_without_modifying_state() {
    let pair = Pair::new();
    let mut a = pair.a.load().checked();
    let mut jump = Connection::new("Jump", "jump.fixture.invalid", "review");
    jump.id = JUMP;
    a.connections.push(jump);
    a.connections[0].jump_host = Some(JUMP);
    pair.a.save(&a).checked();
    pair.establish();
    let mut b = pair.b.load().checked();
    b.connections
        .iter_mut()
        .find(|p| p.id == TARGET)
        .checked()
        .auth = AuthMethod::PrivateKey {
        path: "/synthetic/device-b-preview-key".into(),
    };
    b.connections
        .iter_mut()
        .find(|p| p.id == TARGET)
        .checked()
        .credential_ref = Some(Uuid::from_u128(72_001));
    pair.b.save(&b).checked();
    let mut a = pair.a.load().checked();
    a.connections
        .iter_mut()
        .find(|p| p.id == JUMP)
        .checked()
        .port = 2202;
    pair.a.save(&a).checked();
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    let local = std::fs::read(pair.b.path()).checked();
    let peer = pair.bytes();
    let review = pair.inspect(&pair.b);
    let selections = choices(&review, ProfileSyncChoice::Remote);
    let preview = review.preview(&selections).checked();
    let target = preview
        .route_changes
        .iter()
        .find(|p| p.id == TARGET)
        .checked();
    assert!(target.resets_authentication);
    assert_eq!(target.before.endpoints().len(), 2);
    assert_eq!(target.before.endpoints()[0].port, 22);
    assert_eq!(target.after.endpoints()[0].port, 2202);
    let encoded = format!("{preview:?}");
    assert!(!encoded.contains("preview-key"));
    assert!(!encoded.contains("credential_ref"));
    assert_eq!(std::fs::read(pair.b.path()).checked(), local);
    assert_eq!(pair.bytes(), peer);
    let outcome = Pair::service(&pair.b)
        .apply(review, selections, password(), &AtomicBool::new(false))
        .checked();
    assert!(outcome.published);
    assert_eq!(
        outcome
            .state
            .connections
            .iter()
            .find(|p| p.id == TARGET)
            .checked()
            .auth,
        AuthMethod::Agent
    );
}

#[test]
fn display_only_shared_edits_keep_existing_local_authentication() {
    let pair = Pair::new();
    pair.establish();
    let mut b = pair.b.load().checked();
    b.connections[0].auth = AuthMethod::PrivateKey {
        path: "/synthetic/retained-device-key".into(),
    };
    b.connections[0].credential_ref = Some(Uuid::from_u128(72_002));
    pair.b.save(&b).checked();
    let mut a = pair.a.load().checked();
    a.connections[0].name = "Renamed shared profile".into();
    a.connections[0].favorite = true;
    pair.a.save(&a).checked();
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    let review = pair.inspect(&pair.b);
    let selected = choices(&review, ProfileSyncChoice::Remote);
    assert!(review.preview(&selected).checked().route_changes.is_empty());
    let outcome = Pair::service(&pair.b)
        .apply(review, selected, password(), &AtomicBool::new(false))
        .checked();
    assert!(outcome.published);
    assert_eq!(outcome.state.connections[0].auth, b.connections[0].auth);
    assert_eq!(
        outcome.state.connections[0].credential_ref,
        b.connections[0].credential_ref
    );
}

#[test]
fn local_folder_rename_reassignment_removal_and_peer_round_trip_do_not_echo() {
    let pair = Pair::new();
    let mut a = pair.a.load().checked();
    a.connections[0].group = "Shared legacy label".into();
    pair.a.save(&a).checked();
    pair.establish();
    let mut b = pair.b.load().checked();
    let first = b.create_folder("First local", None).checked();
    b.move_connection(TARGET, Some(first)).checked();
    pair.b.save(&b).checked();
    // Folder membership added after pairing must remain local immediately.
    assert!(pair.inspect(&pair.b).rows().is_empty());
    pair.sync(&pair.b, ProfileSyncChoice::Remote);
    let mut b = pair.b.load().checked();
    let second = b.create_folder("Second local", None).checked();
    b.move_connection(TARGET, Some(second)).checked();
    b.remove_folder(first).checked();
    pair.b.save(&b).checked();
    assert!(pair.inspect(&pair.b).rows().is_empty());
    b = pair.b.load().checked();
    b.move_connection(TARGET, None).checked();
    b.remove_folder(second).checked();
    b.connections[0].name = "Locally approved rename".into();
    pair.b.save(&b).checked();
    pair.sync(&pair.b, ProfileSyncChoice::Local);
    pair.sync(&pair.a, ProfileSyncChoice::Remote);
    let a = pair.a.load().checked();
    assert_eq!(a.connections[0].group, "Shared legacy label");
    assert_eq!(a.connections[0].name, "Locally approved rename");
    assert!(pair.inspect(&pair.a).rows().is_empty());
    assert!(pair.inspect(&pair.b).rows().is_empty());
    let mut b = pair.b.load().checked();
    let final_folder = b.create_folder("Restored local", None).checked();
    b.move_connection(TARGET, Some(final_folder)).checked();
    pair.b.save(&b).checked();
    let mut a = pair.a.load().checked();
    a.soft_delete_connection(TARGET, 21).checked();
    pair.a.save(&a).checked();
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    pair.sync(&pair.b, ProfileSyncChoice::Remote);
    a = pair.a.load().checked();
    a.restore_connection(TARGET).checked();
    pair.a.save(&a).checked();
    pair.sync(&pair.a, ProfileSyncChoice::Local);
    pair.sync(&pair.b, ProfileSyncChoice::Remote);
    let restored = pair.b.load().checked();
    assert_eq!(restored.folder_id_of(TARGET), Some(final_folder));
    assert_eq!(restored.connections[0].group, "Restored local");
    assert!(pair.inspect(&pair.b).rows().is_empty());
}

#[test]
fn byte_capacity_rejection_below_record_ceiling_keeps_original_state_and_peer_absent() {
    let pair = Pair::new();
    let mut state = pair.a.load().checked();
    state.connections.clear();
    for i in 0..300 {
        let mut profile = Connection::new(
            format!("Large synthetic profile {i}"),
            format!("host-{i}.fixture.invalid"),
            "review",
        );
        profile.id = Uuid::from_u128(80_000 + i);
        profile.tags = (0..32)
            .map(|j| format!("{j:02}{}", "x".repeat(62)))
            .collect();
        state.connections.push(profile);
    }
    pair.a.save(&state).checked();
    let original = std::fs::read(pair.a.path()).checked();
    assert!(original.len() < 4 * 1024 * 1024);
    let review = pair.inspect(&pair.a);
    assert_eq!(review.rows().len(), 300);
    let selected = choices(&review, ProfileSyncChoice::Local);
    assert!(matches!(
        Pair::service(&pair.a).apply(review, selected, password(), &AtomicBool::new(false)),
        Err(ProfileSyncError::Storage(Error::TooLarge))
    ));
    assert_eq!(std::fs::read(pair.a.path()).checked(), original);
    assert!(!pair.directory.join(FILE).exists());
    assert!(pair.a.load().checked().profile_sync.is_none());
}
