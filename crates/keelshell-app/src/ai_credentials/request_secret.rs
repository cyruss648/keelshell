//! Vault payloads bind the exact request-secret role and reference identity.

use super::*;
use crate::ai_request_options::{RequestSecret, SecretPurpose, reference_for};
use keelshell_core::AiProxy;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    version: u32,
    reference: Uuid,
    profile_id: Uuid,
    endpoint: String,
    api_style: AiApiStyle,
    purpose: String,
    proxy_url: Option<String>,
    header_value: String,
    username: String,
    password: String,
}

impl Drop for Payload {
    fn drop(&mut self) {
        self.header_value.zeroize();
        self.username.zeroize();
        self.password.zeroize();
    }
}

fn binding(
    profile: &NamedAiProfile,
    purpose: &SecretPurpose,
) -> Result<(String, Option<String>), Error> {
    if profile.backend != AiBackend::Api || reference_for(profile, purpose).is_none() {
        return Err(Error::VaultEntryMismatch);
    }
    Ok(match purpose {
        SecretPurpose::Header(name) => (format!("header:{}", name.to_ascii_lowercase()), None),
        SecretPurpose::Proxy => match &profile.proxy {
            AiProxy::Explicit {
                url,
                credentials: Some(_),
            } => ("proxy".into(), Some(url.clone())),
            _ => return Err(Error::VaultEntryMismatch),
        },
    })
}

fn encode_request(
    profile: &NamedAiProfile,
    purpose: &SecretPurpose,
    reference: Uuid,
    value: RequestSecret,
) -> Result<Zeroizing<String>, Error> {
    let (purpose_name, proxy_url) = binding(profile, purpose)?;
    let mut payload = Payload {
        version: 1,
        reference,
        profile_id: profile.id,
        endpoint: profile.endpoint.clone(),
        api_style: profile.api_style,
        purpose: purpose_name,
        proxy_url,
        header_value: String::new(),
        username: String::new(),
        password: String::new(),
    };
    match (purpose, value) {
        (SecretPurpose::Header(_), RequestSecret::Header(value)) => {
            keelshell_ai::RequestOptions::new(
                vec![(
                    match purpose {
                        SecretPurpose::Header(name) => name.clone(),
                        _ => return Err(Error::VaultEntryMismatch),
                    },
                    value.clone(),
                )],
                keelshell_ai::ProxyRoute::Direct,
            )
            .map_err(|_| Error::VaultInvalidSecret)?;
            payload.header_value = value.to_string();
        }
        (SecretPurpose::Proxy, RequestSecret::Proxy { username, password }) => {
            keelshell_ai::ProxyCredentials::new(username.clone(), password.clone())
                .map_err(|_| Error::VaultInvalidSecret)?;
            payload.username = username.to_string();
            payload.password = password.to_string();
        }
        _ => return Err(Error::VaultEntryMismatch),
    }
    serde_json::to_string(&payload)
        .map(Zeroizing::new)
        .map_err(|_| Error::VaultCorrupt)
}

fn decode_request(
    profile: &NamedAiProfile,
    purpose: &SecretPurpose,
    reference: Uuid,
    raw: Zeroizing<String>,
) -> Result<RequestSecret, Error> {
    let mut payload: Payload = serde_json::from_str(&raw).map_err(|_| Error::VaultCorrupt)?;
    let (purpose_name, proxy_url) = binding(profile, purpose)?;
    if payload.version != 1
        || payload.reference != reference
        || payload.profile_id != profile.id
        || payload.endpoint != profile.endpoint
        || payload.api_style != profile.api_style
        || payload.purpose != purpose_name
        || payload.proxy_url != proxy_url
    {
        return Err(Error::VaultEntryMismatch);
    }
    let value = match purpose {
        SecretPurpose::Header(_) if payload.username.is_empty() && payload.password.is_empty() => {
            RequestSecret::Header(Zeroizing::new(std::mem::take(&mut payload.header_value)))
        }
        SecretPurpose::Proxy if payload.header_value.is_empty() => RequestSecret::Proxy {
            username: Zeroizing::new(std::mem::take(&mut payload.username)),
            password: Zeroizing::new(std::mem::take(&mut payload.password)),
        },
        _ => return Err(Error::VaultEntryMismatch),
    };
    // Apply the same value bounds on save and unlock, even for imported ciphertext.
    encode_request(profile, purpose, reference, value.clone())?;
    Ok(value)
}

