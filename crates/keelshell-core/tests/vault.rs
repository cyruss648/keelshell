use std::{fs, sync::OnceLock};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use keelshell_core::{
    Connection, CredentialKind, CredentialMetadata, CredentialVault, Error, VaultStore,
};
use serde_json::Value;
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const MASTER: &str = "correct horse battery staple";
const CONNECTION: Uuid = Uuid::from_u128(1);
const PASSWORD_REF: Uuid = Uuid::from_u128(2);
const KEY_REF: Uuid = Uuid::from_u128(3);
const SECRET: &str = "p@ss word 中文";
const NEW_MASTER: &str = "fresh master passphrase 中文";
const AI_PROFILE: Uuid = Uuid::from_u128(4);
const AI_REF: Uuid = Uuid::from_u128(5);
const AI_SECRET: &str = "synthetic-ai-key-for-vault-test";

// Captured from the schema 2 implementation before AI entries and rotation existed.
const ORIGINAL_SCHEMA2: &[u8] = br#"{
  "manifest": {
    "schema_version": 2,
    "kdf": {
      "algorithm": "argon2id",
      "memory_kib": 65536,
      "iterations": 3,
      "lanes": 1,
      "salt": "P/dPy3w2NujfMD9LlyZEYw=="
    },
    "entries": [
      {
        "id": "00000000-0000-0000-0000-000000000002",
        "connection_id": "00000000-0000-0000-0000-000000000001",
        "kind": "password",
        "nonce": "rbNgvmaMgNdXxk67X6cfd82TuMb79N/9",
        "ciphertext": "sUSLv6mIdG5nyoIGHplY4//fOlBPx1sY33W90bVges0="
      },
      {
        "id": "00000000-0000-0000-0000-000000000003",
        "connection_id": "00000000-0000-0000-0000-000000000001",
        "kind": "private_key_passphrase",
        "nonce": "7F8gA3sGKw+RIUQQRAEQU6Q9c4mBb+p7",
        "ciphertext": "L8MoF0KfmHatmaqiRVO80PfvBDJK4yQNwFAs7/Oe"
      }
    ]
  },
  "nonce": "2JUgEPw7Ra/qjwEypkWghZYM2HOowA0B",
  "authentication": "zi3A0Joj6cFq75EgHkklgQ=="
}"#;

fn fixture() -> Result<&'static [u8], Box<dyn std::error::Error>> {
    static BYTES: OnceLock<Result<Vec<u8>, Error>> = OnceLock::new();
    BYTES
        .get_or_init(|| {
            let mut vault = CredentialVault::create(MASTER)?;
            vault.set(PASSWORD_REF, CONNECTION, CredentialKind::Password, SECRET)?;
            vault.set(
                KEY_REF,
                CONNECTION,
                CredentialKind::PrivateKeyPassphrase,
                "key-passphrase",
            )?;
            vault.to_bytes()
        })
        .as_deref()
        .map_err(|error| format!("test vault setup failed: {error}").into())
}

fn sample() -> Result<CredentialVault, Box<dyn std::error::Error>> {
    Ok(CredentialVault::unlock(fixture()?, MASTER)?)
}

