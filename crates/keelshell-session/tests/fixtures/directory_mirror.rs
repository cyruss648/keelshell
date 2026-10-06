//! Exact reviewed deletions over the controlled TCP/SSH/SFTP peer.
use super::*;
use keelshell_core::{
    DirectoryEntryKind as Kind, DirectoryEntrySnapshot as Entry,
    DirectorySyncDeletePolicy as Policy, DirectorySyncDirection as Direction, compare_directories,
    hash_directory_content, plan_directory_mirror, plan_directory_sync,
};
fn file(path: &str, bytes: &[u8]) -> Entry {
    Entry::new(path, Kind::File, Some(bytes.len() as u64), Some(0)).with_content_hash(
        hash_directory_content(bytes).unwrap_or_else(|error| panic!("fixture digest: {error}")),
    )
}
fn empty(path: &str) -> Entry {
    Entry::new(path, Kind::Directory, None, Some(0))
}
#[tokio::test]
async fn remote_mirror_requires_exact_policy_path_direction_and_rechecked_contents()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    sftp.mkdir("/mirror/empty").await?;
    sftp.write("/mirror/old", b"old").await?;
    let local = tempfile::tempdir()?;
    let local_root = local.path().canonicalize()?;
    let report = compare_directories(&[], &[empty("empty"), file("old", b"old")])?;
    let plan = plan_directory_mirror(&report, Direction::LeftToRight)?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let generic = plan_directory_sync(&report, Direction::LeftToRight, Policy::IncludeDeletes)?;
    let generic = generic.clone().confirm(generic.review_token())?;
    let spec = TransferSpec::upload(&local_root, "/mirror");
    let permitted = || true;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    for result in [
        scope.remove_remote_reviewed(&generic, "old").await,
        scope.remove_remote_reviewed(&confirmed, "../old").await,
        scope.remove_local_reviewed(&confirmed, "old").await,
    ] {
        assert!(matches!(result, Err(SessionError::Invalid(_))));
    }
    drop(scope);
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    server
        .filesystem
        .replace_external_file("/mirror/old", b"changed")?;
    assert!(matches!(
        scope.remove_remote_reviewed(&confirmed, "old").await,
        Err(SessionError::Invalid(_))
    ));
    assert_eq!(sftp.read("/mirror/old", 100).await?, b"changed");
    server
        .filesystem
        .replace_external_file("/mirror/old", b"old")?;
    // Correcting target bytes must not silently revive the previous approval.
    assert!(matches!(
        scope.remove_remote_reviewed(&confirmed, "old").await,
        Err(SessionError::Invalid(_))
    ));
    drop(scope);
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    tokio::fs::write(local_root.join("old"), b"appeared source").await?;
    assert!(matches!(
        scope.remove_remote_reviewed(&confirmed, "old").await,
        Err(SessionError::Invalid(_))
    ));
    tokio::fs::remove_file(local_root.join("old")).await?;
    assert!(matches!(
        scope.remove_remote_reviewed(&confirmed, "empty").await,
        Err(SessionError::Invalid(_))
    ));
    drop(scope);
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    scope.remove_remote_reviewed(&confirmed, "old").await?;
    scope.remove_remote_reviewed(&confirmed, "empty").await?;
    assert!(sftp.inspect_entry("/mirror/old").await?.is_none());
    assert!(sftp.inspect_entry("/mirror/empty").await?.is_none());
    drop(scope);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}
#[tokio::test]
async fn local_mirror_rejects_remote_source_appearance_then_removes_exact_file_and_empty_directory()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    tokio::fs::write(root.join("old"), b"old").await?;
    tokio::fs::create_dir(root.join("empty")).await?;
    let report = compare_directories(&[empty("empty"), file("old", b"old")], &[])?;
    let plan = plan_directory_mirror(&report, Direction::RightToLeft)?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let spec = TransferSpec::download("/mirror", &root);
    let permitted = || true;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    server
        .filesystem
        .replace_external_file("/mirror/old", b"source appeared")?;
    assert!(matches!(
        scope.remove_local_reviewed(&confirmed, "old").await,
        Err(SessionError::Invalid(_))
    ));
    assert_eq!(tokio::fs::read(root.join("old")).await?, b"old");
    drop(scope);
    sftp.remove("/mirror/old").await?;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    scope.remove_local_reviewed(&confirmed, "old").await?;
    scope.remove_local_reviewed(&confirmed, "empty").await?;
    assert!(!root.join("old").exists() && !root.join("empty").exists());
    drop(scope);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}
