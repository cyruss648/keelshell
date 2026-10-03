#![cfg(unix)]

use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use keelshell_core::compile_snippet_template;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn run_shell(source: &str, cwd: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Regular files avoid pipe-capacity deadlocks while the parent enforces the
    // deadline. Every script and payload is controlled test data, never a server.
    let stdout = tempfile::NamedTempFile::new_in(cwd)?;
    let stderr = tempfile::NamedTempFile::new_in(cwd)?;
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(source)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", cwd)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(stdout.as_file().try_clone()?)
        .stderr(stderr.as_file().try_clone()?)
        .spawn()?;
    let until = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < until => thread::sleep(Duration::from_millis(5)),
            outcome => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    format!("controlled shell timed out or failed to wait: {outcome:?}").into(),
                );
            }
        }
    };
    assert!(status.success(), "controlled shell status: {status:?}");
    assert!(fs::read(stderr.path())?.is_empty());
    Ok(fs::read(stdout.path())?)
}

fn render(source: &str, name: &str, value: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(compile_snippet_template(source)?.render(&BTreeMap::from([(name.into(), value.into())]))?)
}

#[test]
fn arbitrary_literal_values_remain_exactly_one_argument_without_side_effects() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let template = compile_snippet_template("set -- {{value}}")?;
    for value in [
        "",
        "中文 🦀",
        "first\n\tsecond",
        "with'single'quotes",
        "double\"quote",
        "back\\slash",
        "$(touch marker)",
        "`touch marker`",
        "'; touch marker; #",
        "a; touch marker",
        "x > marker",
        "x | touch marker",
        "* ? [glob]",
        "~ $HOME ${HOME}",
        "--help",
        "-n",
        "{{another}}",
        "# comment",
        "a=b",
        "$(printf $(touch marker))",
        "e\u{301}👩\u{200d}💻",
    ] {
        let rendered = template.render(&BTreeMap::from([("value".into(), value.into())]))?;
        let script = format!("{rendered}\ncommand printf '%s\\000' \"$#\" \"$@\"");
        assert_eq!(
            run_shell(&script, scratch.path())?,
            format!("1\0{value}\0").as_bytes(),
            "{value:?}"
        );
        assert!(!scratch.path().join("marker").exists());
        assert_eq!(fs::read_dir(scratch.path())?.count(), 0);
    }
    Ok(())
}

#[test]
fn assignment_and_option_prefixes_retain_posix_meaning_with_empty_and_special_values() -> TestResult
{
    let scratch = tempfile::tempdir()?;
    for value in ["", "a b'c", "$(touch marker)", "line\nwith\ttab"] {
        let assignment = render("NAME={{value}}", "value", value)?;
        assert_eq!(
            run_shell(
                &format!("{assignment}\ncommand printf '%s\\000' \"$NAME\""),
                scratch.path()
            )?,
            format!("{value}\0").as_bytes()
        );
        let option = render("set -- --output={{value}}", "value", value)?;
        assert_eq!(
            run_shell(
                &format!("{option}\ncommand printf '%s\\000' \"$#\" \"$@\""),
                scratch.path()
            )?,
            format!("1\0--output={value}\0").as_bytes()
        );
        let argument = render("set -- NAME={{value}}", "value", value)?;
        assert_eq!(
            run_shell(
                &format!("{argument}\ncommand printf '%s\\000' \"$#\" \"$@\""),
                scratch.path()
            )?,
            format!("1\0NAME={value}\0").as_bytes()
        );
        assert!(!scratch.path().join("marker").exists());
    }
    Ok(())
}

#[test]
fn repeated_values_and_multiline_templates_preserve_exact_bytes_and_argument_count() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let value = "中文\n'$(touch marker)'";
    let rendered = render(
        "set -- {{value}} \\\n\t{{value}}\n# retained comment",
        "value",
        value,
    )?;
    let script = format!("{rendered}\ncommand printf '%s\\000' \"$#\" \"$@\"");
    assert_eq!(
        run_shell(&script, scratch.path())?,
        format!("2\0{value}\0{value}\0").as_bytes()
    );
    assert!(!scratch.path().join("marker").exists());
    Ok(())
}

#[test]
fn quoted_interpolation_is_not_a_sandbox_for_explicit_eval() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let source = render(
        "eval {{command}}",
        "command",
        "printf '%s' explicit-eval > marker",
    )?;
    run_shell(&source, scratch.path())?;
    // This deliberately demonstrates the documented boundary using harmless,
    // controlled input in a private directory: eval explicitly reparses its data.
    assert_eq!(fs::read(scratch.path().join("marker"))?, b"explicit-eval");
    Ok(())
}