fn write_owner_only(path: &std::path::Path, bytes: &[u8]) -> TestResult {
    fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[test]
fn vault_encrypts_and_round_trips_both_credential_roles_without_secret_debug() -> TestResult {
    let vault = sample()?;
    let bytes = vault.to_bytes()?;
    let serialized = String::from_utf8_lossy(&bytes);
    assert!(!serialized.contains(SECRET));
    assert!(!serialized.contains("key-passphrase"));
    assert!(!serialized.contains(MASTER));
    assert_eq!(format!("{vault:?}"), "CredentialVault { entries: 2, .. }");
    assert_eq!(
        vault
            .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
            .as_str(),
        SECRET
    );
    assert_eq!(
        vault
            .get(KEY_REF, CONNECTION, CredentialKind::PrivateKeyPassphrase)?
            .as_str(),
        "key-passphrase"
    );
    // Each serialization creates a new manifest authentication nonce.
    assert_ne!(bytes, vault.to_bytes()?);
    Ok(())
}

#[test]
fn wrong_password_cannot_unlock_either_empty_or_nonempty_vault() -> TestResult {
    let empty = CredentialVault::create(MASTER)?.to_bytes()?;
    assert!(matches!(
        CredentialVault::unlock(&empty, "wrong password"),
        Err(Error::VaultUnlockFailed)
    ));
    assert!(CredentialVault::unlock(&empty, MASTER)?.is_empty());
    assert!(matches!(
        CredentialVault::unlock(fixture()?, "wrong password"),
        Err(Error::VaultUnlockFailed)
    ));
    Ok(())
}

#[test]
fn manifest_rejects_deleted_reordered_inserted_and_replaced_entries_before_unlock() -> TestResult {
    let original: Value = serde_json::from_slice(fixture()?)?;
    let mut deleted = original.clone();
    deleted["manifest"]["entries"]
        .as_array_mut()
        .ok_or("missing entries")?
        .remove(0);
    let mut emptied = original.clone();
    emptied["manifest"]["entries"] = serde_json::json!([]);
    let mut reordered = original.clone();
    reordered["manifest"]["entries"]
        .as_array_mut()
        .ok_or("missing entries")?
        .swap(0, 1);
    let mut inserted = original.clone();
    let mut additional = inserted["manifest"]["entries"][0].clone();
    additional["id"] = serde_json::json!(Uuid::new_v4());
    inserted["manifest"]["entries"]
        .as_array_mut()
        .ok_or("missing entries")?
        .push(additional);
    let mut replaced = original.clone();
    replaced["manifest"]["entries"][0]["ciphertext"] =
        original["manifest"]["entries"][1]["ciphertext"].clone();
    for document in [deleted, emptied, reordered, inserted, replaced] {
        assert!(matches!(
            CredentialVault::unlock(&serde_json::to_vec(&document)?, MASTER),
            Err(Error::VaultUnlockFailed)
        ));
    }
    Ok(())
}

#[test]
fn envelope_authenticates_entry_identity_role_nonce_and_kdf_salt() -> TestResult {
    let original: Value = serde_json::from_slice(fixture()?)?;
    for field in ["id", "connection_id", "kind", "nonce", "ciphertext"] {
        let mut tampered = original.clone();
        tampered["manifest"]["entries"][0][field] = match field {
            "id" | "connection_id" => serde_json::json!(Uuid::new_v4()),
            "kind" => serde_json::json!("private_key_passphrase"),
            "nonce" => serde_json::json!(BASE64.encode([0_u8; 24])),
            _ => {
                let mut ciphertext = BASE64.decode(
                    tampered["manifest"]["entries"][0][field]
                        .as_str()
                        .ok_or("missing cipher")?,
                )?;
                ciphertext[0] ^= 1;
                serde_json::json!(BASE64.encode(ciphertext))
            }
        };
        assert!(matches!(
            CredentialVault::unlock(&serde_json::to_vec(&tampered)?, MASTER),
            Err(Error::VaultUnlockFailed)
        ));
    }
    let mut salt = original;
    salt["manifest"]["kdf"]["salt"] = serde_json::json!(BASE64.encode([0_u8; 16]));
    assert!(matches!(
        CredentialVault::unlock(&serde_json::to_vec(&salt)?, MASTER),
        Err(Error::VaultUnlockFailed)
    ));
    Ok(())
}

#[test]
fn invalid_kdf_parameters_and_legacy_documents_fail_before_expensive_work() -> TestResult {
    let original: Value = serde_json::from_slice(fixture()?)?;
    for field in ["memory_kib", "iterations", "lanes"] {
        let mut document = original.clone();
        document["manifest"]["kdf"][field] = serde_json::json!(u32::MAX);
        assert!(matches!(
            CredentialVault::unlock(&serde_json::to_vec(&document)?, MASTER),
            Err(Error::VaultUnsupportedKdf)
        ));
    }
    let mut unsupported = original.clone();
    unsupported["manifest"]["kdf"]["algorithm"] = serde_json::json!("argon2i");
    assert!(matches!(
        CredentialVault::unlock(&serde_json::to_vec(&unsupported)?, MASTER),
        Err(Error::VaultUnsupportedKdf)
    ));
    assert!(matches!(
        CredentialVault::unlock(br#"{"schema_version":1,"kdf":{},"entries":[]}"#, MASTER),
        Err(Error::VaultCorrupt)
    ));
    assert!(matches!(
        CredentialVault::unlock(fixture()?, ""),
        Err(Error::VaultInvalidPassphrase)
    ));
    assert!(matches!(
        CredentialVault::unlock(fixture()?, &"x".repeat(4097)),
        Err(Error::VaultInvalidPassphrase)
    ));
    assert!(matches!(
        CredentialVault::unlock(&vec![b' '; 4 * 1024 * 1024 + 1], MASTER),
        Err(Error::TooLarge)
    ));
    Ok(())
}

#[test]
fn references_cannot_be_copied_to_other_profiles_or_rebound_during_update() -> TestResult {
    let mut vault = sample()?;
    let unrelated = Uuid::new_v4();
    assert!(matches!(
        vault.get(unrelated, CONNECTION, CredentialKind::Password),
        Err(Error::VaultEntryNotFound)
    ));
    assert!(matches!(
        vault.get(PASSWORD_REF, unrelated, CredentialKind::Password),
        Err(Error::VaultEntryMismatch)
    ));
    assert!(matches!(
        vault.get(
            PASSWORD_REF,
            CONNECTION,
            CredentialKind::PrivateKeyPassphrase
        ),
        Err(Error::VaultEntryMismatch)
    ));
    assert!(matches!(
        vault.set(PASSWORD_REF, unrelated, CredentialKind::Password, "changed"),
        Err(Error::VaultEntryMismatch)
    ));
    assert!(matches!(
        vault.set(
            PASSWORD_REF,
            CONNECTION,
            CredentialKind::PrivateKeyPassphrase,
            "changed"
        ),
        Err(Error::VaultEntryMismatch)
    ));
    assert_eq!(
        vault
            .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
            .as_str(),
        SECRET
    );
    vault.set(
        PASSWORD_REF,
        CONNECTION,
        CredentialKind::Password,
        "updated",
    )?;
    assert_eq!(vault.len(), 2);
    assert!(vault.remove(KEY_REF));
    assert!(!vault.remove(KEY_REF));
    let restored = CredentialVault::unlock(&vault.to_bytes()?, MASTER)?;
    assert_eq!(restored.len(), 1);
    assert_eq!(
        restored
            .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
            .as_str(),
        "updated"
    );
    Ok(())
}

#[test]
fn invalid_secret_input_cannot_modify_existing_entry() -> TestResult {
    let mut vault = sample()?;
    for secret in [
        "".to_owned(),
        "has\0nul".to_owned(),
        "x".repeat(1024 * 1024 + 1),
    ] {
        assert!(matches!(
            vault.set(PASSWORD_REF, CONNECTION, CredentialKind::Password, &secret),
            Err(Error::VaultInvalidSecret)
        ));
    }
    assert!(matches!(
        vault.set(Uuid::nil(), CONNECTION, CredentialKind::Password, "secret"),
        Err(Error::VaultInvalidEntry)
    ));
    assert_eq!(
        vault
            .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
            .as_str(),
        SECRET
    );
    Ok(())
}

#[test]
fn aggregate_document_limit_rolls_back_new_and_replacement_entries() -> TestResult {
    let mut vault = sample()?;
    let secret = "x".repeat(1024 * 1024);
    for _ in 0..2 {
        vault.set(
            Uuid::new_v4(),
            CONNECTION,
            CredentialKind::Password,
            &secret,
        )?;
    }
    let rejected_ref = Uuid::new_v4();
    assert!(matches!(
        vault.set(rejected_ref, CONNECTION, CredentialKind::Password, &secret),
        Err(Error::TooLarge)
    ));
    assert!(matches!(
        vault.get(rejected_ref, CONNECTION, CredentialKind::Password),
        Err(Error::VaultEntryNotFound)
    ));
    assert!(matches!(
        vault.set(PASSWORD_REF, CONNECTION, CredentialKind::Password, &secret),
        Err(Error::TooLarge)
    ));
    assert_eq!(vault.len(), 4);
    assert_eq!(
        vault
            .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
            .as_str(),
        SECRET
    );
    assert!(vault.to_bytes()?.len() <= 4 * 1024 * 1024);
    Ok(())
}

#[test]
fn vault_store_creates_owner_only_file_only_on_save_and_reuses_saved_snapshot() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("nested/vault.json");
    let store = VaultStore::new(&path);
    let mut vault = store.load(MASTER)?;
    assert!(!path.exists());
    vault.set(PASSWORD_REF, CONNECTION, CredentialKind::Password, SECRET)?;
    store.save(&mut vault)?;
    assert!(!fs::read_to_string(&path)?.contains(SECRET));
    vault.set(
        PASSWORD_REF,
        CONNECTION,
        CredentialKind::Password,
        "updated",
    )?;
    store.save(&mut vault)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(path.with_extension("json.lock"))?
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(path.parent().ok_or("missing parent")?)?
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    assert_eq!(
        store
            .load(MASTER)?
            .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
            .as_str(),
        "updated"
    );
    Ok(())
}

#[test]
fn independent_writers_cannot_lose_saved_changes() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, fixture()?)?;
    let first = VaultStore::new(&path);
    let second = VaultStore::new(&path);
    let mut first_edit = first.load(MASTER)?;
    let mut stale = second.load(MASTER)?;
    first_edit.remove(KEY_REF);
    first.save(&mut first_edit)?;
    let committed = fs::read(&path)?;
    stale.set(
        PASSWORD_REF,
        CONNECTION,
        CredentialKind::Password,
        "stale edit",
    )?;
    assert!(matches!(second.save(&mut stale), Err(Error::VaultConflict)));
    assert_eq!(fs::read(&path)?, committed);
    Ok(())
}

