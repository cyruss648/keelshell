//! Reuse process-wide file exclusion without opening an unrelated SFTP channel.

use super::queue::{Admission, Reservation, ResourceClaims, local_claim};
use super::*;

/// A receive operation owns both its new destination and private staging name.
/// Dropping an unconfirmed ticket retains the same quarantine as SFTP writes.
pub(crate) struct ZmodemLocalReservation(Reservation);

impl ZmodemLocalReservation {
    pub(crate) async fn acquire(session: &SshSession, paths: &[PathBuf]) -> Result<Self> {
        if session.is_closed() || paths.len() != 2 {
            return Err(SessionError::Invalid(
                "Zmodem requires two live local claims",
            ));
        }
        let mut claims = ResourceClaims::default();
        for path in paths {
            claims.local.push(local_claim(path, true).await?);
        }
        if session.is_closed() {
            return Err(SessionError::Closed);
        }
        match TransferReservations::admit_resources(&session.transfer_reservations, &claims) {
            Admission::Ready(ticket) => Ok(Self(ticket)),
            Admission::Busy => Err(SessionError::MutationBusy),
            Admission::Quarantined => Err(SessionError::MutationQuarantined),
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.0.is_active()
    }

    pub(crate) fn finish(self, known: bool) {
        self.0.finish(known);
    }
}

/// Inspect the vnode attached to the retained descriptor. In particular, macOS
/// `ls -e /dev/fd/N` does not inspect that vnode's ACL and cannot prove privacy.
#[cfg(target_os = "macos")]
pub(crate) fn private_file_acl(file: &std::fs::File, clear: bool) -> Result<()> {
    use std::os::fd::AsFd;

    if clear {
        calcifer_macos_acl::clear_acl(file.as_fd())?;
    }
    if !calcifer_macos_acl::read_acl(file.as_fd())?.is_empty() {
        return Err(SessionError::Invalid(
            "local staging file retains an extended ACL",
        ));
    }
    Ok(())
}
