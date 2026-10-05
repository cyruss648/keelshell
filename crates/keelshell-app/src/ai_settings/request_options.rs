//! Per-profile request editors retain invalid text and never serialize values.

use std::collections::BTreeSet;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::{component::input::InputState, *};
use keelshell_core::{AiBackend, AiCustomHeader, AiProxy, AiSecretRef};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::AiSettingsPanel;
use crate::ai_request_options::{RequestSecret, SecretPurpose, reference_for};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    Temporary,
    Environment,
    Vault,
}

#[derive(Clone, PartialEq, Eq)]
struct HeaderValues {
    name: String,
    source: Source,
    reference: String,
    value: Zeroizing<String>,
    id: Uuid,
}

#[derive(Clone, PartialEq, Eq)]
struct Values {
    headers: Vec<HeaderValues>,
    proxy: bool,
    proxy_url: String,
    proxy_auth: bool,
    proxy_source: Source,
    proxy_reference: String,
    username: Zeroizing<String>,
    password: Zeroizing<String>,
    proxy_id: Uuid,
}

pub(super) struct HeaderEditor {
    name: Entity<InputState>,
    reference: Entity<InputState>,
    value: Entity<InputState>,
    source: Source,
    id: Uuid,
}

pub(super) struct RequestEditor {
    headers: Vec<HeaderEditor>,
    proxy: bool,
    proxy_url: Entity<InputState>,
    proxy_auth: bool,
    proxy_source: Source,
    proxy_reference: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    proxy_id: Uuid,
    values: Values,
    pending_headers: BTreeSet<Uuid>,
    pending_proxy: bool,
}

fn source(reference: &AiSecretRef) -> (Source, String, Uuid) {
    match reference {
        AiSecretRef::Environment { name } => (Source::Environment, name.clone(), Uuid::new_v4()),
        AiSecretRef::SecretStore { id } => (Source::Vault, id.to_string(), *id),
        AiSecretRef::Ephemeral { id } => (Source::Temporary, String::new(), *id),
    }
}

fn reference(kind: Source, value: &str, id: Uuid) -> Result<AiSecretRef, ()> {
    let reference = match kind {
        Source::Temporary => AiSecretRef::Ephemeral { id },
        Source::Environment => AiSecretRef::Environment { name: value.into() },
        Source::Vault => AiSecretRef::SecretStore {
            id: Uuid::parse_str(value).map_err(|_| ())?,
        },
    };
    reference.validate().map_err(|_| ())?;
    Ok(reference)
}

impl RequestEditor {
    fn read(&self, cx: &App) -> Values {
        Values {
            headers: self
                .headers
                .iter()
                .map(|h| HeaderValues {
                    name: h.name.read(cx).value().to_string(),
                    source: h.source,
                    reference: h.reference.read(cx).value().to_string(),
                    value: Zeroizing::new(h.value.read(cx).value().to_string()),
                    id: h.id,
                })
                .collect(),
            proxy: self.proxy,
            proxy_url: self.proxy_url.read(cx).value().to_string(),
            proxy_auth: self.proxy_auth,
            proxy_source: self.proxy_source,
            proxy_reference: self.proxy_reference.read(cx).value().to_string(),
            username: Zeroizing::new(self.username.read(cx).value().to_string()),
            password: Zeroizing::new(self.password.read(cx).value().to_string()),
            proxy_id: self.proxy_id,
        }
    }

    fn metadata(values: &Values) -> Result<(Vec<AiCustomHeader>, AiProxy), ()> {
        let headers = values
            .headers
            .iter()
            .map(|h| {
                Ok(AiCustomHeader {
                    name: h.name.clone(),
                    value_ref: reference(h.source, &h.reference, h.id)?,
                })
            })
            .collect::<Result<Vec<_>, ()>>()?;
        let proxy = if values.proxy {
            AiProxy::Explicit {
                url: values.proxy_url.clone(),
                credentials: if values.proxy_auth {
                    Some(reference(
                        values.proxy_source,
                        &values.proxy_reference,
                        values.proxy_id,
                    )?)
                } else {
                    None
                },
            }
        } else {
            AiProxy::Direct
        };
        Ok((headers, proxy))
    }
}

