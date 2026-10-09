//! Fixed signature metadata for the checked native Codex Darwin bundle.
//!
//! No general resources, interpreters, helpers or configuration are copied.

use std::{
    fs::{File, Metadata},
    io::{Read, Seek, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

use nix::fcntl::{OFlag, open, openat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::LocalAgentError;
use crate::RequestCancellation;

const MAX_METADATA: u64 = 64 * 1024;
const FILES: [&str; 3] = [
    "Info.plist",
    "_CodeSignature/CodeResources",
    "embedded.provisionprofile",
];

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSnapshot {
    identity: [u64; 2],
    size: u64,
    digest: [u8; 32],
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BundleSnapshot {
    contents_identity: [u64; 2],
    signature_identity: [u64; 2],
    files: [FileSnapshot; 3],
}

pub(super) struct BundleReview {
    pub(super) snapshot: BundleSnapshot,
    files: [File; 3],
    // Keep the reviewed metadata namespaces alive until the supplier is cleaned.
    _contents: File,
    _signature: File,
}

fn contents_path(executable: &Path) -> Result<Option<PathBuf>, LocalAgentError> {
    let Some(macos) = executable.parent() else {
        return Ok(None);
    };
    if macos.file_name() != Some(std::ffi::OsStr::new("MacOS")) {
        return Ok(None);
    }
    let contents = macos
        .parent()
        .ok_or(LocalAgentError::UnsupportedExecutable)?;
    let bundle = contents
        .parent()
        .ok_or(LocalAgentError::UnsupportedExecutable)?;
    // A bundle layout implies signature/resource dependencies. Only the exact
    // supported Codex native layout is admitted; arbitrary sibling copying is
    // never an escape hatch for other binaries.
    if executable.file_name() != Some(std::ffi::OsStr::new("codex"))
        || contents.file_name() != Some(std::ffi::OsStr::new("Contents"))
        || bundle.extension() != Some(std::ffi::OsStr::new("app"))
    {
        return Err(LocalAgentError::UnsupportedExecutable);
    }
    Ok(Some(contents.to_owned()))
}

fn directory(path: &Path) -> Result<File, LocalAgentError> {
    open(
        path,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        nix::sys::stat::Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| LocalAgentError::ExecutableChanged)
}

fn metadata(file: &File) -> Result<Metadata, LocalAgentError> {
    file.metadata()
        .map_err(|_| LocalAgentError::ExecutableChanged)
}

fn stable(before: &Metadata, after: &Metadata) -> bool {
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

fn read_file(
    mut file: File,
    cancellation: &RequestCancellation,
) -> Result<(File, FileSnapshot), LocalAgentError> {
    if cancellation.is_cancelled() {
        return Err(LocalAgentError::Cancelled);
    }
    let before = metadata(&file)?;
    if !before.is_file() || before.len() == 0 || before.len() > MAX_METADATA {
        return Err(LocalAgentError::UnsupportedExecutable);
    }
    file.rewind()
        .map_err(|_| LocalAgentError::ExecutableChanged)?;
    let mut digest = Sha256::new();
    let mut count = 0;
    let mut buffer = [0; 8192];
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
        count += length as u64;
        if count > MAX_METADATA {
            return Err(LocalAgentError::UnsupportedExecutable);
        }
        digest.update(&buffer[..length]);
    }
    if !stable(&before, &metadata(&file)?) || count != before.len() {
        return Err(LocalAgentError::ExecutableChanged);
    }
    Ok((
        file,
        FileSnapshot {
            identity: [before.dev(), before.ino()],
            size: count,
            digest: digest.finalize().into(),
        },
    ))
}

impl BundleReview {
    pub(super) fn acquire(
        executable: &Path,
        cancellation: &RequestCancellation,
    ) -> Result<Option<Self>, LocalAgentError> {
        let Some(path) = contents_path(executable)? else {
            return Ok(None);
        };
        let contents = directory(&path)?;
        let signature = File::from(
            openat(
                &contents,
                "_CodeSignature",
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                nix::sys::stat::Mode::empty(),
            )
            .map_err(|_| LocalAgentError::ExecutableChanged)?,
        );
        let contents_metadata = metadata(&contents)?;
        let signature_metadata = metadata(&signature)?;
        let mut reviews = Vec::with_capacity(3);
        for (index, relative) in FILES.iter().enumerate() {
            let (parent, name) = if index == 1 {
                (&signature, "CodeResources")
            } else {
                (&contents, *relative)
            };
            let file = File::from(
                openat(
                    parent,
                    name,
                    OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    nix::sys::stat::Mode::empty(),
                )
                .map_err(|_| LocalAgentError::ExecutableChanged)?,
            );
            reviews.push(read_file(file, cancellation)?);
        }
        let [(a, aa), (b, bb), (c, cc)]: [(File, FileSnapshot); 3] = reviews
            .try_into()
            .map_err(|_| LocalAgentError::UnsupportedExecutable)?;
        Ok(Some(Self {
            snapshot: BundleSnapshot {
                contents_identity: [contents_metadata.dev(), contents_metadata.ino()],
                signature_identity: [signature_metadata.dev(), signature_metadata.ino()],
                files: [aa, bb, cc],
            },
            files: [a, b, c],
            _contents: contents,
            _signature: signature,
        }))
    }

    pub(super) fn clone_handles(&self) -> Result<Self, LocalAgentError> {
        Ok(Self {
            snapshot: self.snapshot.clone(),
            files: [
                self.files[0]
                    .try_clone()
                    .map_err(|_| LocalAgentError::ExecutableChanged)?,
                self.files[1]
                    .try_clone()
                    .map_err(|_| LocalAgentError::ExecutableChanged)?,
                self.files[2]
                    .try_clone()
                    .map_err(|_| LocalAgentError::ExecutableChanged)?,
            ],
            _contents: self
                ._contents
                .try_clone()
                .map_err(|_| LocalAgentError::ExecutableChanged)?,
            _signature: self
                ._signature
                .try_clone()
                .map_err(|_| LocalAgentError::ExecutableChanged)?,
        })
    }

    pub(super) fn recheck(
        &self,
        executable: &Path,
        cancellation: &RequestCancellation,
    ) -> Result<(), LocalAgentError> {
        let current =
            Self::acquire(executable, cancellation)?.ok_or(LocalAgentError::ExecutableChanged)?;
        if current.snapshot != self.snapshot {
            return Err(LocalAgentError::ExecutableChanged);
        }
        Ok(())
    }

    pub(super) fn same_contents(&self, other: Option<&Self>) -> bool {
        other.is_some_and(|other| {
            self.snapshot
                .files
                .iter()
                .zip(&other.snapshot.files)
                .all(|(a, b)| a.size == b.size && a.digest == b.digest)
        })
    }

    pub(super) fn copy_into(
        &self,
        temporary: &Path,
        cancellation: &RequestCancellation,
    ) -> Result<PathBuf, LocalAgentError> {
        let app = temporary.join("CodexCLI.app");
        let contents = app.join("Contents");
        for path in [
            &app,
            &contents,
            &contents.join("MacOS"),
            &contents.join("_CodeSignature"),
        ] {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(path)
                .map_err(|_| LocalAgentError::ScratchFailed)?;
        }
        for (index, relative) in FILES.iter().enumerate() {
            let source = self.files[index]
                .try_clone()
                .map_err(|_| LocalAgentError::ExecutableChanged)?;
            let (mut source, current) = read_file(source, cancellation)?;
            if current != self.snapshot.files[index] {
                return Err(LocalAgentError::ExecutableChanged);
            }
            source
                .rewind()
                .map_err(|_| LocalAgentError::ExecutableChanged)?;
            let source_before = metadata(&source)?;
            let path = contents.join(relative);
            let mut destination = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(OFlag::O_NOFOLLOW.bits() | OFlag::O_CLOEXEC.bits())
                .open(&path)
                .map_err(|_| LocalAgentError::ScratchFailed)?;
            // A bounded held source is copied, then read back in full. A source
            // edit during copying cannot silently become the reviewed metadata.
            std::io::copy(
                &mut Read::by_ref(&mut source).take(MAX_METADATA + 1),
                &mut destination,
            )
            .map_err(|_| LocalAgentError::ScratchFailed)?;
            if !stable(&source_before, &metadata(&source)?) {
                return Err(LocalAgentError::ExecutableChanged);
            }
            destination
                .flush()
                .map_err(|_| LocalAgentError::ScratchFailed)?;
            drop(destination);
            let target = File::from(
                open(
                    &path,
                    OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    nix::sys::stat::Mode::empty(),
                )
                .map_err(|_| LocalAgentError::ScratchFailed)?,
            );
            let (target, copied) = read_file(target, cancellation)?;
            let m = metadata(&target)?;
            if copied.size != current.size
                || copied.digest != current.digest
                || m.uid() != nix::unistd::geteuid().as_raw()
                || m.nlink() != 1
                || m.mode() & 0o777 != 0o600
            {
                return Err(LocalAgentError::ExecutableChanged);
            }
        }
        Ok(contents.join("MacOS/codex"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().expect("owned signature fixture");
        let contents = root.path().join("CodexCLI.app/Contents");
        std::fs::create_dir_all(contents.join("MacOS")).expect("owned native directory");
        std::fs::create_dir(contents.join("_CodeSignature")).expect("owned signature directory");
        for relative in FILES {
            std::fs::write(contents.join(relative), relative.as_bytes())
                .expect("owned signature metadata");
        }
        let exe = contents.join("MacOS/codex");
        std::fs::write(&exe, b"test-only native placeholder").expect("owned test body");
        (root, exe)
    }

    #[test]
    fn copy_preserves_only_fixed_metadata_and_owned_permissions() {
        let (_root, exe) = fixture();
        let cancel = RequestCancellation::new();
        let review = BundleReview::acquire(&exe, &cancel)
            .expect("review")
            .expect("bundle");
        let destination = tempfile::tempdir().expect("owned output");
        let copied = review
            .copy_into(destination.path(), &cancel)
            .expect("copy fixed metadata");
        let current = BundleReview::acquire(&copied, &cancel)
            .expect("readback")
            .expect("bundle");
        assert!(review.same_contents(Some(&current)));
        let contents = copied.parent().expect("MacOS").parent().expect("Contents");
        for relative in FILES {
            let m = std::fs::symlink_metadata(contents.join(relative)).expect("metadata");
            assert_eq!(m.mode() & 0o777, 0o600);
            assert_eq!(m.uid(), nix::unistd::geteuid().as_raw());
            assert_eq!(m.nlink(), 1);
        }
        assert!(!contents.join("CodeResources").exists());
        assert_eq!(std::fs::read_dir(contents).expect("namespace").count(), 4);
    }

    #[test]
    fn same_size_metadata_edit_and_replacement_revoke_original_bundle() {
        let (_root, exe) = fixture();
        let cancel = RequestCancellation::new();
        let review = BundleReview::acquire(&exe, &cancel)
            .expect("review")
            .expect("bundle");
        let path = exe
            .parent()
            .expect("MacOS")
            .parent()
            .expect("Contents")
            .join("Info.plist");
        std::fs::write(&path, b"other.data").expect("same size edit");
        assert_eq!(
            review.recheck(&exe, &cancel),
            Err(LocalAgentError::ExecutableChanged)
        );
        std::fs::write(&path, b"Info.plist").expect("restore body");
        let approved = BundleReview::acquire(&exe, &cancel)
            .expect("review")
            .expect("bundle");
        std::fs::rename(&path, path.with_extension("old")).expect("replace identity");
        std::fs::write(&path, b"Info.plist").expect("same body new inode");
        assert_eq!(
            approved.recheck(&exe, &cancel),
            Err(LocalAgentError::ExecutableChanged)
        );
    }

    #[test]
    fn signature_links_fifo_and_oversize_fail_before_copy() {
        for variant in ["link", "fifo", "oversize"] {
            let (root, exe) = fixture();
            let path = exe
                .parent()
                .expect("MacOS")
                .parent()
                .expect("Contents")
                .join("Info.plist");
            std::fs::remove_file(&path).expect("owned metadata removal");
            match variant {
                "link" => std::os::unix::fs::symlink(root.path().join("missing"), &path)
                    .expect("owned adversary link"),
                "fifo" => nix::unistd::mkfifo(
                    &path,
                    nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
                )
                .expect("owned FIFO without writer"),
                _ => std::fs::write(&path, vec![b'x'; MAX_METADATA as usize + 1])
                    .expect("bounded oversized metadata"),
            }
            let start = std::time::Instant::now();
            assert!(BundleReview::acquire(&exe, &RequestCancellation::new()).is_err());
            assert!(start.elapsed() < std::time::Duration::from_secs(1));
        }
    }

    #[test]
    fn cancelled_copy_creates_no_executable_and_unknown_bundle_layout_refuses() {
        let (_root, exe) = fixture();
        let review = BundleReview::acquire(&exe, &RequestCancellation::new())
            .expect("review")
            .expect("bundle");
        let cancel = RequestCancellation::new();
        cancel.cancel();
        let destination = tempfile::tempdir().expect("owned output");
        assert_eq!(
            review.copy_into(destination.path(), &cancel),
            Err(LocalAgentError::Cancelled)
        );
        assert!(
            !destination
                .path()
                .join("CodexCLI.app/Contents/MacOS/codex")
                .exists()
        );
        assert!(matches!(
            BundleReview::acquire(&exe.with_file_name("other"), &RequestCancellation::new()),
            Err(LocalAgentError::UnsupportedExecutable)
        ));
    }
}
