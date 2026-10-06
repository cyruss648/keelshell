//! Independent counterexamples use the production transport over real loopback TCP.
use super::*;
use keelshell_core::{
    DirectoryEntryKind as Kind, DirectoryEntrySnapshot as Entry,
    DirectorySyncDirection as Direction, DirectorySyncOperation, compare_directories,
    hash_directory_content, plan_directory_mirror,
};
use keelshell_session::sftp::SftpSession;
use std::sync::atomic::{AtomicBool, Ordering};

fn file(path: &str, bytes: &[u8]) -> Entry {
    Entry::new(path, Kind::File, Some(bytes.len() as u64), Some(0)).with_content_hash(
        hash_directory_content(bytes).unwrap_or_else(|error| panic!("isolated digest: {error}")),
    )
}
fn directory(path: &str) -> Entry {
    Entry::new(path, Kind::Directory, None, Some(0))
}
async fn snapshot(sftp: &SftpSession) -> Result<Vec<Entry>, Box<dyn Error>> {
    let mut result = Vec::new();
    for entry in sftp.snapshot_tree_limited("/mirror", 100, 8).await? {
        let relative = entry
            .path
            .strip_prefix("/mirror/")
            .ok_or("escaped fixture root")?;
        result.push(match entry.permissions.map(|mode| mode & 0o170000) {
            Some(0o040000) => directory(relative),
            Some(0o100000) => file(relative, &sftp.read_regular(&entry.path, 1024).await?),
            _ => return Err("unexpected fixture entry type".into()),
        });
    }
    Ok(result)
}
async fn finish(
    mut server: Server,
    session: SshSession,
    sftp: SftpSession,
) -> Result<(), Box<dyn Error>> {
    sftp.close().await?;
    session.close().await?;
    server.disconnect.send_replace(true);
    server.task.abort();
    let result = (&mut server.task).await;
    assert!(result.is_err_and(|error| error.is_cancelled()));
    let stopped = tokio::net::TcpStream::connect(server.address).await;
    assert!(stopped.is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionRefused));
    eprintln!("owned fixture listener joined and original port refused");
    Ok(())
}

#[tokio::test]
async fn real_nonempty_destination_snapshot_expands_all_reviewed_rows_in_both_directions()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    sftp.mkdir("/mirror/tree").await?;
    sftp.write("/mirror/tree/child", b"retained").await?;
    let remote = snapshot(&sftp).await?;
    assert_eq!(remote.len(), 2);
    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
        let report = match direction {
            Direction::LeftToRight => compare_directories(&[file("would-copy", b"new")], &remote)?,
            Direction::RightToLeft => compare_directories(&remote, &[file("would-copy", b"new")])?,
        };
        let plan = plan_directory_mirror(&report, direction)?;
        let deletes: Vec<_> = plan
            .operations()
            .iter()
            .filter_map(|op| match op {
                DirectorySyncOperation::Delete { path, .. } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            deletes
                .windows(2)
                .any(|paths| paths == ["tree/child", "tree"])
        );
    }
    assert_eq!(sftp.read("/mirror/tree/child", 100).await?, b"retained");
    assert!(sftp.inspect_entry("/mirror/would-copy").await?.is_none());
    assert_eq!(server.filesystem.atomic_writes_started(), 0);
    finish(server, session, sftp).await
}

