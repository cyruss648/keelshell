use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AiProfileCatalog, BatchAuditRecord, ConnectionFolder, ConnectionProxy, DeletedConnection,
    Error, RecentConnection, ReconnectPolicy, ValidationError,
};

/// Current persisted state and connection-export schema. Unknown versions fail closed.
pub const SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_CONNECTIONS: usize = 10_000;

/// How to obtain authentication at connection time. No secret is serialized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthMethod {
    /// Use the user's SSH agent.
    #[default]
    Agent,
    /// Prompt for an ephemeral password or obtain it from an OS credential store.
    Password,
    /// Read the named private key at connection time; passphrases remain ephemeral.
    PrivateKey {
        /// A filesystem reference, never the private-key contents.
        path: PathBuf,
    },
}

// Serde's internally tagged unit variants accept unknown fields. Struct variants
// in the wire type enforce the promise that credentials cannot enter profiles.
impl<'de> Deserialize<'de> for AuthMethod {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
        enum WireAuth {
            Agent {},
            Password {},
            PrivateKey { path: PathBuf },
        }
        match WireAuth::deserialize(deserializer)? {
            WireAuth::Agent {} => Ok(Self::Agent),
            WireAuth::Password {} => Ok(Self::Password),
            WireAuth::PrivateKey { path } => Ok(Self::PrivateKey { path }),
        }
    }
}

/// A saved SSH endpoint with searchable metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    /// Stable identifier independent of display name.
    pub id: Uuid,
    /// User-facing display name.
    pub name: String,
    /// Optional group, such as `Production` or `Personal`.
    pub group: String,
    /// Hostname or IP literal; no URI scheme, userinfo or command-line options.
    pub host: String,
    /// SSH TCP port, in 1..=65535.
    pub port: u16,
    /// Remote account name.
    pub username: String,
    /// Authentication reference, without secret material.
    pub auth: AuthMethod,
    /// Optional reference into the separately encrypted credential vault.
    /// The reference is local metadata and never carries a password or key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<Uuid>,
    /// Optional saved SSH profile used as the immediate jump host. The complete
    /// route is resolved and validated against application state before use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_host: Option<Uuid>,
    /// Optional upstream proxy for this hop, reached through its SSH parent when
    /// present. Proxy passwords are supplied at runtime and never persisted here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ConnectionProxy>,
    /// Bounded preferences for reconnecting an established session. This is not
    /// part of endpoint identity and never stores authentication or runtime state.
    #[serde(default, skip_serializing_if = "ReconnectPolicy::is_manual")]
    pub reconnect: ReconnectPolicy,
    /// Searchable user-defined tags.
    pub tags: Vec<String>,
    /// Whether to display the profile in favorites.
    pub favorite: bool,
}

