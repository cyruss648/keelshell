//! Purpose-bound process credentials and explicit API reference resolution.

use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use keelshell_ai::{AiError, ProxyCredentials, ProxyRoute, RequestOptions};
use keelshell_core::{
    AiApiStyle, AiAuthentication, AiBackend, AiProfileCatalog, AiProxy, AiReasoningCapability,
    AiReasoningSelection, AiSecretRef, NamedAiProfile,
};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SecretPurpose {
    Header(String),
    Proxy,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum RequestSecret {
    Header(Zeroizing<String>),
    Proxy {
        username: Zeroizing<String>,
        password: Zeroizing<String>,
    },
}

impl RequestSecret {
    pub(crate) fn secrets(&self) -> Vec<&str> {
        match self {
            Self::Header(value) => vec![value.as_str()],
            Self::Proxy { username, password } => vec![username.as_str(), password.as_str()],
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct BoundSecret {
    reference: AiSecretRef,
    endpoint: String,
    protocol: AiApiStyle,
    proxy_url: Option<String>,
    value: RequestSecret,
    // Derived representations are process secrets too. Keep them with their
    // owner so replacing/clearing any profile also releases the redaction cache.
    derived: Vec<Zeroizing<String>>,
}

impl BoundSecret {
    fn secrets(&self) -> impl Iterator<Item = &str> {
        self.value
            .secrets()
            .into_iter()
            .chain(self.derived.iter().map(|value| value.as_str()))
    }
}

/// Process-only credentials. No serialization or Debug implementation exists.
/// API authentication remains indexed by profile; additional secrets are bound
/// to profile, reference, protocol, endpoint and their exact header/proxy purpose.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct EphemeralCredentials {
    authentication: BTreeMap<Uuid, Zeroizing<String>>,
    requests: BTreeMap<(Uuid, SecretPurpose), BoundSecret>,
    // Retained editor values are known secrets even when their draft cannot be
    // bound to a valid delivery purpose. They never authorize header delivery.
    request_drafts: BTreeMap<Uuid, Vec<Zeroizing<String>>>,
}

impl EphemeralCredentials {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, id: &Uuid) -> Option<&Zeroizing<String>> {
        self.authentication.get(id)
    }
    pub fn insert(&mut self, id: Uuid, key: Zeroizing<String>) -> Option<Zeroizing<String>> {
        self.authentication.insert(id, key)
    }
    pub fn remove(&mut self, id: &Uuid) -> Option<Zeroizing<String>> {
        self.authentication.remove(id)
    }
    #[cfg(test)]
    pub fn contains_key(&self, id: &Uuid) -> bool {
        self.authentication.contains_key(id)
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.all_secrets().len()
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.authentication.is_empty() && self.requests.is_empty() && self.request_drafts.is_empty()
    }
    pub(crate) fn all_secrets(&self) -> Vec<&str> {
        self.authentication
            .values()
            .map(|v| v.as_str())
            .chain(self.requests.values().flat_map(BoundSecret::secrets))
            .chain(self.request_drafts.values().flatten().map(|v| v.as_str()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(crate) fn clear_requests(&mut self, profile: Uuid) {
        self.requests.retain(|(id, _), _| *id != profile);
        self.request_drafts.remove(&profile);
    }

    pub(crate) fn retain_request_drafts(&mut self, profile: Uuid, values: Vec<Zeroizing<String>>) {
        if values.is_empty() {
            self.request_drafts.remove(&profile);
        } else {
            self.request_drafts.insert(profile, values);
        }
    }
    pub(crate) fn remove_request(&mut self, profile: Uuid, purpose: &SecretPurpose) {
        self.requests.remove(&(profile, purpose.clone()));
    }

    pub(crate) fn insert_request(
        &mut self,
        profile: &NamedAiProfile,
        purpose: SecretPurpose,
        reference: AiSecretRef,
        value: RequestSecret,
    ) {
        if !matches!(
            (&purpose, &value),
            (SecretPurpose::Header(_), RequestSecret::Header(_))
                | (SecretPurpose::Proxy, RequestSecret::Proxy { .. })
        ) || profile.backend != AiBackend::Api
            || reference_for(profile, &purpose) != Some(&reference)
            || value.secrets().iter().any(|secret| secret.is_empty())
        {
            self.remove_request(profile.id, &purpose);
            return;
        }
        let proxy_url = match (&purpose, &profile.proxy) {
            (SecretPurpose::Proxy, AiProxy::Explicit { url, .. }) => Some(url.clone()),
            _ => None,
        };
        let derived = match &value {
            RequestSecret::Header(_) => Vec::new(),
            RequestSecret::Proxy { username, password } => {
                let pair = Zeroizing::new(format!("{}:{}", username.as_str(), password.as_str()));
                let bare = Zeroizing::new(STANDARD.encode(pair.as_bytes()));
                let prefixed = Zeroizing::new(format!("Basic {}", bare.as_str()));
                vec![bare, prefixed]
            }
        };
        self.requests.insert(
            (profile.id, purpose),
            BoundSecret {
                reference,
                endpoint: profile.endpoint.clone(),
                protocol: profile.api_style,
                proxy_url,
                value,
                derived,
            },
        );
    }

    pub(crate) fn request(
        &self,
        profile: &NamedAiProfile,
        purpose: &SecretPurpose,
        reference: &AiSecretRef,
    ) -> Option<&RequestSecret> {
        if profile.backend != AiBackend::Api {
            return None;
        }
        let bound = self.requests.get(&(profile.id, purpose.clone()))?;
        let proxy_url = match (purpose, &profile.proxy) {
            (SecretPurpose::Proxy, AiProxy::Explicit { url, .. }) => Some(url.as_str()),
            _ => None,
        };
        (bound.reference == *reference
            && bound.endpoint == profile.endpoint
            && bound.protocol == profile.api_style
            && bound.proxy_url.as_deref() == proxy_url)
            .then_some(&bound.value)
    }
}

/// Validate the exact metadata snapshot before Apply and before disk dispatch.
/// Only editable/provider-supplied text is checked; generated UUIDs, enum tags
/// and numeric limits cannot become credential disclosure candidates.
pub(crate) fn validate_catalog_metadata(
    catalog: &AiProfileCatalog,
    credentials: &EphemeralCredentials,
) -> Result<(), AiError> {
    catalog
        .validate()
        .map_err(|_| AiError::InvalidRequestOptions)?;
    let guard = RequestOptions::default().with_context_secrets(&credentials.all_secrets())?;
    let reference = |reference: &AiSecretRef| match reference {
        AiSecretRef::Environment { name } => guard.validate_metadata_text(name),
        AiSecretRef::Ephemeral { .. } | AiSecretRef::SecretStore { .. } => Ok(()),
    };
    for profile in &catalog.profiles {
        for text in [&profile.name, &profile.endpoint, &profile.model] {
            guard.validate_metadata_text(text)?;
        }
        if let AiBackend::LocalAgent { executable, .. } = &profile.backend {
            guard.validate_metadata_text(executable)?;
        }
        match &profile.authentication {
            AiAuthentication::None => {}
            AiAuthentication::Bearer { credential } => {
                if let Some(value) = credential {
                    reference(value)?;
                }
            }
            AiAuthentication::Header { name, credential } => {
                // Core admits this fixed protocol name without regard to ASCII
                // case. Its public spelling must have the same save semantics.
                if !name.eq_ignore_ascii_case("x-api-key") {
                    guard.validate_metadata_header_name(name)?;
                }
                if let Some(value) = credential {
                    reference(value)?;
                }
            }
        }
        for header in &profile.custom_headers {
            guard.validate_metadata_header_name(&header.name)?;
            reference(&header.value_ref)?;
        }
        if let AiProxy::Explicit { url, credentials } = &profile.proxy {
            guard.validate_metadata_text(url)?;
            if let Some(value) = credentials {
                reference(value)?;
            }
        }
        for (model, settings) in &profile.reasoning_by_model {
            guard.validate_metadata_text(model)?;
            if let AiReasoningCapability::Effort { values } = &settings.capability {
                for value in values {
                    guard.validate_metadata_text(value)?;
                }
            }
            if let AiReasoningSelection::Effort(value) | AiReasoningSelection::Text(value) =
                &settings.selection
            {
                guard.validate_metadata_text(value)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn reference_for<'a>(
    profile: &'a NamedAiProfile,
    purpose: &SecretPurpose,
) -> Option<&'a AiSecretRef> {
    match purpose {
        SecretPurpose::Header(name) => profile
            .custom_headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| &h.value_ref),
        SecretPurpose::Proxy => match &profile.proxy {
            AiProxy::Explicit { credentials, .. } => credentials.as_ref(),
            AiProxy::Direct => None,
        },
    }
}

fn environment(name: &str) -> Result<Zeroizing<String>, AiError> {
    // Only an explicitly selected reference is read at the user's request action.
    // Missing/non-Unicode/oversize values fail closed; diagnostics omit its name/value.
    let value = std::env::var(name).map_err(|_| AiError::InvalidRequestOptions)?;
    if value.is_empty() || value.len() > 8192 || value.chars().any(char::is_control) {
        return Err(AiError::InvalidRequestOptions);
    }
    Ok(Zeroizing::new(value))
}

pub(crate) fn resolve_authentication(
    profile: &NamedAiProfile,
    credentials: &EphemeralCredentials,
) -> Result<Option<Zeroizing<String>>, AiError> {
    resolve_authentication_with_environment(profile, credentials, environment)
}

fn resolve_authentication_with_environment(
    profile: &NamedAiProfile,
    credentials: &EphemeralCredentials,
    mut lookup: impl FnMut(&str) -> Result<Zeroizing<String>, AiError>,
) -> Result<Option<Zeroizing<String>>, AiError> {
    let reference = match &profile.authentication {
        AiAuthentication::None => return Ok(None),
        AiAuthentication::Bearer { credential } | AiAuthentication::Header { credential, .. } => {
            credential
        }
    };
    let key = match reference {
        Some(AiSecretRef::Environment { name }) => lookup(name)?,
        _ => credentials
            .get(&profile.id)
            .cloned()
            .ok_or(AiError::InvalidApiKey)?,
    };
    if key.is_empty() || key.chars().any(char::is_control) || key.len() > 8192 {
        return Err(AiError::InvalidApiKey);
    }
    Ok(Some(key))
}

pub(crate) fn resolve_options(
    profile: &NamedAiProfile,
    credentials: &EphemeralCredentials,
) -> Result<RequestOptions, AiError> {
    resolve_options_with_environment(profile, credentials, environment)
}

fn resolve_options_with_environment(
    profile: &NamedAiProfile,
    credentials: &EphemeralCredentials,
    mut lookup: impl FnMut(&str) -> Result<Zeroizing<String>, AiError>,
) -> Result<RequestOptions, AiError> {
    if profile.backend != AiBackend::Api {
        return Err(AiError::InvalidRequestOptions);
    }
    let mut headers = Vec::new();
    for header in &profile.custom_headers {
        let purpose = SecretPurpose::Header(header.name.to_ascii_lowercase());
        let value = match &header.value_ref {
            AiSecretRef::Environment { name } => lookup(name)?,
            reference => match credentials.request(profile, &purpose, reference) {
                Some(RequestSecret::Header(value)) => value.clone(),
                _ => return Err(AiError::InvalidRequestOptions),
            },
        };
        headers.push((header.name.clone(), value));
    }
    let route = match &profile.proxy {
        AiProxy::Direct => ProxyRoute::Direct,
        AiProxy::Explicit {
            url,
            credentials: reference,
        } => {
            let authentication = match reference {
                None => None,
                Some(AiSecretRef::Environment { name }) => {
                    // A JSON object avoids ambiguous username:password splitting.
                    #[derive(serde::Deserialize)]
                    #[serde(deny_unknown_fields)]
                    struct Pair {
                        username: String,
                        password: String,
                    }
                    impl Drop for Pair {
                        fn drop(&mut self) {
                            self.username.zeroize();
                            self.password.zeroize();
                        }
                    }
                    let raw = lookup(name)?;
                    let mut pair: Pair =
                        serde_json::from_str(&raw).map_err(|_| AiError::InvalidRequestOptions)?;
                    Some(ProxyCredentials::new(
                        Zeroizing::new(std::mem::take(&mut pair.username)),
                        Zeroizing::new(std::mem::take(&mut pair.password)),
                    )?)
                }
                Some(reference) => {
                    match credentials.request(profile, &SecretPurpose::Proxy, reference) {
                        Some(RequestSecret::Proxy { username, password }) => {
                            Some(ProxyCredentials::new(username.clone(), password.clone())?)
                        }
                        _ => return Err(AiError::InvalidRequestOptions),
                    }
                }
            };
            ProxyRoute::explicit(url, authentication)?
        }
    };
    RequestOptions::new(headers, route)?.with_context_secrets(&credentials.all_secrets())
}

#[cfg(test)]
mod tests {
    use super::*;
    use keelshell_core::{AiCustomHeader, AiPreset};

    fn profile() -> NamedAiProfile {
        let mut p = NamedAiProfile::draft(AiPreset::OpenAi);
        p.name = "Fixture".into();
        p.model = "model".into();
        p
    }

    #[test]
    fn save_metadata_preserves_equivalent_fixed_authentication_header_names() {
        for name in ["x-api-key", "X-Api-Key", "X-API-KEY"] {
            let mut profile = NamedAiProfile::draft(AiPreset::Claude);
            profile.name = "Fixture".into();
            profile.model = "model".into();
            profile.authentication = AiAuthentication::Header {
                name: name.into(),
                credential: None,
            };
            let mut credentials = EphemeralCredentials::new();
            credentials.insert(profile.id, Zeroizing::new("X-API-KEY".into()));
            let catalog = AiProfileCatalog {
                active_id: Some(profile.id),
                profiles: vec![profile],
            };
            assert!(catalog.validate().is_ok(), "legal fixed name {name}");
            assert!(validate_catalog_metadata(&catalog, &credentials).is_ok());
        }
    }

    #[test]
    fn save_metadata_fixed_authentication_name_does_not_exempt_custom_headers() {
        for name in ["x-extra-x-api-key", "X-Extra-X-API-KEY"] {
            let mut profile = NamedAiProfile::draft(AiPreset::Claude);
            profile.name = "Fixture".into();
            profile.model = "model".into();
            profile.custom_headers = vec![AiCustomHeader {
                name: name.into(),
                value_ref: AiSecretRef::Ephemeral { id: Uuid::new_v4() },
            }];
            let mut credentials = EphemeralCredentials::new();
            credentials.insert(profile.id, Zeroizing::new("X-API-KEY".into()));
            let catalog = AiProfileCatalog {
                active_id: Some(profile.id),
                profiles: vec![profile],
            };
            assert!(catalog.validate().is_ok(), "legal custom name {name}");
            assert!(matches!(
                validate_catalog_metadata(&catalog, &credentials),
                Err(AiError::CredentialInContext)
            ));
        }
    }

    #[test]
    fn save_metadata_rejects_known_values_in_editable_fields_and_references() {
        const SECRET: &str = "metadata_guard_secret";
        let mut credentials = EphemeralCredentials::new();
        let owner = Uuid::new_v4();
        credentials.retain_request_drafts(owner, vec![Zeroizing::new(SECRET.into())]);
        for field in 0..9 {
            let mut profile = profile();
            match field {
                0 => profile.name = SECRET.into(),
                1 => profile.model = SECRET.into(),
                2 => {
                    profile.endpoint = format!("https://provider.example/{SECRET}/chat/completions")
                }
                3 => {
                    profile.endpoint =
                        "https://provider.example/%6detadata_guard_secret/chat/completions".into()
                }
                4 => {
                    profile.proxy = AiProxy::Explicit {
                        url: format!("http://{SECRET}.example"),
                        credentials: None,
                    }
                }
                5 => profile.custom_headers.push(AiCustomHeader {
                    name: SECRET.to_ascii_uppercase(),
                    value_ref: AiSecretRef::Ephemeral { id: Uuid::new_v4() },
                }),
                6 => {
                    profile.authentication = AiAuthentication::Bearer {
                        credential: Some(AiSecretRef::Environment {
                            name: SECRET.into(),
                        }),
                    }
                }
                7 => {
                    profile.backend = AiBackend::LocalAgent {
                        agent: keelshell_core::AiLocalAgent::Codex,
                        executable: format!("/owned/{SECRET}/codex"),
                        limits: Default::default(),
                    }
                }
                _ => {
                    profile
                        .reasoning_by_model
                        .insert(SECRET.into(), Default::default());
                }
            }
            let catalog = AiProfileCatalog {
                active_id: Some(profile.id),
                profiles: vec![profile],
            };
            assert!(catalog.validate().is_ok(), "fixture metadata {field}");
            assert!(
                matches!(
                    validate_catalog_metadata(&catalog, &credentials),
                    Err(AiError::CredentialInContext)
                ),
                "metadata {field}"
            );
        }
        credentials.clear_requests(owner);
        let mut released = profile();
        released.model = SECRET.into();
        let catalog = AiProfileCatalog {
            active_id: Some(released.id),
            profiles: vec![released],
        };
        assert!(validate_catalog_metadata(&catalog, &credentials).is_ok());
    }

    #[test]
    fn save_metadata_guard_is_bounded_without_discarding_retained_values() {
        let profile = profile();
        let catalog = AiProfileCatalog {
            active_id: Some(profile.id),
            profiles: vec![profile],
        };
        let mut credentials = EphemeralCredentials::new();
        credentials.retain_request_drafts(
            Uuid::new_v4(),
            vec![Zeroizing::new("x".repeat(1024 * 1024 + 1))],
        );
        assert!(matches!(
            validate_catalog_metadata(&catalog, &credentials),
            Err(AiError::ContextTooLarge)
        ));
        assert_eq!(credentials.all_secrets()[0].len(), 1024 * 1024 + 1);
    }

    #[test]
    fn retained_drafts_only_guard_context_and_follow_replacement_and_owner_clear()
    -> Result<(), AiError> {
        let active = profile();
        let owner = Uuid::new_v4();
        let mut credentials = EphemeralCredentials::new();
        credentials.retain_request_drafts(
            owner,
            vec![
                Zeroizing::new("retained-draft-secret".into()),
                Zeroizing::new("retained-draft-secret".into()),
            ],
        );
        assert!(!credentials.is_empty());
        assert_eq!(credentials.len(), 1);
        let options = resolve_options(&active, &credentials)?;
        assert!(options.header_names().next().is_none());
        assert!(options.proxy_url().is_none());
        assert!(
            !options
                .redact_for_review("retained-draft-secret")
                .contains("retained-draft-secret")
        );
        credentials.retain_request_drafts(owner, vec![Zeroizing::new("replacement-draft".into())]);
        assert!(!credentials.all_secrets().contains(&"retained-draft-secret"));
        assert!(credentials.all_secrets().contains(&"replacement-draft"));
        assert!(
            !options
                .redact_for_review("retained-draft-secret")
                .contains("retained-draft-secret")
        );
        credentials.clear_requests(owner);
        assert!(credentials.is_empty());
        Ok(())
    }

    #[test]
    fn retained_draft_snapshots_fail_closed_at_count_value_and_total_bounds() {
        let active = profile();
        let owner = Uuid::new_v4();
        let cases = [
            (0..4097)
                .map(|index| Zeroizing::new(format!("distinct-draft-{index}")))
                .collect(),
            vec![Zeroizing::new("x".repeat(1024 * 1024 + 1))],
            (0..9)
                .map(|index| Zeroizing::new(format!("{index}{}", "x".repeat(1024 * 1024 - 1))))
                .collect(),
        ];
        for values in cases {
            let mut credentials = EphemeralCredentials::new();
            credentials.retain_request_drafts(owner, values);
            assert!(matches!(
                resolve_options(&active, &credentials),
                Err(AiError::ContextTooLarge)
            ));
        }
    }

    #[test]
    fn retained_draft_snapshots_accept_exact_unique_count_value_and_total_bounds()
    -> Result<(), AiError> {
        let active = profile();
        let owner = Uuid::new_v4();
        let cases = [
            (0..4096)
                .map(|index| Zeroizing::new(format!("distinct-draft-{index}")))
                .collect(),
            vec![Zeroizing::new("x".repeat(1024 * 1024))],
            (0..8)
                .map(|index| Zeroizing::new(format!("{index}{}", "x".repeat(1024 * 1024 - 1))))
                .collect(),
        ];
        for values in cases {
            let mut credentials = EphemeralCredentials::new();
            credentials.retain_request_drafts(owner, values);
            let options = resolve_options(&active, &credentials)?;
            assert!(options.header_names().next().is_none());
        }
        Ok(())
    }

    #[test]
    fn request_options_environment_references_are_explicit_strict_and_frozen() -> Result<(), AiError>
    {
        let mut p = profile();
        p.custom_headers = vec![AiCustomHeader {
            name: "x-project".into(),
            value_ref: AiSecretRef::Environment {
                name: "FIXTURE_HEADER".into(),
            },
        }];
        p.proxy = AiProxy::Explicit {
            url: "http://localhost:8888".into(),
            credentials: Some(AiSecretRef::Environment {
                name: "FIXTURE_PROXY".into(),
            }),
        };
        let mut names = Vec::new();
        let options = resolve_options_with_environment(&p, &EphemeralCredentials::new(), |name| {
            names.push(name.to_owned());
            Ok(Zeroizing::new(
                match name {
                    "FIXTURE_HEADER" => "synthetic-header",
                    "FIXTURE_PROXY" => {
                        r#"{"username":"synthetic-user","password":"synthetic-password"}"#
                    }
                    _ => return Err(AiError::InvalidRequestOptions),
                }
                .into(),
            ))
        })?;
        assert_eq!(names, ["FIXTURE_HEADER", "FIXTURE_PROXY"]);
        let provider =
            keelshell_ai::ProviderConfig::new(&p.endpoint, &p.model)?.with_request_options(options);
        let review =
            keelshell_ai::ContextDraft::new("synthetic-header synthetic-user synthetic-password")
                .prepare(&provider, &[], 4096)?;
        for value in ["synthetic-header", "synthetic-user", "synthetic-password"] {
            assert!(!review.preview_json().contains(value));
            assert!(!format!("{review:?}").contains(value));
        }
        assert!(
            resolve_options_with_environment(&p, &EphemeralCredentials::new(), |_| Err(
                AiError::InvalidRequestOptions
            ))
            .is_err()
        );
        for value in [
            r#"{"username":"user","password":"pass","extra":"secret"}"#,
            r#"{"username":"user"}"#,
            "user:password",
        ] {
            assert!(
                resolve_options_with_environment(&p, &EphemeralCredentials::new(), |name| Ok(
                    Zeroizing::new(
                        if name == "FIXTURE_HEADER" {
                            "synthetic-header"
                        } else {
                            value
                        }
                        .into()
                    )
                ))
                .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn request_options_auth_environment_resolution_is_explicit_bounded_and_frozen()
    -> Result<(), AiError> {
        let mut p = profile();
        p.authentication = AiAuthentication::Bearer {
            credential: Some(AiSecretRef::Environment {
                name: "FIXTURE_AUTH".into(),
            }),
        };
        let credentials = EphemeralCredentials::new();
        let first = resolve_authentication_with_environment(&p, &credentials, |name| {
            assert_eq!(name, "FIXTURE_AUTH");
            Ok(Zeroizing::new("synthetic-first".into()))
        })?
        .unwrap_or_else(|| panic!("resolved key"));
        let second = resolve_authentication_with_environment(&p, &credentials, |_| {
            Ok(Zeroizing::new("synthetic-next".into()))
        })?;
        assert_eq!(first.as_str(), "synthetic-first");
        assert!(second.is_some_and(|value| value.as_str() == "synthetic-next"));
        assert!(
            resolve_authentication_with_environment(&p, &credentials, |_| Err(
                AiError::InvalidRequestOptions
            ))
            .is_err()
        );
        for invalid in [String::new(), "synthetic\r\nvalue".into(), "x".repeat(8193)] {
            assert!(
                resolve_authentication_with_environment(&p, &credentials, |_| Ok(Zeroizing::new(
                    invalid.clone()
                )))
                .is_err()
            );
        }
        p.authentication = AiAuthentication::None;
        assert!(
            resolve_authentication_with_environment(&p, &credentials, |_| panic!(
                "no-auth must not resolve references"
            ))?
            .is_none()
        );
        Ok(())
    }

    #[test]
    fn inactive_proxy_derived_values_follow_owner_replacement_and_clear() -> Result<(), AiError> {
        let mut p = profile();
        let reference = AiSecretRef::Ephemeral { id: Uuid::new_v4() };
        p.proxy = AiProxy::Explicit {
            url: "http://localhost:8888".into(),
            credentials: Some(reference.clone()),
        };
        let mut credentials = EphemeralCredentials::new();
        credentials.insert_request(
            &p,
            SecretPurpose::Proxy,
            reference.clone(),
            RequestSecret::Proxy {
                username: Zeroizing::new("synthetic-user".into()),
                password: Zeroizing::new("synthetic-pass".into()),
            },
        );
        let basic = "c3ludGhldGljLXVzZXI6c3ludGhldGljLXBhc3M=";
        let prefixed = format!("Basic {basic}");
        assert!(credentials.all_secrets().contains(&basic));
        assert!(credentials.all_secrets().contains(&prefixed.as_str()));
        let metadata = serde_json::to_string(&p).map_err(|_| AiError::Serialization)?;
        for secret in ["synthetic-user", "synthetic-pass", basic, prefixed.as_str()] {
            assert!(!metadata.contains(secret));
        }
        let mut other = profile();
        let other_reference = AiSecretRef::Ephemeral { id: Uuid::new_v4() };
        other.custom_headers.push(AiCustomHeader {
            name: "x-keep".into(),
            value_ref: other_reference.clone(),
        });
        credentials.insert_request(
            &other,
            SecretPurpose::Header("x-keep".into()),
            other_reference,
            RequestSecret::Header(Zeroizing::new("unrelated-secret".into())),
        );
        credentials.insert_request(
            &p,
            SecretPurpose::Proxy,
            reference,
            RequestSecret::Proxy {
                username: Zeroizing::new("replacement-user".into()),
                password: Zeroizing::new("replacement-password".into()),
            },
        );
        assert!(!credentials.all_secrets().contains(&basic));
        assert!(!credentials.all_secrets().contains(&prefixed.as_str()));
        assert!(credentials.all_secrets().contains(&"unrelated-secret"));
        resolve_options(&p, &credentials)?;
        credentials.clear_requests(p.id);
        assert_eq!(credentials.all_secrets(), ["unrelated-secret"]);
        Ok(())
    }

    #[test]
    fn request_options_slots_refuse_other_reference_protocol_backend_and_proxy_route()
    -> Result<(), AiError> {
        let mut p = profile();
        let reference = AiSecretRef::Ephemeral { id: Uuid::new_v4() };
        let purpose = SecretPurpose::Header("x-project".into());
        p.custom_headers = vec![AiCustomHeader {
            name: "x-project".into(),
            value_ref: reference.clone(),
        }];
        let mut credentials = EphemeralCredentials::new();
        credentials.insert_request(
            &p,
            purpose.clone(),
            reference.clone(),
            RequestSecret::Header(Zeroizing::new("synthetic-header".into())),
        );
        credentials.insert_request(
            &p,
            purpose.clone(),
            reference.clone(),
            RequestSecret::Proxy {
                username: Zeroizing::new("user".into()),
                password: Zeroizing::new("password".into()),
            },
        );
        assert!(resolve_options(&p, &credentials).is_err());
        credentials.insert_request(
            &p,
            purpose.clone(),
            reference.clone(),
            RequestSecret::Header(Zeroizing::new("synthetic-header".into())),
        );
        resolve_options(&p, &credentials)?;
        for changed in 0..5 {
            let mut next = p.clone();
            match changed {
                0 => next.id = Uuid::new_v4(),
                1 => next.endpoint = "https://other.invalid/v1/chat/completions".into(),
                2 => next.api_style = AiApiStyle::Responses,
                3 => {
                    next.custom_headers[0].value_ref = AiSecretRef::Ephemeral { id: Uuid::new_v4() }
                }
                _ => {
                    next.backend = AiBackend::LocalAgent {
                        agent: keelshell_core::AiLocalAgent::Codex,
                        executable: "/synthetic/codex".into(),
                        limits: Default::default(),
                    }
                }
            };
            assert!(resolve_options(&next, &credentials).is_err());
        }
        p.proxy = AiProxy::Explicit {
            url: "http://localhost:8888".into(),
            credentials: Some(reference.clone()),
        };
        credentials.insert_request(
            &p,
            SecretPurpose::Proxy,
            reference.clone(),
            RequestSecret::Proxy {
                username: Zeroizing::new("user".into()),
                password: Zeroizing::new("password".into()),
            },
        );
        resolve_options(&p, &credentials)?;
        p.proxy = AiProxy::Explicit {
            url: "socks5://localhost:8888".into(),
            credentials: Some(reference),
        };
        assert!(resolve_options(&p, &credentials).is_err());
        Ok(())
    }
}
