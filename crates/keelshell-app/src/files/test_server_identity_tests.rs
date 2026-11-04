//! Real SSH/SFTP identity regression; the listener never leaves this test's ownership.
use super::{Checked, Server};
use keelshell_session::{SessionError, sftp::SftpSession};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::{net::TcpListener, runtime::Runtime, sync::oneshot};
use uuid::Uuid;

impl Server {
    fn handoff_listener(&mut self, runtime: &Runtime) -> TcpListener {
        let (reply, listener) = oneshot::channel();
        self.listener_handoff
            .take()
            .unwrap_or_else(|| panic!("fixture listener already handed off"))
            .send(reply)
            .checked("request owned listener handoff");
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), listener)
                .await
                .checked("bounded fixture listener handoff")
                .checked("receive still-bound fixture listener")
        })
    }

    fn join_clients(&mut self, runtime: &Runtime) {
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), &mut self.task)
                .await
                .checked("bounded fixture client drain")
                .checked("join fixture acceptor and SSH clients");
            tokio::time::timeout(Duration::from_secs(5), async {
                while self.active.load(Ordering::Acquire) != 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .checked("bounded fixture subsystem drain");
        });
        assert_eq!(self.filesystem.active_directory_handles(), 0);
    }
}

// Unknown records are process-wide and capacity-limited. Even a failed assertion
// must release only this test's UUID-named target/namespace after inspecting it.
struct OwnQuarantine<'a> {
    runtime: &'a Runtime,
    sftp: &'a SftpSession,
    path: &'a str,
    owned_prefix: String,
    cleared: bool,
}

impl<'a> OwnQuarantine<'a> {
    fn file(runtime: &'a Runtime, sftp: &'a SftpSession, path: &'a str) -> Self {
        let Some((parent, name)) = path.rsplit_once('/') else {
            panic!("owned fixture file must be absolute");
        };
        Self {
            runtime,
            sftp,
            path,
            owned_prefix: format!("{parent}/.{name}.keelshell-"),
            cleared: false,
        }
    }

    fn namespace(runtime: &'a Runtime, sftp: &'a SftpSession, path: &'a str) -> Self {
        Self {
            runtime,
            sftp,
            path,
            owned_prefix: format!("{path}/"),
            cleared: false,
        }
    }

    fn clear(&mut self) -> Result<usize, SessionError> {
        let count = self.runtime.block_on(async {
            let review = match self
                .sftp
                .inspect_remote_mutation_quarantine(self.path)
                .await
            {
                Ok(review) => review,
                Err(SessionError::Invalid("no matching unknown transfer destination")) => {
                    return Ok(0);
                }
                Err(error) => return Err(error),
            };
            if !review.entries().iter().all(|entry| {
                !entry.local
                    && (entry.destination == self.path
                        || entry.destination.starts_with(&self.owned_prefix))
            }) {
                return Err(SessionError::Invalid(
                    "refuse to clear another test's quarantine",
                ));
            }
            self.sftp
                .acknowledge_transfer_quarantine(&review, &AtomicBool::new(false))
                .await?;
            match self
                .sftp
                .inspect_remote_mutation_quarantine(self.path)
                .await
            {
                Err(SessionError::Invalid("no matching unknown transfer destination")) => {
                    Ok(review.entries().len())
                }
                _ => Err(SessionError::Invalid(
                    "owned quarantine cleanup was not observed",
                )),
            }
        })?;
        self.cleared = true;
        Ok(count)
    }
}

impl Drop for OwnQuarantine<'_> {
    fn drop(&mut self) {
        if !self.cleared
            && let Err(error) = self.clear()
        {
            if std::thread::panicking() {
                eprintln!("owned fixture quarantine cleanup failed during unwind: {error:?}");
            } else {
                panic!("owned fixture quarantine cleanup failed: {error:?}");
            }
        }
    }
}

