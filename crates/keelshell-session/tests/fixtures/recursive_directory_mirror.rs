//! Exact expanded subtrees over production SSH/SFTP on isolated TCP listeners.
use super::*;
use keelshell_core::{
    DirectoryEntryKind as Kind, DirectoryEntrySnapshot as Entry,
    DirectorySyncDirection as Direction, DirectorySyncOperation, compare_directories,
    hash_directory_content, plan_directory_mirror,
};
use std::sync::atomic::{AtomicBool, Ordering};

fn directory(path: &str) -> Entry {
    Entry::new(path, Kind::Directory, None, Some(0))
}
fn file(path: &str) -> Entry {
    Entry::new(path, Kind::File, Some(8), Some(0)).with_content_hash(
        hash_directory_content(b"reviewed").unwrap_or_else(|error| panic!("fixture hash: {error}")),
    )
}
fn entries() -> Vec<Entry> {
    vec![
        directory("tree"),
        directory("tree/deep"),
        file("tree/deep/a"),
        file("tree/deep/b"),
        file("tree/later"),
    ]
}
async fn seed_remote(sftp: &keelshell_session::sftp::SftpSession) -> Result<(), Box<dyn Error>> {
    sftp.mkdir("/mirror/tree").await?;
    sftp.mkdir("/mirror/tree/deep").await?;
    for path in [
        "/mirror/tree/deep/a",
        "/mirror/tree/deep/b",
        "/mirror/tree/later",
    ] {
        sftp.write(path, b"reviewed").await?;
    }
    Ok(())
}
async fn seed_local(root: &std::path::Path) -> Result<(), Box<dyn Error>> {
    tokio::fs::create_dir(root.join("tree")).await?;
    tokio::fs::create_dir(root.join("tree/deep")).await?;
    for path in ["tree/deep/a", "tree/deep/b", "tree/later"] {
        tokio::fs::write(root.join(path), b"reviewed").await?;
    }
    Ok(())
}
async fn finish(
    mut server: Server,
    session: SshSession,
    sftp: keelshell_session::sftp::SftpSession,
) -> Result<(), Box<dyn Error>> {
    sftp.close().await?;
    session.close().await?;
    server.disconnect.send_replace(true);
    server.task.abort();
    assert!(
        (&mut server.task)
            .await
            .is_err_and(|error| error.is_cancelled())
    );
    assert!(
        tokio::net::TcpStream::connect(server.address)
            .await
            .is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionRefused)
    );
    eprintln!("recursive mirror owned listener joined and original port refused");
    Ok(())
}
#[tokio::test]
async fn full_nested_subtree_deletes_children_before_parents_in_both_directions()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
        let server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = session.sftp().await?;
        sftp.mkdir("/mirror").await?;
        sftp.write("/outside", b"protected outside").await?;
        let local = tempfile::tempdir()?;
        let root = local.path().canonicalize()?;
        let report = match direction {
            Direction::LeftToRight => {
                seed_remote(&sftp).await?;
                compare_directories(&[], &entries())?
            }
            Direction::RightToLeft => {
                seed_local(&root).await?;
                compare_directories(&entries(), &[])?
            }
        };
        let plan = plan_directory_mirror(&report, direction)?;
        let confirmed = plan.clone().confirm(plan.review_token())?;
        let allowed = || true;
        let spec = if direction == Direction::LeftToRight {
            TransferSpec::upload(&root, "/mirror")
        } else {
            TransferSpec::download("/mirror", &root)
        };
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        let parent = match direction {
            Direction::LeftToRight => scope.remove_remote_reviewed(&confirmed, "tree").await,
            Direction::RightToLeft => scope.remove_local_reviewed(&confirmed, "tree").await,
        };
        assert!(
            matches!(parent, Err(SessionError::Invalid(_))),
            "parent cannot skip its children: {parent:?}"
        );
        let stale_child = match direction {
            Direction::LeftToRight => {
                scope
                    .remove_remote_reviewed(&confirmed, "tree/deep/a")
                    .await
            }
            Direction::RightToLeft => scope.remove_local_reviewed(&confirmed, "tree/deep/a").await,
        };
        assert!(matches!(stale_child, Err(SessionError::Invalid(_))));
        drop(scope);
        // The unchanged complete tree needs a new explicit confirmation/owner
        // after the rejected out-of-order attempt.
        let confirmed = plan.clone().confirm(plan.review_token())?;
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        for operation in plan.operations() {
            let DirectorySyncOperation::Delete { path, .. } = operation else {
                panic!("fixture deletion");
            };
            match direction {
                Direction::LeftToRight => scope.remove_remote_reviewed(&confirmed, path).await?,
                Direction::RightToLeft => scope.remove_local_reviewed(&confirmed, path).await?,
            };
        }
        let replay = match direction {
            Direction::LeftToRight => scope.remove_remote_reviewed(&confirmed, "tree").await,
            Direction::RightToLeft => scope.remove_local_reviewed(&confirmed, "tree").await,
        };
        assert!(matches!(replay, Err(SessionError::Invalid(_))));
        assert!(!root.join("tree").exists());
        assert!(sftp.inspect_entry("/mirror/tree").await?.is_none());
        assert_eq!(sftp.read("/outside", 100).await?, b"protected outside");
        drop(scope);
        finish(server, session, sftp).await?;
    }
    Ok(())
}
#[tokio::test]
async fn added_changed_or_reappearing_subtree_node_latches_refusal_after_one_completed_item()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    for direction in [Direction::LeftToRight, Direction::RightToLeft] {
        for change in ["added", "changed", "reappeared"] {
            let server = serve().await?;
            let session = SshSession::connect(options(&server)).await?;
            let sftp = session.sftp().await?;
            sftp.mkdir("/mirror").await?;
            let local = tempfile::tempdir()?;
            let root = local.path().canonicalize()?;
            let report = match direction {
                Direction::LeftToRight => {
                    seed_remote(&sftp).await?;
                    compare_directories(&[], &entries())?
                }
                Direction::RightToLeft => {
                    seed_local(&root).await?;
                    compare_directories(&entries(), &[])?
                }
            };
            let plan = plan_directory_mirror(&report, direction)?;
            let confirmed = plan.clone().confirm(plan.review_token())?;
            let allowed = || true;
            let spec = if direction == Direction::LeftToRight {
                TransferSpec::upload(&root, "/mirror")
            } else {
                TransferSpec::download("/mirror", &root)
            };
            let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
            match direction {
                Direction::LeftToRight => {
                    scope
                        .remove_remote_reviewed(&confirmed, "tree/deep/a")
                        .await?
                }
                Direction::RightToLeft => {
                    scope
                        .remove_local_reviewed(&confirmed, "tree/deep/a")
                        .await?
                }
            };
            let path = match change {
                "added" => "tree/late",
                "changed" => "tree/later",
                _ => "tree/deep/a",
            };
            let content = if change == "changed" {
                b"changed!".as_slice()
            } else {
                b"reviewed".as_slice()
            };
            match direction {
                Direction::LeftToRight => server
                    .filesystem
                    .replace_external_file(&format!("/mirror/{path}"), content)?,
                Direction::RightToLeft => tokio::fs::write(root.join(path), content).await?,
            };
            let refused = match direction {
                Direction::LeftToRight => {
                    scope
                        .remove_remote_reviewed(&confirmed, "tree/deep/b")
                        .await
                }
                Direction::RightToLeft => {
                    scope.remove_local_reviewed(&confirmed, "tree/deep/b").await
                }
            };
            assert!(
                matches!(refused, Err(SessionError::Invalid(_))),
                "{direction:?}/{change}: {refused:?}"
            );
            // Repairing the changed payload cannot silently restore the old review.
            if change == "changed" {
                match direction {
                    Direction::LeftToRight => server
                        .filesystem
                        .replace_external_file("/mirror/tree/later", b"reviewed")?,
                    Direction::RightToLeft => {
                        tokio::fs::write(root.join("tree/later"), b"reviewed").await?
                    }
                };
            }
            let next = match direction {
                Direction::LeftToRight => {
                    scope.remove_remote_reviewed(&confirmed, "tree/later").await
                }
                Direction::RightToLeft => {
                    scope.remove_local_reviewed(&confirmed, "tree/later").await
                }
            };
            assert!(matches!(next, Err(SessionError::Invalid(_))));
            let retained = match direction {
                Direction::LeftToRight => sftp.read("/mirror/tree/deep/b", 100).await?,
                Direction::RightToLeft => tokio::fs::read(root.join("tree/deep/b")).await?,
            };
            assert_eq!(retained, b"reviewed");
            drop(scope);
            finish(server, session, sftp).await?;
        }
    }
    Ok(())
}
#[tokio::test]
async fn nested_local_delete_rechecks_authority_after_the_last_remote_source_lstat()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    seed_local(&root).await?;
    let plan = plan_directory_mirror(
        &compare_directories(&entries(), &[])?,
        Direction::RightToLeft,
    )?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let permission = AtomicBool::new(true);
    let allowed = || permission.load(Ordering::Acquire);
    let spec = TransferSpec::download("/mirror", &root);
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    scope
        .remove_local_reviewed(&confirmed, "tree/deep/a")
        .await?;
    let canonical = server.filesystem.hold_canonical_path("/mirror")?;
    let mut operation = Box::pin(scope.remove_local_reviewed(&confirmed, "tree/deep/b"));
    tokio::select! {result=&mut operation=>panic!("before canonical gate: {result:?}"), result=tokio::time::timeout(Duration::from_secs(2),async{while canonical.entered()==0{tokio::time::sleep(Duration::from_millis(2)).await;}})=>result?}
    let last = server.filesystem.hold_metadata_path("/mirror/tree")?;
    canonical.release();
    tokio::select! {result=&mut operation=>panic!("before last source LSTAT: {result:?}"), result=tokio::time::timeout(Duration::from_secs(2),async{while last.entered()==0{tokio::time::sleep(Duration::from_millis(2)).await;}})=>result?}
    permission.store(false, Ordering::Release);
    last.release();
    let result = operation.await;
    assert!(matches!(result, Err(SessionError::Closed)), "{result:?}");
    assert!(!root.join("tree/deep/a").exists());
    assert_eq!(
        tokio::fs::read(root.join("tree/deep/b")).await?,
        b"reviewed"
    );
    assert_eq!(tokio::fs::read(root.join("tree/later")).await?, b"reviewed");
    drop(scope);
    finish(server, session, sftp).await
}
#[tokio::test]
async fn dropped_second_real_remove_keeps_partial_subtree_unknown_and_quarantined()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    seed_remote(&sftp).await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let plan = plan_directory_mirror(
        &compare_directories(&[], &entries())?,
        Direction::LeftToRight,
    )?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let allowed = || true;
    let spec = TransferSpec::upload(&root, "/mirror");
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    scope
        .remove_remote_reviewed(&confirmed, "tree/deep/a")
        .await?;
    let hold = server.filesystem.hold_remove_path("/mirror/tree/deep/b")?;
    let mut operation = Box::pin(scope.remove_remote_reviewed(&confirmed, "tree/deep/b"));
    tokio::select! {result=&mut operation=>panic!("before second REMOVE gate: {result:?}"),result=tokio::time::timeout(Duration::from_secs(3),async{while hold.entered()==0{tokio::time::sleep(Duration::from_millis(2)).await;}})=>result?}
    assert!(!hold.expired());
    drop(operation);
    drop(scope);
    hold.release();
    assert!(sftp.inspect_entry("/mirror/tree/deep/a").await?.is_none());
    assert_eq!(sftp.read("/mirror/tree/later", 100).await?, b"reviewed");
    assert!(sftp.inspect_entry("/mirror/tree").await?.is_some());
    assert!(matches!(
        sftp.reserve_directory_sync(&spec, &allowed).await,
        Err(SessionError::MutationQuarantined)
    ));
    assert!(matches!(
        sftp.remove("/mirror/tree/later").await,
        Err(SessionError::MutationQuarantined)
    ));
    finish(server, session, sftp).await
}