/// Background-only request-secret vault operation; it never performs HTTP I/O.
pub(crate) fn operate_request(
    path: PathBuf,
    profile: &NamedAiProfile,
    purpose: &SecretPurpose,
    action: VaultAction,
    master: Zeroizing<String>,
    value: Option<RequestSecret>,
    cancelled: &AtomicBool,
) -> Result<Completion, Error> {
    profile.validate_current_transport()?;
    binding(profile, purpose)?;
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
            let id = Uuid::new_v4();
            let payload = encode_request(
                profile,
                purpose,
                id,
                value.ok_or(Error::VaultInvalidSecret)?,
            )?;
            vault.set(id, profile.id, CredentialKind::AiRequestSecret, &payload)?;
            if cancelled.load(Ordering::Acquire) {
                return Ok(Completion::Cancelled);
            }
            store.save(&mut vault)?;
            Ok(Completion::Saved(id))
        }
        VaultAction::Unlock => {
            let Some(AiSecretRef::SecretStore { id }) = reference_for(profile, purpose) else {
                return Err(Error::VaultEntryNotFound);
            };
            decode_request(
                profile,
                purpose,
                *id,
                vault.get(*id, profile.id, CredentialKind::AiRequestSecret)?,
            )
            .map(Completion::RequestUnlocked)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_payload_rejects_cross_purpose_and_destination() -> Result<(), Error> {
        let mut p = NamedAiProfile::draft(keelshell_core::AiPreset::OpenAi);
        p.name = "Test".into();
        p.model = "model".into();
        let id = Uuid::new_v4();
        p.custom_headers = vec![keelshell_core::AiCustomHeader {
            name: "x-project".into(),
            value_ref: AiSecretRef::SecretStore { id },
        }];
        let purpose = SecretPurpose::Header("x-project".into());
        let raw = encode_request(
            &p,
            &purpose,
            id,
            RequestSecret::Header(Zeroizing::new("synthetic-value".into())),
        )?;
        assert!(matches!(
            decode_request(&p, &purpose, id, raw.clone())?,
            RequestSecret::Header(_)
        ));
        assert!(decode_request(&p, &purpose, Uuid::new_v4(), raw.clone()).is_err());
        let mut changed = p.clone();
        changed.endpoint = "https://other.invalid/v1/chat/completions".into();
        assert!(decode_request(&changed, &purpose, id, raw.clone()).is_err());
        changed = p.clone();
        changed.custom_headers[0].name = "x-other".into();
        assert!(
            decode_request(
                &changed,
                &SecretPurpose::Header("x-other".into()),
                id,
                raw.clone()
            )
            .is_err()
        );
        changed = p.clone();
        changed.proxy = AiProxy::Explicit {
            url: "http://localhost:8888".into(),
            credentials: Some(AiSecretRef::SecretStore { id }),
        };
        assert!(decode_request(&changed, &SecretPurpose::Proxy, id, raw).is_err());
        Ok(())
    }

    #[test]
    fn request_options_real_vault_roundtrip_rejects_api_key_role_and_proxy_redirect()
    -> Result<(), Error> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("vault.json");
        let master = || Zeroizing::new("synthetic master".into());
        let cancelled = AtomicBool::new(false);
        let mut profile = NamedAiProfile::draft(keelshell_core::AiPreset::OpenAi);
        profile.name = "Fixture".into();
        profile.model = "model".into();
        let header = SecretPurpose::Header("x-project".into());
        profile.custom_headers.push(keelshell_core::AiCustomHeader {
            name: "x-project".into(),
            value_ref: AiSecretRef::Ephemeral { id: Uuid::new_v4() },
        });
        let saved = operate_request(
            path.clone(),
            &profile,
            &header,
            VaultAction::Save,
            master(),
            Some(RequestSecret::Header(Zeroizing::new(
                "synthetic-header".into(),
            ))),
            &cancelled,
        )?;
        let Completion::Saved(reference) = saved else {
            panic!("saved reference")
        };
        profile.custom_headers[0].value_ref = AiSecretRef::SecretStore { id: reference };
        let result = operate_request(
            path.clone(),
            &profile,
            &header,
            VaultAction::Unlock,
            master(),
            None,
            &cancelled,
        )?;
        assert!(
            matches!(result, Completion::RequestUnlocked(RequestSecret::Header(value)) if value.as_str() == "synthetic-header")
        );
        profile.authentication = AiAuthentication::Bearer {
            credential: Some(AiSecretRef::SecretStore { id: reference }),
        };
        assert!(
            super::super::operate(
                path.clone(),
                &profile,
                VaultAction::Unlock,
                master(),
                Zeroizing::default(),
                &cancelled
            )
            .is_err()
        );
        let mut api_profile = profile.clone();
        api_profile.authentication = AiAuthentication::Bearer { credential: None };
        let Completion::Saved(api_reference) = super::super::operate(
            path.clone(),
            &api_profile,
            VaultAction::Save,
            master(),
            Zeroizing::new("synthetic-auth-key".into()),
            &cancelled,
        )?
        else {
            panic!("saved key")
        };
        profile.custom_headers[0].value_ref = AiSecretRef::SecretStore { id: api_reference };
        assert!(
            operate_request(
                path.clone(),
                &profile,
                &header,
                VaultAction::Unlock,
                master(),
                None,
                &cancelled
            )
            .is_err()
        );
        profile.proxy = AiProxy::Explicit {
            url: "http://localhost:8888".into(),
            credentials: Some(AiSecretRef::Ephemeral { id: Uuid::new_v4() }),
        };
        let value = RequestSecret::Proxy {
            username: Zeroizing::new("fixture-user".into()),
            password: Zeroizing::new("fixture-password".into()),
        };
        let Completion::Saved(proxy_reference) = operate_request(
            path.clone(),
            &profile,
            &SecretPurpose::Proxy,
            VaultAction::Save,
            master(),
            Some(value),
            &cancelled,
        )?
        else {
            panic!("saved proxy")
        };
        profile.proxy = AiProxy::Explicit {
            url: "http://localhost:8888".into(),
            credentials: Some(AiSecretRef::SecretStore {
                id: proxy_reference,
            }),
        };
        assert!(matches!(
            operate_request(
                path.clone(),
                &profile,
                &SecretPurpose::Proxy,
                VaultAction::Unlock,
                master(),
                None,
                &cancelled
            )?,
            Completion::RequestUnlocked(RequestSecret::Proxy { .. })
        ));
        profile.proxy = AiProxy::Explicit {
            url: "socks5://localhost:8888".into(),
            credentials: Some(AiSecretRef::SecretStore {
                id: proxy_reference,
            }),
        };
        assert!(
            operate_request(
                path.clone(),
                &profile,
                &SecretPurpose::Proxy,
                VaultAction::Unlock,
                master(),
                None,
                &cancelled
            )
            .is_err()
        );
        let bytes = std::fs::read_to_string(path)?;
        for secret in [
            "synthetic master",
            "synthetic-header",
            "synthetic-auth-key",
            "fixture-user",
            "fixture-password",
        ] {
            assert!(!bytes.contains(secret));
        }
        cancelled.store(true, Ordering::Release);
        assert!(matches!(
            operate_request(
                directory.path().join("must-not-exist.json"),
                &profile,
                &SecretPurpose::Proxy,
                VaultAction::Unlock,
                master(),
                None,
                &cancelled
            )?,
            Completion::Cancelled
        ));
        Ok(())
    }
}
