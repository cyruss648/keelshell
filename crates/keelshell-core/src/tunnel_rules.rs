//! Persistent, reviewable metadata for named SSH forwarding rules.
//!
//! A rule describes a future forwarding listener. Loading or validating a rule
//! never opens a socket, starts a listener, reconnects SSH, or reads a secret.
//! The plan and confirmation types deliberately stop before transport execution.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, Connection, Error, SnapshotRevision, ValidationError, model::text};

/// The current version of the persisted rule shape.
pub const TUNNEL_RULE_VERSION: u32 = 1;
/// The maximum number of named rules retained in one application state.
pub const MAX_TUNNEL_RULES: usize = 256;

/// The forwarding mode a named rule will request from a future transport worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelRuleKind {
    /// Listen locally and connect through SSH to the configured target.
    Local,
    /// Ask the SSH server to listen and connect back to the configured target.
    Remote,
    /// Listen on a local loopback address as a SOCKS5 CONNECT proxy.
    Dynamic,
}

/// Backwards-readable name for callers that use “type” rather than “kind”.
pub type TunnelRuleType = TunnelRuleKind;

/// A non-secret named forwarding rule stored in `AppState`.
///
/// `bind_port == 0` asks the future transport adapter to allocate a local or
/// remote listener port. Local and remote rules require both target fields;
/// dynamic rules require neither target field and are restricted to loopback.
/// Passwords, tokens, private-key contents and runtime handles have no fields
/// in this type. Serde rejects any extra field instead of accepting secret data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TunnelRule {
    /// Stable rule identity, independent of its display name.
    pub id: Uuid,
    /// User-facing label for this rule.
    pub name: String,
    /// Saved SSH connection that will own the future forwarding session.
    pub connection_id: Uuid,
    /// Forwarding mode.
    pub kind: TunnelRuleKind,
    /// Address or host name on the listener side.
    pub bind_host: String,
    /// Listener port; zero means allocate a port where the transport supports it.
    pub bind_port: u16,
    /// Target host for local and remote forwarding; absent for dynamic SOCKS5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_host: Option<String>,
    /// Target TCP port for local and remote forwarding; absent for dynamic SOCKS5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_port: Option<u16>,
    /// User intent for a future explicit start/stop review; it is not a startup command.
    pub enabled: bool,
    /// Version of this rule's persisted shape.
    pub rule_version: u32,
}

impl TunnelRule {
    /// Construct a rule with a fresh stable identity and the current rule version.
    ///
    /// Construction validates all shape and endpoint fields but does not check
    /// whether `connection_id` exists in an [`AppState`]. That binding is checked
    /// when the rule is added to or validated as part of application state.
    pub fn new(
        name: impl Into<String>,
        connection_id: Uuid,
        kind: TunnelRuleKind,
        bind_host: impl Into<String>,
        bind_port: u16,
        target_host: Option<String>,
        target_port: Option<u16>,
    ) -> Result<Self, ValidationError> {
        let rule = Self {
            id: Uuid::new_v4(),
            name: name.into().trim().to_owned(),
            connection_id,
            kind,
            bind_host: bind_host.into().trim().to_owned(),
            bind_port,
            target_host: target_host.map(|host| host.trim().to_owned()),
            target_port,
            enabled: false,
            rule_version: TUNNEL_RULE_VERSION,
        };
        rule.validate()?;
        Ok(rule)
    }

    /// Validate identity, endpoint shape, secret exclusion and bounded input.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.is_nil() {
            return Err(ValidationError::new("tunnel_rule.id", "must not be nil"));
        }
        if self.connection_id.is_nil() {
            return Err(ValidationError::new(
                "tunnel_rule.connection_id",
                "must not be nil",
            ));
        }
        if self.rule_version != TUNNEL_RULE_VERSION {
            return Err(ValidationError::new(
                "tunnel_rule.rule_version",
                "is unsupported",
            ));
        }
        text("tunnel_rule.name", &self.name, 120, false)?;
        validate_host("tunnel_rule.bind_host", &self.bind_host)?;
        validate_kind_fields(
            self.kind,
            &self.bind_host,
            self.target_host.as_deref(),
            self.target_port,
        )
    }

    /// Return whether this rule contains only a future listener intent.
    ///
    /// This is intentionally a data property. It does not inspect or alter
    /// process state and does not imply that a listener is currently running.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}