impl Connection {
    /// Create a profile with a fresh ID, port 22 and agent authentication.
    ///
    /// This trims the three text inputs. Call [`Self::validate`] before use.
    pub fn new(
        name: impl Into<String>,
        host: impl Into<String>,
        username: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into().trim().to_owned(),
            group: String::new(),
            host: host.into().trim().to_owned(),
            port: 22,
            username: username.into().trim().to_owned(),
            auth: AuthMethod::Agent,
            credential_ref: None,
            jump_host: None,
            proxy: None,
            reconnect: ReconnectPolicy::Manual,
            tags: Vec::new(),
            favorite: false,
        }
    }

    /// Validate metadata without touching the network or reading a private key.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_nil() {
            return Err(ValidationError::new("connection.id", "must not be nil"));
        }
        if self
            .jump_host
            .is_some_and(|id| id.is_nil() || id == self.id)
        {
            return Err(ValidationError::new(
                "connection.jump_host",
                "must be a non-nil profile other than this connection",
            ));
        }
        text("connection.name", &self.name, 120, false)?;
        text("connection.group", &self.group, 120, true)?;
        text("connection.host", &self.host, 253, false)?;
        if self.host.starts_with('-')
            || self
                .host
                .chars()
                .any(|c| c.is_whitespace() || "/\\@?#".contains(c))
            || (self.host.contains(':') && self.host.parse::<std::net::Ipv6Addr>().is_err())
            || self.host.contains(['[', ']'])
        {
            return Err(ValidationError::new(
                "connection.host",
                "must be a hostname or IP literal",
            ));
        }
        if self.port == 0 {
            return Err(ValidationError::new("connection.port", "must be nonzero"));
        }
        if let Some(proxy) = &self.proxy {
            proxy.validate()?;
            crate::proxy::validate_proxy_host("connection.host", &self.host)?;
        }
        text("connection.username", &self.username, 128, false)?;
        if self.username.starts_with('-')
            || self
                .username
                .chars()
                .any(|c| c.is_whitespace() || "/\\@".contains(c))
        {
            return Err(ValidationError::new(
                "connection.username",
                "contains unsupported characters",
            ));
        }
        if let AuthMethod::PrivateKey { path } = &self.auth {
            let Some(value) = path.to_str() else {
                return Err(ValidationError::new(
                    "connection.auth.path",
                    "must be valid Unicode",
                ));
            };
            text("connection.auth.path", value, 4096, false)?;
        }
        self.reconnect.validate()?;
        validate_tags(&self.tags)
    }

    /// Copy this profile with a new ID and display name, preserving connection options.
    pub fn duplicate(&self, name: impl Into<String>) -> Result<Self, ValidationError> {
        let result = Self {
            id: Uuid::new_v4(),
            name: name.into(),
            // Credential references are bound to the original profile UUID;
            // duplicate profiles must prompt or create a new vault entry.
            credential_ref: None,
            ..self.clone()
        };
        result.validate()?;
        Ok(result)
    }

    /// Match all whitespace-separated query terms across metadata, ignoring case.
    pub fn matches(&self, query: &str) -> bool {
        let fields = format!(
            "{} {} {} {} {} {}",
            self.name,
            self.group,
            self.host,
            self.username,
            self.port,
            self.tags.join(" ")
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|term| fields.contains(&term.to_lowercase()))
    }

    pub(crate) fn endpoint_key(&self) -> (String, u16, &str) {
        (normalized_host(&self.host), self.port, &self.username)
    }
}

/// A saved command template. Selecting one must not execute it automatically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snippet {
    /// Stable command-template identifier.
    pub id: Uuid,
    /// Short display name.
    pub name: String,
    /// Command text, possibly multiline. Users must avoid embedding secrets.
    pub command: String,
    /// Explicitly enable checked `{{name}}` parameters. Older snippets stay literal.
    #[serde(default)]
    pub parameterized: bool,
    /// Explanation shown before inserting the command.
    pub description: String,
    /// Searchable tags.
    pub tags: Vec<String>,
}

impl Snippet {
    /// Construct a snippet with no description or tags.
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            command: command.into(),
            parameterized: false,
            description: String::new(),
            tags: Vec::new(),
        }
    }

    /// Validate bounded text, allowing newlines and tabs only in the command body.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_nil() {
            return Err(ValidationError::new("snippet.id", "must not be nil"));
        }
        text("snippet.name", &self.name, 120, false)?;
        text("snippet.description", &self.description, 2048, true)?;
        if self.command.trim().is_empty()
            || self.command.len() > 65_536
            || self
                .command
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(ValidationError::new(
                "snippet.command",
                "must contain bounded text without terminal control characters",
            ));
        }
        if self.parameterized {
            crate::compile_snippet_template(&self.command).map_err(|_| {
                ValidationError::new("snippet.command", "must be a supported literal template")
            })?;
        }
        validate_tags(&self.tags)
    }
}

/// User-selected interface language. First launch and older settings use Chinese.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    /// Simplified Chinese, independent of the operating system's language.
    #[default]
    #[serde(rename = "zh-CN")]
    ZhCn,
    /// English.
    #[serde(rename = "en")]
    En,
}

impl Language {
    /// Stable locale code shared by persisted settings and translated components.
    pub const fn code(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::En => "en",
        }
    }
}

/// User-selected UI color scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Follow the platform setting.
    System,
    /// Dark UI and terminal colors.
    Dark,
    /// Light UI and terminal colors.
    #[default]
    Light,
}

/// Non-secret AI preferences. API keys are intentionally not represented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSettings {
    /// Whether the user enabled AI integration. No request is sent on startup.
    pub enabled: bool,
    /// Legacy API root or complete chat endpoint, without credentials, query or fragment.
    pub base_url: String,
    /// Explicit provider-specific model identifier.
    pub model: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://localhost:11434/v1".into(),
            model: String::new(),
        }
    }
}

