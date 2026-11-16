#![cfg(test)]
#![cfg(windows)]

use std::time::Duration;

use super::*;
use crate::sftp::queue;

const WAIT: Duration = Duration::from_secs(15);

enum ExtractionStage {
    Build,
    Publish,
}

fn extraction_stage_error(stage: ExtractionStage, error: &SessionError) {
    let stage = match stage {
        ExtractionStage::Build => "build-stage",
        ExtractionStage::Publish => "publish-stage",
    };
    // Never format the error or its payload: even an I/O message may contain a
    // dynamic path. inspect_err below leaves the original typed error intact.
    let typed_error = match error {
        SessionError::Io(_) => "Io",
        SessionError::Ssh(_) => "Ssh",
        SessionError::Proxy(_) => "Proxy",
        SessionError::UnknownHostKey { .. } => "UnknownHostKey",
        SessionError::ChangedHostKey { .. } => "ChangedHostKey",
        SessionError::Authentication => "Authentication",
        SessionError::Credential(_) => "Credential",
        SessionError::Timeout(_) => "Timeout",
        SessionError::Closed => "Closed",
        SessionError::Invalid(_) => "Invalid",
        SessionError::Worker => "Worker",
        SessionError::Rejected(_) => "Rejected",
        SessionError::Sftp(_) => "Sftp",
        SessionError::Unsupported(_) => "Unsupported",
        SessionError::OutputLimit(_) => "OutputLimit",
        SessionError::EntryLimit(_) => "EntryLimit",
        SessionError::MutationBusy => "MutationBusy",
        SessionError::MutationQuarantined => "MutationQuarantined",
        SessionError::MutationUncertain => "MutationUncertain",
        SessionError::UnverifiedMutation(_) => "UnverifiedMutation",
    };
    let (io_kind, raw_os_error) = match error {
        SessionError::Io(error) => (Some(error.kind()), error.raw_os_error()),
        _ => (None, None),
    };
    eprintln!(
        "windows-archive-extraction stage={stage} typed_error=SessionError::{typed_error} \
         io_kind={io_kind:?} raw_os_error={raw_os_error:?}"
    );
}

async fn native<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::time::timeout(WAIT, tokio::task::spawn_blocking(operation))
        .await
        .map_err(|_| SessionError::Timeout("Windows archive native test"))?
        .map_err(|_| SessionError::Worker)?
}

struct Fixture {
    root: PathBuf,
    destination: PathBuf,
    staging: PathBuf,
    reviewed: Manifest,
    // Close retained review capabilities before attempting temporary cleanup.
    _temporary: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Result<Self> {
        let temporary = tempfile::tempdir()?;
        let root = checked_path(&temporary.path().canonicalize()?)?;
        let destination = root.join("published");
        let staging = staging_path(&destination)?;
        let reviewed = destination_blocking(destination.clone(), &AtomicBool::new(false))?;
        Ok(Self {
            root,
            destination,
            staging,
            reviewed,
            _temporary: temporary,
        })
    }
}

struct Lease {
    owner: Arc<queue::TransferReservations>,
    claims: queue::ResourceClaims,
    ticket: queue::Reservation,
}

impl Lease {
    async fn new(fixture: &Fixture) -> Result<Self> {
        let owner = Arc::new(queue::TransferReservations::new(&crate::SshOptions::new(
            format!("archive-{}.invalid", uuid::Uuid::new_v4()),
            "isolated-fixture",
        )));
        let mut claims = queue::ResourceClaims::default();
        for path in [&fixture.destination, &fixture.staging] {
            let mut claim = queue::local_claim(path, true).await?;
            claim.whole_side = false;
            claims.local.push(claim);
        }
        let ticket = match queue::TransferReservations::admit_resources(&owner, &claims) {
            queue::Admission::Ready(ticket) => ticket,
            _ => return Err(SessionError::MutationBusy),
        };
        Ok(Self {
            owner,
            claims,
            ticket,
        })
    }