#[tokio::test]
async fn late_directory_fill_receives_status_refusal_and_keeps_peer_and_owner_usable()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    sftp.mkdir("/mirror/empty").await?;
    let plan = plan_directory_mirror(
        &compare_directories(&[], &snapshot(&sftp).await?)?,
        Direction::LeftToRight,
    )?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let spec = TransferSpec::upload(&root, "/mirror");
    let allowed = || true;
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    let hold = server.filesystem.hold_remove_path("/mirror/empty")?;
    let mut operation = Box::pin(scope.remove_remote_reviewed(&confirmed, "empty"));
    tokio::select! {
        result = &mut operation => panic!("RMDIR finished before exact dispatch gate: {result:?}"),
        result = tokio::time::timeout(Duration::from_secs(3), async { while hold.entered()==0 { tokio::time::sleep(Duration::from_millis(5)).await; } }) => result?,
    }
    assert_eq!(hold.entered(), 1);
    assert!(!hold.expired());
    server
        .filesystem
        .replace_external_file("/mirror/empty/late-child", b"never recurse")?;
    hold.release();
    let result = operation.await;
    assert!(result.is_err());
    assert!(!matches!(result, Err(SessionError::MutationUncertain)));
    assert_eq!(
        sftp.read("/mirror/empty/late-child", 100).await?,
        b"never recurse"
    );
    drop(scope);
    sftp.mkdir("/mirror/after-refusal").await?;
    let next = sftp.reserve_directory_sync(&spec, &allowed).await?;
    drop(next);
    finish(server, session, sftp).await
}

#[cfg(unix)]
#[tokio::test]
async fn replaced_local_leaf_symlink_or_nonempty_directory_is_preserved_without_following()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let outside = tempfile::tempdir()?;
    let protected = outside.path().join("protected");
    tokio::fs::write(&protected, b"outside scope").await?;
    let plan = plan_directory_mirror(
        &compare_directories(&[file("old", b"old")], &[])?,
        Direction::RightToLeft,
    )?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let spec = TransferSpec::download("/mirror", &root);
    let allowed = || true;
    for symlink in [true, false] {
        let target = root.join("old");
        if symlink {
            std::os::unix::fs::symlink(&protected, &target)?;
        } else {
            tokio::fs::create_dir(&target).await?;
            tokio::fs::write(target.join("child"), b"nested scope").await?;
        }
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        assert!(matches!(
            scope.remove_local_reviewed(&confirmed, "old").await,
            Err(SessionError::Invalid(_))
        ));
        drop(scope);
        assert_eq!(tokio::fs::read(&protected).await?, b"outside scope");
        if symlink {
            assert!(
                tokio::fs::symlink_metadata(&target)
                    .await?
                    .file_type()
                    .is_symlink()
            );
            tokio::fs::remove_file(&target).await?;
        } else {
            assert_eq!(
                tokio::fs::read(target.join("child")).await?,
                b"nested scope"
            );
        }
    }
    finish(server, session, sftp).await
}

#[tokio::test]
async fn revocation_during_target_observation_never_dispatches_remove_and_review_hash_tracks_bytes()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    sftp.write("/mirror/old", b"old").await?;
    let report = compare_directories(&[], &snapshot(&sftp).await?)?;
    let plan = plan_directory_mirror(&report, Direction::LeftToRight)?;
    server
        .filesystem
        .replace_external_file("/mirror/old", b"new")?;
    let changed = plan_directory_mirror(
        &compare_directories(&[], &snapshot(&sftp).await?)?,
        Direction::LeftToRight,
    )?;
    assert_ne!(plan.review_fingerprint(), changed.review_fingerprint());
    assert!(changed.clone().confirm(plan.review_token()).is_err());
    server
        .filesystem
        .replace_external_file("/mirror/old", b"old")?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let spec = TransferSpec::upload(&root, "/mirror");
    let authorized = AtomicBool::new(true);
    let allowed = || authorized.load(Ordering::Acquire);
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    let observe = server.filesystem.hold_metadata_path("/mirror/old")?;
    let remove = server.filesystem.hold_remove_path("/mirror/old")?;
    let mut operation = Box::pin(scope.remove_remote_reviewed(&confirmed, "old"));
    tokio::select! {
        result = &mut operation => panic!("remove finished before target observation: {result:?}"),
        result = tokio::time::timeout(Duration::from_secs(3), async { while observe.entered()==0 { tokio::time::sleep(Duration::from_millis(5)).await; } }) => result?,
    }
    authorized.store(false, Ordering::Release);
    observe.release();
    assert!(matches!(operation.await, Err(SessionError::Closed)));
    assert_eq!(remove.entered(), 0);
    remove.release();
    drop(scope);
    assert_eq!(sftp.read("/mirror/old", 100).await?, b"old");
    finish(server, session, sftp).await
}

