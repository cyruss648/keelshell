//! Fixed native failure labels for the platform's unit-test target.
//!
//! Normal library builds return the original error without emitting diagnostics.
//! Keeping it unwrapped preserves raw OS codes, kinds and custom error payloads.

use std::io;

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    NtCreateFile,
    NtQueryVolume,
    NtQueryDirectory,
    NtRename,
    CheckDiskFile,
    QueryBasicInfo,
    QueryStandardInfo,
    QueryFileId,
    ReopenDirectoryRename,
    ObserveSecurity,
    OpenDirectoryCursor,
}

impl Operation {
    #[cfg(test)]
    fn label(self) -> &'static str {
        match self {
            Self::NtCreateFile => "nt-create-file",
            Self::NtQueryVolume => "nt-query-volume",
            Self::NtQueryDirectory => "nt-query-directory",
            Self::NtRename => "nt-rename",
            Self::CheckDiskFile => "check-disk-file",
            Self::QueryBasicInfo => "query-basic-info",
            Self::QueryStandardInfo => "query-standard-info",
            Self::QueryFileId => "query-file-id",
            Self::ReopenDirectoryRename => "reopen-directory-rename",
            Self::ObserveSecurity => "observe-security",
            Self::OpenDirectoryCursor => "open-directory-cursor",
        }
    }
}

pub(crate) fn observe(operation: Operation, error: io::Error, nt_status: Option<u32>) -> io::Error {
    #[cfg(test)]
    eprintln!("{}", describe(operation, &error, nt_status));
    #[cfg(not(test))]
    let _ = (operation, nt_status);
    error
}

#[cfg(test)]
fn describe(operation: Operation, error: &io::Error, nt_status: Option<u32>) -> String {
    // Do not format Display/Debug or walk the source: OS and custom messages
    // may contain paths, account identifiers or peer-controlled text.
    let mut message = format!(
        "native-files operation={} kind={:?} raw_os_error={:?}",
        operation.label(),
        error.kind(),
        error.raw_os_error(),
    );
    if let Some(status) = nt_status {
        message.push_str(&format!(" nt_status=0x{status:08x}"));
    } else {
        message.push_str(" nt_status=None");
    }
    message
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use super::*;

    struct UnprintableError;

    impl fmt::Display for UnprintableError {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            panic!("native diagnostics must not format an error payload")
        }
    }

    impl fmt::Debug for UnprintableError {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            panic!("native diagnostics must not debug an error payload")
        }
    }

    impl std::error::Error for UnprintableError {}

    #[test]
    fn native_failure_diagnostics_never_format_untrusted_error_messages() {
        let error = io::Error::new(io::ErrorKind::PermissionDenied, UnprintableError);
        for operation in [
            Operation::NtCreateFile,
            Operation::NtQueryVolume,
            Operation::NtQueryDirectory,
            Operation::NtRename,
            Operation::CheckDiskFile,
            Operation::QueryBasicInfo,
            Operation::QueryStandardInfo,
            Operation::QueryFileId,
            Operation::ReopenDirectoryRename,
            Operation::ObserveSecurity,
            Operation::OpenDirectoryCursor,
        ] {
            let message = describe(operation, &error, None);
            assert!(message.starts_with("native-files operation="));
            assert!(message.ends_with("raw_os_error=None nt_status=None"));
            assert!(message.len() < 200);
        }
        let returned = observe(Operation::ObserveSecurity, error, None);
        assert_eq!(returned.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            returned
                .get_ref()
                .is_some_and(|source| source.is::<UnprintableError>())
        );
    }

    #[test]
    fn native_failure_observation_preserves_the_original_typed_os_error() {
        // This is a synthetic code on the host; it makes no Windows diagnosis.
        let error = io::Error::from_raw_os_error(4321);
        let original_kind = error.kind();
        let message = describe(Operation::NtCreateFile, &error, Some(0xe123_4567));
        assert!(message.contains("raw_os_error=Some(4321)"));
        assert!(message.ends_with("nt_status=0xe1234567"));
        let returned = observe(Operation::NtCreateFile, error, Some(0xe123_4567));
        assert_eq!(returned.raw_os_error(), Some(4321));
        assert_eq!(returned.kind(), original_kind);
    }
}
