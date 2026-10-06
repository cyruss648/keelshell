//! Independent review of returned failures versus concurrent child admission.
use super::*;
use keelshell_core::{
    ConfirmedDirectorySync, DirectoryEntryKind as Kind, DirectoryEntrySnapshot as Entry,
    DirectorySyncDirection as Direction, compare_directories, hash_directory_content,
    plan_directory_mirror,
};
use keelshell_session::sftp::{FileMutationScope, SftpSession};

fn file(path: &str) -> Entry {
    Entry::new(path, Kind::File, Some(8), Some(0)).with_content_hash(
        hash_directory_content(b"reviewed")
            .unwrap_or_else(|error| panic!("fixture digest: {error}")),
    )
}
fn dir(path: &str) -> Entry {
    Entry::new(path, Kind::Directory, None, Some(0))
}
fn confirmation(
    entries: &[Entry],
    direction: Direction,
) -> Result<ConfirmedDirectorySync, Box<dyn Error>> {
    let report = if direction == Direction::LeftToRight {
        compare_directories(&[], entries)?
    } else {
        compare_directories(entries, &[])?
    };
    let plan = plan_directory_mirror(&report, direction)?;
    Ok(plan.clone().confirm(plan.review_token())?)
}
async fn remove(
    scope: &FileMutationScope<'_>,
    confirmed: &ConfirmedDirectorySync,
    direction: Direction,
    path: &str,
) -> Result<(), SessionError> {
    match direction {
        Direction::LeftToRight => scope.remove_remote_reviewed(confirmed, path).await,
        Direction::RightToLeft => scope.remove_local_reviewed(confirmed, path).await,
    }
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
    let joined = (&mut server.task).await;
    let stopped = tokio::net::TcpStream::connect(server.address).await;
    eprintln!("independent recursive mirror listener joined={joined:?}; original port={stopped:?}");
    assert!(joined.is_err_and(|error| error.is_cancelled()));
    assert!(stopped.is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionRefused));
    Ok(())
}
async fn seed(
    sftp: &SftpSession,
    root: &std::path::Path,
    direction: Direction,
) -> Result<(), Box<dyn Error>> {
    for path in ["a", "b", "c"] {
        if direction == Direction::LeftToRight {
            sftp.write(&format!("/mirror/{path}"), b"reviewed").await?;
        } else {
            tokio::fs::write(root.join(path), b"reviewed").await?;
        }
    }
    Ok(())
}
async fn retained(
    sftp: &SftpSession,
    root: &std::path::Path,
    direction: Direction,
    path: &str,
) -> Result<bool, Box<dyn Error>> {
    Ok(if direction == Direction::LeftToRight {
        sftp.read(&format!("/mirror/{path}"), 64).await? == b"reviewed"
    } else {
        tokio::fs::read(root.join(path)).await? == b"reviewed"
    })
}

#[tokio::test]
async fn admitted_unreviewed_path_withdraws_same_owner_after_partial_completion_in_both_directions()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let mut violations = Vec::new();
    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
        let server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = session.sftp().await?;
        sftp.mkdir("/mirror").await?;
        let local = tempfile::tempdir()?;
        let root = local.path().canonicalize()?;
        seed(&sftp, &root, direction).await?;
        let confirmed = confirmation(&[file("a"), file("b"), file("c")], direction)?;
        let spec = if direction == Direction::LeftToRight {
            TransferSpec::upload(&root, "/mirror")
        } else {
            TransferSpec::download("/mirror", &root)
        };
        let allowed = || true;
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        remove(&scope, &confirmed, direction, "a").await?;
        let refused = remove(&scope, &confirmed, direction, "not-in-reviewed-plan").await;
        let retry = remove(&scope, &confirmed, direction, "b").await;
        let kept = retained(&sftp, &root, direction, "b").await?
            && retained(&sftp, &root, direction, "c").await?;
        eprintln!(
            "admitted invalid path {direction:?}: refused={refused:?}, retry={retry:?}, retained={kept}"
        );
        if !matches!(refused, Err(SessionError::Invalid(_)))
            || !matches!(retry, Err(SessionError::Invalid(_)))
            || !kept
        {
            violations.push(format!(
                "{direction:?}: {refused:?}, {retry:?}, retained={kept}"
            ));
        }
        drop(scope);
        // Known pre-write refusal must permit a newly reviewed exact owner.
        let fresh = confirmation(&[file("b"), file("c")], direction)?;
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        remove(&scope, &fresh, direction, "b").await?;
        remove(&scope, &fresh, direction, "c").await?;
        drop(scope);
        finish(server, session, sftp).await?;
    }
    assert!(violations.is_empty(), "{violations:?}");
    Ok(())
}

