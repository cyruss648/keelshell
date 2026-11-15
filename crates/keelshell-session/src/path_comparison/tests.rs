#![cfg(test)]

use super::*;

fn three_rows() -> PathComparisonReport {
    PathComparisonReport {
        status: PathComparisonStatus::Complete,
        samples: [30, 10, 20]
            .into_iter()
            .enumerate()
            .map(|(index, millis)| PathSample {
                path: PathSide::A,
                kind: PathProbeKind::Exec,
                round: index as u8 + 1,
                elapsed: Duration::from_millis(millis),
                bytes: 0,
                outcome: PathSampleOutcome::Passed,
            })
            .collect(),
        cleanup_unconfirmed: false,
    }
}

#[test]
fn summary_requires_complete_three_distinct_successful_rounds() {
    let report = three_rows();
    assert_eq!(
        report.summary(PathSide::A, PathProbeKind::Exec),
        Some(Duration::from_millis(20))
    );
    assert_eq!(report.summary(PathSide::B, PathProbeKind::Exec), None);
    for status in [
        PathComparisonStatus::Cancelled,
        PathComparisonStatus::Incomplete,
    ] {
        let mut report = three_rows();
        report.status = status;
        assert_eq!(report.summary(PathSide::A, PathProbeKind::Exec), None);
    }
    let mut unknown = three_rows();
    unknown.cleanup_unconfirmed = true;
    assert_eq!(unknown.summary(PathSide::A, PathProbeKind::Exec), None);
    let mut missing = three_rows();
    missing.samples.pop();
    assert_eq!(missing.summary(PathSide::A, PathProbeKind::Exec), None);
    let mut duplicate = three_rows();
    duplicate.samples[2].round = 1;
    assert_eq!(duplicate.summary(PathSide::A, PathProbeKind::Exec), None);
    let mut invalid_round = three_rows();
    invalid_round.samples[2].round = 0;
    assert_eq!(
        invalid_round.summary(PathSide::A, PathProbeKind::Exec),
        None
    );
    let mut failed = three_rows();
    failed.samples[1].outcome = PathSampleOutcome::Failed(PathProbeFailure::InvalidStdout);
    assert_eq!(failed.summary(PathSide::A, PathProbeKind::Exec), None);
}

#[test]
fn equal_and_zero_measured_durations_are_preserved_without_inventing_a_difference() {
    let mut report = three_rows();
    for row in &mut report.samples {
        row.elapsed = Duration::ZERO;
    }
    assert_eq!(
        report.summary(PathSide::A, PathProbeKind::Exec),
        Some(Duration::ZERO)
    );
}

#[cfg(unix)]
#[test]
fn fixed_python_programs_exchange_exact_memory_samples_and_reject_extra_upload()
-> Result<(), Box<dyn std::error::Error>> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    for (kind, extra_input) in [
        (PathProbeKind::Exec, false),
        (PathProbeKind::Upload, false),
        (PathProbeKind::Download, false),
        (PathProbeKind::Upload, true),
    ] {
        let script = kind
            .command()
            .strip_prefix("python3 -I -S -B -c '")
            .and_then(|script| script.strip_suffix('\''))
            .ok_or("invalid fixed Python envelope")?;
        assert!(script.contains("# KEELSHELL_PATH_PROBE"));
        let mut child = Command::new("python3")
            .args(["-I", "-S", "-B", "-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdin = child.stdin.take().ok_or("missing child stdin")?;
        let input = std::thread::spawn(move || -> std::io::Result<()> {
            if kind == PathProbeKind::Upload {
                stdin.write_all(&vec![PAYLOAD_BYTE; PAYLOAD_BYTES])?;
                if extra_input {
                    stdin.write_all(b"!")?;
                }
            }
            Ok(())
        });
        let output = child.wait_with_output()?;
        input.join().map_err(|_| "stdin thread failed")??;
        assert!(output.stderr.is_empty());
        if extra_input {
            assert_eq!(output.status.code(), Some(65));
            assert!(output.stdout.is_empty());
        } else {
            assert!(output.status.success());
            let expected = match kind {
                PathProbeKind::Exec => EXEC_REPLY.to_vec(),
                PathProbeKind::Upload => UPLOAD_REPLY.to_vec(),
                PathProbeKind::Download => vec![PAYLOAD_BYTE; PAYLOAD_BYTES],
            };
            assert_eq!(output.stdout, expected);
        }
    }
    Ok(())
}