#[test]
fn stale_snapshots_from_same_store_cannot_overwrite_a_new_revision() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, fixture()?)?;
    let store = VaultStore::new(&path);
    let mut first = store.load(MASTER)?;
    let mut stale = store.load(MASTER)?;
    first.remove(KEY_REF);
    store.save(&mut first)?;
    let committed = fs::read(&path)?;
    assert!(matches!(store.save(&mut stale), Err(Error::VaultConflict)));
    assert_eq!(fs::read(&path)?, committed);
    Ok(())
}

#[test]
fn save_without_load_cannot_replace_existing_vault_with_another_key() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, fixture()?)?;
    let store = VaultStore::new(&path);
    let mut different = CredentialVault::create("another master password")?;
    assert!(matches!(
        store.save(&mut different),
        Err(Error::VaultNotLoaded)
    ));
    assert_eq!(fs::read(&path)?, fixture()?);
    Ok(())
}

#[test]
fn copied_serialized_snapshot_and_cross_store_snapshot_cannot_replace_loaded_vault() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, fixture()?)?;
    let store = VaultStore::new(&path);
    let _loaded = store.load(MASTER)?;
    let mut copy = sample()?;
    assert!(matches!(store.save(&mut copy), Err(Error::VaultConflict)));
    let different_store = VaultStore::new(&path);
    let mut foreign = different_store.load(MASTER)?;
    assert!(matches!(
        store.save(&mut foreign),
        Err(Error::VaultConflict)
    ));
    let destination = VaultStore::new(temp.path().join("new-vault.json"));
    assert!(matches!(
        destination.save(&mut foreign),
        Err(Error::VaultConflict)
    ));
    assert!(!destination.path().exists());
    assert_eq!(fs::read(&path)?, fixture()?);
    Ok(())
}

