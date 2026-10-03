//! Independently check generated words with a real POSIX shell. Only controlled
//! test strings are parsed in an isolated directory; no product code starts a
//! local shell and no network input reaches this test harness.
#![cfg(unix)]

use std::{
    error::Error,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use keelshell_core::{
    CompletionAnalysis, CompletionPlan, CompletionQuery, LiteralCandidate, analyze_completion,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn plan(input: &str, caret: usize) -> TestResult<CompletionPlan> {
    match analyze_completion(input, caret)? {
        CompletionAnalysis::Ready(plan) => Ok(plan),
        CompletionAnalysis::Unsupported(reason) => Err(reason.into()),
    }
}

fn parsed_arguments(word: &str, directory: &Path) -> TestResult<Vec<u8>> {
    let script = format!("set -- {word}\ncommand printf '%s\\000' \"$#\" \"$@\"");
    controlled_shell(&script, directory)
}

fn controlled_shell(script: &str, directory: &Path) -> TestResult<Vec<u8>> {
    let mut child = Command::new("/bin/sh")
        .args(["-c", script])
        .current_dir(directory)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", directory)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("controlled shell did not finish: {result:?}").into());
            }
        }
    }
    let output = child.wait_with_output()?;
    if !output.status.success() || !output.stderr.is_empty() {
        return Err(format!("controlled shell parse failed: {output:?}").into());
    }
    Ok(output.stdout)
}

#[test]
fn remote_filename_metacharacters_remain_one_literal_posix_argument() -> TestResult {
    let scratch = tempfile::tempdir()?;
    for name in [
        "plain",
        "中文 报告🦀.txt",
        "report's draft.txt",
        "a\"double\"quote",
        "$(touch marker)",
        "`touch marker`",
        "x'; touch marker; #",
        "x > marker",
        "x; touch marker",
        "-leading-option",
        "!history*[glob]?",
        "a\\b",
        "e\u{301}\u{a0}space",
    ] {
        let input = "cat /";
        let mut plan = plan(input, input.len())?;
        let CompletionQuery::Paths { directory, .. } = plan.query(None)? else {
            return Err("expected a path lookup".into());
        };
        let plan = plan.bind_resolved_directory(&directory, "/")?;
        let path = format!("/{name}");
        let edit = plan.edit(
            input,
            input.len(),
            &LiteralCandidate {
                name: name.into(),
                path: path.clone(),
                is_directory: false,
            },
        )?;
        let actual = parsed_arguments(&edit.replacement, scratch.path())?;
        assert_eq!(actual, format!("1\0{path}\0").as_bytes(), "{name:?}");
        assert_eq!(std::fs::read_dir(scratch.path())?.count(), 0, "{name:?}");
    }
    Ok(())
}

#[test]
fn middle_path_completion_preserves_quoted_tail_and_surrounding_multiline_bytes() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let input = "printf 'before\\n'\ncat '/do/notes with spaces.txt' --flag\nprintf 'after\\n'";
    let caret = input.find("/do/").ok_or("missing marker")? + 3;
    let mut plan = plan(input, caret)?;
    let CompletionQuery::Paths {
        directory,
        directories_only,
        ..
    } = plan.query(None)?
    else {
        return Err("expected a path lookup".into());
    };
    assert!(directories_only);
    let plan = plan.bind_resolved_directory(&directory, "/")?;
    let edit = plan.edit(
        input,
        caret,
        &LiteralCandidate {
            name: "documents' archive".into(),
            path: "/documents' archive".into(),
            is_directory: true,
        },
    )?;
    let mut updated = input.to_owned();
    updated.replace_range(edit.range.clone(), &edit.replacement);
    assert!(updated.starts_with("printf 'before\\n'\ncat "));
    assert!(updated.ends_with(" --flag\nprintf 'after\\n'"));
    assert!(updated.is_char_boundary(edit.caret_byte));
    assert_eq!(
        parsed_arguments(&edit.replacement, scratch.path())?,
        b"1\0/documents' archive/notes with spaces.txt\0",
    );
    Ok(())
}

#[test]
fn command_candidates_are_literal_names_without_absolute_path_substitution() -> TestResult {
    let scratch = tempfile::tempdir()?;
    for name in ["if", "deploy's preview", "a=b", "$(touch marker)", "工具"] {
        let input = "";
        let mut plan = plan(input, 0)?;
        assert!(matches!(
            plan.query(None)?,
            CompletionQuery::Commands { .. }
        ));
        let edit = plan.edit(
            input,
            0,
            &LiteralCandidate {
                name: name.into(),
                path: format!("/bin/{name}"),
                is_directory: false,
            },
        )?;
        assert_eq!(
            parsed_arguments(&edit.replacement, scratch.path())?,
            format!("1\0{name}\0").as_bytes()
        );
        assert_eq!(std::fs::read_dir(scratch.path())?.count(), 0);
    }
    Ok(())
}

#[test]
fn completing_assignment_values_preserves_assignment_and_option_semantics() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let name = "value'$(touch marker)";
    for input in ["NAME=/", "tool --config=/"] {
        let mut plan = plan(input, input.len())?;
        let CompletionQuery::Paths { directory, .. } = plan.query(None)? else {
            return Err("expected an assignment value lookup".into());
        };
        let plan = plan.bind_resolved_directory(&directory, "/")?;
        let edit = plan.edit(
            input,
            input.len(),
            &LiteralCandidate {
                name: name.into(),
                path: format!("/{name}"),
                is_directory: false,
            },
        )?;
        let mut updated = input.to_owned();
        updated.replace_range(edit.range, &edit.replacement);
        let actual = if input.starts_with("NAME=") {
            let script = format!("{updated}\ncommand printf '%s\\000' \"$NAME\"");
            controlled_shell(&script, scratch.path())?
        } else {
            parsed_arguments(
                updated.strip_prefix("tool ").ok_or("lost command")?,
                scratch.path(),
            )?
        };
        let expected = if input.starts_with("NAME=") {
            format!("/{name}\0")
        } else {
            format!("1\0--config=/{name}\0")
        };
        assert_eq!(actual, expected.as_bytes());
        assert_eq!(std::fs::read_dir(scratch.path())?.count(), 0);
    }
    Ok(())
}
