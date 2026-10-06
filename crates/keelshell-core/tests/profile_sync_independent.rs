//! Public-API synchronization boundaries with independent, authenticated wire fixtures.
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::AeadInOut};
use keelshell_core::{
    Connection, ProfileSyncChoice, ProfileSyncError, ProfileSyncReview, ProfileSyncService,
    StateStore,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use uuid::Uuid;
use zeroize::Zeroizing;

const PASSWORD: &str = "independent-isolated-sync-password";
const FIRST: Uuid = Uuid::from_u128(910);
const SECOND: Uuid = Uuid::from_u128(911);
const THIRD: Uuid = Uuid::from_u128(912);
const FILE: &str = "keelshell-profiles.ksync";

fn password() -> Zeroizing<String> {
    Zeroizing::new(PASSWORD.into())
}

struct Clients {
    temporary: tempfile::TempDir,
    directory: PathBuf,
    a: Arc<StateStore>,
    b: Arc<StateStore>,
}

impl Clients {
    fn new() -> Self {
        let temporary = tempfile::tempdir().checked();
        let directory = temporary.path().join("shared");
        std::fs::create_dir(&directory).checked();
        let a = Arc::new(StateStore::new(temporary.path().join("a/state.json")));
        let b = Arc::new(StateStore::new(temporary.path().join("b/state.json")));
        let mut state = a.load().checked();
        for (id, name) in [(FIRST, "First"), (SECOND, "Second"), (THIRD, "Third")] {
            let mut profile = Connection::new(name, "fixture.invalid", "fixture");
            profile.id = id;
            state.connections.push(profile);
        }
        a.save(&state).checked();
        b.save(&b.load().checked()).checked();
        Self {
            temporary,
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

    fn choices(
        review: &ProfileSyncReview,
        choice: ProfileSyncChoice,
    ) -> BTreeMap<Uuid, ProfileSyncChoice> {
        review.rows().iter().map(|row| (row.id, choice)).collect()
    }

    fn sync(&self, store: &Arc<StateStore>, choice: ProfileSyncChoice) {
        let review = self.inspect(store);
        let choices = Self::choices(&review, choice);
        let outcome = Self::service(store)
            .apply(review, choices, password(), &AtomicBool::new(false))
            .checked();
        assert!(outcome.published, "isolated publication must complete");
    }

    fn shared_bytes(&self) -> Vec<u8> {
        std::fs::read(self.directory.join(FILE)).checked()
    }

    fn snapshot(&self, store: &Arc<StateStore>) -> Value {
        let state = store.load().checked();
        serde_json::to_value(state.profile_sync.checked()).checked()["baseline"].clone()
    }

    fn edit_all(&self, store: &Arc<StateStore>, prefix: &str) {
        let mut state = store.load().checked();
        for profile in &mut state.connections {
            profile.name = format!("{prefix} {}", profile.id);
        }
        store.save(&state).checked();
    }

    fn replace_authenticated_snapshot(&self, snapshot: &Value, nonce_byte: u8) {
        std::fs::write(
            self.directory.join(FILE),
            authenticated_wire(snapshot, nonce_byte),
        )
        .checked();
    }

    fn assert_rejected_without_local_write(
        &self,
        store: &Arc<StateStore>,
        expected: fn(&ProfileSyncError) -> bool,
    ) {
        let local = std::fs::read(store.path()).checked();
        let shared = self.shared_bytes();
        let result = Self::service(store).inspect(
            self.directory.clone(),
            password(),
            &AtomicBool::new(false),
        );
        match result {
            Err(error) => assert!(expected(&error), "unexpected failure: {error}"),
            Ok(_) => panic!("invalid authenticated peer was accepted"),
        }
        assert_eq!(std::fs::read(store.path()).checked(), local);
        assert_eq!(self.shared_bytes(), shared);
    }
}

// Construct wire data independently of the private production sealing helper.
// Fixed fixture salt/nonces are limited to these isolated, synthetic test bodies.
fn authenticated_wire(snapshot: &Value, nonce_byte: u8) -> Vec<u8> {
    let salt = [0x73_u8; 16];
    let nonce_bytes = [nonce_byte; 24];
    let params = Params::new(64 * 1024, 3, 1, Some(32)).checked();
    let mut key = Zeroizing::new([0_u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(PASSWORD.as_bytes(), &salt, key.as_mut())
        .checked();
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref()).checked();
    let nonce = XNonce::try_from(nonce_bytes.as_slice()).checked();
    let plaintext = Zeroizing::new(serde_json::to_vec(snapshot).checked());
    let mut ciphertext = Zeroizing::new(Vec::with_capacity(plaintext.len() + 16));
    ciphertext.extend_from_slice(&plaintext);
    cipher
        .encrypt_in_place(&nonce, b"keelshell-profile-sync-v1\0", &mut *ciphertext)
        .checked();
    serde_json::to_vec(&json!({
        "schema": 1,
        "kdf": {
            "algorithm": "argon2id", "memory_kib": 64 * 1024,
            "iterations": 3, "lanes": 1, "salt": BASE64.encode(salt)
        },
        "nonce": BASE64.encode(nonce_bytes),
        "ciphertext": BASE64.encode(&*ciphertext)
    }))
    .checked()
}

#[test]
fn selecting_only_some_of_multiple_differences_cannot_save_or_publish() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let review = clients.inspect(&clients.b);
    assert_eq!(review.rows().len(), 3);
    let local = std::fs::read(clients.b.path()).checked();
    let peer = clients.shared_bytes();
    let choices = BTreeMap::from([
        (FIRST, ProfileSyncChoice::Remote),
        (SECOND, ProfileSyncChoice::Remote),
    ]);
    assert!(matches!(
        Clients::service(&clients.b).apply(review, choices, password(), &AtomicBool::new(false)),
        Err(ProfileSyncError::Incomplete)
    ));
    assert_eq!(std::fs::read(clients.b.path()).checked(), local);
    assert_eq!(clients.shared_bytes(), peer);
}

#[test]
fn additional_or_substituted_unreviewed_identity_cannot_save_or_publish() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let local = std::fs::read(clients.b.path()).checked();
    let peer = clients.shared_bytes();
    for replace_existing in [false, true] {
        let review = clients.inspect(&clients.b);
        let mut choices = Clients::choices(&review, ProfileSyncChoice::Remote);
        if replace_existing {
            choices.remove(&THIRD);
        }
        choices.insert(Uuid::from_u128(999), ProfileSyncChoice::Local);
        assert!(matches!(
            Clients::service(&clients.b).apply(
                review,
                choices,
                password(),
                &AtomicBool::new(false)
            ),
            Err(ProfileSyncError::Incomplete)
        ));
        assert_eq!(std::fs::read(clients.b.path()).checked(), local);
        assert_eq!(clients.shared_bytes(), peer);
    }
}

#[test]
fn multiple_offline_conflicts_accept_individual_local_and_remote_choices() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    clients.sync(&clients.b, ProfileSyncChoice::Remote);
    clients.edit_all(&clients.a, "Peer");
    clients.edit_all(&clients.b, "Local");
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let review = clients.inspect(&clients.b);
    assert_eq!(review.rows().len(), 3);
    assert!(review.rows().iter().all(|row| row.conflict));
    let choices = BTreeMap::from([
        (FIRST, ProfileSyncChoice::Local),
        (SECOND, ProfileSyncChoice::Remote),
        (THIRD, ProfileSyncChoice::Local),
    ]);
    assert!(
        Clients::service(&clients.b)
            .apply(review, choices, password(), &AtomicBool::new(false))
            .checked()
            .published
    );
    clients.sync(&clients.a, ProfileSyncChoice::Remote);
    let a = clients.a.load().checked();
    let b = clients.b.load().checked();
    assert_eq!(a.connections, b.connections);
    for profile in &b.connections {
        let prefix = if profile.id == SECOND {
            "Peer"
        } else {
            "Local"
        };
        assert_eq!(profile.name, format!("{prefix} {}", profile.id));
    }
}

#[test]
fn remote_deletion_conflicts_with_local_edit_and_can_explicitly_restore_local() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    clients.sync(&clients.b, ProfileSyncChoice::Remote);
    let mut a = clients.a.load().checked();
    a.soft_delete_connection(FIRST, 7).checked();
    clients.a.save(&a).checked();
    let mut b = clients.b.load().checked();
    b.connections
        .iter_mut()
        .find(|p| p.id == FIRST)
        .checked()
        .name = "Keep this edit".into();
    clients.b.save(&b).checked();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let review = clients.inspect(&clients.b);
    let row = review.rows().iter().find(|r| r.id == FIRST).checked();
    assert!(row.conflict && row.remote.is_none() && row.local.is_some());
    assert_eq!(row.local.as_ref().checked().name, "Keep this edit");
    clients.sync(&clients.b, ProfileSyncChoice::Local);
    clients.sync(&clients.a, ProfileSyncChoice::Remote);
    let a = clients.a.load().checked();
    assert_eq!(
        a.connections.iter().find(|p| p.id == FIRST).checked().name,
        "Keep this edit"
    );
    assert!(
        !a.deleted_connections
            .iter()
            .any(|p| p.connection.id == FIRST)
    );
}