#[test]
fn failed_unlock_discards_previous_save_authority_and_preserves_original() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, fixture()?)?;
    let store = VaultStore::new(&path);
    let mut old = store.load(MASTER)?;
    assert!(matches!(
        store.load("wrong password"),
        Err(Error::VaultUnlockFailed)
    ));
    assert!(matches!(store.save(&mut old), Err(Error::VaultNotLoaded)));
    assert_eq!(fs::read(&path)?, fixture()?);
    Ok(())
}

#[test]
fn changed_or_deleted_disk_snapshot_is_never_silently_recreated() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, fixture()?)?;
    let store = VaultStore::new(&path);
    let mut vault = store.load(MASTER)?;
    let mut edited: Value = serde_json::from_slice(fixture()?)?;
    edited["manifest"]["entries"] = serde_json::json!([]);
    let unauthenticated = serde_json::to_vec(&edited)?;
    write_owner_only(&path, &unauthenticated)?;
    assert!(matches!(store.save(&mut vault), Err(Error::VaultConflict)));
    assert_eq!(fs::read(&path)?, unauthenticated);
    fs::remove_file(&path)?;
    assert!(matches!(store.save(&mut vault), Err(Error::VaultConflict)));
    assert!(!path.exists());
    Ok(())
}

#[test]
fn invalid_existing_file_is_preserved_without_replacement() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, b"invalid original")?;
    let store = VaultStore::new(&path);
    let mut vault = sample()?;
    assert!(matches!(store.load(MASTER), Err(Error::VaultCorrupt)));
    assert!(matches!(store.save(&mut vault), Err(Error::VaultCorrupt)));
    assert_eq!(fs::read(&path)?, b"invalid original");
    Ok(())
}

