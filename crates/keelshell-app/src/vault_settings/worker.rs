//! Blocking operations. Inputs have zeroizing owners; output contains metadata only.

use super::*;
use keelshell_core::{AiAuthentication, AiProxy, AiSecretRef, AppState, StateStore, VaultStore};

#[derive(Clone, Copy)]
pub(super) enum Action {
    Inspect,
    Delete(Uuid),
    Rotate,
}

pub(super) enum Report {
    Ready {
        entries: Vec<CredentialMetadata>,
        in_use: BTreeSet<Uuid>,
    },
    Cancelled,
}
pub(super) enum Failure {
    Core(Error),
    Missing,
    Linked,
    Worker,
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Core(error)
    }
}

fn saved_references(state: &AppState) -> BTreeSet<Uuid> {
    let mut references: BTreeSet<_> = state
        .connections
        .iter()
        .chain(
            state
                .deleted_connections
                .iter()
                .map(|deleted| &deleted.connection),
        )
        .filter_map(|connection| connection.credential_ref)
        .collect();
    for profile in &state.settings.ai_profiles.profiles {
        if let AiAuthentication::Bearer {
            credential: Some(AiSecretRef::SecretStore { id }),
        }
        | AiAuthentication::Header {
            credential: Some(AiSecretRef::SecretStore { id }),
            ..
        } = &profile.authentication
        {
            references.insert(*id);
        }
        for header in &profile.custom_headers {
            if let AiSecretRef::SecretStore { id } = &header.value_ref {
                references.insert(*id);
            }
        }
        if let AiProxy::Explicit {
            credentials: Some(AiSecretRef::SecretStore { id }),
            ..
        } = &profile.proxy
        {
            references.insert(*id);
        }
    }
    references
}

pub(super) fn operate(
    path: PathBuf,
    state_path: PathBuf,
    mut in_use: BTreeSet<Uuid>,
    action: Action,
    master: Zeroizing<String>,
    replacement: Zeroizing<String>,
    cancelled: &AtomicBool,
) -> Result<Report, Failure> {
    if cancelled.load(Ordering::Acquire) {
        return Ok(Report::Cancelled);
    }
    let store = VaultStore::new(path);
    let mut vault = store.load_existing(&master).map_err(|error| match error {
        Error::Io(error) if error.kind() == std::io::ErrorKind::NotFound => Failure::Missing,
        error => Failure::Core(error),
    })?;
    drop(master);
    if cancelled.load(Ordering::Acquire) {
        return Ok(Report::Cancelled);
    }
    // Refresh saved references from disk as well as the workspace's frozen view.
    // This is still two independent files, not a cross-process transaction.
    in_use.extend(saved_references(
        &StateStore::new(state_path.clone()).load()?,
    ));
    match action {
        Action::Inspect => {}
        Action::Delete(reference) => {
            if in_use.contains(&reference) {
                return Err(Failure::Linked);
            }
            if !vault.remove(reference) {
                return Err(Failure::Core(Error::VaultEntryNotFound));
            }
        }
        Action::Rotate => vault.rotate_passphrase(&replacement)?,
    }
    drop(replacement);
    if cancelled.load(Ordering::Acquire) {
        return Ok(Report::Cancelled);
    }
    if let Action::Delete(reference) = action {
        // The state may have changed while Argon2 was running. Recheck immediately
        // before save admission; a subsequent external state write is not atomic
        // with this vault save and remains an explicit documented limitation.
        in_use.extend(saved_references(&StateStore::new(state_path).load()?));
        if in_use.contains(&reference) {
            return Err(Failure::Linked);
        }
    }
    if !matches!(action, Action::Inspect) {
        if cancelled.load(Ordering::Acquire) {
            return Ok(Report::Cancelled);
        }
        // Cancellation past this admission point cannot undo an atomic replacement.
        store.save(&mut vault)?;
    }
    Ok(Report::Ready {
        entries: vault.entries().collect(),
        in_use,
    })
}

#[cfg(test)]
mod tests {
    use super::{Action, Failure, Report, operate, saved_references};
    use keelshell_core::{
        AiAuthentication, AiCustomHeader, AiPreset, AiProxy, AiSecretRef, AppState, Connection,
        DeletedConnection, NamedAiProfile,
    };
    use std::{collections::BTreeSet, sync::atomic::AtomicBool};
    use uuid::Uuid;
    use zeroize::Zeroizing;

    #[test]
    fn every_saved_reference_including_trash_headers_and_proxy_is_protected() {
        let references: Vec<_> = (1..=6).map(Uuid::from_u128).collect();
        let mut state = AppState::default();
        let mut active = Connection::new("active", "localhost", "fixture");
        active.credential_ref = Some(references[0]);
        state.connections.push(active);
        let mut deleted = Connection::new("deleted", "localhost", "fixture");
        deleted.credential_ref = Some(references[1]);
        state.deleted_connections.push(DeletedConnection {
            connection: deleted,
            deleted_at: 1,
        });
        for (index, authentication) in [
            AiAuthentication::Bearer {
                credential: Some(AiSecretRef::SecretStore { id: references[2] }),
            },
            AiAuthentication::Header {
                name: "x-api-key".into(),
                credential: Some(AiSecretRef::SecretStore { id: references[3] }),
            },
        ]
        .into_iter()
        .enumerate()
        {
            let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
            profile.name = format!("profile {index}");
            profile.authentication = authentication;
            profile.custom_headers.push(AiCustomHeader {
                name: "x-tenant".into(),
                value_ref: AiSecretRef::SecretStore { id: references[4] },
            });
            profile.custom_headers.push(AiCustomHeader {
                name: "x-environment".into(),
                value_ref: AiSecretRef::Environment {
                    name: "TEST_REF".into(),
                },
            });
            profile.proxy = AiProxy::Explicit {
                url: "http://localhost:1080".into(),
                credentials: Some(AiSecretRef::SecretStore { id: references[5] }),
            };
            state.settings.ai_profiles.profiles.push(profile);
        }
        assert_eq!(saved_references(&state), references.into_iter().collect());
    }

    #[test]
    fn cancellation_precedes_io_and_inspection_never_creates_a_missing_vault() {
        let directory =
            std::env::temp_dir().join(format!("keelshell-missing-vault-{}", Uuid::new_v4()));
        let path = directory.join("vault.json");
        for action in [
            Action::Inspect,
            Action::Rotate,
            Action::Delete(Uuid::new_v4()),
        ] {
            let result = operate(
                path.clone(),
                directory.join("state.json"),
                BTreeSet::new(),
                action,
                Zeroizing::new("fixture".into()),
                Zeroizing::new("replacement".into()),
                &AtomicBool::new(true),
            );
            assert!(matches!(result, Ok(Report::Cancelled)));
            assert!(!directory.exists());
        }
        let result = operate(
            path.clone(),
            directory.join("state.json"),
            BTreeSet::new(),
            Action::Inspect,
            Zeroizing::new("fixture".into()),
            Zeroizing::new(String::new()),
            &AtomicBool::new(false),
        );
        assert!(matches!(result, Err(Failure::Missing)));
        assert!(!directory.exists());
    }
}
