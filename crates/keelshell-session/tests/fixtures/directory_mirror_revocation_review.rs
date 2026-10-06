//! Non-author lifecycle checks at the final mirror source observation.
use super::*;
use keelshell_core::{
    DirectoryEntryKind as Kind, DirectoryEntrySnapshot as Entry,
    DirectorySyncDirection as Direction, compare_directories, hash_directory_content,
    plan_directory_mirror,
};
use std::sync::atomic::{AtomicBool, Ordering};

fn destination(path: &str, directory: bool) -> Entry {
    if directory {
        Entry::new(path, Kind::Directory, None, Some(0))
    } else {
        Entry::new(path, Kind::File, Some(8), Some(0)).with_content_hash(
            hash_directory_content(b"reviewed")
                .unwrap_or_else(|error| panic!("isolated fixed-size digest: {error}")),
        )
    }
}
async fn stop_fixture(
    mut server: Server,
    session: SshSession,
    sftp: keelshell_session::sftp::SftpSession,
) -> Result<(), Box<dyn Error>> {
    sftp.close().await?;
    session.close().await?;
    server.disconnect.send_replace(true);
    server.task.abort();
    assert!((&mut server.task).await.is_err_and(|e| e.is_cancelled()));
    assert!(
        tokio::net::TcpStream::connect(server.address)
            .await
            .is_err_and(|e| e.kind() == std::io::ErrorKind::ConnectionRefused)
    );
    eprintln!("review-owned fixture joined; original listener port refused");
    Ok(())
}
async fn retained(path: &std::path::Path, directory: bool) -> bool {
    if directory {
        match tokio::fs::read_dir(path).await {
            Ok(mut entries) => matches!(entries.next_entry().await, Ok(None)),
            Err(_) => false,
        }
    } else {
        tokio::fs::read(path)
            .await
            .is_ok_and(|bytes| bytes == b"reviewed")
    }
}
async fn lifecycle(directory: bool, drop_before_reply: bool) -> Result<(), Box<dyn Error>> {
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    for name in ["a-completed", "b-deferred", "c-later"] {
        if directory {
            tokio::fs::create_dir(root.join(name)).await?;
        } else {
            tokio::fs::write(root.join(name), b"reviewed").await?;
        }
    }
    let entries: Vec<_> = ["a-completed", "b-deferred", "c-later"]
        .into_iter()
        .map(|name| destination(name, directory))
        .collect();
    let plan = plan_directory_mirror(&compare_directories(&entries, &[])?, Direction::RightToLeft)?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let spec = TransferSpec::download("/mirror", &root);
    let authorized = AtomicBool::new(true);
    let allowed = || authorized.load(Ordering::Acquire);
    let scope = sftp.reserve_directory_sync(&spec, &allowed).await?;
    scope
        .remove_local_reviewed(&confirmed, "a-completed")
        .await?;
    assert!(!root.join("a-completed").exists());
    let canonical = server.filesystem.hold_canonical_path("/mirror")?;
    let mut operation = Box::pin(scope.remove_local_reviewed(&confirmed, "b-deferred"));
    tokio::select! {
        result = &mut operation => panic!("unexpected pre-final result: {result:?}"),
        result = tokio::time::timeout(Duration::from_secs(2), async {
            while canonical.entered() == 0 { tokio::time::sleep(Duration::from_millis(2)).await; }
        }) => result?,
    }
    let final_observation = server.filesystem.hold_metadata_path("/mirror/b-deferred")?;
    assert!(!canonical.expired());
    canonical.release();
    tokio::select! {
        result = &mut operation => panic!("unexpected result before final LSTAT reply: {result:?}"),
        result = tokio::time::timeout(Duration::from_secs(2), async {
            while final_observation.entered() == 0 { tokio::time::sleep(Duration::from_millis(2)).await; }
        }) => result?,
    }
    assert_eq!(final_observation.entered(), 1);
    assert!(!final_observation.expired());
    let correctly_refused = if drop_before_reply {
        drop(operation);
        final_observation.release();
        true
    } else {
        authorized.store(false, Ordering::Release);
        final_observation.release();
        let result = operation.await;
        eprintln!("final LSTAT revocation result: {result:?}");
        matches!(result, Err(SessionError::Closed))
    };
    let deferred_retained = retained(&root.join("b-deferred"), directory).await;
    let later_retained = retained(&root.join("c-later"), directory).await;
    let mut reused_same_owner = false;
    if correctly_refused && deferred_retained {
        authorized.store(true, Ordering::Release);
        scope
            .remove_local_reviewed(&confirmed, "b-deferred")
            .await?;
        scope.remove_local_reviewed(&confirmed, "c-later").await?;
        reused_same_owner = !root.join("b-deferred").exists() && !root.join("c-later").exists();
    }
    eprintln!(
        "directory={directory}; drop_before_reply={drop_before_reply}; first completed; deferred retained={deferred_retained}; later retained={later_retained}; exact same owner reused={reused_same_owner}"
    );
    drop(scope);
    // Close and join the owned fixture before assertions that deliberately fail
    // against the original production body in the counterexample run.
    stop_fixture(server, session, sftp).await?;
    assert!(correctly_refused && deferred_retained && later_retained && reused_same_owner);
    Ok(())
}

#[tokio::test]
async fn completed_file_then_final_lstat_revocation_preserves_later_items_and_reuses_same_owner()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    lifecycle(false, false).await
}
#[tokio::test]
async fn completed_directory_then_final_lstat_revocation_preserves_later_items_and_reuses_same_owner()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    lifecycle(true, false).await
}
#[tokio::test]
async fn dropped_file_final_lstat_future_keeps_known_owner_and_can_finish_same_review()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    lifecycle(false, true).await
}
#[tokio::test]
async fn dropped_directory_final_lstat_future_keeps_known_owner_and_can_finish_same_review()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    lifecycle(true, true).await
}