fn validate_kind_fields(
    kind: TunnelRuleKind,
    bind_host: &str,
    target_host: Option<&str>,
    target_port: Option<u16>,
) -> Result<(), ValidationError> {
    match kind {
        TunnelRuleKind::Local | TunnelRuleKind::Remote => {
            let Some(target_host) = target_host else {
                return Err(ValidationError::new(
                    "tunnel_rule.target_host",
                    "is required for local and remote rules",
                ));
            };
            validate_host("tunnel_rule.target_host", target_host)?;
            if target_port == Some(0) || target_port.is_none() {
                return Err(ValidationError::new(
                    "tunnel_rule.target_port",
                    "must be a nonzero TCP port for local and remote rules",
                ));
            }
        }
        TunnelRuleKind::Dynamic => {
            if target_host.is_some() || target_port.is_some() {
                return Err(ValidationError::new(
                    "tunnel_rule.target",
                    "dynamic rules cannot persist a target",
                ));
            }
            let Ok(address) = bind_host.parse::<IpAddr>() else {
                return Err(ValidationError::new(
                    "tunnel_rule.bind_host",
                    "dynamic rules require a loopback IP address",
                ));
            };
            if !address.is_loopback() {
                return Err(ValidationError::new(
                    "tunnel_rule.bind_host",
                    "dynamic rules require a loopback IP address",
                ));
            }
        }
    }
    Ok(())
}

fn validate_host(field: &'static str, host: &str) -> Result<(), ValidationError> {
    text(field, host, 253, false)?;
    if host.starts_with('-')
        || host.contains(['[', ']'])
        || host
            .chars()
            .any(|character| character.is_whitespace() || "/\\@?#".contains(character))
        || (host.contains(':') && host.parse::<std::net::Ipv6Addr>().is_err())
    {
        return Err(ValidationError::new(
            field,
            "must be a hostname or IP literal without credentials or URL syntax",
        ));
    }
    Ok(())
}

/// The future explicit operation requested for a validated rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelRuleAction {
    /// Start the listener after the user reviews and confirms the plan.
    Start,
    /// Stop the listener after the user reviews and confirms the plan.
    Stop,
}

/// A side-effect-free, state-bound plan awaiting explicit user review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTunnelRulePlan {
    rule: TunnelRule,
    action: TunnelRuleAction,
    review_token: TunnelRuleReviewToken,
    state_revision: SnapshotRevision,
    connection_snapshot: Connection,
}

impl ValidatedTunnelRulePlan {
    /// Return the exact rule snapshot presented for review.
    pub const fn rule(&self) -> &TunnelRule {
        &self.rule
    }

    /// Return whether the reviewed intent is start or stop.
    pub const fn action(&self) -> TunnelRuleAction {
        self.action
    }

    /// Return the opaque token that must match the exact reviewed plan.
    pub const fn review_token(&self) -> TunnelRuleReviewToken {
        self.review_token
    }

    /// Confirm the exact plan after its caller has displayed it to the user.
    ///
    /// The receipt has no socket, SSH session, task, or storage capability. A
    /// future transport adapter must still bind the rule to a live connection,
    /// revalidate it, and report the observed start/stop result.
    pub fn confirm(
        self,
        state: &AppState,
        token: TunnelRuleReviewToken,
    ) -> Result<ConfirmedTunnelRuleAction, TunnelRuleReviewError> {
        if token != self.review_token {
            return Err(TunnelRuleReviewError::TokenMismatch);
        }
        let current_connection = state
            .connections
            .iter()
            .find(|connection| connection.id == self.rule.connection_id);
        let current_rule = state
            .tunnel_rules
            .iter()
            .find(|rule| rule.id == self.rule.id);
        if state.snapshot != self.state_revision
            || state.validate().is_err()
            || current_connection != Some(&self.connection_snapshot)
            || current_rule != Some(&self.rule)
        {
            return Err(TunnelRuleReviewError::StateChanged);
        }
        Ok(ConfirmedTunnelRuleAction { plan: self })
    }
}