#[test]
fn cooperative_lock_blocks_both_load_and_save() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, fixture()?)?;
    let lock_path = path.with_extension("json.lock");
    write_owner_only(&lock_path, b"")?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path)?;
    lock.lock()?;
    let store = VaultStore::new(&path);
    let mut vault = sample()?;
    assert!(matches!(store.load(MASTER), Err(Error::Busy)));
    assert!(matches!(store.save(&mut vault), Err(Error::Busy)));
    assert_eq!(fs::read(&path)?, fixture()?);
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinked_vault_lock_and_parent_are_rejected_without_touching_targets() -> TestResult {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    let target = temp.path().join("target");
    write_owner_only(&target, fixture()?)?;
    let store = VaultStore::new(&path);
    let mut vault = sample()?;
    symlink(&target, &path)?;
    assert!(matches!(store.load(MASTER), Err(Error::UnsafePath)));
    assert!(matches!(store.save(&mut vault), Err(Error::UnsafePath)));
    fs::remove_file(&path)?;
    write_owner_only(&path, fixture()?)?;
    // save may have created the real lock before detecting the vault symlink.
    let lock_path = path.with_extension("json.lock");
    if lock_path.exists() {
        fs::remove_file(&lock_path)?;
    }
    symlink(&target, &lock_path)?;
    assert!(matches!(store.load(MASTER), Err(Error::UnsafePath)));
    assert!(matches!(store.save(&mut vault), Err(Error::UnsafePath)));
    let linked_parent = temp.path().join("linked-parent");
    symlink(temp.path(), &linked_parent)?;
    let linked_store = VaultStore::new(linked_parent.join("vault.json"));
    assert!(matches!(linked_store.load(MASTER), Err(Error::UnsafePath)));
    assert!(matches!(
        linked_store.save(&mut vault),
        Err(Error::UnsafePath)
    ));
    assert_eq!(fs::read(&target)?, fixture()?);
    assert_eq!(fs::read(&path)?, fixture()?);
    Ok(())
}

#[cfg(unix)]
#[test]
fn insecure_vault_and_lock_permissions_fail_closed() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, fixture()?)?;
    let store = VaultStore::new(&path);
    let mut vault = sample()?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
    assert!(matches!(
        store.load(MASTER),
        Err(Error::InsecurePermissions)
    ));
    assert!(matches!(
        store.save(&mut vault),
        Err(Error::InsecurePermissions)
    ));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    let lock_path = path.with_extension("json.lock");
    fs::set_permissions(lock_path, fs::Permissions::from_mode(0o644))?;
    assert!(matches!(
        store.load(MASTER),
        Err(Error::InsecurePermissions)
    ));
    assert!(matches!(
        store.save(&mut vault),
        Err(Error::InsecurePermissions)
    ));
    assert_eq!(fs::read(&path)?, fixture()?);
    Ok(())
}

#[test]
fn connection_exports_clear_local_vault_references() -> TestResult {
    let mut connection = Connection::new("host", "host.test", "operator");
    connection.credential_ref = Some(PASSWORD_REF);
    let mut state = keelshell_core::AppState::default();
    state.connections.push(connection);
    assert!(!state.export_connections()?.contains("credential_ref"));
    Ok(())
}

#[test]
fn original_schema2_document_remains_readable_with_authenticated_metadata() -> TestResult {
    let vault = CredentialVault::unlock(ORIGINAL_SCHEMA2, MASTER)?;
    assert_eq!(vault.entries().len(), 2);
    assert_eq!(
        vault.entries().collect::<Vec<_>>(),
        vec![
            CredentialMetadata {
                reference: PASSWORD_REF,
                owner_id: CONNECTION,
                kind: CredentialKind::Password,
            },
            CredentialMetadata {
                reference: KEY_REF,
                owner_id: CONNECTION,
                kind: CredentialKind::PrivateKeyPassphrase,
            },
        ]
    );
    assert_eq!(
        vault
            .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
            .as_str(),
        SECRET
    );
    assert_eq!(
        vault
            .get(KEY_REF, CONNECTION, CredentialKind::PrivateKeyPassphrase)?
            .as_str(),
        "key-passphrase"
    );
    Ok(())
}