#[tokio::test]
async fn revoked_local_mirror_authority_after_final_remote_observation_never_removes_target()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    final_local_observation_revocation(false).await
}

#[tokio::test]
async fn revoked_empty_local_directory_after_final_remote_observation_never_removes_target()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    final_local_observation_revocation(true).await
}

async fn final_local_observation_revocation(empty_directory: bool) -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let target = root.join("old");
    let entry = if empty_directory {
        tokio::fs::create_dir(&target).await?;
        directory("old")
    } else {
        tokio::fs::write(&target, b"reviewed target").await?;
        file("old", b"reviewed target")
    };
    let plan = plan_directory_mirror(&compare_directories(&[entry], &[])?, Direction::RightToLeft)?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let spec = TransferSpec::download("/mirror", &root);
    let authorized = AtomicBool::new(true);
    let allowed = || authorized.load(Ordering::Acquire);
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    // This canonical root gate occurs inside revalidation, after the first
    // source absence observation. Arm the leaf gate only while it is held.
    let root_hold = server.filesystem.hold_canonical_path("/mirror")?;
    let mut operation = Box::pin(scope.remove_local_reviewed(&confirmed, "old"));
    tokio::select! {
        result = &mut operation => panic!("local removal finished before revalidation: {result:?}"),
        result = tokio::time::timeout(Duration::from_secs(3), async { while root_hold.entered()==0 { tokio::time::sleep(Duration::from_millis(5)).await; } }) => result?,
    }
    assert!(!root_hold.expired());
    let leaf_hold = server.filesystem.hold_metadata_path("/mirror/old")?;
    root_hold.release();
    tokio::select! {
        result = &mut operation => panic!("local removal finished before final source LSTAT: {result:?}"),
        result = tokio::time::timeout(Duration::from_secs(3), async { while leaf_hold.entered()==0 { tokio::time::sleep(Duration::from_millis(5)).await; } }) => result?,
    }
    assert_eq!(leaf_hold.entered(), 1);
    assert!(!leaf_hold.expired());
    if empty_directory {
        assert!(tokio::fs::symlink_metadata(&target).await?.is_dir());
    } else {
        assert_eq!(tokio::fs::read(&target).await?, b"reviewed target");
    }
    authorized.store(false, Ordering::Release);
    leaf_hold.release();
    let result = operation.await;
    let retained = if empty_directory {
        tokio::fs::symlink_metadata(&target).await?.is_dir()
            && tokio::fs::read_dir(&target)
                .await?
                .next_entry()
                .await?
                .is_none()
    } else {
        tokio::fs::read(&target).await? == b"reviewed target"
    };
    eprintln!(
        "final source LSTAT entered once; authority revoked; empty_directory={empty_directory}; removal result={result:?}; target retained={retained}"
    );
    drop(scope);
    authorized.store(true, Ordering::Release);
    // A refused pre-dispatch syscall must not leave unknown local mutation
    // quarantine. A fresh owner can immediately reserve the same exact trees.
    let next = sftp.reserve_directory_sync(&spec, &allowed).await?;
    drop(next);
    eprintln!("fresh exact owner readmitted: no unknown pending/quarantine");
    finish(server, session, sftp).await?;
    assert!(matches!(result, Err(SessionError::Closed)));
    assert!(retained);
    Ok(())
}

