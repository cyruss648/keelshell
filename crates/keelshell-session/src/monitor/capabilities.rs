//! Connection-local observations, rather than assumptions about installed tools.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use super::{ExecOutput, MonitorError, MonitorResult};

/// One independently collected Linux monitoring facility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    /// Core CPU, memory, uptime and network counters from `/proc`.
    Resources,
    /// Mounted-filesystem capacity from `df -Pk`.
    Filesystems,
    /// Fixed process columns from `ps`.
    Processes,
    /// Listening TCP/UDP rows from `ss`.
    Sockets,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Resources => "Linux /proc resources",
            Self::Filesystems => "filesystem capacity",
            Self::Processes => "process listing",
            Self::Sockets => "listening sockets",
        })
    }
}

/// Evidence-backed reason why a monitoring facility could not be collected.
///
/// No raw remote diagnostic or command argument is retained in this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityReason {
    /// The executable was absent from the authenticated user's command search.
    CommandMissing,
    /// The executable explicitly rejected a required option.
    UnsupportedOptions,
    /// Execution or reading the requested data was explicitly denied.
    PermissionDenied,
    /// The command succeeded but its output did not satisfy the fixed parser.
    InvalidOutput,
    /// A command failed without evidence sufficient for a more specific cause.
    CommandFailed,
    /// The target did not identify itself as Linux.
    UnsupportedPlatform,
    /// SSH failed; this is transient and is not cached as a missing capability.
    Transport,
}

impl std::fmt::Display for CapabilityReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::CommandMissing => "required command is missing",
            Self::UnsupportedOptions => "required command options are unsupported",
            Self::PermissionDenied => "permission was denied",
            Self::InvalidOutput => "command output has an unsupported or invalid format",
            Self::CommandFailed => "command failed; the cause is unknown",
            Self::UnsupportedPlatform => "only Linux targets are supported",
            Self::Transport => "SSH collection did not complete",
        })
    }
}

/// Last observed capability state; an unknown state never means an empty result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CapabilityState {
    /// This facility has not been checked on this authenticated connection.
    #[default]
    Unknown,
    /// A successful response was validated by the corresponding parser.
    Available,
    /// The last attempt failed for the given typed reason.
    Unavailable(CapabilityReason),
}

/// One capability observation and its local response time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CapabilityObservation {
    /// Whether collection has been observed to work.
    pub state: CapabilityState,
    /// Receipt time of the attempt; absent until an actual check completes.
    pub checked_at: Option<Instant>,
}

/// Independent observations belonging to one `LinuxMonitor` and its SSH session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonitorCapabilities {
    /// Core `/proc` collection.
    pub resources: CapabilityObservation,
    /// Optional `df` collection.
    pub filesystems: CapabilityObservation,
    /// Process collection.
    pub processes: CapabilityObservation,
    /// Socket diagnostics, checked only on explicit request.
    pub sockets: CapabilityObservation,
}

impl MonitorCapabilities {
    /// Read an observation without starting any remote operation.
    pub fn observation(&self, capability: Capability) -> CapabilityObservation {
        match capability {
            Capability::Resources => self.resources,
            Capability::Filesystems => self.filesystems,
            Capability::Processes => self.processes,
            Capability::Sockets => self.sockets,
        }
    }

    fn observation_mut(&mut self, capability: Capability) -> &mut CapabilityObservation {
        match capability {
            Capability::Resources => &mut self.resources,
            Capability::Filesystems => &mut self.filesystems,
            Capability::Processes => &mut self.processes,
            Capability::Sockets => &mut self.sockets,
        }
    }
}

/// Immutable cached observations with proof of their connection and revision.
///
/// Observation times remain the times of actual checks. A current report can
/// therefore prove an older negative cache entry without pretending to recheck
/// the failed command. Use [`super::LinuxMonitor::accepts_capability_report`]
/// before adopting it across an asynchronous boundary.
#[derive(Clone)]
pub struct CapabilityReport {
    observations: MonitorCapabilities,
    source: Arc<CapabilityCache>,
    generation: u64,
    revision: u64,
}