#[test]
fn ai_api_keys_enforce_owner_and_role_bindings_and_authenticate_metadata() -> TestResult {
    let mut vault = sample()?;
    vault.set(AI_REF, AI_PROFILE, CredentialKind::AiApiKey, AI_SECRET)?;
    let expected = CredentialMetadata {
        reference: AI_REF,
        owner_id: AI_PROFILE,
        kind: CredentialKind::AiApiKey,
    };
    let mut entries = vault.entries();
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries.next().map(|entry| entry.reference),
        Some(PASSWORD_REF)
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(entries.last(), Some(expected));
    for role in [
        CredentialKind::Password,
        CredentialKind::PrivateKeyPassphrase,
    ] {
        assert!(matches!(
            vault.get(AI_REF, AI_PROFILE, role),
            Err(Error::VaultEntryMismatch)
        ));
        assert!(matches!(
            vault.set(AI_REF, AI_PROFILE, role, "changed"),
            Err(Error::VaultEntryMismatch)
        ));
    }
    assert!(matches!(
        vault.get(AI_REF, CONNECTION, CredentialKind::AiApiKey),
        Err(Error::VaultEntryMismatch)
    ));
    assert!(matches!(
        vault.set(AI_REF, CONNECTION, CredentialKind::AiApiKey, "changed"),
        Err(Error::VaultEntryMismatch)
    ));
    assert!(matches!(
        vault.get(PASSWORD_REF, CONNECTION, CredentialKind::AiApiKey),
        Err(Error::VaultEntryMismatch)
    ));
    assert!(matches!(
        vault.set(
            PASSWORD_REF,
            CONNECTION,
            CredentialKind::AiApiKey,
            "changed"
        ),
        Err(Error::VaultEntryMismatch)
    ));
    let bytes = vault.to_bytes()?;
    let document: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(document["manifest"]["schema_version"], 2);
    assert_eq!(document["manifest"]["entries"][2]["kind"], "ai_api_key");
    assert!(!String::from_utf8_lossy(&bytes).contains(AI_SECRET));
    let restored = CredentialVault::unlock(&bytes, MASTER)?;
    assert_eq!(restored.entries().last(), Some(expected));
    assert_eq!(
        restored
            .get(AI_REF, AI_PROFILE, CredentialKind::AiApiKey)?
            .as_str(),
        AI_SECRET
    );
    for (field, replacement) in [
        ("id", serde_json::json!(Uuid::new_v4())),
        ("connection_id", serde_json::json!(CONNECTION)),
        ("kind", serde_json::json!("password")),
        ("kind", serde_json::json!("private_key_passphrase")),
    ] {
        let mut tampered = document.clone();
        tampered["manifest"]["entries"][2][field] = replacement;
        assert!(matches!(
            CredentialVault::unlock(&serde_json::to_vec(&tampered)?, MASTER),
            Err(Error::VaultUnlockFailed)
        ));
    }
    Ok(())
}

#[test]
fn rotation_preserves_all_credentials_with_fresh_salt_and_nonces() -> TestResult {
    let mut vault = sample()?;
    vault.set(AI_REF, AI_PROFILE, CredentialKind::AiApiKey, AI_SECRET)?;
    let metadata = vault.entries().collect::<Vec<_>>();
    let before: Value = serde_json::from_slice(&vault.to_bytes()?)?;
    vault.rotate_passphrase(NEW_MASTER)?;
    let bytes = vault.to_bytes()?;
    let after: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(after["manifest"]["schema_version"], 2);
    assert_ne!(
        before["manifest"]["kdf"]["salt"],
        after["manifest"]["kdf"]["salt"]
    );
    assert_ne!(before["nonce"], after["nonce"]);
    assert_eq!(vault.entries().collect::<Vec<_>>(), metadata);
    for index in 0..vault.len() {
        for field in ["nonce", "ciphertext"] {
            assert_ne!(
                before["manifest"]["entries"][index][field],
                after["manifest"]["entries"][index][field]
            );
        }
    }
    assert!(matches!(
        CredentialVault::unlock(&bytes, MASTER),
        Err(Error::VaultUnlockFailed)
    ));
    let restored = CredentialVault::unlock(&bytes, NEW_MASTER)?;
    assert_eq!(restored.entries().collect::<Vec<_>>(), metadata);
    for (reference, owner, kind, secret) in [
        (PASSWORD_REF, CONNECTION, CredentialKind::Password, SECRET),
        (
            KEY_REF,
            CONNECTION,
            CredentialKind::PrivateKeyPassphrase,
            "key-passphrase",
        ),
        (AI_REF, AI_PROFILE, CredentialKind::AiApiKey, AI_SECRET),
    ] {
        assert_eq!(vault.get(reference, owner, kind)?.as_str(), secret);
        assert_eq!(restored.get(reference, owner, kind)?.as_str(), secret);
    }
    Ok(())
}

