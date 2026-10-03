use super::*;

#[test]
fn literal_validation_preserves_unicode_and_shell_metacharacters() {
    for name in ["中文", "a b", "a'\"$()`;*?[]\\"] {
        assert!(safe_name(name));
    }
    for name in [
        "",
        ".",
        "..",
        "a/b",
        "bad\n",
        "bad\u{202e}",
        "bad\u{fffd}",
        "bad\0",
    ] {
        assert!(!safe_name(name));
    }
    assert!(
        CompletionQuery::Paths {
            directory: "/home/../literal space".into(),
            prefix: "a$()".into(),
            directories_only: false
        }
        .validate()
        .is_ok()
    );
    assert!(
        CompletionQuery::Paths {
            directory: "relative".into(),
            prefix: "".into(),
            directories_only: false
        }
        .validate()
        .is_err()
    );
}

#[test]
fn probe_is_strictly_framed_and_does_not_accept_missing_exit_status() -> Result<()> {
    let output = |bytes: Vec<u8>, exit_status| crate::ExecOutput {
        stdout: bytes,
        stderr: Vec::new(),
        exit_status,
    };
    let mut bytes = FRAME.to_vec();
    bytes.extend_from_slice(b"/bin::relative:/usr/bin:/bin:\xff\0");
    let (dirs, skipped, limited) = probe_directories(output(bytes, Some(0)))?;
    assert_eq!(dirs, ["/bin", "/usr/bin"]);
    assert_eq!(skipped, 4);
    assert!(!limited);
    for bytes in [
        b"banner\0/bin\0".to_vec(),
        [FRAME, b"/bin\0junk"].concat(),
        [FRAME, b"/bin\0\0"].concat(),
    ] {
        assert_eq!(
            probe_directories(output(bytes, Some(0))),
            Err(CompletionError::InvalidResponse)
        );
    }
    assert_eq!(
        probe_directories(output([FRAME, b"/bin\0"].concat(), None)),
        Err(CompletionError::UnsupportedEnvironment)
    );
    Ok(())
}

#[test]
fn path_probe_directory_limit_is_not_reset_by_invalid_components() -> Result<()> {
    let bytes = [
        FRAME,
        format!("{}:/bin\0", ["relative"; 32].join(":")).as_bytes(),
    ]
    .concat();
    let (dirs, skipped, limited) = probe_directories(crate::ExecOutput {
        stdout: bytes,
        stderr: Vec::new(),
        exit_status: Some(0),
    })?;
    assert!(dirs.is_empty());
    assert_eq!(skipped, 32);
    assert!(limited);
    Ok(())
}