#[tokio::test]
async fn competing_invalid_child_returns_busy_without_withdrawing_the_active_review()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let mut violations = Vec::new();
    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
        let server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = session.sftp().await?;
        sftp.mkdir("/mirror").await?;
        let local = tempfile::tempdir()?;
        let root = local.path().canonicalize()?;
        seed(&sftp, &root, direction).await?;
        let confirmed = confirmation(&[file("a"), file("b"), file("c")], direction)?;
        let spec = if direction == Direction::LeftToRight {
            TransferSpec::upload(&root, "/mirror")
        } else {
            TransferSpec::download("/mirror", &root)
        };
        let allowed = || true;
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        remove(&scope, &confirmed, direction, "a").await?;
        let hold = server.filesystem.hold_metadata_path("/mirror/b")?;
        let mut active = Box::pin(remove(&scope, &confirmed, direction, "b"));
        tokio::select! {
            result = &mut active => return Err(format!("active child completed before held observation: {result:?}").into()),
            observed = tokio::time::timeout(Duration::from_secs(2), async {
                while hold.entered() == 0 { tokio::time::sleep(Duration::from_millis(2)).await; }
            }) => observed?,
        }
        let busy = remove(&scope, &confirmed, direction, "unreviewed").await;
        let opposite = if direction == Direction::LeftToRight {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        };
        let wrong_direction_busy = remove(&scope, &confirmed, opposite, "b").await;
        let gate_live = !hold.expired();
        hold.release();
        let original = active.await;
        let last = remove(&scope, &confirmed, direction, "c").await;
        eprintln!(
            "busy {direction:?}: invalid={busy:?}, opposite={wrong_direction_busy:?}, original={original:?}, later={last:?}"
        );
        if !matches!(busy, Err(SessionError::MutationBusy))
            || !matches!(wrong_direction_busy, Err(SessionError::MutationBusy))
            || !gate_live
            || original.is_err()
            || last.is_err()
        {
            violations.push(format!("{direction:?}: Busy changed live owner"));
        }
        drop(scope);
        finish(server, session, sftp).await?;
    }
    assert!(violations.is_empty(), "{violations:?}");
    Ok(())
}

#[tokio::test]
async fn explicit_rmdir_status_failure_then_restored_directory_cannot_revive_old_confirmation()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    sftp.mkdir("/mirror/empty").await?;
    sftp.write("/mirror/a", b"reviewed").await?;
    sftp.write("/mirror/z", b"reviewed").await?;
    let confirmed = confirmation(
        &[file("a"), dir("empty"), file("z")],
        Direction::LeftToRight,
    )?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let spec = TransferSpec::upload(&root, "/mirror");
    let allowed = || true;
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    scope.remove_remote_reviewed(&confirmed, "a").await?;
    let hold = server.filesystem.hold_remove_path("/mirror/empty")?;
    let mut removal = Box::pin(scope.remove_remote_reviewed(&confirmed, "empty"));
    tokio::select! {
        result = &mut removal => return Err(format!("RMDIR completed before request gate: {result:?}").into()),
        observed = tokio::time::timeout(Duration::from_secs(2), async {
            while hold.entered() == 0 { tokio::time::sleep(Duration::from_millis(2)).await; }
        }) => observed?,
    }
    server
        .filesystem
        .replace_external_file("/mirror/empty/late", b"external child")?;
    let gate_live = !hold.expired();
    hold.release();
    let refused = removal.await;
    let mut external = server.filesystem.clone();
    let removed_external =
        russh_sftp::server::Handler::remove(&mut external, u32::MAX, "/mirror/empty/late".into())
            .await?;
    let retry = scope.remove_remote_reviewed(&confirmed, "z").await;
    let kept = sftp.read("/mirror/z", 64).await? == b"reviewed";
    let pass = gate_live
        && matches!(refused, Err(SessionError::Sftp(_)))
        && removed_external.status_code == russh_sftp::protocol::StatusCode::Ok
        && matches!(retry, Err(SessionError::Invalid(_)))
        && kept;
    eprintln!(
        "STATUS refusal={refused:?}; restored empty directory; retry={retry:?}; z retained={kept}"
    );
    drop(scope);
    let fresh = confirmation(&[dir("empty"), file("z")], Direction::LeftToRight)?;
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    scope.remove_remote_reviewed(&fresh, "empty").await?;
    scope.remove_remote_reviewed(&fresh, "z").await?;
    drop(scope);
    finish(server, session, sftp).await?;
    assert!(pass);
    Ok(())
}

