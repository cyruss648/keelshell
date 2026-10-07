//! A captured terminal lifetime augments, but never replaces, desktop consent.
use keelshell_mcp::{AuthorizationLease, McpFailure};
use tokio::sync::watch;

use crate::terminal::TransportState;

pub(crate) type LifecycleSource = Option<watch::Receiver<TransportState>>;

#[derive(Clone)]
pub(crate) struct SessionAuthorization {
    lease: AuthorizationLease,
    lifecycle: LifecycleSource,
}

impl SessionAuthorization {
    pub(crate) fn new(lease: AuthorizationLease, lifecycle: LifecycleSource) -> Self {
        Self { lease, lifecycle }
    }

    pub(crate) fn check(&self) -> Result<(), McpFailure> {
        self.lease.check()?;
        if lifecycle_current(self.lifecycle.as_ref()) {
            Ok(())
        } else {
            Err(McpFailure::StaleSession)
        }
    }

    pub(crate) async fn revoked(&self) {
        tokio::select! {
            biased;
            _ = self.lease.revoked() => {},
            _ = lifecycle_lost(self.lifecycle.clone()) => {},
        }
    }
}

pub(crate) fn lifecycle_current(source: Option<&watch::Receiver<TransportState>>) -> bool {
    // Production SSH bridges attach a sticky typed lifecycle. Byte-only owned
    // transports have no such producer; their UI owner still revokes the lease.
    source.is_none_or(|source| {
        source.has_changed().is_ok() && matches!(*source.borrow(), TransportState::Ready)
    })
}

pub(crate) async fn lifecycle_lost(source: LifecycleSource) {
    let Some(mut source) = source else {
        std::future::pending::<()>().await;
        return;
    };
    while lifecycle_current(Some(&source)) {
        if source.changed().await.is_err() {
            break;
        }
    }
}
