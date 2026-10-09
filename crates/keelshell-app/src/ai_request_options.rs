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

#[derive(Clone, PartialEq, Eq)]
struct BoundLocalEnvironment {
    reference: AiSecretRef,
    endpoint: String,
    backend: AiBackend,
    protocol: AiApiStyle,
    value: Zeroizing<String>,
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
    // Every explicitly loaded local environment value remains a disclosure guard
    // until its profile is deleted; clearing delivery does not forget the value.
    local_environment_values: BTreeMap<Uuid, Vec<Zeroizing<String>>>,
    local_environment_keys: BTreeMap<Uuid, BoundLocalEnvironment>,
}

impl EphemeralCredentials {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, id: &Uuid) -> Option<&Zeroizing<String>> {
        self.authentication.get(id)
    }
    pub fn insert(&mut self, id: Uuid, key: Zeroizing<String>) -> Option<Zeroizing<String>> {
        self.local_environment_keys.remove(&id);
        self.authentication.insert(id, key)
    }
    pub fn remove(&mut self, id: &Uuid) -> Option<Zeroizing<String>> {
        self.local_environment_keys.remove(id);
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
        self.authentication.is_empty()
            && self.requests.is_empty()
            && self.request_drafts.is_empty()
            && self.local_environment_values.is_empty()
    }
    pub(crate) fn all_secrets(&self) -> Vec<&str> {
        self.authentication
            .values()
            .map(|v| v.as_str())
            .chain(self.requests.values().flat_map(BoundSecret::secrets))
            .chain(self.request_drafts.values().flatten().map(|v| v.as_str()))
            .chain(
                self.local_environment_values
                    .values()
                    .flatten()
                    .map(|v| v.as_str()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(crate) fn bind_local_environment(
        &mut self,
        profile: &NamedAiProfile,
        value: Zeroizing<String>,
    ) {
        let reference = match &profile.authentication {
            AiAuthentication::Bearer {
                credential: Some(reference @ AiSecretRef::Environment { .. }),
            }
            | AiAuthentication::Header {
                credential: Some(reference @ AiSecretRef::Environment { .. }),
                ..
            } => reference.clone(),
            _ => return,
        };
        if profile.backend == AiBackend::Api {
            return;
        }
        self.authentication.insert(profile.id, value.clone());
        self.local_environment_keys.insert(
            profile.id,
            BoundLocalEnvironment {
                reference,
                endpoint: profile.endpoint.clone(),
                backend: profile.backend.clone(),
                protocol: profile.api_style,
                value,
            },
        );
    }

    pub(crate) fn local_environment_key(
        &self,
        profile: &NamedAiProfile,
    ) -> Option<&Zeroizing<String>> {
        let bound = self.local_environment_keys.get(&profile.id)?;
        let reference = match &profile.authentication {
            AiAuthentication::Bearer { credential }
            | AiAuthentication::Header { credential, .. } => credential.as_ref()?,
            AiAuthentication::None => return None,
        };
        (profile.backend != AiBackend::Api
            && bound.reference == *reference
            && bound.endpoint == profile.endpoint
            && bound.protocol == profile.api_style
            && bound.backend.same_credential_destination(&profile.backend))
        .then_some(&bound.value)
    }

    pub(crate) fn retain_local_environment(&mut self, profile: Uuid, value: Zeroizing<String>) {
        let values = self.local_environment_values.entry(profile).or_default();
        if !values.contains(&value) {
            values.push(value);
        }
    }

    pub(crate) fn local_environment_capacity(&self, profile: Uuid) -> bool {
        self.local_environment_values
            .get(&profile)
            .is_none_or(|values| values.len() < 16)
    }

    pub(crate) fn clear_local_environment(&mut self, profile: Uuid) {
        self.local_environment_values.remove(&profile);
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
/// Editable text and selected inference numbers are checked in their persisted
/// and wire forms. Generated UUIDs, fixed schema tags and existing numeric limits
/// remain outside this admission boundary.
pub(crate) fn validate_catalog_metadata(
    catalog: &AiProfileCatalog,
    credentials: &EphemeralCredentials,
) -> Result<(), AiError> {
    catalog
        .validate()
        .map_err(|_| AiError::InvalidRequestOptions)?;
    validate_catalog_secrets(catalog, credentials)
}

/// Check every metadata value against all known and retained draft secrets.
/// Incomplete drafts are included; callers separately admit their structure and
/// transport. Capability probes may run before a model has been selected.
pub(crate) fn validate_catalog_secrets(
    catalog: &AiProfileCatalog,
    credentials: &EphemeralCredentials,
) -> Result<(), AiError> {
    let guard = RequestOptions::default().with_context_secrets(&credentials.all_secrets())?;
    let reference = |reference: &AiSecretRef| match reference {
        AiSecretRef::Environment { name } => guard.validate_metadata_text(name),
        AiSecretRef::Ephemeral { .. } | AiSecretRef::SecretStore { .. } => Ok(()),
    };
    for profile in &catalog.profiles {
        for text in [&profile.name, &profile.endpoint, &profile.model] {
            guard.validate_metadata_text(text)?;
        }
        if let AiBackend::LocalAgent {
            executable,
            working_directory,
            ..
        } = &profile.backend
        {
            guard.validate_metadata_text(executable)?;
            if let keelshell_core::AiLocalAgentWorkingDirectory::Selected { path } =
                working_directory
            {
                guard.validate_metadata_text(path)?;
            }
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
        for (model, sampling) in &profile.sampling_by_model {
            guard.validate_metadata_text(model)?;
            for value in [sampling.temperature, sampling.top_p].into_iter().flatten() {
                guard.validate_metadata_text(&value.millis().to_string())?;
                // The AI adapter emits JSON f64 numbers: explicit zero is 0.0,
                // whereas metadata stores integer thousandths. Check both exact
                // representations before either Apply or disk dispatch.
                let wire = serde_json::to_string(&(f64::from(value.millis()) / 1000.))
                    .map_err(|_| AiError::Serialization)?;
                guard.validate_metadata_text(&wire)?;
            }
        }
        for (model, settings) in &profile.reasoning_by_model {
            guard.validate_metadata_text(model)?;
            if let AiReasoningCapability::Effort { values } = &settings.capability {
                for value in values {
                    guard.validate_metadata_text(value)?;
                }
            }
            if let AiReasoningCapability::Messages { efforts, .. } = &settings.capability {
                for effort in efforts {
                    guard.validate_metadata_text(effort.as_str())?;
                }
            }
            if let AiReasoningSelection::Messages(options) = &settings.selection {
                if let Some(effort) = options.effort {
                    guard.validate_metadata_text(effort.as_str())?;
                }
                // This selected mode is a persisted value, not merely a fixed schema key.
                guard.validate_metadata_text(match options.thinking {
                    keelshell_core::AiMessagesThinking::ProviderDefault => "provider_default",
                    keelshell_core::AiMessagesThinking::Adaptive => "adaptive",
                    keelshell_core::AiMessagesThinking::Disabled => "disabled",
                    keelshell_core::AiMessagesThinking::LegacyBudget(_) => "legacy_budget",
                })?;
                if let keelshell_core::AiMessagesThinking::LegacyBudget(tokens) = options.thinking {
                    guard.validate_metadata_text(&tokens.to_string())?;
                }
            }
            if let AiReasoningSelection::Budget(tokens) = &settings.selection {
                // Legacy stored Messages choices use this same integer on wire.
                guard.validate_metadata_text(&tokens.to_string())?;
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

pub(crate) fn environment(name: &str) -> Result<Zeroizing<String>, AiError> {
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

/// Convert the selected model's admitted metadata into fixed protocol fields.
pub(crate) fn inference_options(
    profile: &NamedAiProfile,
) -> Result<keelshell_ai::InferenceOptions, AiError> {
    use keelshell_ai::{InferenceOptions, ReasoningEffort, ReasoningOption, SamplingOption};
    profile
        .validate_inference_transport()
        .map_err(|_| AiError::InvalidInferenceOptions)?;
    let reasoning = match profile
        .reasoning_by_model
        .get(&profile.model)
        .map(|r| &r.selection)
    {
        None | Some(AiReasoningSelection::ProviderDefault) => ReasoningOption::ProviderDefault,
        Some(AiReasoningSelection::Effort(v)) => {
            ReasoningOption::Effort(ReasoningEffort::parse(v)?)
        }
        Some(AiReasoningSelection::Thinking(v)) => ReasoningOption::Thinking(*v),
        Some(AiReasoningSelection::Budget(v)) => ReasoningOption::TokenBudget(*v),
        Some(AiReasoningSelection::Messages(options)) => ReasoningOption::Messages {
            effort: options
                .effort
                .map(|v| ReasoningEffort::parse(v.as_str()))
                .transpose()?,
            thinking: match options.thinking {
                keelshell_core::AiMessagesThinking::ProviderDefault => {
                    keelshell_ai::MessagesThinking::ProviderDefault
                }
                keelshell_core::AiMessagesThinking::Adaptive => {
                    keelshell_ai::MessagesThinking::Adaptive
                }
                keelshell_core::AiMessagesThinking::Disabled => {
                    keelshell_ai::MessagesThinking::Disabled
                }
                keelshell_core::AiMessagesThinking::LegacyBudget(n) => {
                    keelshell_ai::MessagesThinking::LegacyBudget(n)
                }
            },
        },
        Some(AiReasoningSelection::Text(_)) => return Err(AiError::InvalidInferenceOptions),
    };
    let sampling = match profile.sampling_by_model.get(&profile.model) {
        Some(s) if s.temperature.is_some() => {
            SamplingOption::Temperature(s.temperature.map_or(0, |v| v.millis()))
        }
        Some(s) if s.top_p.is_some() => SamplingOption::TopP(s.top_p.map_or(0, |v| v.millis())),
        _ => SamplingOption::ProviderDefault,
    };
    Ok(InferenceOptions {
        reasoning,
        sampling,
    })
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
                        working_directory: Default::default(),
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
    fn inference_numeric_metadata_guards_stored_and_exact_wire_values_on_inactive_models() {
        use keelshell_core::{AiModelSampling, AiSamplingValue};
        for (millis, secret) in [(125, "125"), (125, "0.125"), (0, "0.0"), (1000, "1.0")] {
            for temperature in [true, false] {
                let mut inactive = profile();
                inactive.name = "Inactive profile".into();
                inactive.sampling_by_model.insert(
                    "inactive-model".into(),
                    AiModelSampling {
                        declared_supported: true,
                        temperature: temperature.then(|| {
                            AiSamplingValue::from_millis(millis)
                                .unwrap_or_else(|error| panic!("sampling fixture: {error}"))
                        }),
                        top_p: (!temperature).then(|| {
                            AiSamplingValue::from_millis(millis)
                                .unwrap_or_else(|error| panic!("sampling fixture: {error}"))
                        }),
                    },
                );
                let active = profile();
                let mut credentials = EphemeralCredentials::new();
                credentials.insert(inactive.id, Zeroizing::new(secret.into()));
                let catalog = AiProfileCatalog {
                    active_id: Some(active.id),
                    profiles: vec![active, inactive],
                };
                assert!(catalog.validate().is_ok());
                assert!(
                    matches!(
                        validate_catalog_metadata(&catalog, &credentials),
                        Err(AiError::CredentialInContext)
                    ),
                    "selected sampling {millis}, wire/storage secret {secret}"
                );
            }
        }
    }

    #[test]
    fn inference_numeric_metadata_guards_selected_budgets_but_preserves_limit_exemptions() {
        use keelshell_core::{AiMessagesInference, AiMessagesThinking, AiModelReasoning};
        for legacy in [false, true] {
            let mut messages = NamedAiProfile::draft(AiPreset::Claude);
            messages.name = "Messages fixture".into();
            messages.model = "model".into();
            messages.max_output_tokens = Some(8192);
            messages.reasoning_by_model.insert(
                "model".into(),
                AiModelReasoning {
                    capability: if legacy {
                        AiReasoningCapability::TokenBudget {
                            min: 1024,
                            max: 999999,
                        }
                    } else {
                        AiReasoningCapability::Messages {
                            efforts: vec![],
                            adaptive: false,
                            disabled: false,
                            manual_budget: true,
                        }
                    },
                    selection: if legacy {
                        AiReasoningSelection::Budget(2048)
                    } else {
                        AiReasoningSelection::Messages(AiMessagesInference {
                            effort: None,
                            thinking: AiMessagesThinking::LegacyBudget(2048),
                        })
                    },
                },
            );
            let mut credentials = EphemeralCredentials::new();
            credentials.retain_request_drafts(Uuid::new_v4(), vec![Zeroizing::new("2048".into())]);
            let mut catalog = AiProfileCatalog {
                active_id: Some(messages.id),
                profiles: vec![messages],
            };
            assert!(catalog.validate().is_ok(), "legal selected manual budget");
            assert!(matches!(
                validate_catalog_metadata(&catalog, &credentials),
                Err(AiError::CredentialInContext)
            ));
            catalog.profiles[0]
                .reasoning_by_model
                .get_mut("model")
                .unwrap_or_else(|| panic!("reasoning fixture"))
                .selection = AiReasoningSelection::ProviderDefault;
            for fixed_limit in ["1024", "8192", "999999"] {
                credentials = EphemeralCredentials::new();
                credentials.retain_request_drafts(
                    Uuid::new_v4(),
                    vec![Zeroizing::new(fixed_limit.into())],
                );
                assert!(
                    validate_catalog_metadata(&catalog, &credentials).is_ok(),
                    "existing numeric limit {fixed_limit}"
                );
            }
        }
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
                        working_directory: Default::default(),
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