impl AiSettings {
    /// Validate persistent preferences; transport-specific TLS policy is separate.
    pub fn validate(&self) -> Result<(), ValidationError> {
        text("settings.ai.base_url", &self.base_url, 2048, false)?;
        let parsed = url::Url::parse(&self.base_url)
            .map_err(|_| ValidationError::new("settings.ai.base_url", "must be an HTTP(S) URL"))?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(ValidationError::new(
                "settings.ai.base_url",
                "must be HTTP(S) without credentials, query or fragment",
            ));
        }
        text("settings.ai.model", &self.model, 200, !self.enabled)
    }
}

/// Persisted UI and terminal preferences.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Interface language; configurations without this field use Chinese.
    #[serde(default)]
    pub language: Language,
    /// Terminal font size in logical pixels, from 8 to 40.
    pub font_size: f32,
    /// Maximum terminal scrollback rows, from 100 to 100,000.
    pub scrollback_lines: usize,
    /// Preferred appearance.
    pub theme: Theme,
    /// Legacy compatibility preferences, never authoritative for named profiles.
    /// Retained so earlier state can be migrated and old UI drafts can be read.
    pub ai: AiSettings,
    /// Authoritative named provider configurations. No request is sent on load.
    /// Missing fields in older documents are migrated exactly once; a present
    /// empty catalog deliberately stays empty, even if legacy AI was enabled.
    pub ai_profiles: AiProfileCatalog,
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Option normally conflates absent and null. Null must fail rather than
        // trigger a legacy migration that could reactivate deleted settings.
        fn present_catalog<'de, D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<AiProfileCatalog>, D::Error> {
            AiProfileCatalog::deserialize(deserializer).map(Some)
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireSettings {
            #[serde(default)]
            language: Language,
            font_size: f32,
            scrollback_lines: usize,
            theme: Theme,
            #[serde(default)]
            ai: AiSettings,
            #[serde(default, deserialize_with = "present_catalog")]
            ai_profiles: Option<AiProfileCatalog>,
        }
        let wire = WireSettings::deserialize(deserializer)?;
        let ai_profiles = match wire.ai_profiles {
            Some(catalog) => catalog,
            None => AiProfileCatalog::from_legacy(&wire.ai).map_err(serde::de::Error::custom)?,
        };
        Ok(Self {
            language: wire.language,
            font_size: wire.font_size,
            scrollback_lines: wire.scrollback_lines,
            theme: wire.theme,
            ai: wire.ai,
            ai_profiles,
        })
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::default(),
            font_size: 14.0,
            scrollback_lines: 10_000,
            theme: Theme::Light,
            ai: AiSettings::default(),
            ai_profiles: AiProfileCatalog::default(),
        }
    }
}

impl Settings {
    /// Validate resource limits and all nested settings.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !self.font_size.is_finite() || !(8.0..=40.0).contains(&self.font_size) {
            return Err(ValidationError::new(
                "settings.font_size",
                "must be between 8 and 40",
            ));
        }
        if !(100..=100_000).contains(&self.scrollback_lines) {
            return Err(ValidationError::new(
                "settings.scrollback_lines",
                "must be between 100 and 100000",
            ));
        }
        self.ai.validate()?;
        self.ai_profiles.validate()
    }
}

/// Opaque optimistic-concurrency token issued only by [`crate::StateStore`].
///
/// Cloning state preserves its token. Tokens are not serialized or exported; a
/// deserialized document must be loaded through its store before replacing an
/// existing file. Callers should retain the state returned by `save`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnapshotRevision(Option<Uuid>);

impl SnapshotRevision {
    pub(crate) fn fresh() -> Self {
        Self(Some(Uuid::new_v4()))
    }
    pub(crate) fn is_loaded(self) -> bool {
        self.0.is_some()
    }
}