#[test]
fn rotation_rekeys_an_empty_vault() -> TestResult {
    let mut vault = CredentialVault::create(MASTER)?;
    vault.rotate_passphrase(NEW_MASTER)?;
    let bytes = vault.to_bytes()?;
    assert!(matches!(
        CredentialVault::unlock(&bytes, MASTER),
        Err(Error::VaultUnlockFailed)
    ));
    assert!(CredentialVault::unlock(&bytes, NEW_MASTER)?.is_empty());
    assert_eq!(vault.entries().len(), 0);
    Ok(())
}

#[test]
fn invalid_rotation_retains_original_key_manifest_and_save_snapshot() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, ORIGINAL_SCHEMA2)?;
    let store = VaultStore::new(&path);
    let mut vault = store.load(MASTER)?;
    let before: Value = serde_json::from_slice(&vault.to_bytes()?)?;
    for invalid in [String::new(), "x".repeat(4097)] {
        assert!(matches!(
            vault.rotate_passphrase(&invalid),
            Err(Error::VaultInvalidPassphrase)
        ));
        let after: Value = serde_json::from_slice(&vault.to_bytes()?)?;
        assert_eq!(before["manifest"], after["manifest"]);
        assert_eq!(
            vault
                .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
                .as_str(),
            SECRET
        );
    }
    store.save(&mut vault)?;
    assert_eq!(store.load(MASTER)?.len(), 2);
    Ok(())
}

#[test]
fn loaded_rotation_persists_and_allows_second_save_and_new_password_reload() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, ORIGINAL_SCHEMA2)?;
    let store = VaultStore::new(&path);
    let mut vault = store.load(MASTER)?;
    vault.rotate_passphrase(NEW_MASTER)?;
    assert_eq!(fs::read(&path)?, ORIGINAL_SCHEMA2);
    store.save(&mut vault)?;
    assert!(matches!(
        CredentialVault::unlock(&fs::read(&path)?, MASTER),
        Err(Error::VaultUnlockFailed)
    ));
    vault.set(AI_REF, AI_PROFILE, CredentialKind::AiApiKey, AI_SECRET)?;
    store.save(&mut vault)?;
    let mut restored = store.load(NEW_MASTER)?;
    assert_eq!(
        restored
            .get(AI_REF, AI_PROFILE, CredentialKind::AiApiKey)?
            .as_str(),
        AI_SECRET
    );
    restored.remove(KEY_REF);
    store.save(&mut restored)?;
    let final_vault = VaultStore::new(&path).load(NEW_MASTER)?;
    assert_eq!(final_vault.len(), 2);
    assert_eq!(
        final_vault
            .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
            .as_str(),
        SECRET
    );
    Ok(())
}

#[test]
fn stale_old_key_writer_cannot_overwrite_committed_rotation() -> TestResult {
    for same_store in [false, true] {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("vault.json");
        write_owner_only(&path, ORIGINAL_SCHEMA2)?;
        let first = VaultStore::new(&path);
        let second = VaultStore::new(&path);
        let stale_store = if same_store { &first } else { &second };
        let mut rotation = first.load(MASTER)?;
        let mut stale = stale_store.load(MASTER)?;
        rotation.rotate_passphrase(NEW_MASTER)?;
        first.save(&mut rotation)?;
        let committed = fs::read(&path)?;
        stale.remove(KEY_REF);
        assert!(matches!(
            stale_store.save(&mut stale),
            Err(Error::VaultConflict)
        ));
        assert_eq!(fs::read(&path)?, committed);
        assert_eq!(CredentialVault::unlock(&committed, NEW_MASTER)?.len(), 2);
    }
    Ok(())
}