#[test]
fn independent_filesystem_on_reused_endpoint_does_not_inherit_quarantine() {
    // A dedicated current-thread runtime owns every protocol task, including
    // russh's inner tasks, and drops them on unwind. No child process is spawned.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .checked("identity regression runtime");
    let mut original = Server::new(&runtime);
    let original_fingerprint = original.fingerprint.clone();
    let first_session = original.connect(&runtime);
    let inspector = runtime
        .block_on(first_session.sftp())
        .checked("original fixture inspector");
    let basename = format!("identity-{}", Uuid::new_v4());
    let path = format!("/{basename}");
    let original_bytes = b"original target must remain unchanged";
    // Setup is real mutation too: own its unknown outcome before dispatch.
    let mut cleanup = OwnQuarantine::file(&runtime, &inspector, &path);
    runtime.block_on(async {
        inspector
            .write(&path, original_bytes)
            .await
            .checked("seed original target over real SFTP");
        assert!(matches!(
            inspector.inspect_remote_mutation_quarantine(&path).await,
            Err(SessionError::Invalid(
                "no matching unknown transfer destination"
            ))
        ));
    });
    runtime.block_on(async {
        let writer = first_session.sftp().await.checked("atomic mutation subsystem");
        let held = original
            .filesystem
            .hold_atomic_writes_after_first()
            .checked("hold actual second atomic WRITE");
        let bytes = vec![b'x'; 128 * 1024];
        let mut mutation = Box::pin(writer.write_atomic(&path, &bytes));
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::select! {
                result = &mut mutation => panic!("mutation completed before held WRITE: {result:?}"),
                _ = async {
                    while held.entered() == 0 {
                        tokio::task::yield_now().await;
                    }
                } => {}
            }
        })
        .await
        .checked("observe dispatched SFTP WRITE before cancellation");
        assert!(!held.expired());
        drop(mutation);
        held.release();
        writer.close().await.checked("join cancelled atomic temporary cleanup");
    });
    let writes_before_rejections = original.filesystem.transfer_writes_started();
    let before = runtime
        .block_on(inspector.inspect_remote_mutation_quarantine(&path))
        .checked("inspect own actual unknown mutation");
    assert!(!before.entries().is_empty());
    eprintln!(
        "identity regression: endpoint={} fingerprint={} quarantine_ids={:?} writes={writes_before_rejections}",
        original.address,
        original.fingerprint,
        before
            .entries()
            .iter()
            .map(|entry| entry.reservation_id)
            .collect::<Vec<_>>(),
    );

    // A distinct authenticated connection to the same Server must retain the
    // exact pinned key and quarantine, independently of SshSession clone identity.
    let reconnect = original.connect(&runtime);
    assert_eq!(original.fingerprint, original_fingerprint);
    let reconnected = runtime
        .block_on(reconnect.sftp())
        .checked("same fixture reconnected SFTP");
    runtime.block_on(async {
        let rejected = reconnected.write_atomic(&path, b"must stay blocked").await;
        assert!(
            matches!(rejected, Err(SessionError::MutationQuarantined)),
            "{rejected:?}"
        );
        assert_eq!(
            reconnected
                .inspect_remote_mutation_quarantine(&path)
                .await
                .checked("reconnect retains quarantine")
                .entries(),
            before.entries(),
        );
    });
    eprintln!("identity regression: same-server reconnect rejected with original quarantine IDs");

    let listener = original.handoff_listener(&runtime);
    let mut independent = Server::from_listener(&runtime, listener);
    assert_eq!(independent.address, original.address);
    let independent_session = independent.connect(&runtime);
    let fresh = runtime
        .block_on(independent_session.sftp())
        .checked("independent filesystem real SFTP");
    let fresh_root = format!("/approved-{}", Uuid::new_v4());
    let fresh_path = format!("{fresh_root}/中文.txt");
    // Include mkdir and its descendants before either setup request is sent.
    let mut fresh_cleanup = OwnQuarantine::namespace(&runtime, &fresh, &fresh_root);
    let seed_result = runtime.block_on(async {
        assert!(
            fresh
                .list("/")
                .await
                .checked("fresh filesystem is empty")
                .is_empty()
        );
        fresh
            .mkdir(&fresh_root)
            .await
            .checked("fresh namespace creation");
        // In-place writes conservatively claim the whole remote side. The
        // previous quarantine is deliberately at a different pathname.
        fresh.write(&fresh_path, "受控中文\n".as_bytes()).await
    });
    eprintln!(
        "identity regression: reused_endpoint={} new_fingerprint={} independent_seed={seed_result:?}",
        independent.address, independent.fingerprint,
    );
    runtime.block_on(async {
        if seed_result.is_ok() {
            assert_eq!(
                fresh
                    .read_regular(&fresh_path, 1024)
                    .await
                    .checked("read complete independent seed"),
                "受控中文\n".as_bytes(),
            );
        }
        let rejected = inspector
            .write_atomic(&path, b"old scope still blocked")
            .await;
        assert!(
            matches!(rejected, Err(SessionError::MutationQuarantined)),
            "{rejected:?}"
        );
        assert_eq!(
            inspector
                .inspect_remote_mutation_quarantine(&path)
                .await
                .checked("old quarantine survives new filesystem")
                .entries(),
            before.entries(),
        );
        assert_eq!(
            inspector
                .read_regular(&path, 1024)
                .await
                .checked("read original target"),
            original_bytes,
        );
        assert_eq!(
            original.filesystem.transfer_writes_started(),
            writes_before_rejections
        );
    });
    eprintln!(
        "identity regression: old scope still rejected; exact quarantine IDs and original bytes unchanged"
    );

    let fresh_cleared = fresh_cleanup
        .clear()
        .checked("acknowledge only independent setup quarantine");
    drop(fresh_cleanup);
    let cleared = cleanup
        .clear()
        .checked("acknowledge only owned quarantine after proof");
    assert_eq!(cleared, before.entries().len());
    drop(cleanup);
    runtime.block_on(async {
        reconnected.close().await.checked("close reconnected SFTP");
        inspector.close().await.checked("close original inspector");
        fresh.close().await.checked("close independent SFTP");
        reconnect.close().await.checked("disconnect reconnect");
        first_session
            .close()
            .await
            .checked("disconnect original SSH");
        independent_session
            .close()
            .await
            .checked("disconnect independent SSH");
    });
    let listener = independent.handoff_listener(&runtime);
    original.join_clients(&runtime);
    independent.join_clients(&runtime);
    drop(listener);
    eprintln!(
        "identity regression: acknowledged {cleared} original and {fresh_cleared} independent owned records; both servers joined; active SFTP subsystems and directory handles zero"
    );

    // Defer the expected old-key failure until owned quarantine and protocol
    // cleanup have completed, so this regression never consumes suite capacity.
    assert!(
        seed_result.is_ok(),
        "independent filesystem inherited unrelated quarantine: {seed_result:?}"
    );
    assert_ne!(original.fingerprint, independent.fingerprint);
}

