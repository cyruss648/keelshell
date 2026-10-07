//! A strict Ask bootstrap; no arbitrary command templates or inherited directory FDs.

use super::LocalAgentError;

const FLAG: &str = "--keelshell-internal-local-ask-v1";

#[cfg(target_os = "macos")]
#[path = "bootstrap_bundle.rs"]
mod bundle;

/// Handle the private local Ask launcher before runtime or UI initialization.
///
/// Unix applications integrating selected directories must call this first in
/// `main`, on their initial single thread. The only accepted invocation is one
/// fixed flag and a bounded, strict control frame on stdin. Directory identities,
/// exact checked CLI version, native executable hash and the fixed Ask policy are
/// verified before safe child-only `fchdir` and `exec`. No arbitrary argv, shell,
/// environment mapping, model context or credential is accepted in that frame.
/// Normal application invocations return `false` without reading stdin or files.
/// Launcher failures terminate with private typed exit codes and no user values.
pub fn run_local_agent_directory_launcher() -> bool {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(FLAG)) {
        return false;
    }
    if arguments.next().is_some() {
        std::process::exit(77);
    }
    #[cfg(unix)]
    let result = unix::launch();
    #[cfg(not(unix))]
    let result: Result<(), LocalAgentError> = Err(LocalAgentError::DirectoryUnsupported);
    // Success execs over this process; no desktop initialization is reached.
    std::process::exit(match result {
        Err(
            LocalAgentError::DirectoryChanged
            | LocalAgentError::DirectoryMissing
            | LocalAgentError::DirectorySymlink
            | LocalAgentError::DirectoryNotDirectory,
        ) => 76,
        Err(LocalAgentError::ExecutableChanged) => 78,
        Err(
            LocalAgentError::DirectoryMetadataChanged | LocalAgentError::DirectoryMetadataInvalid,
        ) => 80,
        Err(LocalAgentError::SpawnFailed) => 79,
        _ => 77,
    });
}

#[cfg(unix)]
pub(super) use unix::{ExecutableReview, wrap};

#[cfg(unix)]
pub(super) fn exit_error(code: Option<i32>) -> Option<LocalAgentError> {
    match code {
        Some(76) => Some(LocalAgentError::DirectoryChanged),
        Some(77) => Some(LocalAgentError::DirectoryUnsupported),
        Some(78) => Some(LocalAgentError::ExecutableChanged),
        Some(79) => Some(LocalAgentError::SpawnFailed),
        Some(80) => Some(LocalAgentError::DirectoryMetadataChanged),
        _ => None,
    }
}

