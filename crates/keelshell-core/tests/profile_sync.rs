//! Two independent local stores sharing only the encrypted transport directory.
use keelshell_core::{
    AuthMethod, Connection, ProfileSyncChoice, ProfileSyncError, ProfileSyncReview,
    ProfileSyncService, StateStore,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
};
use uuid::Uuid;
use zeroize::Zeroizing;
const PASSWORD: &str = "isolated-sync-fixture-password";
const HOST: Uuid = Uuid::from_u128(700);
fn password() -> Zeroizing<String> {
    Zeroizing::new(PASSWORD.into())
}
fn choices(review: &ProfileSyncReview, c: ProfileSyncChoice) -> BTreeMap<Uuid, ProfileSyncChoice> {
    review.rows().iter().map(|r| (r.id, c)).collect()
}
struct Clients {
    _temp: tempfile::TempDir,
    dir: std::path::PathBuf,
    a: Arc<StateStore>,
    b: Arc<StateStore>,
}
impl Clients {
    fn new() -> Self {
        let t = tempfile::tempdir().checked();
        let dir = t.path().join("shared");
        std::fs::create_dir(&dir).checked();
        let a = Arc::new(StateStore::new(t.path().join("a/state.json")));
        let b = Arc::new(StateStore::new(t.path().join("b/state.json")));
        let mut state = a.load().checked();
        let mut c = Connection::new("Fixture", "fixture.invalid", "fixture");
        c.id = HOST;
        c.auth = AuthMethod::PrivateKey {
            path: "/fixture/private-key".into(),
        };
        c.credential_ref = Some(Uuid::from_u128(701));
        state.connections.push(c);
        a.save(&state).checked();
        let s = b.load().checked();
        b.save(&s).checked();
        Self {
            _temp: t,
            dir,
            a,
            b,
        }
    }
    fn inspect(&self, store: &Arc<StateStore>) -> ProfileSyncReview {
        ProfileSyncService::new(store.clone())
            .inspect(self.dir.clone(), password(), &AtomicBool::new(false))
            .checked()
    }
    fn sync(&self, store: &Arc<StateStore>, choice: ProfileSyncChoice) {
        let r = self.inspect(store);
        let c = choices(&r, choice);
        let result = ProfileSyncService::new(store.clone())
            .apply(r, c, password(), &AtomicBool::new(false))
            .checked();
        assert!(result.published);
    }
    fn edit(&self, store: &Arc<StateStore>, name: &str) {
        let mut s = store.load().checked();
        s.connections[0].name = name.into();
        store.save(&s).checked();
    }
}
#[test]
fn two_clients_pull_review_publish_and_keep_credentials_device_local() {
    let c = Clients::new();
    assert!(!c.dir.join("keelshell-profiles.ksync").exists());
    let r = c.inspect(&c.a);
    assert!(r.creates_channel());
    assert!(!c.dir.join("keelshell-profiles.ksync").exists());
    let encoded = serde_json::to_string(&r.rows()[0].local).checked();
    assert!(!encoded.contains("private-key"));
    assert!(!encoded.contains("credential_ref"));
    c.sync(&c.a, ProfileSyncChoice::Local);
    c.sync(&c.b, ProfileSyncChoice::Remote);
    let a = c.a.load().checked();
    let b = c.b.load().checked();
    assert_eq!(b.connections[0].name, "Fixture");
    assert_eq!(b.connections[0].auth, AuthMethod::Agent);
    assert!(b.connections[0].credential_ref.is_none());
    assert!(matches!(
        a.connections[0].auth,
        AuthMethod::PrivateKey { .. }
    ));
    let bytes = std::fs::read(c.dir.join("keelshell-profiles.ksync")).checked();
    let text = String::from_utf8(bytes).checked();
    assert!(!text.contains("fixture.invalid"));
    assert!(!text.contains("Fixture"));
    assert!(!text.contains(PASSWORD));
    let state = std::fs::read(c.a.path()).checked();
    assert!(!String::from_utf8(state).checked().contains(PASSWORD));
}
#[test]
fn offline_conflicts_require_exact_explicit_choices_and_converge() {
    let c = Clients::new();
    c.sync(&c.a, ProfileSyncChoice::Local);
    c.sync(&c.b, ProfileSyncChoice::Remote);
    c.edit(&c.a, "Device A offline");
    c.edit(&c.b, "Device B offline");
    c.sync(&c.a, ProfileSyncChoice::Local);
    let r = c.inspect(&c.b);
    assert_eq!(r.rows().len(), 1);
    assert!(r.rows()[0].conflict);
    let before = c.b.load().checked();
    assert!(matches!(
        ProfileSyncService::new(c.b.clone()).apply(
            r,
            BTreeMap::new(),
            password(),
            &AtomicBool::new(false)
        ),
        Err(ProfileSyncError::Incomplete)
    ));
    assert_eq!(c.b.load().checked(), before);
    c.sync(&c.b, ProfileSyncChoice::Local);
    c.sync(&c.a, ProfileSyncChoice::Remote);
    assert_eq!(c.a.load().checked().connections[0].name, "Device B offline");
}
#[test]
fn deletion_tombstone_beats_old_snapshot_and_conflicts_with_offline_edit() {
    let c = Clients::new();
    c.sync(&c.a, ProfileSyncChoice::Local);
    c.sync(&c.b, ProfileSyncChoice::Remote);
    let old = std::fs::read(c.dir.join("keelshell-profiles.ksync")).checked();
    c.edit(&c.b, "offline edit");
    let mut s = c.a.load().checked();
    s.soft_delete_connection(HOST, 9).checked();
    c.a.save(&s).checked();
    c.sync(&c.a, ProfileSyncChoice::Local);
    let r = c.inspect(&c.b);
    assert!(r.rows()[0].conflict);
    assert!(r.rows()[0].remote.is_none());
    c.sync(&c.b, ProfileSyncChoice::Remote);
    assert!(c.b.load().checked().connections.is_empty());
    std::fs::write(c.dir.join("keelshell-profiles.ksync"), old).checked();
    assert!(matches!(
        ProfileSyncService::new(c.b.clone()).inspect(
            c.dir.clone(),
            password(),
            &AtomicBool::new(false)
        ),
        Err(ProfileSyncError::Replay)
    ));
}
#[test]
fn wrong_password_tamper_and_cancel_do_not_reset_or_write() {
    let c = Clients::new();
    c.sync(&c.a, ProfileSyncChoice::Local);
    let path = c.dir.join("keelshell-profiles.ksync");
    let good = std::fs::read(&path).checked();
    let before = c.b.load().checked();
    let svc = ProfileSyncService::new(c.b.clone());
    assert!(
        svc.inspect(
            c.dir.clone(),
            Zeroizing::new("wrong".into()),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let mut json: serde_json::Value = serde_json::from_slice(&good).checked();
    let cipher = json["ciphertext"].as_str().checked();
    json["ciphertext"] = format!(
        "{}{}",
        if cipher.starts_with('A') { "B" } else { "A" },
        &cipher[1..]
    )
    .into();
    std::fs::write(&path, serde_json::to_vec(&json).checked()).checked();
    assert!(
        svc.inspect(c.dir.clone(), password(), &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(c.b.load().checked(), before);
    std::fs::write(&path, &good).checked();
    let r = c.inspect(&c.b);
    let ch = choices(&r, ProfileSyncChoice::Remote);
    assert!(matches!(
        svc.apply(r, ch, password(), &AtomicBool::new(true)),
        Err(ProfileSyncError::Cancelled)
    ));
    assert_eq!(std::fs::read(&path).checked(), good);
    assert_eq!(c.b.load().checked(), before);
}
#[test]
fn local_or_peer_change_after_review_refuses_stale_approval() {
    let c = Clients::new();
    c.sync(&c.a, ProfileSyncChoice::Local);
    let r = c.inspect(&c.b);
    let ch = choices(&r, ProfileSyncChoice::Remote);
    let mut s = c.b.load().checked();
    s.settings.scrollback_lines = 1234;
    c.b.save(&s).checked();
    assert!(matches!(
        ProfileSyncService::new(c.b.clone()).apply(r, ch, password(), &AtomicBool::new(false)),
        Err(ProfileSyncError::Stale)
    ));
    let r = c.inspect(&c.b);
    let ch = choices(&r, ProfileSyncChoice::Remote);
    c.edit(&c.a, "new peer");
    c.sync(&c.a, ProfileSyncChoice::Local);
    assert!(matches!(
        ProfileSyncService::new(c.b.clone()).apply(r, ch, password(), &AtomicBool::new(false)),
        Err(ProfileSyncError::Stale)
    ));
}
#[test]
fn disabled_client_keeps_replay_anchor_and_explicit_pull_can_reenable() {
    let c = Clients::new();
    c.sync(&c.a, ProfileSyncChoice::Local);
    let svc = ProfileSyncService::new(c.a.clone());
    let s = svc.disable(&AtomicBool::new(false)).checked();
    assert!(!s.profile_sync.checked().enabled());
    c.sync(&c.a, ProfileSyncChoice::Local);
    assert!(c.a.load().checked().profile_sync.checked().enabled());
}

trait Checked<T> {
    fn checked(self) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("sync fixture failed: {error:?}"),
        }
    }
}
impl<T> Checked<T> for Option<T> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Some(value) => value,
            None => panic!("sync fixture missing expected value"),
        }
    }
}