/// Versioned local application state. A fresh state contains no example hosts.
/// Equality compares application data, excluding the process-local snapshot token.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppState {
    /// Schema understood by this build.
    pub schema_version: u32,
    /// User-created SSH profiles, in display order.
    pub connections: Vec<Connection>,
    /// Persistent folder tree; an explicit empty tree disables legacy group migration.
    pub folders: Vec<ConnectionFolder>,
    /// Folder membership for active and deleted profiles. Absence means the root.
    pub connection_folders: BTreeMap<Uuid, Uuid>,
    /// Soft-deleted profiles, retaining their identities and local credential references.
    pub deleted_connections: Vec<DeletedConnection>,
    /// Most recently successful SSH connections, newest first and bounded to 50.
    pub recent_connections: Vec<RecentConnection>,
    /// User command templates, initially populated with read-only Linux diagnostics.
    pub snippets: Vec<Snippet>,
    /// Bounded non-secret history of reviewed SSH batch executions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch_audits: Vec<BatchAuditRecord>,
    /// Non-secret preferences.
    pub settings: Settings,
    /// Explicitly trusted host identities, keyed by normalized `[host]:port`.
    /// This local trust database is excluded from connection export/import.
    #[serde(default)]
    pub known_hosts: BTreeMap<String, String>,
    /// Explicit trust for routed SSH endpoints, keyed by canonical versioned
    /// route identity JSON. Excluded from connection export/import.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub route_known_hosts: BTreeMap<String, String>,
    /// Opaque load/save token. Preserve it with the edited snapshot; do not copy
    /// a newer token onto older application data to bypass conflict detection.
    #[serde(skip)]
    pub snapshot: SnapshotRevision,
}

impl PartialEq for AppState {
    fn eq(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version
            && self.connections == other.connections
            && self.folders == other.folders
            && self.connection_folders == other.connection_folders
            && self.deleted_connections == other.deleted_connections
            && self.recent_connections == other.recent_connections
            && self.snippets == other.snippets
            && self.batch_audits == other.batch_audits
            && self.settings == other.settings
            && self.known_hosts == other.known_hosts
            && self.route_known_hosts == other.route_known_hosts
    }
}

impl Default for AppState {
    fn default() -> Self {
        let snippets = [
            (
                "System overview",
                "uname -a; uptime",
                "Kernel and system load (Unix).",
            ),
            ("Disk usage", "df -h", "Filesystem space usage (Unix)."),
            (
                "Listening sockets",
                "ss -lntup",
                "Listening TCP/UDP sockets (Linux).",
            ),
        ]
        .into_iter()
        .map(|(name, command, description)| {
            let mut snippet = Snippet::new(name, command);
            snippet.description = description.into();
            snippet.tags = vec!["diagnostics".into(), "read-only".into()];
            snippet
        })
        .collect();
        Self {
            schema_version: SCHEMA_VERSION,
            connections: Vec::new(),
            folders: Vec::new(),
            connection_folders: BTreeMap::new(),
            deleted_connections: Vec::new(),
            recent_connections: Vec::new(),
            snippets,
            batch_audits: Vec::new(),
            settings: Settings::default(),
            known_hosts: BTreeMap::new(),
            route_known_hosts: BTreeMap::new(),
            snapshot: SnapshotRevision::default(),
        }
    }
}

/// Counts from an all-or-nothing validated connection import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportReport {
    /// Profiles inserted with fresh local IDs.
    pub added: usize,
    /// Profiles skipped because their ID or normalized route was already present.
    pub skipped: usize,
}

impl AppState {
    /// Validate schema, IDs, bounded collections and every nested domain object.
    pub fn validate(&self) -> Result<(), Error> {
        schema(self.schema_version)?;
        self.validate_connection_library()?;
        validate_known_hosts(&self.known_hosts)?;
        self.validate_route_known_hosts()?;
        if self.snippets.len() > 2000 {
            return Err(ValidationError::new("snippets", "at most 2000 are supported").into());
        }
        let mut ids = HashSet::new();
        for snippet in &self.snippets {
            snippet.validate()?;
            if !ids.insert(snippet.id) {
                return Err(ValidationError::new("snippet.id", "must be unique").into());
            }
        }
        crate::batch_audit::validate_batch_audits(&self.batch_audits)?;
        self.settings.validate()?;
        Ok(())
    }

    /// Return an explicitly stored fingerprint for a normalized host and port.
    /// Invalid destinations return `None`; no network request or trust inference occurs.
    pub fn host_key(&self, host: &str, port: u16) -> Option<&str> {
        let key = host_identity_key(host, port).ok()?;
        self.known_hosts.get(&key).map(String::as_str)
    }

