#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

fn directory() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("owned temporary directory");
    let root = temp.path().canonicalize().expect("canonical owned root");
    let selected = root.join("selected 运维 workspace");
    std::fs::create_dir(&selected).expect("selected directory");
    (temp, selected)
}

async fn selected(
    path: PathBuf,
    kind: LocalAgentKind,
) -> Result<ValidatedLocalAgentDirectory, LocalAgentError> {
    LocalAgentWorkingDirectory::Selected(path)
        .validate_directory(kind, &RequestCancellation::new())
        .await?
        .ok_or(LocalAgentError::DirectoryInvalid)
}

#[tokio::test]
async fn native_absolute_unicode_directory_is_reviewed_without_creation_or_cli() {
    let (_temp, path) = directory();
    let guard = selected(path.clone(), LocalAgentKind::Codex)
        .await
        .expect("review");
    assert_eq!(guard.selected_path(), path);
    assert_eq!(guard.canonical_path(), path);
    assert_eq!(std::fs::read_dir(path).expect("directory").count(), 0);
    assert!(!format!("{guard:?}").contains("运维"));
}

#[tokio::test]
async fn absent_file_relative_and_precancelled_directories_have_distinct_errors() {
    let (_temp, path) = directory();
    assert_eq!(
        selected(path.join("absent"), LocalAgentKind::Codex)
            .await
            .unwrap_err(),
        LocalAgentError::DirectoryMissing
    );
    let file = path.join("file");
    std::fs::write(&file, b"fixture").expect("file");
    assert_eq!(
        selected(file, LocalAgentKind::ClaudeCode)
            .await
            .unwrap_err(),
        LocalAgentError::DirectoryNotDirectory
    );
    assert_eq!(
        selected("relative".into(), LocalAgentKind::Codex)
            .await
            .unwrap_err(),
        LocalAgentError::DirectoryInvalid
    );
    let cancellation = RequestCancellation::new();
    cancellation.cancel();
    assert_eq!(
        LocalAgentWorkingDirectory::Selected(path)
            .validate_directory(LocalAgentKind::Codex, &cancellation)
            .await
            .unwrap_err(),
        LocalAgentError::Cancelled
    );
    assert_eq!(
        LocalAgentWorkingDirectory::Isolated
            .validate_directory(LocalAgentKind::ClaudeCode, &cancellation)
            .await
            .unwrap_err(),
        LocalAgentError::Cancelled
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symbolic_link_directory_and_metadata_never_acquire_authority() {
    use std::os::unix::fs::symlink;
    let (temp, path) = directory();
    let link = temp.path().canonicalize().expect("root").join("link");
    symlink(&path, &link).expect("symlink");
    assert_eq!(
        selected(link, LocalAgentKind::ClaudeCode)
            .await
            .unwrap_err(),
        LocalAgentError::DirectorySymlink
    );
    symlink(&path, path.join(".codex")).expect("metadata symlink");
    assert_eq!(
        selected(path, LocalAgentKind::Codex).await.unwrap_err(),
        LocalAgentError::DirectorySymlink
    );
}

#[tokio::test]
async fn codex_config_syntax_size_and_type_are_bounded_admission_requirements() {
    let (_temp, path) = directory();
    let folder = path.join(".codex");
    std::fs::create_dir(&folder).expect("folder");
    let config = folder.join("config.toml");
    std::fs::write(&config, b"invalid = [").expect("invalid TOML");
    assert_eq!(
        selected(path.clone(), LocalAgentKind::Codex)
            .await
            .unwrap_err(),
        LocalAgentError::DirectoryMetadataInvalid
    );
    std::fs::write(&config, vec![b' '; MAX_CONFIG_BYTES as usize + 1]).expect("oversized TOML");
    assert_eq!(
        selected(path.clone(), LocalAgentKind::Codex)
            .await
            .unwrap_err(),
        LocalAgentError::DirectoryMetadataInvalid
    );
    std::fs::remove_file(&config).expect("remove fixture");
    std::fs::create_dir(&config).expect("non-file config");
    assert_eq!(
        selected(path.clone(), LocalAgentKind::Codex)
            .await
            .unwrap_err(),
        LocalAgentError::DirectoryMetadataInvalid
    );
    // Bare Claude does not read Codex metadata, so a separate supplier's file
    // does not expand its admission requirements.
    selected(path, LocalAgentKind::ClaudeCode)
        .await
        .expect("Claude directory");
}

#[cfg(unix)]
#[tokio::test]
async fn fifo_config_is_rejected_without_waiting_for_a_writer() {
    let (_temp, path) = directory();
    std::fs::create_dir(path.join(".codex")).expect("folder");
    nix::unistd::mkfifo(
        &path.join(".codex/config.toml"),
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )
    .expect("owned fifo");
    let started = Instant::now();
    assert_eq!(
        selected(path, LocalAgentKind::Codex).await.unwrap_err(),
        LocalAgentError::DirectoryMetadataInvalid
    );
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[cfg(unix)]
#[tokio::test]
async fn changed_appeared_and_replaced_metadata_revoke_review() {
    let (_temp, path) = directory();
    let cancellation = RequestCancellation::new();
    let guard = selected(path.clone(), LocalAgentKind::Codex)
        .await
        .expect("absent review");
    std::fs::create_dir(path.join(".codex")).expect("folder");
    assert_eq!(
        guard.recheck(&cancellation).await.unwrap_err(),
        LocalAgentError::DirectoryMetadataChanged
    );
    let config = path.join(".codex/config.toml");
    std::fs::write(&config, b"model='a'\n").expect("config");
    let guard = selected(path.clone(), LocalAgentKind::Codex)
        .await
        .expect("config review");
    std::fs::write(&config, b"model='b'\n").expect("same-size change");
    assert_eq!(
        guard.recheck(&cancellation).await.unwrap_err(),
        LocalAgentError::DirectoryMetadataChanged
    );
    let guard = selected(path.clone(), LocalAgentKind::Codex)
        .await
        .expect("changed config review");
    std::fs::rename(&config, path.join(".codex/old.toml")).expect("move config");
    std::fs::write(&config, b"model='b'\n").expect("replacement");
    assert_eq!(
        guard.recheck(&cancellation).await.unwrap_err(),
        LocalAgentError::DirectoryMetadataChanged
    );
}

#[cfg(unix)]
#[tokio::test]
async fn renamed_and_replaced_directory_revokes_the_reviewed_identity() {
    let (_temp, path) = directory();
    let guard = selected(path.clone(), LocalAgentKind::ClaudeCode)
        .await
        .expect("review");
    let snapshot = guard.snapshot();
    std::fs::rename(&path, path.with_file_name("moved reviewed directory"))
        .expect("move reviewed inode");
    std::fs::create_dir(&path).expect("replacement");
    assert_eq!(
        guard
            .recheck(&RequestCancellation::new())
            .await
            .unwrap_err(),
        LocalAgentError::DirectoryChanged
    );
    assert_eq!(
        ValidatedLocalAgentDirectory::reopen_child(&snapshot, LocalAgentKind::ClaudeCode)
            .unwrap_err(),
        LocalAgentError::DirectoryChanged
    );
}

#[tokio::test]
async fn cancellation_discards_a_late_owned_worker_result() {
    struct OwnedResult(Arc<AtomicBool>);
    impl Drop for OwnedResult {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let cancellation = RequestCancellation::new();
    let worker_entered = entered.clone();
    let worker_release = release.clone();
    let worker_dropped = dropped.clone();
    let token = cancellation.clone();
    let task = tokio::spawn(async move {
        bounded_check(token, move || {
            worker_entered.store(true, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(2);
            while !worker_release.load(Ordering::SeqCst) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(2));
            }
            Ok(OwnedResult(worker_dropped))
        })
        .await
    });
    let deadline = Instant::now() + Duration::from_secs(1);
    while !entered.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    cancellation.cancel();
    assert!(matches!(
        task.await.expect("worker consumer"),
        Err(LocalAgentError::Cancelled)
    ));
    release.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(1);
    while !dropped.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}