#[tokio::test]
async fn dropped_real_remove_and_failed_readback_keep_destination_quarantine()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    sftp.write("/mirror/old", b"old").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let report = compare_directories(&[], &[file("old", b"old")])?;
    let plan = plan_directory_mirror(&report, Direction::LeftToRight)?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let spec = TransferSpec::upload(&root, "/mirror");
    let permitted = || true;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    let hold = server.filesystem.hold_remove_path("/mirror/old")?;
    let mut pending = Box::pin(scope.remove_remote_reviewed(&confirmed, "old"));
    tokio::select! { result=&mut pending=>panic!("remove completed before owned reply hold: {result:?}"), result=tokio::time::timeout(Duration::from_secs(3),async{while hold.entered()==0{tokio::time::sleep(Duration::from_millis(5)).await;}})=>{result?;} }
    assert!(!hold.expired());
    drop(pending);
    drop(scope);
    hold.release();
    assert!(matches!(
        sftp.remove("/mirror/old").await,
        Err(SessionError::MutationQuarantined)
    ));
    assert!(matches!(
        sftp.reserve_directory_sync(&spec, &permitted).await,
        Err(SessionError::MutationQuarantined)
    ));
    // A failed verification cannot clear pending/retired ownership. Use an
    // unrelated root to avoid treating the earlier unknown as fresh authority.
    sftp.mkdir("/verify").await?;
    let verify_spec = TransferSpec::upload(&root, "/verify");
    let owner = sftp
        .reserve_directory_sync(&verify_spec, &permitted)
        .await?;
    assert!(matches!(
        owner
            .verify_mutation(async {
                Err::<(), _>(SessionError::Invalid("controlled readback failure"))
            })
            .await,
        Err(SessionError::MutationUncertain)
    ));
    assert!(matches!(
        owner.verify_mutation(async { Ok(()) }).await,
        Err(SessionError::MutationUncertain)
    ));
    drop(owner);
    assert!(matches!(
        sftp.mkdir("/verify/blocked").await,
        Err(SessionError::MutationQuarantined)
    ));
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn an_empty_directory_filled_after_revalidation_is_a_known_protocol_refusal()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    sftp.mkdir("/mirror/empty").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    let report = compare_directories(&[], &[empty("empty")])?;
    let plan = plan_directory_mirror(&report, Direction::LeftToRight)?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    let spec = TransferSpec::upload(&root, "/mirror");
    let permitted = || true;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    let hold = server.filesystem.hold_remove_path("/mirror/empty")?;
    let mut pending = Box::pin(scope.remove_remote_reviewed(&confirmed, "empty"));
    tokio::select! {
        result = &mut pending => panic!("RMDIR completed before exact owned hold: {result:?}"),
        result = tokio::time::timeout(Duration::from_secs(3), async {
            while hold.entered() == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }) => result?,
    }
    assert!(!hold.expired());
    server
        .filesystem
        .replace_external_file("/mirror/empty/late", b"retain")?;
    hold.release();
    let error = match pending.await {
        Err(error) => error,
        Ok(()) => panic!("nonempty directory must not be removed"),
    };
    assert!(!matches!(error, SessionError::MutationUncertain));
    assert_eq!(sftp.read("/mirror/empty/late", 100).await?, b"retain");
    assert!(sftp.inspect_entry("/mirror/empty").await?.is_some());
    drop(scope);
    // A received STATUS refusal is known; it must not manufacture an unknown
    // write or permit a recursive fallback. Fresh ordinary admission succeeds.
    sftp.mkdir("/mirror/after-known-refusal").await?;
    sftp.close().await?;
    session.close().await?;
    Ok(())
}

#[tokio::test]
async fn a_reviewed_local_regular_file_replaced_by_a_directory_is_never_removed()
-> Result<(), Box<dyn Error>> {
    let _scenario = SCENARIOS.lock().await;
    let server = serve().await?;
    let session = SshSession::connect(options(&server)).await?;
    let sftp = session.sftp().await?;
    sftp.mkdir("/mirror").await?;
    let local = tempfile::tempdir()?;
    let root = local.path().canonicalize()?;
    tokio::fs::write(root.join("old"), b"old").await?;
    let report = compare_directories(&[file("old", b"old")], &[])?;
    let plan = plan_directory_mirror(&report, Direction::RightToLeft)?;
    let confirmed = plan.clone().confirm(plan.review_token())?;
    tokio::fs::remove_file(root.join("old")).await?;
    tokio::fs::create_dir(root.join("old")).await?;
    let spec = TransferSpec::download("/mirror", &root);
    let permitted = || true;
    let scope = sftp.reserve_directory_sync(&spec, &permitted).await?;
    assert!(matches!(
        scope.remove_local_reviewed(&confirmed, "old").await,
        Err(SessionError::Invalid(_))
    ));
    assert!(root.join("old").is_dir());
    drop(scope);
    sftp.close().await?;
    session.close().await?;
    Ok(())
}