#[tokio::test]
async fn returned_read_timeout_withdraws_review_but_creates_no_unknown_write_quarantine()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let mut configured = options(&server);
    configured.timeout = Duration::from_secs(1);
    let session = SshSession::connect(configured).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    seed(&sftp, &root, Direction::RightToLeft).await?;
    let confirmed = confirmation(&[file("a"), file("b"), file("c")], Direction::RightToLeft)?;
    let spec = TransferSpec::download("/mirror", &root);
    let allowed = || true;
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    scope.remove_local_reviewed(&confirmed, "a").await?;
    let hold = server.filesystem.hold_metadata_path("/mirror/b")?;
    let mut removal = Box::pin(scope.remove_local_reviewed(&confirmed, "b"));
    tokio::select! {
        result = &mut removal => return Err(format!("read returned before held observation: {result:?}").into()),
        observed = tokio::time::timeout(Duration::from_millis(800), async {
            while hold.entered() == 0 { tokio::time::sleep(Duration::from_millis(2)).await; }
        }) => observed?,
    }
    let refused = removal.await;
    let gate_live = !hold.expired();
    hold.release();
    let retry = scope.remove_local_reviewed(&confirmed, "c").await;
    let kept = retained(&sftp, &root, Direction::RightToLeft, "b").await?
        && retained(&sftp, &root, Direction::RightToLeft, "c").await?;
    let pass = gate_live
        && matches!(
            refused,
            Err(SessionError::Timeout(_) | SessionError::Sftp(_))
        )
        && matches!(retry, Err(SessionError::Invalid(_)))
        && kept;
    eprintln!("returned read timeout={refused:?}; retry={retry:?}; no-write bytes retained={kept}");
    drop(scope);
    let fresh = confirmation(&[file("b"), file("c")], Direction::RightToLeft)?;
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    scope.remove_local_reviewed(&fresh, "b").await?;
    scope.remove_local_reviewed(&fresh, "c").await?;
    drop(scope);
    finish(server, session, sftp).await?;
    assert!(pass);
    Ok(())
}

#[tokio::test]
async fn nested_shell_metacharacters_are_literal_sftp_and_local_paths_in_both_directions()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let parent = "树-$(touch marker);'quoted'";
    let child = format!("{parent}/leaf-$HOME;'");
    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
        let server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = session.sftp().await?;
        sftp.mkdir("/mirror").await?;
        sftp.write("/outside", b"protected").await?;
        let local = tempfile::tempdir()?;
        let root = local.path().canonicalize()?;
        if direction == Direction::LeftToRight {
            sftp.mkdir(&format!("/mirror/{parent}")).await?;
            sftp.write(&format!("/mirror/{child}"), b"reviewed").await?;
        } else {
            tokio::fs::create_dir(root.join(parent)).await?;
            tokio::fs::write(root.join(&child), b"reviewed").await?;
        }
        let confirmed = confirmation(&[dir(parent), file(&child)], direction)?;
        let spec = if direction == Direction::LeftToRight {
            TransferSpec::upload(&root, "/mirror")
        } else {
            TransferSpec::download("/mirror", &root)
        };
        let allowed = || true;
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        remove(&scope, &confirmed, direction, &child).await?;
        remove(&scope, &confirmed, direction, parent).await?;
        let literal_absent = if direction == Direction::LeftToRight {
            sftp.inspect_entry(&format!("/mirror/{parent}"))
                .await?
                .is_none()
        } else {
            !root.join(parent).exists()
        };
        let protected = sftp.read("/outside", 64).await? == b"protected";
        let marker_absent =
            !root.join("marker").exists() && sftp.inspect_entry("/marker").await?.is_none();
        drop(scope);
        finish(server, session, sftp).await?;
        assert!(literal_absent && protected && marker_absent);
    }
    Ok(())
}