impl Values {
    fn retained_secrets(&self) -> Vec<Zeroizing<String>> {
        let mut secrets: Vec<_> = self
            .headers
            .iter()
            .map(|h| h.value.clone())
            .chain([self.username.clone(), self.password.clone()])
            .filter(|value| !value.is_empty())
            .collect();
        // The raw fields remain known even for an invalid or disabled proxy.
        // Keep bounded Basic representations known without granting delivery.
        if !self.username.is_empty()
            && !self.password.is_empty()
            && self.username.len() <= 255
            && self.password.len() <= 255
        {
            let pair = Zeroizing::new(format!(
                "{}:{}",
                self.username.as_str(),
                self.password.as_str()
            ));
            let basic = Zeroizing::new(STANDARD.encode(pair.as_bytes()));
            secrets.push(Zeroizing::new(format!("Basic {}", basic.as_str())));
            secrets.push(basic);
        }
        secrets.sort_unstable_by(|left, right| left.as_str().cmp(right.as_str()));
        secrets.dedup_by(|left, right| left.as_str() == right.as_str());
        secrets
    }
}

impl AiSettingsPanel {
    pub(super) fn load_request_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.profile().cloned() else {
            return;
        };
        if self.request_editors.contains_key(&profile.id) {
            return;
        }
        let headers = profile
            .custom_headers
            .iter()
            .map(|h| {
                let (source, reference, id) = source(&h.value_ref);
                let value = match self.credentials.request(
                    &profile,
                    &SecretPurpose::Header(h.name.to_ascii_lowercase()),
                    &h.value_ref,
                ) {
                    Some(RequestSecret::Header(value)) => value.clone(),
                    _ => Zeroizing::default(),
                };
                HeaderEditor {
                    name: self.request_field(&h.name, false, window, cx),
                    reference: self.request_field(&reference, false, window, cx),
                    value: self.request_field(&value, true, window, cx),
                    source,
                    id,
                }
            })
            .collect();
        let (proxy, proxy_url, proxy_ref) = match &profile.proxy {
            AiProxy::Direct => (false, String::new(), None),
            AiProxy::Explicit { url, credentials } => (true, url.clone(), credentials.as_ref()),
        };
        let (proxy_source, proxy_reference, proxy_id) =
            proxy_ref
                .map(source)
                .unwrap_or((Source::Temporary, String::new(), Uuid::new_v4()));
        let (username, password) = match proxy_ref
            .and_then(|r| self.credentials.request(&profile, &SecretPurpose::Proxy, r))
        {
            Some(RequestSecret::Proxy { username, password }) => {
                (username.clone(), password.clone())
            }
            _ => (Zeroizing::default(), Zeroizing::default()),
        };
        let mut editor = RequestEditor {
            headers,
            proxy,
            proxy_url: self.request_field(&proxy_url, false, window, cx),
            proxy_auth: proxy_ref.is_some(),
            proxy_source,
            proxy_reference: self.request_field(&proxy_reference, false, window, cx),
            username: self.request_field(&username, true, window, cx),
            password: self.request_field(&password, true, window, cx),
            proxy_id,
            values: Values {
                headers: vec![],
                proxy: false,
                proxy_url: String::new(),
                proxy_auth: false,
                proxy_source,
                proxy_reference: String::new(),
                username: Zeroizing::default(),
                password: Zeroizing::default(),
                proxy_id,
            },
            pending_headers: BTreeSet::new(),
            pending_proxy: false,
        };
        editor.values = editor.read(cx);
        self.request_editors.insert(profile.id, editor);
    }

    fn request_field(
        &mut self,
        value: &str,
        masked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let entity = cx.new(|cx| {
            let mut state = InputState::new(window, cx).masked(masked);
            state.set_value(value, window, cx);
            state
        });
        self._subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |panel, _, event, window, cx| {
                if matches!(event, super::InputEvent::Change) {
                    panel.sync_editor(cx);
                    panel.clear_pending_request_fields(window, cx);
                }
            },
        ));
        entity
    }

    pub(super) fn sync_request_editor(
        &mut self,
        destination_changed: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let Some(editor) = self.request_editors.get_mut(&id) else {
            return false;
        };
        let mut values = editor.read(cx);
        // Pending UI clears must never re-admit values from the old destination.
        for h in &mut values.headers {
            if editor.pending_headers.contains(&h.id)
                && let Some(old) = editor.values.headers.iter().find(|old| old.id == h.id)
            {
                h.value = old.value.clone();
                h.source = old.source;
                h.reference.clone_from(&old.reference);
            }
        }
        if editor.pending_proxy {
            values.username = editor.values.username.clone();
            values.password = editor.values.password.clone();
            values.proxy_source = editor.values.proxy_source;
            values
                .proxy_reference
                .clone_from(&editor.values.proxy_reference);
        }
        if values == editor.values && !destination_changed {
            return false;
        }
        if destination_changed {
            editor.pending_proxy = true;
            for h in &mut values.headers {
                h.value.clear();
                h.source = Source::Temporary;
                h.reference.clear();
                editor.pending_headers.insert(h.id);
            }
            values.username.clear();
            values.password.clear();
            values.proxy_source = Source::Temporary;
            values.proxy_reference.clear();
            self.credentials.clear_requests(id);
        }
        for h in &mut values.headers {
            if let Some(old) = editor.values.headers.iter().find(|old| old.id == h.id)
                && (old.name != h.name || old.source != h.source || old.reference != h.reference)
            {
                h.value.clear();
                editor.pending_headers.insert(h.id);
                if old.name != h.name {
                    h.source = Source::Temporary;
                    h.reference.clear();
                } else if old.source != h.source {
                    h.reference.clear();
                }
            }
        }
        if values.proxy != editor.values.proxy
            || values.proxy_url != editor.values.proxy_url
            || values.proxy_auth != editor.values.proxy_auth
            || values.proxy_source != editor.values.proxy_source
            || values.proxy_reference != editor.values.proxy_reference
        {
            values.username.clear();
            values.password.clear();
            editor.pending_proxy = true;
            if values.proxy != editor.values.proxy || values.proxy_url != editor.values.proxy_url {
                values.proxy_source = Source::Temporary;
                values.proxy_reference.clear();
            } else if values.proxy_source != editor.values.proxy_source {
                values.proxy_reference.clear();
            }
        }
        for h in &mut values.headers {
            if h.source == Source::Vault
                && editor
                    .values
                    .headers
                    .iter()
                    .find(|old| old.id == h.id)
                    .is_some_and(|old| old.value != h.value)
            {
                h.source = Source::Temporary;
                h.reference.clear();
                editor.pending_headers.insert(h.id);
            }
        }
        if values.proxy_source == Source::Vault
            && (values.username != editor.values.username
                || values.password != editor.values.password)
        {
            values.proxy_source = Source::Temporary;
            values.proxy_reference.clear();
            editor.pending_proxy = true;
        }
        self.credentials.clear_requests(id);
        if let Some(profile) = self.catalog.profiles.iter_mut().find(|p| p.id == id) {
            if let Ok((headers, proxy)) = RequestEditor::metadata(&values) {
                profile.custom_headers = headers;
                profile.proxy = proxy;
            }
            for h in &values.headers {
                if matches!(h.source, Source::Temporary | Source::Vault)
                    && !h.value.is_empty()
                    && let Ok(reference) = reference(h.source, &h.reference, h.id)
                {
                    self.credentials.insert_request(
                        profile,
                        SecretPurpose::Header(h.name.to_ascii_lowercase()),
                        reference,
                        RequestSecret::Header(h.value.clone()),
                    );
                }
            }
            if values.proxy
                && values.proxy_auth
                && matches!(values.proxy_source, Source::Temporary | Source::Vault)
                && !values.username.is_empty()
                && !values.password.is_empty()
                && let Ok(reference) = reference(
                    values.proxy_source,
                    &values.proxy_reference,
                    values.proxy_id,
                )
            {
                self.credentials.insert_request(
                    profile,
                    SecretPurpose::Proxy,
                    reference,
                    RequestSecret::Proxy {
                        username: values.username.clone(),
                        password: values.password.clone(),
                    },
                );
            }
        }
        // Purpose/reference validation controls delivery, never whether a
        // still-retained draft participates in disclosure guards elsewhere.
        self.credentials
            .retain_request_drafts(id, values.retained_secrets());
        editor.values = values;
        true
    }

    pub(super) fn clear_pending_request_fields(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for editor in self
            .request_editors
            .values_mut()
            .filter(|e| !e.pending_headers.is_empty() || e.pending_proxy)
        {
            for h in &mut editor.headers {
                if let Some(values) = editor.values.headers.iter().find(|v| v.id == h.id) {
                    h.source = values.source;
                    h.reference
                        .update(cx, |f, cx| f.set_value(&values.reference, window, cx));
                    h.value
                        .update(cx, |f, cx| f.set_value(values.value.as_str(), window, cx));
                }
            }
            editor.proxy_source = editor.values.proxy_source;
            editor.proxy_reference.update(cx, |f, cx| {
                f.set_value(&editor.values.proxy_reference, window, cx)
            });
            editor.username.update(cx, |f, cx| {
                f.set_value(editor.values.username.as_str(), window, cx)
            });
            editor.password.update(cx, |f, cx| {
                f.set_value(editor.values.password.as_str(), window, cx)
            });
            editor.pending_headers.clear();
            editor.pending_proxy = false;
        }
    }

    pub(super) fn request_draft_valid(&self, id: Uuid) -> bool {
        let Some(profile) = self.catalog.profiles.iter().find(|p| p.id == id) else {
            return false;
        };
        if profile.backend != AiBackend::Api {
            return true;
        }
        self.request_editors.get(&id).is_none_or(|e| {
            RequestEditor::metadata(&e.values).is_ok_and(|(headers, proxy)| {
                let mut profile = profile.clone();
                profile.custom_headers = headers;
                profile.proxy = proxy;
                if profile.name.is_empty() {
                    profile.name = "Options validation".into();
                }
                if profile.model.is_empty() {
                    profile.model = "options-validation".into();
                }
                if profile.validate().is_err() {
                    return false;
                }
                let supplied_headers = e
                    .values
                    .headers
                    .iter()
                    .filter(|h| h.source != Source::Environment && !h.value.is_empty())
                    .map(|h| (h.name.clone(), h.value.clone()))
                    .collect();
                if keelshell_ai::RequestOptions::new(
                    supplied_headers,
                    keelshell_ai::ProxyRoute::Direct,
                )
                .is_err()
                {
                    return false;
                }
                if e.values.proxy
                    && e.values.proxy_auth
                    && e.values.proxy_source != Source::Environment
                    && (!e.values.username.is_empty() || !e.values.password.is_empty())
                    && keelshell_ai::ProxyCredentials::new(
                        e.values.username.clone(),
                        e.values.password.clone(),
                    )
                    .is_err()
                {
                    return false;
                }
                true
            })
        })
    }

    fn add_request_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        let Some(id) = self.selected else {
            return;
        };
        if self
            .request_editors
            .get(&id)
            .is_none_or(|e| e.headers.len() >= 32)
        {
            return;
        }
        let header = HeaderEditor {
            name: self.request_field("", false, window, cx),
            reference: self.request_field("", false, window, cx),
            value: self.request_field("", true, window, cx),
            source: Source::Temporary,
            id: Uuid::new_v4(),
        };
        if let Some(editor) = self.request_editors.get_mut(&id) {
            editor.headers.push(header);
        }
        self.sync_editor(cx);
    }

    pub(super) fn clear_request_secret(
        &mut self,
        purpose: &SecretPurpose,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.selected else {
            return;
        };
        self.credentials.remove_request(id, purpose);
        if let Some(editor) = self.request_editors.get_mut(&id) {
            match purpose {
                SecretPurpose::Header(name) => {
                    for h in &editor.headers {
                        if h.name.read(cx).value().eq_ignore_ascii_case(name) {
                            h.value.update(cx, |f, cx| f.set_value("", window, cx));
                        }
                    }
                }
                SecretPurpose::Proxy => {
                    editor
                        .username
                        .update(cx, |f, cx| f.set_value("", window, cx));
                    editor
                        .password
                        .update(cx, |f, cx| f.set_value("", window, cx));
                }
            }
        }
        self.sync_editor(cx);
        self.changed(false, cx);
    }

    pub(super) fn focus_request_secret(
        &self,
        purpose: &SecretPurpose,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor = self.selected.and_then(|id| self.request_editors.get(&id));
        let input = match purpose {
            SecretPurpose::Header(name) => editor
                .and_then(|editor| {
                    editor
                        .headers
                        .iter()
                        .find(|header| header.name.read(cx).value().eq_ignore_ascii_case(name))
                })
                .map(|header| &header.value),
            SecretPurpose::Proxy => editor.map(|editor| &editor.password),
        };
        if let Some(input) = input {
            input.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
    }

    pub(super) fn apply_request_vault_result(
        &mut self,
        purpose: &SecretPurpose,
        reference: Option<Uuid>,
        value: Option<RequestSecret>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.selected else {
            return;
        };
        if let Some(editor) = self.request_editors.get_mut(&id) {
            match purpose {
                SecretPurpose::Header(name) => {
                    if let Some(h) = editor
                        .headers
                        .iter_mut()
                        .find(|h| h.name.read(cx).value().eq_ignore_ascii_case(name))
                    {
                        if let Some(reference) = reference {
                            h.source = Source::Vault;
                            h.reference
                                .update(cx, |f, cx| f.set_value(reference.to_string(), window, cx));
                        }
                        if let Some(RequestSecret::Header(value)) = &value {
                            h.value
                                .update(cx, |f, cx| f.set_value(value.as_str(), window, cx));
                        }
                    }
                }
                SecretPurpose::Proxy => {
                    if let Some(reference) = reference {
                        editor.proxy_source = Source::Vault;
                        editor
                            .proxy_reference
                            .update(cx, |f, cx| f.set_value(reference.to_string(), window, cx));
                    }
                    if let Some(RequestSecret::Proxy { username, password }) = &value {
                        editor
                            .username
                            .update(cx, |f, cx| f.set_value(username.as_str(), window, cx));
                        editor
                            .password
                            .update(cx, |f, cx| f.set_value(password.as_str(), window, cx));
                    }
                }
            } // Treat a verified vault result as the new editor signature so ordinary source-change clearing cannot discard it.
            editor.values = editor.read(cx);
            self.credentials
                .retain_request_drafts(id, editor.values.retained_secrets());
            if let Ok((headers, proxy)) = RequestEditor::metadata(&editor.values)
                && let Some(profile) = self.catalog.profiles.iter_mut().find(|p| p.id == id)
            {
                profile.custom_headers = headers;
                profile.proxy = proxy;
                let value = value.or_else(|| match purpose {
                    SecretPurpose::Header(name) => editor
                        .values
                        .headers
                        .iter()
                        .find(|h| h.name.eq_ignore_ascii_case(name))
                        .map(|h| RequestSecret::Header(h.value.clone())),
                    SecretPurpose::Proxy => Some(RequestSecret::Proxy {
                        username: editor.values.username.clone(),
                        password: editor.values.password.clone(),
                    }),
                });
                if let Some(reference) = reference_for(profile, purpose).cloned()
                    && let Some(value) = value
                {
                    self.credentials
                        .insert_request(profile, purpose.clone(), reference, value);
                }
            }
        }
        self.changed(false, cx);
    }
}

mod view;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod draft_tests;
