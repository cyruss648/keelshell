//! AI keys share the authenticated vault, with an encrypted destination binding.

use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use keelshell_core::{
    AiApiStyle, AiAuthentication, AiSecretRef, CredentialKind, Error, NamedAiProfile, VaultStore,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VaultAction {
    Save,
    Unlock,
}

pub(crate) enum Completion {
    Saved(Uuid),
    Unlocked(Zeroizing<String>),
    Cancelled,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundKey {
    version: u32,
    profile_id: Uuid,
    endpoint: String,
    api_style: AiApiStyle,
    authentication: String,
    key: String,
}

impl Drop for BoundKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

pub(crate) fn reference(profile: &NamedAiProfile) -> Option<Uuid> {
    match &profile.authentication {
        AiAuthentication::Bearer {
            credential: Some(AiSecretRef::SecretStore { id }),
        } => Some(*id),
        _ => None,
    }
}

fn encode(profile: &NamedAiProfile, key: Zeroizing<String>) -> Result<Zeroizing<String>, Error> {
    let payload = BoundKey {
        version: 1,
        profile_id: profile.id,
        endpoint: profile.endpoint.clone(),
        api_style: profile.api_style,
        authentication: "bearer".into(),
        key: key.to_string(),
    };
    serde_json::to_string(&payload)
        .map(Zeroizing::new)
        .map_err(|_| Error::VaultCorrupt)
}

fn decode(
    profile: &NamedAiProfile,
    payload: Zeroizing<String>,
) -> Result<Zeroizing<String>, Error> {
    let mut payload: BoundKey = serde_json::from_str(&payload).map_err(|_| Error::VaultCorrupt)?;
    if payload.version != 1
        || payload.profile_id != profile.id
        || payload.endpoint != profile.endpoint
        || payload.api_style != profile.api_style
        || payload.authentication != "bearer"
        || !matches!(profile.authentication, AiAuthentication::Bearer { .. })
    {
        return Err(Error::VaultEntryMismatch);
    }
    if payload.key.is_empty() || payload.key.contains('\0') {
        return Err(Error::VaultInvalidSecret);
    }
    Ok(Zeroizing::new(std::mem::take(&mut payload.key)))
}

/// Run only on a background executor. No profile metadata or network is written.
pub(crate) fn operate(
    path: PathBuf,
    profile: &NamedAiProfile,
    action: VaultAction,
    master: Zeroizing<String>,
    key: Zeroizing<String>,
    cancelled: &AtomicBool,
) -> Result<Completion, Error> {
    profile.validate_current_transport()?;
    if !matches!(profile.authentication, AiAuthentication::Bearer { .. }) {
        return Err(Error::VaultEntryMismatch);
    }
    if action == VaultAction::Save && (key.is_empty() || key.contains('\0')) {
        return Err(Error::VaultInvalidSecret);
    }
    if cancelled.load(Ordering::Acquire) {
        return Ok(Completion::Cancelled);
    }
    let store = VaultStore::new(path);
    let mut vault = store.load(&master)?;
    drop(master);
    if cancelled.load(Ordering::Acquire) {
        return Ok(Completion::Cancelled);
    }
    match action {
        VaultAction::Save => {
            // A draft can be cancelled after this write. Its orphaned ciphertext
            // is safe to clean later; an existing reference is never overwritten.
            let id = Uuid::new_v4();
            let payload = encode(profile, key)?;
            vault.set(id, profile.id, CredentialKind::AiApiKey, &payload)?;
            if cancelled.load(Ordering::Acquire) {
                return Ok(Completion::Cancelled);
            }
            store.save(&mut vault)?;
            Ok(Completion::Saved(id))
        }
        VaultAction::Unlock => {
            let id = reference(profile).ok_or(Error::VaultEntryNotFound)?;
            decode(
                profile,
                vault.get(id, profile.id, CredentialKind::AiApiKey)?,
            )
            .map(Completion::Unlocked)
        }
    }
}

#[cfg(test)]
mod tests;