/// Opaque token for one exact in-memory rule action plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TunnelRuleReviewToken(Uuid);

impl TunnelRuleReviewToken {
    /// Return a non-secret hexadecimal receipt suitable for UI display or logs.
    pub fn hex(self) -> String {
        self.0.simple().to_string()
    }
}

/// Acknowledged rule action that still carries no execution capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedTunnelRuleAction {
    plan: ValidatedTunnelRulePlan,
}

impl ConfirmedTunnelRuleAction {
    /// Borrow the exact state-bound plan that was explicitly confirmed.
    pub const fn plan(&self) -> &ValidatedTunnelRulePlan {
        &self.plan
    }

    /// Consume the receipt for a future transport-specific executor.
    pub fn into_plan(self) -> ValidatedTunnelRulePlan {
        self.plan
    }
}

/// Failure while confirming a named-rule action plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TunnelRuleReviewError {
    /// The supplied token was not produced for this exact plan.
    #[error("tunnel rule review token does not match the plan")]
    TokenMismatch,
    /// The reviewed state, connection route or rule changed before confirmation.
    #[error("tunnel rule review is stale because its state changed")]
    StateChanged,
}

impl AppState {
    /// Add a validated rule bound to an active saved connection.
    ///
    /// This only changes the in-memory state. Persist it with
    /// [`crate::StateStore::save`]. It never starts a listener.
    pub fn add_tunnel_rule(&mut self, rule: TunnelRule) -> Result<Uuid, Error> {
        let mut candidate = self.clone();
        candidate.tunnel_rules.push(rule.clone());
        candidate.validate()?;
        *self = candidate;
        Ok(rule.id)
    }

