//! Bounded saved-profile routes and route-scoped SSH host trust.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState, Connection, ConnectionProxy, Error, ValidationError,
    model::{MAX_CONNECTIONS, host_identity_key, validate_fingerprint},
};

mod import;
pub(crate) use import::import_profiles;

/// Maximum number of SSH jump hosts before the final destination.
pub const MAX_JUMP_HOSTS: usize = 4;
const LEGACY_IDENTITY_VERSION: u32 = 1;
const PROXY_IDENTITY_VERSION: u32 = 2;

/// A normalized network endpoint, independent of display metadata or local IDs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteEndpoint {
    /// Lowercase DNS name without a trailing dot, or a canonical IP literal.
    pub host: String,
    /// SSH TCP port.
    pub port: u16,
    /// Case-sensitive account name on this endpoint.
    pub username: String,
    /// Canonical upstream proxy metadata for this hop, without a password.
    /// Absent values are omitted to preserve existing proxy-free identity keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ConnectionProxy>,
}

impl RouteEndpoint {
    /// Validate profile metadata and return the canonical network endpoint.
    /// No network, credential-store or private-key I/O is performed.
    pub fn from_connection(connection: &Connection) -> Result<Self, ValidationError> {
        connection.validate()?;
        endpoint_from(connection)
    }
}

/// A versioned, ordered route identity for credentials and scoped host trust.
///
/// Proxy-free routes retain version 1; a prefix containing any proxy uses
/// version 2. A direct connection has one endpoint and no proxy.
/// Deserialization does not grant trust:
/// compare the value with a freshly resolved route, or call [`Self::validate`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteIdentity {
    version: u32,
    endpoints: Vec<RouteEndpoint>,
}

impl RouteIdentity {
    /// Return endpoints in connection order, first hop through final target.
    pub fn endpoints(&self) -> &[RouteEndpoint] {
        &self.endpoints
    }

    /// Validate the version, resource bound and canonical endpoint spellings.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.version != identity_version(&self.endpoints)
            || self.endpoints.is_empty()
            || self.endpoints.len() > MAX_JUMP_HOSTS + 1
        {
            return Err(route_error("unsupported identity version or depth"));
        }
        for endpoint in &self.endpoints {
            let profile = Connection {
                id: Uuid::from_u128(1),
                name: "Route".into(),
                group: String::new(),
                host: endpoint.host.clone(),
                port: endpoint.port,
                username: endpoint.username.clone(),
                auth: crate::AuthMethod::Agent,
                credential_ref: None,
                jump_host: None,
                proxy: endpoint.proxy.clone(),
                reconnect: crate::ReconnectPolicy::Manual,
                tags: Vec::new(),
                favorite: false,
            };
            profile.validate()?;
            if endpoint_from(&profile)? != *endpoint {
                return Err(route_error("route endpoints must use canonical spelling"));
            }
        }
        Ok(())
    }

    pub(crate) fn key(&self) -> Result<String, ValidationError> {
        serde_json::to_string(self).map_err(|_| route_error("route identity could not be encoded"))
    }

    fn direct_endpoint(&self) -> Option<&RouteEndpoint> {
        match self.endpoints.as_slice() {
            [endpoint] if endpoint.proxy.is_none() => Some(endpoint),
            _ => None,
        }
    }
}

/// An immutable, already validated connection-order metadata snapshot.
///
/// No passwords, key contents or decrypted credentials are included. Callers
/// must recheck this snapshot before accepting delayed authentication or trust.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionRoute {
    hops: Vec<Connection>,
    identity: RouteIdentity,
}

impl ConnectionRoute {
    /// Build a direct route for a one-time SSH connection.
    ///
    /// This constructor deliberately does not add the profile to [`AppState`].
    /// Callers can use the returned route for host-key scoping and transport
    /// setup while keeping one-time connection metadata out of the persistent
    /// connection library. Jump hosts are intentionally not accepted here;
    /// saved routes should use [`AppState::connection_route`] so every hop is
    /// resolved against the validated library.
    pub fn direct(connection: Connection) -> Result<Self, ValidationError> {
        connection.validate()?;
        if connection.jump_host.is_some() || connection.credential_ref.is_some() {
            return Err(route_error(
                "direct routes cannot contain jump hosts or credential references",
            ));
        }
        let endpoint = endpoint_from(&connection)?;
        let identity = RouteIdentity {
            version: identity_version(std::slice::from_ref(&endpoint)),
            endpoints: vec![endpoint],
        };
        Ok(Self {
            hops: vec![connection],
            identity,
        })
    }

    /// Return saved profiles in connection order, first hop through target.
    pub fn hops(&self) -> &[Connection] {
        &self.hops
    }

    /// Return the complete normalized identity, including direct routes.
    pub fn identity(&self) -> RouteIdentity {
        self.identity.clone()
    }