#[test]
fn stale_rotation_cannot_overwrite_a_newer_save_with_original_key() -> TestResult {
    for same_store in [false, true] {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("vault.json");
        write_owner_only(&path, ORIGINAL_SCHEMA2)?;
        let first = VaultStore::new(&path);
        let second = VaultStore::new(&path);
        let stale_store = if same_store { &first } else { &second };
        let mut update = first.load(MASTER)?;
        let mut rotation = stale_store.load(MASTER)?;
        rotation.rotate_passphrase(NEW_MASTER)?;
        update.remove(KEY_REF);
        first.save(&mut update)?;
        let committed = fs::read(&path)?;
        assert!(matches!(
            stale_store.save(&mut rotation),
            Err(Error::VaultConflict)
        ));
        assert_eq!(fs::read(&path)?, committed);
        assert_eq!(CredentialVault::unlock(&committed, MASTER)?.len(), 1);
        assert_eq!(
            rotation
                .get(KEY_REF, CONNECTION, CredentialKind::PrivateKeyPassphrase)?
                .as_str(),
            "key-passphrase"
        );
    }
    Ok(())
}

#[test]
fn rotated_vault_can_retry_save_after_cooperative_lock_is_released() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, ORIGINAL_SCHEMA2)?;
    let store = VaultStore::new(&path);
    let mut vault = store.load(MASTER)?;
    vault.rotate_passphrase(NEW_MASTER)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path.with_extension("json.lock"))?;
    lock.lock()?;
    assert!(matches!(store.save(&mut vault), Err(Error::Busy)));
    assert_eq!(fs::read(&path)?, ORIGINAL_SCHEMA2);
    drop(lock);
    store.save(&mut vault)?;
    assert_eq!(store.load(NEW_MASTER)?.len(), 2);
    Ok(())
}

#[test]
fn load_existing_missing_vault_creates_neither_files_nor_directories() -> TestResult {
    let temp = tempfile::tempdir()?;
    for path in [
        temp.path().join("vault.json"),
        temp.path().join("missing/nested/vault.json"),
    ] {
        let store = VaultStore::new(&path);
        assert!(matches!(
            store.load_existing(MASTER),
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound
        ));
        assert!(!path.exists());
        assert!(!path.with_extension("json.lock").exists());
        assert_eq!(fs::read_dir(temp.path())?.count(), 0);
        // The separate create-on-load API retains its original in-memory behavior.
        assert!(store.load(MASTER)?.is_empty());
        assert_eq!(fs::read_dir(temp.path())?.count(), 0);
    }
    Ok(())
}

#[test]
fn load_existing_authenticates_saved_credentials_and_establishes_save_snapshot() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, ORIGINAL_SCHEMA2)?;
    let store = VaultStore::new(&path);
    let mut vault = store.load_existing(MASTER)?;
    assert_eq!(
        vault
            .get(PASSWORD_REF, CONNECTION, CredentialKind::Password)?
            .as_str(),
        SECRET
    );
    assert!(vault.remove(KEY_REF));
    store.save(&mut vault)?;
    assert_eq!(store.load_existing(MASTER)?.len(), 1);
    Ok(())
}

#[test]
fn removed_existing_vault_revokes_save_baseline_and_cannot_be_recreated() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, ORIGINAL_SCHEMA2)?;
    let store = VaultStore::new(&path);
    let mut previous = store.load_existing(MASTER)?;
    fs::remove_file(&path)?;
    assert!(matches!(
        store.load_existing(MASTER),
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(matches!(
        store.save(&mut previous),
        Err(Error::VaultConflict)
    ));
    assert!(!path.exists());
    // Even restoring identical bytes cannot restore the discarded save authority.
    write_owner_only(&path, ORIGINAL_SCHEMA2)?;
    assert!(matches!(
        store.save(&mut previous),
        Err(Error::VaultNotLoaded)
    ));
    assert_eq!(fs::read(&path)?, ORIGINAL_SCHEMA2);
    Ok(())
}

#[test]
fn failed_existing_authentication_revokes_previous_save_baseline() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("vault.json");
    write_owner_only(&path, ORIGINAL_SCHEMA2)?;
    let store = VaultStore::new(&path);
    let mut previous = store.load_existing(MASTER)?;
    assert!(matches!(
        store.load_existing("incorrect passphrase"),
        Err(Error::VaultUnlockFailed)
    ));
    assert!(matches!(
        store.save(&mut previous),
        Err(Error::VaultNotLoaded)
    ));
    assert_eq!(fs::read(&path)?, ORIGINAL_SCHEMA2);
    Ok(())
}