#[test]
fn unknown_envelope_and_kdf_fields_are_not_accepted() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let good: Value = serde_json::from_slice(&clients.shared_bytes()).checked();
    for in_kdf in [false, true] {
        let mut candidate = good.clone();
        if in_kdf {
            candidate["kdf"]["unknown_policy"] = json!("ignored");
        } else {
            candidate["unknown_policy"] = json!("ignored");
        }
        std::fs::write(
            clients.directory.join(FILE),
            serde_json::to_vec(&candidate).checked(),
        )
        .checked();
        clients.assert_rejected_without_local_write(&clients.b, |e| {
            matches!(e, ProfileSyncError::Storage(_))
        });
    }
}

#[test]
fn unknown_authenticated_snapshot_record_and_profile_fields_are_not_accepted() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let original = clients.snapshot(&clients.a);
    for level in 0..3 {
        let mut candidate = original.clone();
        match level {
            0 => candidate["unknown_policy"] = json!("ignored"),
            1 => candidate["records"][FIRST.to_string()]["unknown_policy"] = json!("ignored"),
            _ => {
                candidate["records"][FIRST.to_string()]["profile"]["credential_ref"] =
                    json!(Uuid::from_u128(913));
            }
        }
        clients.replace_authenticated_snapshot(&candidate, 20 + level);
        clients.assert_rejected_without_local_write(&clients.b, |e| {
            matches!(e, ProfileSyncError::Invalid)
        });
    }
}