    /// Whether a fresh route still addresses the profiles used by this session.
    ///
    /// This compares ordered profile IDs, canonical endpoints (including proxies),
    /// authentication methods and jump references. Display metadata, credential
    /// references and reconnect preferences may change without retargeting a session.
    /// A match does not grant host trust or authorize reusing an old credential:
    /// callers must resolve active profiles again and check each current host pin.
    pub fn same_reconnect_target(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.hops.len() == other.hops.len()
            && self
                .hops
                .iter()
                .zip(&other.hops)
                .all(|(a, b)| a.id == b.id && a.auth == b.auth && a.jump_host == b.jump_host)
    }

    /// Return the identity through a zero-based hop; invalid indices return `None`.
    pub fn identity_prefix(&self, hop: usize) -> Option<RouteIdentity> {
        let endpoints = self.identity.endpoints.get(..=hop)?.to_vec();
        Some(RouteIdentity {
            version: identity_version(&endpoints),
            endpoints,
        })
    }

    /// Return a host-trust scope through one hop. Only a proxy-free first hop
    /// uses the legacy direct trust namespace; all other scopes bind the route.
    pub fn host_key_scope(&self, hop: usize) -> Option<HostKeyScope> {
        Some(HostKeyScope {
            identity: self.identity_prefix(hop)?,
        })
    }
}

/// Validated host-trust scope produced by a resolved connection route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostKeyScope {
    identity: RouteIdentity,
}

impl AppState {
    /// Resolve one active profile through at most four active jump hosts.
    ///
    /// Missing, deleted, cyclic, duplicate or over-depth routes are rejected.
    /// The returned owned snapshot is unaffected by later configuration edits.
    pub fn connection_route(&self, id: Uuid) -> Result<ConnectionRoute, Error> {
        let index = RouteIndex::new(self)?;
        if !index.active.contains(&id) {
            return Err(Error::ConnectionNotFound);
        }
        Ok(index.resolve(id, true)?)
    }

    /// Read an explicitly trusted key in this exact scope, without fallback.
    /// Direct scopes retain the legacy `[host]:port` trust namespace.
    pub fn host_key_for_scope(&self, scope: &HostKeyScope) -> Option<&str> {
        if let Some(endpoint) = scope.identity.direct_endpoint() {
            self.host_key(&endpoint.host, endpoint.port)
        } else {
            self.route_known_hosts
                .get(&scope.identity.key().ok()?)
                .map(String::as_str)
        }
    }

    /// Persist an explicitly approved key in an exact resolved scope in memory.
    ///
    /// The caller must verify the route snapshot is still current and save this
    /// state separately. A changed key is never approved automatically.
    pub fn trust_host_key_for_scope(
        &mut self,
        scope: &HostKeyScope,
        fingerprint: &str,
    ) -> Result<(), Error> {
        scope.identity.validate()?;
        validate_fingerprint(fingerprint)?;
        if let Some(endpoint) = scope.identity.direct_endpoint() {
            return Ok(self.trust_host_key(&endpoint.host, endpoint.port, fingerprint)?);
        }
        let key = scope.identity.key()?;
        if !self.route_known_hosts.contains_key(&key)
            && self.route_known_hosts.len() + self.known_hosts.len() >= MAX_CONNECTIONS
        {
            return Err(route_error("at most 10000 host pins are supported").into());
        }
        self.route_known_hosts.insert(key, fingerprint.to_owned());
        Ok(())
    }

    /// Atomically replace an active saved profile without changing its position.
    ///
    /// Route changes clear credentials and recent-success entries for affected
    /// downstream profiles, including credential references retained in trash.
    /// Changing this profile's authentication also invalidates its dependents.
    /// Renaming or retagging keeps those references. Vault ciphertext is not erased.
    /// Missing IDs, invalid routes or other validation errors leave state unchanged.
    pub fn update_connection(&mut self, connection: Connection) -> Result<(), Error> {
        self.validate()?;
        let position = self
            .connections
            .iter()
            .position(|item| item.id == connection.id)
            .ok_or(Error::ConnectionNotFound)?;
        let changed_id = connection.id;
        let auth_changed = self.connections[position].auth != connection.auth;
        let mut candidate = self.clone();
        candidate.connections[position] = connection;
        candidate.validate()?;
        let old = RouteIndex::new(self)?;
        let next = RouteIndex::new(&candidate)?;
        let mut invalidated = HashSet::new();
        for id in old.profiles.keys() {
            let previous = old.resolve(*id, false)?;
            let current = next.resolve(*id, false)?;
            if previous.identity != current.identity
                || (auth_changed && current.hops.iter().any(|hop| hop.id == changed_id))
            {
                invalidated.insert(*id);
            }
        }
        for profile in candidate.connections.iter_mut().chain(
            candidate
                .deleted_connections
                .iter_mut()
                .map(|entry| &mut entry.connection),
        ) {
            if invalidated.contains(&profile.id) {
                profile.credential_ref = None;
            }
        }
        candidate
            .recent_connections
            .retain(|entry| !invalidated.contains(&entry.connection_id));
        *self = candidate;
        Ok(())
    }