#[tokio::test]
async fn new_subtree_node_during_route_revalidation_is_seen_before_the_next_remove()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    seed_remote(&sftp).await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let plan = plan_directory_mirror(
        &compare_directories(&[], &entries())?,
        Direction::LeftToRight,
    )?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let allowed = || true;
    let spec = TransferSpec::upload(&root, "/mirror");
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    scope
        .remove_remote_reviewed(&confirmed, "tree/deep/a")
        .await?;
    let hold = server.filesystem.hold_canonical_path("/")?;
    let mut operation = Box::pin(scope.remove_remote_reviewed(&confirmed, "tree/deep/b"));
    tokio::select! {result=&mut operation=>panic!("before route revalidation gate: {result:?}"),result=tokio::time::timeout(Duration::from_secs(3),async{while hold.entered()==0{tokio::time::sleep(Duration::from_millis(2)).await;}})=>result?}
    server
        .filesystem
        .replace_external_file("/mirror/tree/late", b"unreviewed")?;
    hold.release();
    let result = operation.await;
    assert!(
        matches!(result, Err(SessionError::Invalid(_))),
        "{result:?}"
    );
    assert_eq!(sftp.read("/mirror/tree/deep/b", 100).await?, b"reviewed");
    assert_eq!(sftp.read("/mirror/tree/late", 100).await?, b"unreviewed");
    drop(scope);
    finish(server, session, sftp).await
}
#[tokio::test]
async fn actual_remote_tree_depth_budget_refuses_a_truncated_recursive_snapshot()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    let mut path = String::from("/mirror");
    for _ in 0..33 {
        path.push_str("/a");
        sftp.mkdir(&path).await?;
    }
    let result = sftp.snapshot_tree_limited("/mirror", 100, 32).await;
    assert!(
        matches!(result, Err(SessionError::Invalid(_))),
        "no omitted directory may become review evidence: {result:?}"
    );
    assert!(sftp.inspect_entry(&path).await?.is_some());
    finish(server, session, sftp).await
}