    /// Store a SHA256 fingerprint only after the user approves this exact identity.
    ///
    /// This can replace an existing pin and therefore must not be called as an
    /// automatic response to a changed-host-key error. Saving state persists it.
    pub fn trust_host_key(
        &mut self,
        host: &str,
        port: u16,
        fingerprint: &str,
    ) -> Result<(), ValidationError> {
        let key = host_identity_key(host, port)?;
        validate_fingerprint(fingerprint)?;
        if !self.known_hosts.contains_key(&key)
            && self.known_hosts.len() + self.route_known_hosts.len() >= MAX_CONNECTIONS
        {
            return Err(ValidationError::new(
                "known_hosts",
                "at most 10000 pins are supported",
            ));
        }
        self.known_hosts.insert(key, fingerprint.to_owned());
        Ok(())
    }

    /// Search profiles in saved display order, requiring all query terms to match.
    pub fn search_connections(&self, query: &str) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|connection| connection.matches(query))
            .collect()
    }

    /// Return distinct nonempty group labels, sorted alphabetically.
    pub fn groups(&self) -> Vec<&str> {
        let mut groups: Vec<_> = self
            .connections
            .iter()
            .map(|c| c.group.as_str())
            .filter(|group| !group.is_empty())
            .collect();
        groups.sort_unstable();
        groups.dedup();
        groups
    }

    /// Duplicate a saved profile, retaining options and assigning a fresh identity.
    pub fn duplicate_connection(&mut self, id: Uuid) -> Result<Uuid, Error> {
        self.validate_connection_library()?;
        if self.connections.len() + self.deleted_connections.len() >= MAX_CONNECTIONS {
            return Err(ValidationError::new("connections", "at most 10000 are supported").into());
        }
        let source = self
            .connections
            .iter()
            .find(|c| c.id == id)
            .ok_or(Error::ConnectionNotFound)?;
        let name: String = source.name.chars().take(110).collect();
        let copy = source.duplicate(format!("{name} (copy)"))?;
        let copied_id = copy.id;
        self.connections.push(copy);
        if let Some(folder_id) = self.folder_id_of(id) {
            self.connection_folders.insert(copied_id, folder_id);
        }
        Ok(copied_id)
    }

    /// Permanently remove an active profile and return its metadata.
    /// Prefer [`Self::soft_delete_connection`] for user-facing deletion.
    ///
    /// This only changes the in-memory snapshot. Call [`crate::StateStore::save`]
    /// to persist the deletion. Secrets are not part of a connection profile, so
    /// returning the value is safe for an undo affordance in the UI.
    pub fn remove_connection(&mut self, id: Uuid) -> Result<Connection, Error> {
        self.validate_connection_library()?;
        self.ensure_no_jump_dependents(id, true)?;
        let index = self
            .connections
            .iter()
            .position(|connection| connection.id == id)
            .ok_or(Error::ConnectionNotFound)?;
        self.connection_folders.remove(&id);
        self.recent_connections
            .retain(|recent| recent.connection_id != id);
        Ok(self.connections.remove(index))
    }

    /// Toggle the favorite marker for a saved profile and return its new value.
    ///
    /// The marker is metadata only; it does not affect connection credentials or
    /// transport state. Call [`crate::StateStore::save`] to persist the change.
    pub fn toggle_connection_favorite(&mut self, id: Uuid) -> Result<bool, Error> {
        let connection = self
            .connections
            .iter_mut()
            .find(|connection| connection.id == id)
            .ok_or(Error::ConnectionNotFound)?;
        connection.favorite = !connection.favorite;
        Ok(connection.favorite)
    }
}

pub(crate) fn schema(version: u32) -> Result<(), Error> {
    if version != SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema {
            found: version,
            expected: SCHEMA_VERSION,
        });
    }
    Ok(())
}

pub(crate) fn validate_connections(connections: &[Connection]) -> Result<(), ValidationError> {
    if connections.len() > MAX_CONNECTIONS {
        return Err(ValidationError::new(
            "connections",
            "at most 10000 are supported",
        ));
    }
    let mut ids = HashSet::new();
    for connection in connections {
        connection.validate()?;
        if !ids.insert(connection.id) {
            return Err(ValidationError::new("connection.id", "must be unique"));
        }
    }
    Ok(())
}