    /// Replace one rule atomically after validating its active connection binding.
    pub fn update_tunnel_rule(&mut self, rule: TunnelRule) -> Result<(), Error> {
        let mut candidate = self.clone();
        let existing = candidate
            .tunnel_rules
            .iter_mut()
            .find(|existing| existing.id == rule.id)
            .ok_or(Error::TunnelRuleNotFound)?;
        *existing = rule;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Remove a named rule without touching a connection or runtime listener.
    pub fn remove_tunnel_rule(&mut self, id: Uuid) -> Result<TunnelRule, Error> {
        let mut candidate = self.clone();
        let index = candidate
            .tunnel_rules
            .iter()
            .position(|rule| rule.id == id)
            .ok_or(Error::TunnelRuleNotFound)?;
        let removed = candidate.tunnel_rules.remove(index);
        candidate.validate()?;
        *self = candidate;
        Ok(removed)
    }

    /// Build a bounded, side-effect-free plan for an explicit rule start or stop.
    ///
    /// Loading a state, setting `enabled`, or creating this plan never starts or
    /// stops a listener. The future executor must require the returned plan's
    /// token and a separate user confirmation before doing transport work.
    pub fn plan_tunnel_rule_action(
        &self,
        id: Uuid,
        action: TunnelRuleAction,
    ) -> Result<ValidatedTunnelRulePlan, Error> {
        self.validate()?;
        let rule = self
            .tunnel_rules
            .iter()
            .find(|rule| rule.id == id)
            .ok_or(Error::TunnelRuleNotFound)?
            .clone();
        if matches!(action, TunnelRuleAction::Start) && !rule.enabled {
            return Err(ValidationError::new(
                "tunnel_rule.enabled",
                "must be enabled before planning a start",
            )
            .into());
        }
        let connection_snapshot = self
            .connections
            .iter()
            .find(|connection| connection.id == rule.connection_id)
            .cloned()
            .ok_or(Error::ConnectionNotFound)?;
        Ok(ValidatedTunnelRulePlan {
            rule,
            action,
            review_token: TunnelRuleReviewToken(Uuid::new_v4()),
            state_revision: self.snapshot,
            connection_snapshot,
        })
    }

    pub(crate) fn ensure_no_tunnel_rule_dependents(&self, id: Uuid) -> Result<(), Error> {
        if self
            .tunnel_rules
            .iter()
            .any(|rule| rule.connection_id == id)
        {
            return Err(Error::ConnectionReferencedByTunnelRule);
        }
        Ok(())
    }

    pub(crate) fn validate_tunnel_rules(&self) -> Result<(), ValidationError> {
        if self.tunnel_rules.len() > MAX_TUNNEL_RULES {
            return Err(ValidationError::new(
                "tunnel_rules",
                "at most 256 named rules are supported",
            ));
        }
        let connection_ids: std::collections::HashSet<_> = self
            .connections
            .iter()
            .map(|connection| connection.id)
            .collect();
        let mut ids = std::collections::HashSet::new();
        for rule in &self.tunnel_rules {
            rule.validate()?;
            if !ids.insert(rule.id) {
                return Err(ValidationError::new("tunnel_rule.id", "must be unique"));
            }
            if !connection_ids.contains(&rule.connection_id) {
                return Err(ValidationError::new(
                    "tunnel_rule.connection_id",
                    "must reference an active connection",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::{AppState, Connection};

    fn local_rule(connection_id: Uuid) -> TunnelRule {
        TunnelRule::new(
            "web",
            connection_id,
            TunnelRuleKind::Local,
            "127.0.0.1",
            0,
            Some("db.internal".into()),
            Some(5432),
        )
        .expect("test rule is valid")
    }

    #[test]
    fn rule_validation_requires_mode_specific_targets_and_loopback_dynamic_bind() {
        let id = Uuid::new_v4();
        let mut local = local_rule(id);
        local.target_host = None;
        assert_eq!(
            local.validate().expect_err("target required").field,
            "tunnel_rule.target_host"
        );

        let dynamic = TunnelRule::new(
            "proxy",
            id,
            TunnelRuleKind::Dynamic,
            "127.0.0.1",
            0,
            None,
            None,
        )
        .expect("dynamic rule is valid");
        assert!(dynamic.validate().is_ok());
        let mut public = dynamic;
        public.bind_host = "0.0.0.0".into();
        assert_eq!(
            public.validate().expect_err("public SOCKS bind").field,
            "tunnel_rule.bind_host"
        );
    }

    #[test]
    fn rule_plan_confirmation_is_pure_and_token_bound() {
        let connection = Connection::new("host", "host.test", "user");
        let mut state = AppState::default();
        state.connections.push(connection.clone());
        let mut rule = local_rule(connection.id);
        rule.enabled = true;
        state.add_tunnel_rule(rule.clone()).expect("rule added");
        let plan = state
            .plan_tunnel_rule_action(rule.id, TunnelRuleAction::Start)
            .expect("start plan");
        let token = plan.review_token();
        assert!(plan.clone().confirm(&state, token).is_ok());
        assert_eq!(
            plan.clone()
                .confirm(&state, TunnelRuleReviewToken(Uuid::new_v4())),
            Err(TunnelRuleReviewError::TokenMismatch)
        );
        assert_eq!(state.tunnel_rules.len(), 1);
    }

    #[test]
    fn changing_the_bound_connection_invalidates_an_old_review_plan() {
        let connection = Connection::new("host", "host.test", "user");
        let mut state = AppState::default();
        state.connections.push(connection.clone());
        let mut rule = local_rule(connection.id);
        rule.enabled = true;
        state.add_tunnel_rule(rule.clone()).expect("rule added");
        let plan = state
            .plan_tunnel_rule_action(rule.id, TunnelRuleAction::Start)
            .expect("start plan");
        let token = plan.review_token();

        let mut changed = connection;
        changed.host = "other.test".into();
        state
            .update_connection(changed)
            .expect("connection route remains valid");

        assert_eq!(
            plan.confirm(&state, token),
            Err(TunnelRuleReviewError::StateChanged)
        );
    }

    #[test]
    fn serde_rejects_secret_shaped_extra_fields() {
        let connection = Uuid::new_v4();
        let rule = local_rule(connection);
        let mut value = serde_json::to_value(rule).expect("rule serializes");
        value["password"] = serde_json::json!("never-store");
        assert!(serde_json::from_value::<TunnelRule>(value).is_err());
    }
}