#[test]
fn file_setup_unwind_cleans_owned_quarantine_and_refuses_foreign_entries() {
    setup_write_unwind(false);
}

#[test]
fn namespace_setup_unwind_cleans_owned_quarantine_and_refuses_foreign_entries() {
    setup_write_unwind(true);
}

fn setup_write_unwind(namespace: bool) {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .checked("setup unwind runtime");
    let mut server = Server::new(&runtime);
    let session = server.connect(&runtime);
    let inspector = runtime
        .block_on(session.sftp())
        .checked("setup unwind inspector");
    let root = format!("/setup-{}", Uuid::new_v4());
    let path = if namespace {
        format!("{root}/中文.txt")
    } else {
        root.clone()
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _cleanup = if namespace {
            OwnQuarantine::namespace(&runtime, &inspector, &root)
        } else {
            OwnQuarantine::file(&runtime, &inspector, &root)
        };
        runtime.block_on(async {
            if namespace {
                inspector
                    .mkdir(&root)
                    .await
                    .checked("owned setup directory");
            }
            let writer = session.sftp().await.checked("setup mutation channel");
            let held = server
                .filesystem
                .hold_transfer_writes_after_first(&path)
                .checked("hold actual second setup WRITE");
            let data = vec![b's'; 128 * 1024];
            let mut mutation = Box::pin(writer.write(&path, &data));
            tokio::time::timeout(Duration::from_secs(5), async {
                tokio::select! {
                    result = &mut mutation => panic!("setup completed before hold: {result:?}"),
                    _ = async {
                        while held.entered() == 0 {
                            tokio::task::yield_now().await;
                        }
                    } => {}
                }
            })
            .await
            .checked("observe dispatched setup WRITE");
            assert!(!held.expired());
            drop(mutation);
            held.release();
            tokio::time::timeout(Duration::from_secs(5), async {
                while server.filesystem.prepared_successful_write_statuses() != 2 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .checked("observe held setup handler finish before inspection");
            writer.close().await.checked("close cancelled setup writer");
            assert_eq!(
                inspector
                    .inspect_remote_mutation_quarantine(&root)
                    .await
                    .checked("actual unknown setup result")
                    .entries()
                    .len(),
                1
            );
        });
        let foreign_path = format!("/unowned-{}", Uuid::new_v4());
        let mut foreign = OwnQuarantine::file(&runtime, &inspector, &foreign_path);
        let before = runtime
            .block_on(inspector.inspect_remote_mutation_quarantine(&root))
            .checked("capture exact owned setup record");
        assert!(matches!(
            foreign.clear(),
            Err(SessionError::Invalid(
                "refuse to clear another test's quarantine"
            ))
        ));
        // This deliberately unrelated guard dispatched no mutation. Its
        // failed inspection must leave the actual owner's record intact.
        foreign.cleared = true;
        assert_eq!(
            runtime
                .block_on(inspector.inspect_remote_mutation_quarantine(&root))
                .checked("foreign cleanup left owned record intact")
                .entries(),
            before.entries()
        );
        panic!("owned setup unwind witness");
    }));
    assert_eq!(
        outcome
            .as_ref()
            .err()
            .and_then(|payload| payload.downcast_ref::<&str>())
            .copied(),
        Some("owned setup unwind witness"),
        "the expected panic must occur only after actual quarantine and foreign refusal"
    );
    let cleared_on_unwind = matches!(
        runtime.block_on(inspector.inspect_remote_mutation_quarantine(&root)),
        Err(SessionError::Invalid(
            "no matching unknown transfer destination"
        ))
    );
    // A cleanup mutation test may intentionally disable Drop. Recover only
    // these owned records before asserting, keeping the negative control isolated.
    if !cleared_on_unwind {
        let mut recovery = if namespace {
            OwnQuarantine::namespace(&runtime, &inspector, &root)
        } else {
            OwnQuarantine::file(&runtime, &inspector, &root)
        };
        recovery
            .clear()
            .checked("recover only failed witness quarantine");
    }
    runtime.block_on(async {
        inspector.close().await.checked("close setup inspector");
        session.close().await.checked("disconnect setup fixture");
    });
    let listener = server.handoff_listener(&runtime);
    server.join_clients(&runtime);
    drop(listener);
    assert!(cleared_on_unwind, "setup quarantine survived guard unwind");
}