    pub(crate) fn validate_connection_routes(&self) -> Result<(), ValidationError> {
        let index = RouteIndex::new(self)?;
        for id in index.profiles.keys() {
            index.resolve(*id, index.active.contains(id))?;
        }
        Ok(())
    }

    pub(crate) fn validate_route_known_hosts(&self) -> Result<(), ValidationError> {
        if self.route_known_hosts.len() + self.known_hosts.len() > MAX_CONNECTIONS {
            return Err(route_error("at most 10000 host pins are supported"));
        }
        for (key, fingerprint) in &self.route_known_hosts {
            let identity: RouteIdentity = serde_json::from_str(key)
                .map_err(|_| route_error("host pin requires a canonical route identity"))?;
            identity.validate()?;
            if identity.direct_endpoint().is_some() || identity.key()? != *key {
                return Err(route_error(
                    "scoped host pins require canonical routed or proxied identities",
                ));
            }
            validate_fingerprint(fingerprint)?;
        }
        Ok(())
    }

    pub(crate) fn ensure_no_jump_dependents(
        &self,
        id: Uuid,
        include_deleted: bool,
    ) -> Result<(), ValidationError> {
        if self
            .connections
            .iter()
            .any(|entry| entry.jump_host == Some(id))
            || (include_deleted
                && self
                    .deleted_connections
                    .iter()
                    .any(|entry| entry.connection.jump_host == Some(id)))
        {
            return Err(route_error("profile is still referenced as a jump host"));
        }
        Ok(())
    }
}

pub(crate) struct RouteIndex<'a> {
    pub(crate) profiles: HashMap<Uuid, &'a Connection>,
    pub(crate) active: HashSet<Uuid>,
}

impl<'a> RouteIndex<'a> {
    pub(crate) fn new(state: &'a AppState) -> Result<Self, ValidationError> {
        if state.connections.len() + state.deleted_connections.len() > MAX_CONNECTIONS {
            return Err(route_error(
                "at most 10000 active and deleted profiles are supported",
            ));
        }
        let mut profiles = HashMap::new();
        let active = state.connections.iter().map(|profile| profile.id).collect();
        for profile in state.connections.iter().chain(
            state
                .deleted_connections
                .iter()
                .map(|entry| &entry.connection),
        ) {
            profile.validate()?;
            if profiles.insert(profile.id, profile).is_some() {
                return Err(route_error("profile IDs must be unique"));
            }
        }
        Ok(Self { profiles, active })
    }

    pub(crate) fn resolve(
        &self,
        id: Uuid,
        active_only: bool,
    ) -> Result<ConnectionRoute, ValidationError> {
        let mut hops = Vec::with_capacity(MAX_JUMP_HOSTS + 1);
        let mut seen = HashSet::new();
        let mut next = Some(id);
        while let Some(id) = next {
            if !seen.insert(id) {
                return Err(route_error("jump host cycles are not allowed"));
            }
            if hops.len() > MAX_JUMP_HOSTS {
                return Err(route_error("at most four jump hosts are supported"));
            }
            let profile = self
                .profiles
                .get(&id)
                .ok_or_else(|| route_error("jump host must reference an existing profile"))?;
            if active_only && !self.active.contains(&id) {
                return Err(route_error("active routes cannot use deleted jump hosts"));
            }
            hops.push((*profile).clone());
            next = profile.jump_host;
        }
        hops.reverse();
        let endpoints: Vec<_> = hops.iter().map(endpoint_from).collect::<Result<_, _>>()?;
        let identity = RouteIdentity {
            version: identity_version(&endpoints),
            endpoints,
        };
        Ok(ConnectionRoute { hops, identity })
    }
}

fn endpoint_from(profile: &Connection) -> Result<RouteEndpoint, ValidationError> {
    host_identity_key(&profile.host, profile.port)?;
    let (host, port, username) = profile.endpoint_key();
    Ok(RouteEndpoint {
        host,
        port,
        username: username.to_owned(),
        proxy: profile
            .proxy
            .as_ref()
            .map(ConnectionProxy::canonicalized)
            .transpose()?,
    })
}

fn identity_version(endpoints: &[RouteEndpoint]) -> u32 {
    if endpoints.iter().any(|endpoint| endpoint.proxy.is_some()) {
        PROXY_IDENTITY_VERSION
    } else {
        LEGACY_IDENTITY_VERSION
    }
}

pub(crate) fn route_error(reason: &'static str) -> ValidationError {
    ValidationError::new("connection.jump_host", reason)
}