#[test]
fn established_client_refuses_another_directory_even_with_identical_channel_bytes() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let alternate = clients.temporary.path().join("different-directory");
    std::fs::create_dir(&alternate).checked();
    std::fs::write(alternate.join(FILE), clients.shared_bytes()).checked();
    let before = std::fs::read(clients.a.path()).checked();
    assert!(matches!(
        Clients::service(&clients.a).inspect(alternate, password(), &AtomicBool::new(false)),
        Err(ProfileSyncError::Channel)
    ));
    assert_eq!(std::fs::read(clients.a.path()).checked(), before);
}

#[test]
fn acknowledged_generation_rejects_a_different_valid_ciphertext_of_the_same_snapshot() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    assert!(clients.inspect(&clients.a).rows().is_empty());
    let good = clients.shared_bytes();
    let snapshot = clients.snapshot(&clients.a);
    clients.replace_authenticated_snapshot(&snapshot, 30);
    assert_ne!(clients.shared_bytes(), good);
    // A fresh client authenticates this wire body; the established client must
    // reject the changed receipt rather than treating it as a new generation.
    assert_eq!(clients.inspect(&clients.b).rows().len(), 3);
    clients
        .assert_rejected_without_local_write(&clients.a, |e| matches!(e, ProfileSyncError::Replay));
}

#[test]
fn higher_generation_cannot_drop_a_previously_acknowledged_live_record() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let mut snapshot = clients.snapshot(&clients.a);
    let generation = snapshot["generation"].as_u64().checked();
    snapshot["generation"] = json!(generation + 1);
    snapshot["records"]
        .as_object_mut()
        .checked()
        .remove(&FIRST.to_string());
    clients.replace_authenticated_snapshot(&snapshot, 31);
    clients
        .assert_rejected_without_local_write(&clients.a, |e| matches!(e, ProfileSyncError::Replay));
}

#[test]
fn higher_generation_cannot_drop_a_previously_acknowledged_deletion_tombstone() {
    let clients = Clients::new();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let mut state = clients.a.load().checked();
    state.soft_delete_connection(FIRST, 11).checked();
    clients.a.save(&state).checked();
    clients.sync(&clients.a, ProfileSyncChoice::Local);
    let mut snapshot = clients.snapshot(&clients.a);
    assert!(snapshot["records"][FIRST.to_string()]["profile"].is_null());
    snapshot["generation"] = json!(snapshot["generation"].as_u64().checked() + 1);
    snapshot["records"]
        .as_object_mut()
        .checked()
        .remove(&FIRST.to_string());
    clients.replace_authenticated_snapshot(&snapshot, 32);
    clients
        .assert_rejected_without_local_write(&clients.a, |e| matches!(e, ProfileSyncError::Replay));
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