#[tokio::test]
async fn source_appearance_revokes_partial_subtree_review_even_after_source_is_removed()
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
        let report = match direction {
            Direction::LeftToRight => {
                seed_remote(&sftp).await?;
                compare_directories(&[], &entries())?
            }
            Direction::RightToLeft => {
                seed_local(&root).await?;
                compare_directories(&entries(), &[])?
            }
        };
        let plan = plan_directory_mirror(&report, direction)?;
        let confirmed = plan.clone().confirm(plan.review_token())?;
        let spec = if direction == Direction::LeftToRight {
            TransferSpec::upload(&root, "/mirror")
        } else {
            TransferSpec::download("/mirror", &root)
        };
        let allowed = || true;
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        match direction {
            Direction::LeftToRight => {
                scope
                    .remove_remote_reviewed(&confirmed, "tree/deep/a")
                    .await?
            }
            Direction::RightToLeft => {
                scope
                    .remove_local_reviewed(&confirmed, "tree/deep/a")
                    .await?
            }
        }
        match direction {
            Direction::LeftToRight => tokio::fs::write(root.join("tree"), b"new source").await?,
            Direction::RightToLeft => server
                .filesystem
                .replace_external_file("/mirror/tree", b"new source")?,
        }
        let refused = match direction {
            Direction::LeftToRight => {
                scope
                    .remove_remote_reviewed(&confirmed, "tree/deep/b")
                    .await
            }
            Direction::RightToLeft => scope.remove_local_reviewed(&confirmed, "tree/deep/b").await,
        };
        if !matches!(refused, Err(SessionError::Invalid(_))) {
            violations.push(format!(
                "{direction:?}: source appearance was not refused: {refused:?}"
            ));
        }
        // A deliberate external-writer fixture restores source absence. Neither
        // that restoration nor a later child may revive the existing approval.
        match direction {
            Direction::LeftToRight => tokio::fs::remove_file(root.join("tree")).await?,
            Direction::RightToLeft => {
                let mut external = server.filesystem.clone();
                let status = russh_sftp::server::Handler::remove(
                    &mut external,
                    u32::MAX,
                    "/mirror/tree".into(),
                )
                .await?;
                assert_eq!(status.status_code, russh_sftp::protocol::StatusCode::Ok);
            }
        }
        let retried = match direction {
            Direction::LeftToRight => scope.remove_remote_reviewed(&confirmed, "tree/later").await,
            Direction::RightToLeft => scope.remove_local_reviewed(&confirmed, "tree/later").await,
        };
        let retained = match direction {
            Direction::LeftToRight => sftp.inspect_entry("/mirror/tree/later").await?.is_some(),
            Direction::RightToLeft => root.join("tree/later").exists(),
        };
        if !matches!(retried, Err(SessionError::Invalid(_))) || !retained {
            violations.push(format!(
                "{direction:?}: old review revived: {retried:?}, retained={retained}"
            ));
        }
        drop(scope);
        finish(server, session, sftp).await?;
    }
    assert!(violations.is_empty(), "{violations:?}");
    Ok(())
}