#[tokio::test]
async fn remote_copy_entries_recheck_authority_after_canonical_await_before_any_write()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let spec = TransferSpec::upload(&root, "/mirror");
    let authorized = AtomicBool::new(true);
    let allowed = || authorized.load(Ordering::Acquire);
    for directory_entry in [false, true] {
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        let hold = server.filesystem.hold_canonical_path("/mirror")?;
        let mut operation = Box::pin(async {
            if directory_entry {
                scope.mkdir_remote("/mirror/new-directory").await
            } else {
                scope
                    .write_remote_atomic("/mirror/new-file", b"never publish")
                    .await
            }
        });
        tokio::select! {
            result = &mut operation => panic!("copy finished before canonical gate: {result:?}"),
            result = tokio::time::timeout(Duration::from_secs(3), async { while hold.entered()==0 { tokio::time::sleep(Duration::from_millis(5)).await; } }) => result?,
        }
        assert!(!hold.expired());
        authorized.store(false, Ordering::Release);
        hold.release();
        assert!(matches!(operation.await, Err(SessionError::Closed)));
        drop(scope);
        assert!(sftp.inspect_entry("/mirror/new-file").await?.is_none());
        assert!(sftp.inspect_entry("/mirror/new-directory").await?.is_none());
        assert!(sftp.list("/mirror").await?.is_empty());
        assert_eq!(server.filesystem.atomic_writes_started(), 0);
        authorized.store(true, Ordering::Release);
    }
    finish(server, session, sftp).await
}

#[tokio::test]
async fn local_copy_syscall_guard_never_invokes_revoked_closure_or_leaves_unknown_owner()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    sftp.write("/mirror/source", b"reviewed source").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let spec = TransferSpec::download("/mirror", &root);
    let authorized = AtomicBool::new(true);
    let allowed = || authorized.load(Ordering::Acquire);
    for directory_entry in [false, true] {
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        let hold = server.filesystem.hold_metadata_path("/mirror/source")?;
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut operation = Box::pin(async {
            let observed = sftp.inspect_entry("/mirror/source").await?;
            assert!(observed.is_some());
            scope.local_operation(|| {
                calls.fetch_add(1, Ordering::AcqRel);
                if directory_entry {
                    std::fs::create_dir(root.join("new-directory"))
                } else {
                    std::fs::write(root.join("new-file"), b"never publish")
                }
            })
        });
        tokio::select! {
            result = &mut operation => panic!("local operation finished before source observation: {result:?}"),
            result = tokio::time::timeout(Duration::from_secs(3), async { while hold.entered()==0 { tokio::time::sleep(Duration::from_millis(5)).await; } }) => result?,
        }
        assert!(!hold.expired());
        authorized.store(false, Ordering::Release);
        hold.release();
        assert!(matches!(operation.await, Err(SessionError::Closed)));
        assert_eq!(calls.load(Ordering::Acquire), 0);
        assert!(!root.join("new-file").exists() && !root.join("new-directory").exists());
        drop(scope);
        authorized.store(true, Ordering::Release);
        let next = sftp.reserve_directory_sync(&spec, &allowed).await?;
        drop(next);
    }
    finish(server, session, sftp).await
}

#[tokio::test]
async fn remote_delete_checks_revocation_at_final_tree_validation_before_remove_dispatch()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    sftp.write("/mirror/old", b"retain target").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let plan = plan_directory_mirror(
        &compare_directories(&[], &[file("old", b"retain target")])?,
        Direction::LeftToRight,
    )?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let spec = TransferSpec::upload(&root, "/mirror");
    let authorized = AtomicBool::new(true);
    let allowed = || authorized.load(Ordering::Acquire);
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    let hold = server.filesystem.hold_canonical_path("/")?;
    let remove = server.filesystem.hold_remove_path("/mirror/old")?;
    let mut operation = Box::pin(scope.remove_remote_reviewed(&confirmed, "old"));
    tokio::select! {
        result = &mut operation => panic!("remote REMOVE finished before final tree validation: {result:?}"),
        result = tokio::time::timeout(Duration::from_secs(3), async { while hold.entered()==0 { tokio::time::sleep(Duration::from_millis(5)).await; } }) => result?,
    }
    assert!(!hold.expired());
    authorized.store(false, Ordering::Release);
    hold.release();
    assert!(matches!(operation.await, Err(SessionError::Closed)));
    assert_eq!(remove.entered(), 0);
    remove.release();
    drop(scope);
    assert_eq!(sftp.read("/mirror/old", 100).await?, b"retain target");
    authorized.store(true, Ordering::Release);
    let next = sftp.reserve_directory_sync(&spec, &allowed).await?;
    drop(next);
    finish(server, session, sftp).await
}