pub(crate) fn host_identity_key(host: &str, port: u16) -> Result<String, ValidationError> {
    text("known_hosts.host", host, 253, false)?;
    if port == 0
        || host.starts_with('-')
        || host
            .chars()
            .any(|c| c.is_whitespace() || "/\\@?#[]".contains(c))
        || (host.contains(':') && host.parse::<std::net::Ipv6Addr>().is_err())
    {
        return Err(ValidationError::new(
            "known_hosts.host",
            "must be a hostname or IP literal with a nonzero port",
        ));
    }
    let host = normalized_host(host);
    if host.is_empty() {
        return Err(ValidationError::new(
            "known_hosts.host",
            "must not be empty",
        ));
    }
    Ok(format!("[{host}]:{port}"))
}

pub(crate) fn normalized_host(host: &str) -> String {
    host.parse::<std::net::IpAddr>().map_or_else(
        |_| host.trim_end_matches('.').to_lowercase(),
        |address| address.to_string(),
    )
}

pub(crate) fn validate_fingerprint(fingerprint: &str) -> Result<(), ValidationError> {
    let Some(encoded) = fingerprint.strip_prefix("SHA256:") else {
        return Err(ValidationError::new(
            "known_hosts.fingerprint",
            "must be an unpadded SHA256 base64 fingerprint",
        ));
    };
    if encoded.len() != 43
        || !encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/'))
        || !encoded
            .as_bytes()
            .last()
            .is_some_and(|b| b"AEIMQUYcgkosw048".contains(b))
    {
        return Err(ValidationError::new(
            "known_hosts.fingerprint",
            "must encode exactly 32 bytes using unpadded base64",
        ));
    }
    Ok(())
}

fn validate_known_hosts(hosts: &BTreeMap<String, String>) -> Result<(), ValidationError> {
    if hosts.len() > MAX_CONNECTIONS {
        return Err(ValidationError::new(
            "known_hosts",
            "at most 10000 pins are supported",
        ));
    }
    for (key, fingerprint) in hosts {
        let parsed = key.strip_prefix('[').and_then(|key| key.rsplit_once("]:"));
        let Some((host, port)) = parsed else {
            return Err(ValidationError::new(
                "known_hosts",
                "keys must use normalized [host]:port format",
            ));
        };
        let port = port.parse::<u16>().map_err(|_| {
            ValidationError::new("known_hosts", "keys must contain a valid TCP port")
        })?;
        if host_identity_key(host, port)? != *key {
            return Err(ValidationError::new(
                "known_hosts",
                "keys must use normalized [host]:port format",
            ));
        }
        validate_fingerprint(fingerprint)?;
    }
    Ok(())
}

fn validate_tags(tags: &[String]) -> Result<(), ValidationError> {
    if tags.len() > 32 {
        return Err(ValidationError::new("tags", "at most 32 are supported"));
    }
    for tag in tags {
        text("tag", tag, 64, false)?;
    }
    Ok(())
}

pub(crate) fn text(
    field: &'static str,
    value: &str,
    max_chars: usize,
    empty: bool,
) -> Result<(), ValidationError> {
    if (!empty && value.trim().is_empty())
        || value.trim() != value
        || value.chars().count() > max_chars
        || value.chars().any(char::is_control)
    {
        return Err(ValidationError::new(
            field,
            "must be bounded text without outer whitespace or control characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod locale_tests {
    use super::{Language, Settings, Theme};

    #[test]
    fn language_defaults_to_chinese_for_older_settings() -> Result<(), serde_json::Error> {
        let mut old = serde_json::to_value(Settings::default())?;
        if let Some(object) = old.as_object_mut() {
            object.remove("language");
        }
        let loaded: Settings = serde_json::from_value(old)?;
        assert_eq!(loaded.language, Language::ZhCn);
        assert_eq!(loaded.theme, Theme::Light);
        assert_eq!(loaded.language.code(), "zh-CN");
        Ok(())
    }

    #[test]
    fn explicitly_selected_english_roundtrips() -> Result<(), serde_json::Error> {
        let settings = Settings {
            language: Language::En,
            ..Settings::default()
        };
        let text = serde_json::to_string(&settings)?;
        let loaded: Settings = serde_json::from_str(&text)?;
        assert_eq!(loaded.language, Language::En);
        assert_eq!(loaded.language.code(), "en");
        assert!(text.contains("\"language\":\"en\""));
        Ok(())
    }
}
