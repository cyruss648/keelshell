//! Native filesystem primitives behind a safe, handle-owning boundary.
//!
//! Windows operations resolve individual names relative to retained directory
//! handles, create private objects with their security descriptor already set,
//! and publish the retained object without replacing an existing destination.
//! Callers own review, cancellation and mutation uncertainty. This crate never
//! removes failed staging or grants authority to retry a filesystem mutation.

mod path;
pub use path::{MAX_WINDOWS_PATH_BYTES, WindowsPath, filename_alias, validate_windows_component};

#[cfg(any(windows, test))]
mod directory_records;

#[cfg(any(windows, test))]
mod native_diagnostics;

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;
#[cfg(windows)]
pub use windows::{
    Directory, DirectoryStamp, FileInfo, Identity, PrivateEntry, metadata, verify_private,
};