    fn worker(&self) -> MutationWorker {
        MutationWorker {
            ticket: self.ticket.clone(),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    fn quarantined(&self) -> bool {
        matches!(
            queue::TransferReservations::admit_resources(&self.owner, &self.claims),
            queue::Admission::Quarantined
        )
    }
}

fn members() -> Result<Vec<ArchiveMember>> {
    Ok(vec![
        ArchiveMember::new_directory("空目录").map_err(|_| invalid_archive())?,
        ArchiveMember::new_file("资料/子目录/🦀.txt", b"reviewed\0bytes\r\n".to_vec())
            .map_err(|_| invalid_archive())?,
        ArchiveMember::new_file("zero.bin", Vec::new()).map_err(|_| invalid_archive())?,
    ])
}

#[test]
fn windows_archive_slash_members_join_literal_windows_components() -> Result<()> {
    let target = member_target(Path::new(r"C:\reviewed"), "资料/子目录/🦀.txt")?;
    assert_eq!(target, PathBuf::from(r"C:\reviewed\资料\子目录\🦀.txt"));
    Ok(())
}

#[test]
fn windows_archive_rejects_device_stream_traversal_and_direction_marker_components() -> Result<()> {
    for name in [
        "CON.txt",
        "COM¹.log",
        "stream:data",
        "trailing.",
        "a\\b",
        "a/\u{202e}name",
        "../escape",
        "a//b",
    ] {
        assert!(
            member_target(Path::new(r"C:\reviewed"), name).is_err(),
            "{name}"
        );
    }
    assert!(member_target(Path::new(r"C:\reviewed"), &"界".repeat(256)).is_err());
    Ok(())
}

#[test]
fn windows_archive_rejects_unc_device_namespace_and_non_normalized_drive_paths() {
    for path in [
        r"\\host\share\file",
        r"\\?\UNC\host\share\file",
        r"\\.\C:\file",
        r"C:relative",
        r"C:\a\..\file",
        "C:/file",
        r"C:\a\\file",
    ] {
        assert!(checked_path(Path::new(path)).is_err(), "{path}");
    }
}

#[test]
fn windows_archive_selected_extended_drive_and_unicode_case_aliases_overlap() {
    for sources in [
        vec![
            PathBuf::from(r"C:\data\file"),
            PathBuf::from(r"\\?\c:\data\file"),
        ],
        vec![
            PathBuf::from(r"C:\Straße"),
            PathBuf::from(r"C:\STRASSE\child"),
        ],
        vec![PathBuf::from(r"C:\Σ"), PathBuf::from(r"C:\ς\child")],
    ] {
        assert!(selected_sources(&sources).is_err());
    }
}

#[test]
fn windows_archive_implicit_directory_aliases_and_file_parents_are_rejected() -> Result<()> {
    for paths in [
        vec![("Straße/a", false), ("STRASSE/b", false)],
        vec![("parent", false), ("parent/child", false)],
        vec![("same", true), ("same", true)],
    ] {
        let members: Vec<_> = paths
            .into_iter()
            .map(|(path, directory)| {
                if directory {
                    ArchiveMember::new_directory(path)
                } else {
                    ArchiveMember::new_file(path, Vec::new())
                }
            })
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| invalid_archive())?;
        assert!(validate_destinations(Path::new(r"C:\target"), &members).is_err());
    }
    Ok(())
}

#[test]
fn windows_archive_implicit_directory_inventory_has_its_own_entry_bound() -> Result<()> {
    let members = (0..MAX_ENTRIES / 2 + 1)
        .map(|index| ArchiveMember::new_file(format!("dir-{index}/file"), Vec::new()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| invalid_archive())?;
    assert!(matches!(
        validate_destinations(Path::new(r"C:\target"), &members),
        Err(SessionError::EntryLimit(MAX_ENTRIES))
    ));
    Ok(())
}

#[test]
fn windows_archive_destinations_enforce_combined_absolute_path_bound() -> Result<()> {
    let root = PathBuf::from(format!("C:\\{}", vec!["a".repeat(200); 19].join("\\")));
    let member = ArchiveMember::new_file(
        format!("{}/{}", "b".repeat(200), "c".repeat(200)),
        Vec::new(),
    )
    .map_err(|_| invalid_archive())?;
    assert!(validate_destinations(&root, &[member]).is_err());
    Ok(())
}

#[test]
fn windows_archive_member_depth_is_bounded_before_any_native_io() {
    let accepted = vec!["part"; MAX_DEPTH].join("/");
    let rejected = vec!["part"; MAX_DEPTH + 1].join("/");
    assert!(member_target(Path::new(r"C:\target"), &accepted).is_ok());
    assert!(member_target(Path::new(r"C:\target"), &rejected).is_err());
}

#[test]
fn windows_archive_cancel_guard_marks_its_worker_on_drop() {
    let stop = Arc::new(AtomicBool::new(false));
    drop(CancelOnDrop(stop.clone()));
    assert!(matches!(stopped(&stop), Err(SessionError::Closed)));
}

#[tokio::test]
async fn windows_archive_fresh_scan_manifest_equality_ignores_handle_values() -> Result<()> {
    native(|| {
        let fixture = Fixture::new()?;
        let source = fixture.root.join("资料");
        std::fs::create_dir(&source)?;
        std::fs::create_dir(source.join("空目录"))?;
        std::fs::write(source.join("🦀.txt"), b"reviewed\0bytes\r\n")?;
        let stop = AtomicBool::new(false);
        let (first, first_members) = scan_blocking(vec![source.clone()], &stop)?;
        let (second, second_members) = scan_blocking(vec![source], &stop)?;
        assert_eq!(first, second);
        assert_eq!(first_members, second_members);
        let payload = first_members
            .iter()
            .find(|member| member.kind() == ArchiveEntryKind::File)
            .ok_or_else(changed)?;
        assert_eq!(payload.bytes(), b"reviewed\0bytes\r\n");
        let digest: [u8; 32] = Sha256::digest(payload.bytes()).into();
        assert_eq!(
            first.entries.iter().find_map(|entry| entry.digest),
            Some(digest)
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_manifest_clone_retains_every_original_ancestor_capability() -> Result<()> {
    native(|| {
        let fixture = Fixture::new()?;
        let outer = fixture.root.join("outer");
        let inner = outer.join("inner");
        std::fs::create_dir_all(&inner)?;
        let source = inner.join("file");
        std::fs::write(&source, b"original")?;
        let stop = AtomicBool::new(false);
        let (manifest, members) = scan_blocking(vec![source.clone()], &stop)?;
        let retained = manifest.clone();
        assert_eq!(
            retained.ancestors.len(),
            source.parent().ok_or_else(changed)?.ancestors().count()
        );
        drop(manifest);
        let moved = fixture.root.join("moved");
        std::fs::rename(&outer, &moved)?;
        std::fs::create_dir_all(&inner)?;
        std::fs::write(&source, b"replacement")?;
        std::fs::write(moved.join("retained-only"), b"evidence")?;
        assert!(
            retained
                .ancestors
                .get(&outer)
                .ok_or_else(changed)?
                .held
                .entries()?
                .iter()
                .any(|name| name == "retained-only")
        );
        assert!(verify_sources(&retained, &members, &stop).is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_scan_rechecks_children_even_when_directory_stamps_match() -> Result<()> {
    native(|| {
        let fixture = Fixture::new()?;
        let source = fixture.root.join("source");
        std::fs::create_dir(&source)?;
        std::fs::write(source.join("reviewed"), b"reviewed")?;
        let stop = AtomicBool::new(false);
        let (mut manifest, members) = scan_blocking(vec![source.clone()], &stop)?;
        std::fs::write(source.join("unexpected"), b"new child")?;
        // Isolate the inventory invariant from timestamps, including filesystems
        // with coarse timestamps or a caller able to restore directory times.
        for entry in &mut manifest.entries {
            if entry.stamp.directory {
                entry.stamp = metadata(entry.held.as_file())?;
            }
        }
        assert!(verify_sources(&manifest, &members, &stop).is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_scan_rechecks_digest_and_bytes_even_when_file_stamps_match() -> Result<()>
{
    native(|| {
        let fixture = Fixture::new()?;
        let source = fixture.root.join("file");
        std::fs::write(&source, b"reviewed")?;
        let stop = AtomicBool::new(false);
        let (mut manifest, members) = scan_blocking(vec![source.clone()], &stop)?;
        std::fs::write(&source, b"tampered")?;
        for entry in &mut manifest.entries {
            entry.stamp = metadata(entry.held.as_file())?;
        }
        assert!(verify_sources(&manifest, &members, &stop).is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_scan_rejects_hard_linked_sources() -> Result<()> {
    native(|| {
        let fixture = Fixture::new()?;
        let source = fixture.root.join("original");
        std::fs::write(&source, b"reviewed")?;
        std::fs::hard_link(&source, fixture.root.join("alias"))?;
        assert!(scan_blocking(vec![source], &AtomicBool::new(false)).is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires Windows Developer Mode or symbolic-link privilege"]
async fn windows_archive_scan_rejects_source_and_ancestor_reparse_points() -> Result<()> {
    native(|| {
        let fixture = Fixture::new()?;
        let real = fixture.root.join("real");
        std::fs::create_dir(&real)?;
        std::fs::write(real.join("file"), b"reviewed")?;
        let link = fixture.root.join("link");
        std::os::windows::fs::symlink_dir(&real, &link)?;
        let stop = AtomicBool::new(false);
        assert!(scan_blocking(vec![link.clone()], &stop).is_err());
        assert!(scan_blocking(vec![link.join("file")], &stop).is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_destination_observations_ignore_sibling_write_timestamps() -> Result<()> {
    native(|| {
        let fixture = Fixture::new()?;
        std::fs::write(fixture.root.join("sibling"), b"unrelated")?;
        let next = destination_blocking(fixture.destination.clone(), &AtomicBool::new(false))?;
        assert_eq!(fixture.reviewed, next);
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_destination_refuses_existing_unicode_case_alias() -> Result<()> {
    native(|| {
        let fixture = Fixture::new()?;
        std::fs::write(fixture.root.join("Straße"), b"existing owner")?;
        assert!(
            destination_blocking(fixture.root.join("STRASSE"), &AtomicBool::new(false)).is_err()
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_private_nested_tree_publishes_after_child_creation_handles_close()
-> Result<()> {
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    native(move || {
        let members = members()?;
        let worker = lease.worker();
        let stage = build_stage(
            &fixture.reviewed,
            &members,
            &fixture.destination,
            &fixture.staging,
            &worker,
        )
        .inspect_err(|error| extraction_stage_error(ExtractionStage::Build, error))?;
        let identity = metadata(stage.entry.as_file())?.identity;
        for entry in stage.tree.entries.values() {
            verify_private(entry.held.as_file(), entry.stamp.directory)?;
        }
        let mut unverified = publish_stage(
            stage,
            &fixture.reviewed,
            &members,
            &fixture.destination,
            &fixture.staging,
            &worker,
        )
        .inspect_err(|error| extraction_stage_error(ExtractionStage::Publish, error))?;
        assert_eq!(
            std::fs::read(
                fixture
                    .destination
                    .join("资料")
                    .join("子目录")
                    .join("🦀.txt")
            )?,
            b"reviewed\0bytes\r\n"
        );
        assert!(fixture.destination.join("空目录").is_dir());
        assert!(!fixture.staging.exists());
        let parent = verify_ancestors(&fixture.reviewed.ancestors, &worker.stop)?;
        let published = parent_in(&parent, &fixture.destination)?
            .open_directory(basename(&fixture.destination)?)?;
        assert_eq!(metadata(published.as_file())?.identity, identity);
        unverified.ticket = None;
        lease.ticket.clone().finish(true);
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_empty_private_tree_publishes_with_exact_root_identity() -> Result<()> {
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    native(move || {
        let worker = lease.worker();
        let stage = build_stage(
            &fixture.reviewed,
            &[],
            &fixture.destination,
            &fixture.staging,
            &worker,
        )?;
        let identity = metadata(stage.entry.as_file())?.identity;
        let mut unverified = publish_stage(
            stage,
            &fixture.reviewed,
            &[],
            &fixture.destination,
            &fixture.staging,
            &worker,
        )?;
        let current = verify_ancestors(&fixture.reviewed.ancestors, &worker.stop)?;
        let root = parent_in(&current, &fixture.destination)?
            .open_directory(basename(&fixture.destination)?)?;
        assert_eq!(metadata(root.as_file())?.identity, identity);
        assert!(root.entries()?.is_empty());
        assert!(!fixture.staging.exists());
        unverified.ticket = None;
        lease.ticket.clone().finish(true);
        Ok(())
    })
    .await
}

async fn collision(directory: bool) -> Result<()> {
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    native(move || {
        let members = members()?;
        let worker = lease.worker();
        let stage = build_stage(
            &fixture.reviewed,
            &members,
            &fixture.destination,
            &fixture.staging,
            &worker,
        )?;
        if directory {
            std::fs::create_dir(&fixture.destination)?;
        } else {
            std::fs::write(&fixture.destination, b"external owner")?;
        }
        assert!(
            publish_stage(
                stage,
                &fixture.reviewed,
                &members,
                &fixture.destination,
                &fixture.staging,
                &worker,
            )
            .is_err()
        );
        if directory {
            assert!(std::fs::read_dir(&fixture.destination)?.next().is_none());
        } else {
            assert_eq!(std::fs::read(&fixture.destination)?, b"external owner");
        }
        assert!(fixture.staging.join("空目录").is_dir());
        assert!(lease.quarantined());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_publication_preserves_existing_file_and_quarantines_stage() -> Result<()> {
    collision(false).await
}

#[tokio::test]
async fn windows_archive_publication_preserves_existing_empty_directory_and_quarantines_stage()
-> Result<()> {
    collision(true).await
}

#[tokio::test]
async fn windows_archive_changed_staging_content_is_not_published_or_deleted() -> Result<()> {
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    native(move || {
        let members = vec![
            ArchiveMember::new_file("file", b"reviewed".to_vec()).map_err(|_| invalid_archive())?,
        ];
        let worker = lease.worker();
        let stage = build_stage(
            &fixture.reviewed,
            &members,
            &fixture.destination,
            &fixture.staging,
            &worker,
        )?;
        std::fs::write(fixture.staging.join("file"), b"tampered")?;
        assert!(
            publish_stage(
                stage,
                &fixture.reviewed,
                &members,
                &fixture.destination,
                &fixture.staging,
                &worker,
            )
            .is_err()
        );
        assert!(!fixture.destination.exists());
        assert_eq!(std::fs::read(fixture.staging.join("file"))?, b"tampered");
        assert!(lease.quarantined());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_changed_staging_children_are_not_published_or_deleted() -> Result<()> {
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    native(move || {
        let worker = lease.worker();
        let members = members()?;
        let stage = build_stage(
            &fixture.reviewed,
            &members,
            &fixture.destination,
            &fixture.staging,
            &worker,
        )?;
        std::fs::write(fixture.staging.join("unexpected"), b"evidence")?;
        assert!(
            publish_stage(
                stage,
                &fixture.reviewed,
                &members,
                &fixture.destination,
                &fixture.staging,
                &worker,
            )
            .is_err()
        );
        assert!(fixture.staging.join("unexpected").is_file());
        assert!(!fixture.destination.exists());
        assert!(lease.quarantined());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_cancel_before_publication_preserves_complete_staging() -> Result<()> {
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    native(move || {
        let worker = lease.worker();
        let members = members()?;
        let stage = build_stage(
            &fixture.reviewed,
            &members,
            &fixture.destination,
            &fixture.staging,
            &worker,
        )?;
        worker.stop.store(true, Ordering::Release);
        assert!(matches!(
            publish_stage(
                stage,
                &fixture.reviewed,
                &members,
                &fixture.destination,
                &fixture.staging,
                &worker,
            ),
            Err(SessionError::Closed)
        ));
        assert!(fixture.staging.join("空目录").is_dir());
        assert!(!fixture.destination.exists());
        assert!(lease.quarantined());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_unobserved_successful_publication_remains_quarantined() -> Result<()> {
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    native(move || {
        let worker = lease.worker();
        let members = members()?;
        let stage = build_stage(
            &fixture.reviewed,
            &members,
            &fixture.destination,
            &fixture.staging,
            &worker,
        )?;
        let unobserved = publish_stage(
            stage,
            &fixture.reviewed,
            &members,
            &fixture.destination,
            &fixture.staging,
            &worker,
        )?;
        drop(unobserved);
        lease.ticket.clone().finish(true);
        assert!(fixture.destination.join("空目录").is_dir());
        assert!(lease.quarantined());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_abandoned_stage_result_keeps_isolation_after_pending_clears() -> Result<()>
{
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    let pending = Arc::new(AtomicBool::new(false));
    let reviewed = fixture.reviewed.clone();
    let destination = fixture.destination.clone();
    let staging = fixture.staging.clone();
    let mut task = submit_mutation(pending.clone(), lease.worker(), move |worker| {
        build_stage(&reviewed, &members()?, &destination, &staging, worker)
    });
    let stage = tokio::time::timeout(WAIT, observe_mutation(&pending, &mut task))
        .await
        .map_err(|_| SessionError::Timeout("Windows archive stage test"))??;
    assert!(!pending.load(Ordering::Acquire));
    drop(stage);
    lease.ticket.clone().finish(true);
    assert!(lease.quarantined());
    native(move || {
        assert!(fixture.staging.join("空目录").is_dir());
        assert!(!fixture.destination.exists());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_dropping_stage_await_quarantines_its_detached_worker_result() -> Result<()>
{
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    let pending = Arc::new(AtomicBool::new(false));
    let reviewed = fixture.reviewed.clone();
    let destination = fixture.destination.clone();
    let staging = fixture.staging.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let cancel = CancelOnDrop(stop.clone());
    let worker = MutationWorker {
        ticket: lease.ticket.clone(),
        stop,
    };
    let (completed, ready) = tokio::sync::oneshot::channel();
    let task = submit_mutation(pending.clone(), worker, move |worker| {
        let stage = build_stage(&reviewed, &members()?, &destination, &staging, worker)?;
        let _ = completed.send(());
        Ok(stage)
    });
    tokio::time::timeout(WAIT, ready)
        .await
        .map_err(|_| SessionError::Timeout("Windows archive detached stage setup"))?
        .map_err(|_| SessionError::Worker)?;
    // A real abandoned await drops both its join interest and cancellation guard.
    // An unobserved worker result retains pending as well as quarantine.
    drop(task);
    drop(cancel);
    lease.ticket.clone().finish(true);
    tokio::time::timeout(WAIT, async {
        let mut interval = tokio::time::interval(Duration::from_millis(1));
        while lease.ticket.is_active() {
            interval.tick().await;
        }
    })
    .await
    .map_err(|_| SessionError::Timeout("Windows archive detached result quarantine"))?;
    assert!(pending.load(Ordering::Acquire));
    assert!(lease.quarantined());
    native(move || {
        assert!(fixture.staging.join("空目录").is_dir());
        assert!(!fixture.destination.exists());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn windows_archive_panicking_worker_keeps_pending_and_partial_stage_isolation() -> Result<()>
{
    let fixture = native(Fixture::new).await?;
    let lease = Lease::new(&fixture).await?;
    let pending = Arc::new(AtomicBool::new(false));
    let reviewed = fixture.reviewed.clone();
    let destination = fixture.destination.clone();
    let staging = fixture.staging.clone();
    let task = submit_mutation(
        pending.clone(),
        lease.worker(),
        move |worker| -> Result<()> {
            let _stage = build_stage(&reviewed, &members()?, &destination, &staging, worker)?;
            panic!("controlled Windows archive worker failure");
        },
    );
    let result = tokio::time::timeout(WAIT, task)
        .await
        .map_err(|_| SessionError::Timeout("Windows archive panic test"))?;
    assert!(matches!(result, Err(error) if error.is_panic()));
    assert!(pending.load(Ordering::Acquire));
    assert!(lease.quarantined());
    native(move || {
        assert!(fixture.staging.join("空目录").is_dir());
        Ok(())
    })
    .await
}

#[test]
fn windows_archive_queued_cancel_retains_ticket_until_worker_confirms_stop() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let fixture = native(Fixture::new).await?;
        let Lease {
            owner,
            claims,
            ticket,
        } = Lease::new(&fixture).await?;
        let (release, released) = std::sync::mpsc::channel();
        let (started, ready) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = started.send(());
            released.recv_timeout(WAIT)
        });
        tokio::time::timeout(WAIT, ready)
            .await
            .map_err(|_| SessionError::Timeout("Windows archive queue setup"))?
            .map_err(|_| SessionError::Worker)?;
        let pending = Arc::new(AtomicBool::new(false));
        let ran = Arc::new(AtomicBool::new(false));
        let ran_worker = ran.clone();
        let worker = MutationWorker {
            ticket: ticket.clone(),
            stop: Arc::new(AtomicBool::new(false)),
        };
        let cancel = CancelOnDrop(worker.stop.clone());
        let mut task = submit_mutation(pending.clone(), worker, move |_| {
            ran_worker.store(true, Ordering::Release);
            Ok(())
        });
        assert!(pending.load(Ordering::Acquire));
        drop(cancel);
        // Even a scope reporting known completion cannot release the queued
        // worker's cloned reservation before that worker acknowledges its stop.
        ticket.finish(true);
        assert!(matches!(
            queue::TransferReservations::admit_resources(&owner, &claims),
            queue::Admission::Busy
        ));
        release.send(()).map_err(|_| SessionError::Worker)?;
        tokio::time::timeout(WAIT, blocker)
            .await
            .map_err(|_| SessionError::Timeout("Windows archive queue release"))?
            .map_err(|_| SessionError::Worker)?
            .map_err(|_| SessionError::Worker)?;
        let result = tokio::time::timeout(WAIT, observe_mutation(&pending, &mut task))
            .await
            .map_err(|_| SessionError::Timeout("Windows archive queued cancellation"))?;
        assert!(matches!(result, Err(SessionError::Closed)));
        assert!(!pending.load(Ordering::Acquire));
        assert!(!ran.load(Ordering::Acquire));
        match queue::TransferReservations::admit_resources(&owner, &claims) {
            queue::Admission::Ready(ticket) => ticket.finish(true),
            _ => return Err(SessionError::MutationBusy),
        }
        native(move || {
            assert!(!fixture.staging.exists());
            Ok(())
        })
        .await
    })
}
