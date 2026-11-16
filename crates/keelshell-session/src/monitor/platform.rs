//! Connection-scoped identity probes, independent of optional collectors.

use super::{
    ExecOutput, LinuxMonitor, MonitorEnvironment, MonitorError, MonitorPlatform, MonitorResult,
    MonitorRuntime, windows,
};

const PROBE_OUTPUT_LIMIT: usize = 4096;
pub(super) const POSIX_PROBE: &str = r#"LC_ALL=C; export LC_ALL
ks_uname=''
for ks_candidate in /usr/bin/uname /bin/uname; do
  if [ -f "$ks_candidate" ] && [ -x "$ks_candidate" ]; then ks_uname="$ks_candidate"; break; fi
done
[ -n "$ks_uname" ] || exit 66
ks_platform="$("$ks_uname" -s)" || exit 65
case "$ks_platform" in
  Linux|Darwin) ;;
  '') exit 65 ;;
  *) ks_platform=Unsupported ;;
esac
printf '@@KS:monitor-platform:1@@\nplatform\t%s\nruntime\tposix\n@@KS:end@@\n' "$ks_platform"
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Route {
    Posix(MonitorPlatform),
    WindowsPowerShell([u32; 4]),
    UnsupportedPosix,
}

impl Route {
    pub(super) fn environment(self) -> Option<MonitorEnvironment> {
        match self {
            Self::Posix(platform) => Some(MonitorEnvironment {
                platform,
                runtime: MonitorRuntime::Posix,
            }),
            Self::WindowsPowerShell(version) => Some(MonitorEnvironment {
                platform: MonitorPlatform::Windows,
                runtime: MonitorRuntime::WindowsPowerShell { version },
            }),
            Self::UnsupportedPosix => None,
        }
    }
}

fn parse_posix(output: &ExecOutput) -> Option<Route> {
    if output.exit_status != Some(0)
        || !output.stderr.is_empty()
        || output.stdout.len() > PROBE_OUTPUT_LIMIT
    {
        return None;
    }
    let text = std::str::from_utf8(&output.stdout)
        .ok()?
        .replace("\r\n", "\n");
    let platform = text
        .strip_prefix("@@KS:monitor-platform:1@@\nplatform\t")?
        .strip_suffix("\nruntime\tposix\n@@KS:end@@\n")?;
    match platform {
        "Linux" => Some(Route::Posix(MonitorPlatform::Linux)),
        "Darwin" => Some(Route::Posix(MonitorPlatform::Darwin)),
        "Unsupported" => Some(Route::UnsupportedPosix),
        _ => None,
    }
}

impl LinuxMonitor {
    pub(super) fn ensure_current(&self, generation: u64) -> MonitorResult<()> {
        if self.capabilities.is_current(generation) {
            Ok(())
        } else {
            Err(MonitorError::Cancelled)
        }
    }

    pub(super) async fn exec_current(
        &self,
        generation: u64,
        command: &str,
        max_bytes: usize,
    ) -> MonitorResult<ExecOutput> {
        self.ensure_current(generation)?;
        let output = self
            .session
            .exec_limited_guarded(command, max_bytes, || {
                self.capabilities.is_current(generation)
            })
            .await?;
        self.ensure_current(generation)?;
        output.ok_or(MonitorError::Cancelled)
    }

    pub(super) async fn route(&self, generation: u64) -> MonitorResult<Route> {
        self.ensure_current(generation)?;
        // Only the first caller probes. The lock wait is bounded separately
        // from each SSH command, which retains the connection's exec deadline.
        let _admission = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            self.capabilities.probe_gate.lock(),
        )
        .await
        .map_err(|_| {
            MonitorError::Session(crate::SessionError::Timeout(
                "monitoring platform probe admission",
            ))
        })?;
        self.ensure_current(generation)?;
        if let Some(route) = self.capabilities.route().await {
            self.ensure_current(generation)?;
            return Ok(route);
        }

        let posix = self
            .exec_current(generation, POSIX_PROBE, PROBE_OUTPUT_LIMIT)
            .await?;
        self.ensure_current(generation)?;
        let route = if let Some(route) = parse_posix(&posix) {
            route
        } else {
            // A shell error is not an OS identity. A second fixed probe may
            // establish Windows, but two failed probes leave it unconfirmed.
            let command = windows::command(windows::Operation::Probe)?;
            self.ensure_current(generation)?;
            let output = self
                .exec_current(generation, &command, PROBE_OUTPUT_LIMIT)
                .await?;
            self.ensure_current(generation)?;
            match windows::parse_probe(output) {
                Ok(version) => Route::WindowsPowerShell(version),
                Err(_) => return Err(MonitorError::EnvironmentUnconfirmed),
            }
        };
        if !self.capabilities.record_route(generation, route).await {
            return Err(MonitorError::Cancelled);
        }
        self.ensure_current(generation)?;
        Ok(route)
    }

    pub(super) async fn require_linux(&self, generation: u64) -> MonitorResult<()> {
        match self.route(generation).await? {
            Route::Posix(MonitorPlatform::Linux) => Ok(()),
            _ => Err(MonitorError::Unsupported(
                "this reviewed operation requires a confirmed Linux target",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(text: &str, status: Option<u32>) -> ExecOutput {
        ExecOutput {
            stdout: text.as_bytes().to_vec(),
            stderr: Vec::new(),
            exit_status: status,
        }
    }

    #[test]
    fn identity_requires_a_complete_successful_single_posix_frame() {
        let frame = "@@KS:monitor-platform:1@@\nplatform\tLinux\nruntime\tposix\n@@KS:end@@\n";
        assert_eq!(
            parse_posix(&output(frame, Some(0))),
            Some(Route::Posix(MonitorPlatform::Linux)),
        );
        assert_eq!(
            parse_posix(&output(&frame.replace('\n', "\r\n"), Some(0))),
            Some(Route::Posix(MonitorPlatform::Linux)),
        );
        for (text, status) in [
            (frame.to_owned(), None),
            (frame.to_owned(), Some(1)),
            (frame.replace("Linux", "Windows"), Some(0)),
            (frame.replace("posix", "PowerShell"), Some(0)),
            (format!("{frame}{frame}"), Some(0)),
            (format!("banner\n{frame}"), Some(0)),
            (format!("{frame}\n"), Some(0)),
            (frame.replace("@@KS:end@@\n", ""), Some(0)),
        ] {
            assert_eq!(parse_posix(&output(&text, status)), None);
        }
        let mut diagnostic = output(frame, Some(0));
        diagnostic.stderr = b"permission denied".to_vec();
        assert_eq!(parse_posix(&diagnostic), None);
    }

    #[test]
    fn only_a_complete_identity_can_establish_an_unsupported_posix_target() {
        let frame =
            "@@KS:monitor-platform:1@@\nplatform\tUnsupported\nruntime\tposix\n@@KS:end@@\n";
        assert_eq!(
            parse_posix(&output(frame, Some(0))),
            Some(Route::UnsupportedPosix),
        );
        assert_eq!(parse_posix(&output("command not found", Some(127))), None);
        assert_eq!(Route::UnsupportedPosix.environment(), None);
    }
}