impl CapabilityReport {
    /// Read the original observations without starting any collection.
    pub fn observations(&self) -> MonitorCapabilities {
        self.observations
    }

    pub(super) fn is_current_for(&self, cache: &Arc<CapabilityCache>) -> bool {
        Arc::ptr_eq(&self.source, cache)
            && self.generation == cache.generation.load(Ordering::Acquire)
            && self.revision == cache.revision.load(Ordering::Acquire)
    }
}

#[derive(Default)]
pub(super) struct CapabilityCache {
    generation: AtomicU64,
    revision: AtomicU64,
    observations: tokio::sync::Mutex<MonitorCapabilities>,
}

impl CapabilityCache {
    pub fn invalidate_pending(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    pub async fn reset(&self) {
        let mut observations = self.observations.lock().await;
        self.invalidate_pending();
        *observations = MonitorCapabilities::default();
    }

    pub async fn report(&self) -> MonitorCapabilities {
        *self.observations.lock().await
    }

    pub async fn authenticated_report(self: &Arc<Self>) -> CapabilityReport {
        let observations = self.observations.lock().await;
        CapabilityReport {
            observations: *observations,
            source: self.clone(),
            generation: self.generation.load(Ordering::Acquire),
            revision: self.revision.load(Ordering::Acquire),
        }
    }

    pub async fn begin(&self, capability: Capability) -> (u64, CapabilityState) {
        let observations = self.observations.lock().await;
        (
            self.generation.load(Ordering::Acquire),
            observations.observation(capability).state,
        )
    }

    pub async fn record(&self, generation: u64, capability: Capability, state: CapabilityState) {
        let mut observations = self.observations.lock().await;
        if generation == self.generation.load(Ordering::Acquire) {
            // Revoke reports before changing their bytes. The second generation
            // check handles a UI invalidation racing this short critical section.
            self.revision.fetch_add(1, Ordering::AcqRel);
            let previous = observations.observation(capability);
            *observations.observation_mut(capability) = CapabilityObservation {
                state,
                checked_at: Some(Instant::now()),
            };
            if generation != self.generation.load(Ordering::Acquire) {
                *observations.observation_mut(capability) = previous;
            }
        }
    }
}

pub(super) fn cached_reason(state: CapabilityState) -> Option<CapabilityReason> {
    match state {
        CapabilityState::Unavailable(reason) if reason != CapabilityReason::Transport => {
            Some(reason)
        }
        _ => None,
    }
}

pub(super) fn error_state(error: &MonitorError) -> CapabilityState {
    CapabilityState::Unavailable(match error {
        MonitorError::CapabilityUnavailable { reason, .. } => *reason,
        MonitorError::Session(_) => CapabilityReason::Transport,
        MonitorError::Unsupported(_) => CapabilityReason::UnsupportedPlatform,
        MonitorError::InvalidData(_) => CapabilityReason::InvalidOutput,
        _ => CapabilityReason::CommandFailed,
    })
}

pub(super) fn failure_reason(status: u32, stderr: &[u8]) -> CapabilityReason {
    if status == 127 {
        return CapabilityReason::CommandMissing;
    }
    let diagnostic = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    if status == 126
        || diagnostic.contains("permission denied")
        || diagnostic.contains("operation not permitted")
    {
        CapabilityReason::PermissionDenied
    } else if [
        "invalid option",
        "unrecognized option",
        "unknown option",
        "illegal option",
        "unsupported option",
    ]
    .iter()
    .any(|message| diagnostic.contains(message))
    {
        CapabilityReason::UnsupportedOptions
    } else {
        CapabilityReason::CommandFailed
    }
}

pub(super) fn command_text(output: ExecOutput, capability: Capability) -> MonitorResult<String> {
    match output.exit_status {
        Some(0 | 64) | None => super::checked_text(output, "monitoring collection"),
        Some(66) => Err(MonitorError::CapabilityUnavailable {
            capability,
            reason: CapabilityReason::CommandMissing,
        }),
        Some(status) => Err(MonitorError::CapabilityUnavailable {
            capability,
            reason: failure_reason(status, &output.stderr),
        }),
    }
}

#[cfg(test)]
mod tests;
