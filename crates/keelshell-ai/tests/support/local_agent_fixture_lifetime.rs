//! Real filesystem ownership checks between the controller's serial scenarios.

use std::{path::Path, sync::Mutex};

static PREVIOUS_ROOT: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

pub(super) fn record_new_root(root: &Path) {
    let mut previous = PREVIOUS_ROOT.lock().unwrap();
    if let Some(path) = previous.as_ref() {
        assert!(
            !path.try_exists().unwrap(),
            "the completed scenario must release its fixture files before the next one"
        );
    }
    // Keep only the path for the next assertion, never an owner or open handle.
    *previous = Some(root.to_path_buf());
}
