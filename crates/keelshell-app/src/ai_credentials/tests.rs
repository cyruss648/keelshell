use super::*;
use keelshell_core::{AiApiStyle, AiAuthentication, AiPreset, AppState, StateStore};

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("keelshell-ai-vault-{}", Uuid::new_v4())))
    }
    fn path(&self) -> PathBuf {
        self.0.join("vault.json")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn profile() -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
    profile.name = "Provider".into();
    profile.endpoint = "https://provider.example/v1/chat/completions".into();
    profile.model = "model".into();
    profile
}
fn secret(value: &str) -> Zeroizing<String> {
    Zeroizing::new(value.into())
}

#[test]
fn binding_rejects_identity_endpoint_protocol_auth_but_allows_name_and_model() -> Result<(), Error>
{
    let profile = profile();
    let payload = encode(&profile, secret("fixture-private-key"))?;
    let mut rename = profile.clone();
    rename.name = "Renamed".into();
    rename.model = "new-model".into();
    assert_eq!(&*decode(&rename, payload.clone())?, "fixture-private-key");
    for changed in 0..4 {
        let mut retarget = profile.clone();
        match changed {
            0 => retarget.id = Uuid::new_v4(),
            1 => retarget.endpoint = "https://other.example/v1/chat/completions".into(),
            2 => retarget.api_style = AiApiStyle::Responses,
            _ => retarget.authentication = AiAuthentication::None,
        }
        assert!(matches!(
            decode(&retarget, payload.clone()),
            Err(Error::VaultEntryMismatch)
        ));
    }
    Ok(())
}

#[test]
fn restart_unlocks_bound_key_without_plaintext_in_metadata_or_vault() -> Result<(), Error> {
    let fixture = Fixture::new();
    let mut profile = profile();
    let cancelled = AtomicBool::new(false);
    let Completion::Saved(id) = operate(
        fixture.path(),
        &profile,
        VaultAction::Save,
        secret("master fixture"),
        secret("private-api-key-fixture"),
        &cancelled,
    )?
    else {
        panic!("save must return reference");
    };
    profile.authentication = AiAuthentication::Bearer {
        credential: Some(AiSecretRef::SecretStore { id }),
    };
    let state_store = StateStore::new(fixture.0.join("state.json"));
    let mut state: AppState = state_store.load()?;
    state.settings.ai_profiles.upsert(profile.clone())?;
    state_store.save(&state)?;
    let reopened_state = StateStore::new(fixture.0.join("state.json")).load()?;
    let reopened = &reopened_state.settings.ai_profiles.profiles[0];
    for path in [fixture.path(), fixture.0.join("state.json")] {
        let data = std::fs::read_to_string(path)?;
        assert!(!data.contains("private-api-key-fixture"));
        assert!(!data.contains("master fixture"));
        assert!(!data.contains("unlocked"));
    }
    assert!(matches!(
        operate(
            fixture.path(),
            reopened,
            VaultAction::Unlock,
            secret("wrong master"),
            secret(""),
            &cancelled
        ),
        Err(Error::VaultUnlockFailed)
    ));
    let Completion::Unlocked(key) = operate(
        fixture.path(),
        reopened,
        VaultAction::Unlock,
        secret("master fixture"),
        secret(""),
        &cancelled,
    )?
    else {
        panic!("explicit unlock must return key");
    };
    assert_eq!(key.as_str(), "private-api-key-fixture");
    let mut retarget = reopened.clone();
    retarget.endpoint = "https://other.example/v1/chat/completions".into();
    assert!(matches!(
        operate(
            fixture.path(),
            &retarget,
            VaultAction::Unlock,
            secret("master fixture"),
            secret(""),
            &cancelled
        ),
        Err(Error::VaultEntryMismatch)
    ));
    Ok(())
}

#[test]
fn resaving_uses_new_entry_and_cancelled_admission_writes_nothing() -> Result<(), Error> {
    let fixture = Fixture::new();
    let profile = profile();
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        operate(
            fixture.path(),
            &profile,
            VaultAction::Save,
            secret("master"),
            secret("first"),
            &cancelled
        )?,
        Completion::Cancelled
    ));
    assert!(!fixture.path().exists());
    cancelled.store(false, Ordering::Release);
    let mut references = Vec::new();
    for value in ["first", "second"] {
        let Completion::Saved(id) = operate(
            fixture.path(),
            &profile,
            VaultAction::Save,
            secret("master"),
            secret(value),
            &cancelled,
        )?
        else {
            panic!("saved reference");
        };
        references.push(id);
    }
    assert_ne!(references[0], references[1]);
    let vault = VaultStore::new(fixture.path()).load("master")?;
    assert_eq!(vault.len(), 2);
    assert_eq!(
        decode(
            &profile,
            vault.get(references[0], profile.id, CredentialKind::AiApiKey)?
        )?
        .as_str(),
        "first"
    );
    Ok(())
}

#[test]
fn anthropic_x_api_key_roundtrips_as_a_distinct_vault_binding() -> Result<(), Error> {
    let fixture = Fixture::new();
    let mut profile = profile();
    profile.api_style = AiApiStyle::AnthropicMessages;
    profile.endpoint = "https://api.anthropic.com/v1/messages".into();
    profile.authentication = AiAuthentication::Header {
        name: "x-api-key".into(),
        credential: None,
    };
    let cancelled = AtomicBool::new(false);
    let Completion::Saved(id) = operate(
        fixture.path(),
        &profile,
        VaultAction::Save,
        secret("master anthropic"),
        secret("sk-ant-fixture"),
        &cancelled,
    )?
    else {
        panic!("save must return reference");
    };
    profile.authentication = AiAuthentication::Header {
        name: "X-API-KEY".into(),
        credential: Some(AiSecretRef::SecretStore { id }),
    };
    let Completion::Unlocked(key) = operate(
        fixture.path(),
        &profile,
        VaultAction::Unlock,
        secret("master anthropic"),
        secret(""),
        &cancelled,
    )?
    else {
        panic!("unlock must return key");
    };
    assert_eq!(key.as_str(), "sk-ant-fixture");
    let data = std::fs::read_to_string(fixture.path())?;
    assert!(!data.contains("sk-ant-fixture"));
    let vault = VaultStore::new(fixture.path()).load("master anthropic")?;
    let payload = vault.get(id, profile.id, CredentialKind::AiApiKey)?;
    assert!(payload.contains("header:x-api-key"));
    Ok(())
}
