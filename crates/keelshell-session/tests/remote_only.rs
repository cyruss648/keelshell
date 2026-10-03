//! Architecture guard for the remote-only product boundary. Test fixtures may
//! use local processes, but shipped application/session source must not create
//! a local shell or reintroduce the removed terminal backend.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

fn compact_production_source(source: &str) -> String {
    // Git can check out CRLF on Windows; line endings must not change whether
    // a trailing inline test module is distinguished from shipped code.
    let normalized = source.replace("\r\n", "\n");
    normalized
        .split("#[cfg(test)]\nmod tests {")
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn inspect_sources(path: &Path) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            inspect_sources(&path)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path)?;
            // Inline unit-test modules at the end of a source file may use an
            // OS process to validate remote shell syntax. A cfg(test) on an
            // external module or helper must not hide the production code
            // which follows it (notably main.rs and workspace.rs).
            let compact = compact_production_source(&source);
            for forbidden in [
                "portable_pty",
                "PtySession",
                "PtyCommand",
                "native_pty_system",
                "default_shell",
                "TerminalView::local",
                "fnlocal(",
                "std::process::Command",
                "tokio::process::Command",
            ] {
                assert!(
                    !compact.contains(forbidden),
                    "remote-only product boundary: {} contains {forbidden}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

#[test]
fn source_guard_recognizes_inline_test_modules_with_lf_and_crlf() {
    let source = "fn remote() {}\n#[cfg(test)]\nmod tests {\nstd::process::Command\n}";
    for source in [source.to_owned(), source.replace('\n', "\r\n")] {
        assert_eq!(compact_production_source(&source), "fnremote(){}");
    }
}

#[test]
fn external_test_module_does_not_hide_following_production_for_either_line_ending() {
    let source = "#[cfg(test)]\nmod tests;\nfn run() { std::process::Command; }";
    for source in [source.to_owned(), source.replace('\n', "\r\n")] {
        assert!(compact_production_source(&source).contains("std::process::Command"));
    }
}

#[test]
fn product_sources_cannot_restore_a_local_shell_fallback() -> Result<(), Box<dyn Error>> {
    // Cargo supplies the current checkout at runtime; a relocated target cache
    // can still contain compile-time paths from the previous repository location.
    let session = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    inspect_sources(&session.join("src"))?;
    inspect_sources(&session.join("../keelshell-app/src"))?;
    assert!(!session.join("src/pty.rs").exists());
    Ok(())
}

#[test]
fn manifests_do_not_depend_on_a_local_terminal_backend() -> Result<(), Box<dyn Error>> {
    // Cargo supplies the current checkout at runtime; a relocated target cache
    // can still contain compile-time paths from the previous repository location.
    let session = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    for path in [session.join("Cargo.toml"), session.join("../../Cargo.toml")] {
        let source = fs::read_to_string(&path)?;
        for line in source.lines() {
            if let Some((name, _)) = line.split_once('=') {
                assert!(
                    !matches!(name.trim(), "portable-pty" | "nix"),
                    "local terminal backend dependency remains in {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}