#[cfg(unix)]
mod unix {
    #[cfg(target_os = "macos")]
    use super::bundle;
    use super::{FLAG, LocalAgentError};
    use crate::{
        RequestCancellation,
        local_agent::{
            LocalAgentConfig, LocalAgentCredential, LocalAgentKind, LocalAgentVersion,
            LocalAgentWorkingDirectory, ValidatedLocalAgentDirectory,
            directory::DirectorySnapshot,
            process::{OutputMode, configure_ask_command, owned_command},
        },
    };
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};
    use std::{
        fs::File,
        io::{Read, Seek},
        os::{
            fd::AsFd,
            unix::{
                fs::{MetadataExt, OpenOptionsExt},
                process::CommandExt,
            },
        },
        path::PathBuf,
    };
    use tokio::process::Command;
    use zeroize::Zeroizing;

    const MAX_FRAME: usize = 64 * 1024;
    const MAX_EXECUTABLE: u64 = 512 * 1024 * 1024;

    #[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
    #[serde(deny_unknown_fields)]
    struct ExecutableSnapshot {
        path: PathBuf,
        identity: [u64; 2],
        size: u64,
        digest: [u8; 32],
        #[cfg(target_os = "macos")]
        bundle: Option<bundle::BundleSnapshot>,
    }

    pub(in crate::local_agent) struct ExecutableReview {
        snapshot: ExecutableSnapshot,
        _file: File,
        requested: PathBuf,
        #[cfg(target_os = "macos")]
        bundle: Option<bundle::BundleReview>,
    }

    impl ExecutableReview {
        pub(in crate::local_agent) fn path(&self) -> &std::path::Path {
            &self.snapshot.path
        }
        pub(in crate::local_agent) async fn acquire(
            path: PathBuf,
            cancellation: &RequestCancellation,
        ) -> Result<Self, LocalAgentError> {
            let cancellation = cancellation.clone();
            let worker_cancel = cancellation.clone();
            crate::local_agent::directory::bounded_check(cancellation, move || {
                Self::read(path, &worker_cancel)
            })
            .await
        }

        fn read(
            path: PathBuf,
            cancellation: &RequestCancellation,
        ) -> Result<Self, LocalAgentError> {
            if cancellation.is_cancelled() {
                return Err(LocalAgentError::Cancelled);
            }
            let requested = path.clone();
            let path = path
                .canonicalize()
                .map_err(|_| LocalAgentError::ExecutableChanged)?;
            let mut file = File::from(
                nix::fcntl::open(
                    &path,
                    nix::fcntl::OFlag::O_RDONLY
                        | nix::fcntl::OFlag::O_NOFOLLOW
                        | nix::fcntl::OFlag::O_CLOEXEC,
                    nix::sys::stat::Mode::empty(),
                )
                .map_err(|_| LocalAgentError::ExecutableChanged)?,
            );
            let metadata = file
                .metadata()
                .map_err(|_| LocalAgentError::ExecutableChanged)?;
            if !metadata.is_file()
                || metadata.len() == 0
                || metadata.len() > MAX_EXECUTABLE
                || metadata.mode() & 0o111 == 0
            {
                return Err(LocalAgentError::ExecutableChanged);
            }
            let mut digest = Sha256::new();
            let mut buffer = vec![0; 64 * 1024];
            let mut count = 0u64;
            loop {
                if cancellation.is_cancelled() {
                    return Err(LocalAgentError::Cancelled);
                }
                let length = file
                    .read(&mut buffer)
                    .map_err(|_| LocalAgentError::ExecutableChanged)?;
                if length == 0 {
                    break;
                }
                count = count.saturating_add(length as u64);
                if count > MAX_EXECUTABLE {
                    return Err(LocalAgentError::ExecutableChanged);
                }
                digest.update(&buffer[..length]);
            }
            let after =
                std::fs::symlink_metadata(&path).map_err(|_| LocalAgentError::ExecutableChanged)?;
            if count != metadata.len()
                || metadata.dev() != after.dev()
                || metadata.ino() != after.ino()
                || metadata.len() != after.len()
                || metadata.mtime() != after.mtime()
                || metadata.mtime_nsec() != after.mtime_nsec()
                || metadata.ctime() != after.ctime()
                || metadata.ctime_nsec() != after.ctime_nsec()
            {
                return Err(LocalAgentError::ExecutableChanged);
            }
            #[cfg(target_os = "macos")]
            let bundle = bundle::BundleReview::acquire(&path, cancellation)?;
            Ok(Self {
                snapshot: ExecutableSnapshot {
                    path,
                    identity: [metadata.dev(), metadata.ino()],
                    size: count,
                    digest: digest.finalize().into(),
                    #[cfg(target_os = "macos")]
                    bundle: bundle.as_ref().map(|review| review.snapshot.clone()),
                },
                _file: file,
                requested,
                #[cfg(target_os = "macos")]
                bundle,
            })
        }

        pub(in crate::local_agent) async fn recheck(
            &self,
            cancellation: &RequestCancellation,
        ) -> Result<(), LocalAgentError> {
            let current = Self::acquire(self.requested.clone(), cancellation).await?;
            if current.snapshot != self.snapshot {
                return Err(LocalAgentError::ExecutableChanged);
            }
            Ok(())
        }
    }

    impl ExecutableReview {
        pub(in crate::local_agent) async fn owned_copy(
            &self,
            temporary: &std::path::Path,
            cancellation: &RequestCancellation,
        ) -> Result<Self, LocalAgentError> {
            let mut source = self
                ._file
                .try_clone()
                .map_err(|_| LocalAgentError::ExecutableChanged)?;
            let expected = self.snapshot.clone();
            let requested = self.requested.clone();
            let temporary = temporary.to_owned();
            #[cfg(target_os = "macos")]
            let bundle = self
                .bundle
                .as_ref()
                .map(|review| review.clone_handles())
                .transpose()?;
            let cancellation = cancellation.clone();
            let worker_cancel = cancellation.clone();
            crate::local_agent::directory::bounded_check(cancellation, move || {
                if requested
                    .canonicalize()
                    .map_err(|_| LocalAgentError::ExecutableChanged)?
                    != expected.path
                {
                    return Err(LocalAgentError::ExecutableChanged);
                }
                source
                    .rewind()
                    .map_err(|_| LocalAgentError::ExecutableChanged)?;
                let before = source
                    .metadata()
                    .map_err(|_| LocalAgentError::ExecutableChanged)?;
                let mut magic = [0; 4];
                source
                    .read_exact(&mut magic)
                    .map_err(|_| LocalAgentError::UnsupportedExecutable)?;
                if !matches!(
                    magic,
                    [0x7f, b'E', b'L', b'F']
                        | [0xfe, 0xed, 0xfa, 0xce]
                        | [0xce, 0xfa, 0xed, 0xfe]
                        | [0xfe, 0xed, 0xfa, 0xcf]
                        | [0xcf, 0xfa, 0xed, 0xfe]
                        | [0xca, 0xfe, 0xba, 0xbe]
                        | [0xbe, 0xba, 0xfe, 0xca]
                        | [0xca, 0xfe, 0xba, 0xbf]
                        | [0xbf, 0xba, 0xfe, 0xca]
                ) {
                    return Err(LocalAgentError::UnsupportedExecutable);
                }
                source
                    .rewind()
                    .map_err(|_| LocalAgentError::ExecutableChanged)?;
                let filename = expected
                    .path
                    .file_name()
                    .ok_or(LocalAgentError::UnsupportedExecutable)?;
                #[cfg(target_os = "macos")]
                let path = if let Some(bundle) = &bundle {
                    bundle.copy_into(&temporary, &worker_cancel)?
                } else {
                    temporary.join(filename)
                };
                #[cfg(not(target_os = "macos"))]
                let path = temporary.join(filename);
                let mut destination = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o700)
                    .custom_flags(
                        nix::fcntl::OFlag::O_NOFOLLOW.bits() | nix::fcntl::OFlag::O_CLOEXEC.bits(),
                    )
                    .open(&path)
                    .map_err(|_| LocalAgentError::ScratchFailed)?;
                let mut digest = Sha256::new();
                let mut buffer = vec![0; 64 * 1024];
                let mut count = 0u64;
                loop {
                    if worker_cancel.is_cancelled() {
                        return Err(LocalAgentError::Cancelled);
                    }
                    let length = source
                        .read(&mut buffer)
                        .map_err(|_| LocalAgentError::ExecutableChanged)?;
                    if length == 0 {
                        break;
                    }
                    count = count.saturating_add(length as u64);
                    if count > MAX_EXECUTABLE {
                        return Err(LocalAgentError::ExecutableChanged);
                    }
                    digest.update(&buffer[..length]);
                    std::io::Write::write_all(&mut destination, &buffer[..length])
                        .map_err(|_| LocalAgentError::ScratchFailed)?;
                }
                let path_after = std::fs::symlink_metadata(&expected.path)
                    .map_err(|_| LocalAgentError::ExecutableChanged)?;
                if !path_after.is_file()
                    || path_after.dev() != expected.identity[0]
                    || path_after.ino() != expected.identity[1]
                {
                    return Err(LocalAgentError::ExecutableChanged);
                }
                let after = source
                    .metadata()
                    .map_err(|_| LocalAgentError::ExecutableChanged)?;
                if before.dev() != after.dev()
                    || before.ino() != after.ino()
                    || before.len() != after.len()
                    || before.mtime() != after.mtime()
                    || before.mtime_nsec() != after.mtime_nsec()
                    || before.ctime() != after.ctime()
                    || before.ctime_nsec() != after.ctime_nsec()
                    || count != expected.size
                    || <[u8; 32]>::from(digest.finalize()) != expected.digest
                    || requested
                        .canonicalize()
                        .map_err(|_| LocalAgentError::ExecutableChanged)?
                        != expected.path
                {
                    return Err(LocalAgentError::ExecutableChanged);
                }
                std::io::Write::flush(&mut destination)
                    .map_err(|_| LocalAgentError::ScratchFailed)?;
                drop(destination);
                let owned = Self::read(path, &worker_cancel)?;
                let owned_metadata = owned
                    ._file
                    .metadata()
                    .map_err(|_| LocalAgentError::ScratchFailed)?;
                if owned_metadata.nlink() != 1 || owned_metadata.mode() & 0o777 != 0o700 {
                    return Err(LocalAgentError::ExecutableChanged);
                }
                if owned.snapshot.digest != expected.digest
                    || owned.snapshot.size != expected.size
                    || owned
                        ._file
                        .metadata()
                        .map_err(|_| LocalAgentError::ScratchFailed)?
                        .uid()
                        != nix::unistd::geteuid().as_raw()
                {
                    return Err(LocalAgentError::ExecutableChanged);
                }
                #[cfg(target_os = "macos")]
                if let Some(bundle) = &bundle {
                    bundle.recheck(&expected.path, &worker_cancel)?;
                    if !bundle.same_contents(owned.bundle.as_ref()) {
                        return Err(LocalAgentError::ExecutableChanged);
                    }
                }
                Ok(owned)
            })
            .await
        }
    }

    impl ExecutableReview {
        #[cfg(target_os = "macos")]
        pub(in crate::local_agent) fn signed_bundle_path(&self) -> Option<&std::path::Path> {
            self.bundle.as_ref().map(|_| self.snapshot.path.as_path())
        }
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Frame {
        schema: u8,
        kind: String,
        version: String,
        executable: ExecutableSnapshot,
        directory: DirectorySnapshot,
        model: String,
        endpoint: String,
        home: PathBuf,
        temporary: PathBuf,
    }

    pub(in crate::local_agent) fn wrap(
        command: Command,
        config: &LocalAgentConfig,
        directory: &ValidatedLocalAgentDirectory,
        binary: &ExecutableReview,
        private_directories: (&std::path::Path, &std::path::Path),
        input: &str,
        version: LocalAgentVersion,
    ) -> Result<(Command, Zeroizing<String>, OutputMode), LocalAgentError> {
        let (home, temporary) = private_directories;
        if !config.kind.supports_version(version) {
            return Err(LocalAgentError::UnsupportedVersion);
        }
        let frame = Frame {
            schema: 1,
            kind: match config.kind {
                LocalAgentKind::Codex => "codex",
                LocalAgentKind::ClaudeCode => "claude_code",
            }
            .into(),
            version: version.to_string(),
            executable: binary.snapshot.clone(),
            directory: directory.snapshot(),
            model: config.model.clone(),
            endpoint: config.endpoint.clone(),
            home: home
                .canonicalize()
                .map_err(|_| LocalAgentError::ScratchFailed)?,
            temporary: temporary
                .canonicalize()
                .map_err(|_| LocalAgentError::ScratchFailed)?,
        };
        let header = Zeroizing::new(
            serde_json::to_string(&frame).map_err(|_| LocalAgentError::DirectoryUnsupported)?,
        );
        if header.len() > MAX_FRAME {
            return Err(LocalAgentError::DirectoryUnsupported);
        }
        let launcher = config
            .directory_launcher
            .as_ref()
            .ok_or(LocalAgentError::DirectoryUnsupported)?;
        let original = command.as_std();
        let mut command = Command::new(launcher);
        command
            .arg(FLAG)
            .env_clear()
            .current_dir(
                original
                    .get_current_dir()
                    .ok_or(LocalAgentError::ScratchFailed)?,
            )
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for (name, value) in original.get_envs() {
            if let Some(value) = value {
                command.env(name, value);
            }
        }
        // Control metadata is consumed before exec; only the unchanged explicit
        // context remains in the supplier's inherited stdin pipe.
        Ok((
            command,
            Zeroizing::new(format!("{}\n{}", header.as_str(), input)),
            OutputMode::GuardedProtocol(config.kind),
        ))
    }

    pub(super) fn launch() -> Result<(), LocalAgentError> {
        let mut bytes = Zeroizing::new(Vec::new());
        let mut next = [0];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if std::time::Instant::now() >= deadline {
                return Err(LocalAgentError::DirectoryValidationTimedOut);
            }
            let stdin = std::io::stdin();
            let mut ready = [nix::poll::PollFd::new(
                stdin.as_fd(),
                nix::poll::PollFlags::POLLIN,
            )];
            if nix::poll::poll(&mut ready, 100u16)
                .map_err(|_| LocalAgentError::DirectoryUnsupported)?
                == 0
            {
                continue;
            }
            // Direct safe read avoids Stdin's buffer reading ahead and consuming
            // explicit context that belongs to the supplier after exec.
            let count = nix::unistd::read(std::io::stdin().as_fd(), &mut next)
                .map_err(|_| LocalAgentError::DirectoryUnsupported)?;
            if count == 0 || bytes.len() >= MAX_FRAME {
                return Err(LocalAgentError::DirectoryUnsupported);
            }
            if next[0] == b'\n' {
                break;
            }
            bytes.push(next[0]);
        }
        let frame: Frame =
            serde_json::from_slice(&bytes).map_err(|_| LocalAgentError::DirectoryUnsupported)?;
        if frame.schema != 1 {
            return Err(LocalAgentError::DirectoryUnsupported);
        }
        let kind = match frame.kind.as_str() {
            "codex" => LocalAgentKind::Codex,
            "claude_code" => LocalAgentKind::ClaudeCode,
            _ => return Err(LocalAgentError::DirectoryUnsupported),
        };
        if !kind.supports_version_text(&frame.version) {
            return Err(LocalAgentError::UnsupportedVersion);
        }
        let scratch = std::env::current_dir().map_err(|_| LocalAgentError::ScratchFailed)?;
        let root = scratch.parent().ok_or(LocalAgentError::ScratchFailed)?;
        if scratch.file_name() != Some(std::ffi::OsStr::new("workspace"))
            || frame.home != root.join("home")
            || frame.temporary != root.join("tmp")
        {
            return Err(LocalAgentError::DirectoryUnsupported);
        }
        let root_meta =
            std::fs::symlink_metadata(root).map_err(|_| LocalAgentError::ScratchFailed)?;
        if !root_meta.is_dir() || root_meta.mode() & 0o077 != 0 {
            return Err(LocalAgentError::DirectoryUnsupported);
        }
        let cancellation = RequestCancellation::new();
        for path in [&frame.home, &frame.temporary] {
            if path
                .canonicalize()
                .map_err(|_| LocalAgentError::ScratchFailed)?
                != *path
            {
                return Err(LocalAgentError::DirectoryUnsupported);
            }
        }
        let directory = ValidatedLocalAgentDirectory::reopen_child(&frame.directory, kind)?;
        let binary = ExecutableReview::read(frame.executable.path.clone(), &cancellation)?;
        if binary.snapshot != frame.executable {
            return Err(LocalAgentError::ExecutableChanged);
        }
        let config = LocalAgentConfig::new(kind, frame.executable.path, root, frame.model)?
            .with_inference_endpoint(&frame.endpoint)?
            .with_working_directory(LocalAgentWorkingDirectory::Selected(
                directory.selected_path().into(),
            ))?;
        let name = match kind {
            LocalAgentKind::Codex => "KEELSHELL_LOCAL_AGENT_TOKEN",
            LocalAgentKind::ClaudeCode => "ANTHROPIC_API_KEY",
        };
        let credential = LocalAgentCredential::new(
            std::env::var(name).map_err(|_| LocalAgentError::MissingCredential)?,
        )?;
        let mut command = owned_command(
            &config,
            &frame.home,
            std::path::Path::new("."),
            &frame.temporary,
        );
        // The parent supplied only its owned private environment. Reject any
        // additional ambient variable instead of granting a general env feature.
        let allowed = [
            "HOME",
            "USERPROFILE",
            "CODEX_HOME",
            "CLAUDE_CONFIG_DIR",
            "XDG_CONFIG_HOME",
            "XDG_CACHE_HOME",
            "APPDATA",
            "LOCALAPPDATA",
            "TMPDIR",
            "TEMP",
            "TMP",
            "LANG",
            "LC_ALL",
            "TERM",
            "NO_COLOR",
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
            "CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY",
            "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL",
            "DISABLE_TELEMETRY",
            "DISABLE_ERROR_REPORTING",
            "DISABLE_UPDATES",
            "PATH",
            name,
            "ANTHROPIC_BASE_URL",
            "__CF_USER_TEXT_ENCODING",
        ];
        for (key, value) in std::env::vars_os() {
            if !allowed
                .iter()
                .any(|allowed| key == std::ffi::OsStr::new(allowed))
            {
                return Err(LocalAgentError::DirectoryUnsupported);
            }
            // Darwin may add __CF_USER_TEXT_ENCODING after env_clear. It is
            // recognized only to discard it; no inherited value is forwarded.
            let _value = value;
        }
        command
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());
        configure_ask_command(
            &mut command,
            &config,
            Some(&directory),
            credential.0.as_str(),
        );
        directory.recheck_child(&cancellation)?;
        directory.enter_child()?;
        // Safe exec replaces this single-thread launcher in the owned group.
        // O_CLOEXEC closes all directory/config/binary handles in the supplier.
        let _error = command.into_std().exec();
        Err(LocalAgentError::SpawnFailed)
    }
}
