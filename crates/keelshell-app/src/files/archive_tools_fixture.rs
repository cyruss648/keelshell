//! Per-server observations and a bounded reply hold for native controller tests.
use super::Checked;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::oneshot;

pub(crate) struct Fixture {
    pub(crate) calls: AtomicUsize,
    response: Mutex<Vec<u8>>,
    hold: Mutex<Option<oneshot::Receiver<()>>>,
}

impl Default for Fixture {
    fn default() -> Self {
        let mut response = b"KEELSHELL_ARCHIVE_TOOLS_V1\0".to_vec();
        for (tool, status, path) in [
            ("tar", "found", "/usr/bin/tar"),
            ("gzip", "found", "/usr/bin/gzip"),
            ("zip", "missing", ""),
            ("unzip", "unverified", "unzip"),
            ("python3", "found", "/usr/bin/python3"),
            ("rz", "missing", ""),
            ("sz", "missing", ""),
        ] {
            for field in [tool, status, path] {
                response.extend_from_slice(field.as_bytes());
                response.push(0);
            }
        }
        Self::with_response(response)
    }
}

impl Fixture {
    pub(crate) fn validation() -> Self {
        let tools: Vec<_> = keelshell_session::archive_tools::ArchiveTool::ALL
            .into_iter()
            .map(|tool| {
                let stream_check = match tool {
                    keelshell_session::archive_tools::ArchiveTool::Tar
                    | keelshell_session::archive_tools::ArchiveTool::Gzip
                    | keelshell_session::archive_tools::ArchiveTool::Python3 => "passed",
                    _ => "not_tested",
                };
                serde_json::json!({
                    "tool": tool.name(),
                    "path": format!("/fixture/bin/{}", tool.name()),
                    "version": format!("{} fixture 1.0", tool.name()),
                    "version_status": "passed",
                    "stream_check": stream_check,
                })
            })
            .collect();
        Self::with_response(
            serde_json::to_vec(&serde_json::json!({
                "protocol": "keelshell.archive-validation.v1",
                "tools": tools,
            }))
            .checked("encode fixture validation report"),
        )
    }

    fn with_response(response: Vec<u8>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            response: Mutex::new(response),
            hold: Mutex::new(None),
        }
    }
    pub(crate) fn reply(&self) -> (Vec<u8>, Option<oneshot::Receiver<()>>) {
        self.calls.fetch_add(1, Ordering::AcqRel);
        (
            self.response.lock().checked("probe response").clone(),
            self.hold.lock().checked("probe reply hold").take(),
        )
    }

    pub(crate) fn set_response(&self, bytes: Vec<u8>) {
        *self.response.lock().checked("probe response") = bytes;
    }

    pub(crate) fn hold_next(&self) -> oneshot::Sender<()> {
        let (release, held) = oneshot::channel();
        let previous = self.hold.lock().checked("probe reply hold").replace(held);
        assert!(
            previous.is_none(),
            "an outstanding probe hold cannot be replaced"
        );
        release
    }
}