#[tokio::test]
async fn local_target_disappearance_cannot_restore_the_partial_review_when_tree_returns()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let mut violations = Vec::new();
    for vanished in ["tree", "tree/deep", "tree/deep/b"] {
        let server = serve().await?;
        let session = SshSession::connect(options(&server)).await?;
        let sftp = session.sftp().await?;
        sftp.mkdir("/mirror").await?;
        let local = tempfile::tempdir()?;
        let root = local.path().canonicalize()?;
        seed_local(&root).await?;
        let plan = plan_directory_mirror(
            &compare_directories(&entries(), &[])?,
            Direction::RightToLeft,
        )?;
        let confirmed = plan.clone().confirm(plan.review_token())?;
        let spec = TransferSpec::download("/mirror", &root);
        let allowed = || true;
        let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
        scope
            .remove_local_reviewed(&confirmed, "tree/deep/a")
            .await?;
        // This controlled filesystem rename models an external writer. The
        // precise parked fixture is restored before testing the old approval.
        tokio::fs::rename(root.join(vanished), root.join("parked")).await?;
        let refused = scope.remove_local_reviewed(&confirmed, "tree/deep/b").await;
        if refused.is_ok() || matches!(refused, Err(SessionError::MutationUncertain)) {
            violations.push(format!("{vanished}: not a known refusal: {refused:?}"));
        }
        tokio::fs::rename(root.join("parked"), root.join(vanished)).await?;
        let retried = scope.remove_local_reviewed(&confirmed, "tree/later").await;
        let retained = root.join("tree/later").exists();
        if !matches!(retried, Err(SessionError::Invalid(_))) || !retained {
            violations.push(format!(
                "{vanished}: old review revived after {refused:?}: {retried:?}, retained={retained}"
            ));
        }
        drop(scope);
        finish(server, session, sftp).await?;
    }
    assert!(violations.is_empty(), "{violations:?}");
    Ok(())
}
