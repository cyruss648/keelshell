use keelshell_platform::{Directory, PrivateEntry};
use std::fs::File;
use std::sync::Arc;

pub fn inject_directory(file: File) -> Directory {
    Directory { file: Arc::new(file) }
}

pub fn inject_publication(file: File) -> PrivateEntry {
    PrivateEntry { file, directory: false }
}
