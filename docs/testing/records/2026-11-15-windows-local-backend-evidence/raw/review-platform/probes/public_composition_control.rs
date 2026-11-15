use keelshell_platform::{Directory, FileInfo, PrivateEntry, metadata};
use std::fs::File;
use std::io;

pub fn observe_external_file(file: &File) -> io::Result<FileInfo> {
    metadata(file)
}

pub fn independent_read_while_source_is_held(parent: &Directory) -> io::Result<()> {
    let source = parent.create_file("review-sharing-stage")?;
    let _reader = parent.open_file("review-sharing-stage")?;
    let _ = metadata(source.as_file())?;
    Ok(())
}

// This valid API composition is the notice-01 native regression candidate.
// This probe typechecks it only. It does not run Windows syscalls.
pub fn retained_private_target_regression(parent: &Directory) -> io::Result<()> {
    let destination = parent.create_directory("review-private-target")?;
    let target = destination.as_directory()?;
    let source: PrivateEntry = parent.create_file("review-rename-stage")?;
    target.rename_no_replace(&source, "published")
}
