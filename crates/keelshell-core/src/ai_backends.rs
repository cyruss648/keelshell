//! Persisted AI execution choices. Validation is lexical and never starts a CLI.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

/// Specifically selected local CLI protocol, independent of an API preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiLocalAgent {
    /// OpenAI Codex CLI, admitted by runtime version and capability checks.
    Codex,
    /// Anthropic Claude Code, admitted by runtime version and capability checks.
    ClaudeCode,
}

/// User-facing, non-secret budgets for a single isolated local Ask invocation.
///
/// Values are whole seconds and KiB (1024 bytes), independent of model tokens.
/// Missing metadata keeps the original 120 s / 1024 KiB / 2048 KiB behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AiLocalAgentLimits {
    timeout_seconds: u16,
    answer_kib: u16,
    output_kib: u16,
}

impl Default for AiLocalAgentLimits {
    fn default() -> Self {
        Self {
            timeout_seconds: 120,
            answer_kib: 1024,
            output_kib: 2048,
        }
    }
}

impl<'de> Deserialize<'de> for AiLocalAgentLimits {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(default, deny_unknown_fields)]
        struct Wire {
            timeout_seconds: u16,
            answer_kib: u16,
            output_kib: u16,
        }
        impl Default for Wire {
            fn default() -> Self {
                let limits = AiLocalAgentLimits::default();
                Self {
                    timeout_seconds: limits.timeout_seconds,
                    answer_kib: limits.answer_kib,
                    output_kib: limits.output_kib,
                }
            }
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.timeout_seconds, wire.answer_kib, wire.output_kib)
            .map_err(serde::de::Error::custom)
    }
}

impl AiLocalAgentLimits {
    /// Validate 1–300 seconds, 1–1024 KiB answer and 1–8192 KiB total output.
    ///
    /// The answer must fit within the total stdout/stderr budget. Invalid values
    /// are rejected, never clamped or silently replaced by defaults.
    pub fn new(
        timeout_seconds: u16,
        answer_kib: u16,
        output_kib: u16,
    ) -> Result<Self, ValidationError> {
        if !(1..=300).contains(&timeout_seconds) {
            return Err(ValidationError::new(
                "ai.profile.backend.limits.timeout_seconds",
                "must be a whole number from 1 to 300",
            ));
        }
        if !(1..=1024).contains(&answer_kib) {
            return Err(ValidationError::new(
                "ai.profile.backend.limits.answer_kib",
                "must be a whole number from 1 to 1024",
            ));
        }
        if !(1..=8192).contains(&output_kib) || answer_kib > output_kib {
            return Err(ValidationError::new(
                "ai.profile.backend.limits.output_kib",
                "must be a whole number from 1 to 8192 and cover the answer budget",
            ));
        }
        Ok(Self {
            timeout_seconds,
            answer_kib,
            output_kib,
        })
    }

    /// Total local deadline, including version and capability admission.
    pub fn timeout_seconds(self) -> u16 {
        self.timeout_seconds
    }
    /// Maximum complete answer size in UTF-8 KiB, not a token count.
    pub fn answer_kib(self) -> u16 {
        self.answer_kib
    }
    /// Combined CLI stdout/stderr budget in KiB, including protocol overhead.
    pub fn output_kib(self) -> u16 {
        self.output_kib
    }
}

/// Execution metadata for a named profile; it never contains credentials or argv.
#[derive(Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiBackend {
    /// Use the selected HTTP body protocol. Older profiles keep this choice.
    #[default]
    Api,
    /// Run a specifically selected CLI using fixed arguments and isolated data.
    LocalAgent {
        /// The fixed protocol adapter; this is not a user-defined shell command.
        agent: AiLocalAgent,
        /// Absolute native executable path. No filesystem lookup occurs on load.
        executable: String,
        /// Total local deadline and bounded answer/protocol output budgets.
        limits: AiLocalAgentLimits,
    },
}

// Tagged unit variants otherwise discard unexpected fields, including secrets.
impl<'de> Deserialize<'de> for AiBackend {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Api {},
            LocalAgent {
                agent: AiLocalAgent,
                executable: String,
                #[serde(default)]
                limits: AiLocalAgentLimits,
            },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Api {} => Self::Api,
            Wire::LocalAgent {
                agent,
                executable,
                limits,
            } => Self::LocalAgent {
                agent,
                executable,
                limits,
            },
        })
    }
}

impl fmt::Debug for AiBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api => f.write_str("Api"),
            Self::LocalAgent { agent, .. } => f
                .debug_struct("LocalAgent")
                .field("agent", agent)
                .finish_non_exhaustive(),
        }
    }
}

impl AiBackend {
    /// Compare the destination authorized to receive an encrypted API key.
    ///
    /// A local deadline/output change cannot redirect a credential, so it is
    /// excluded. Adapter type and exact executable path remain bound. Request
    /// approvals must still compare the complete backend, including limits.
    pub fn same_credential_destination(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Api, Self::Api) => true,
            (
                Self::LocalAgent {
                    agent, executable, ..
                },
                Self::LocalAgent {
                    agent: other_agent,
                    executable: other_executable,
                    ..
                },
            ) => agent == other_agent && executable == other_executable,
            _ => false,
        }
    }

    /// Validate bounded, absolute metadata without reading files or credentials.
    ///
    /// Unix and Windows drive paths can be stored on either platform. Runtime
    /// admission still requires a usable native executable on the current host.
    /// Relative paths, shell launchers and control characters are rejected.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let Self::LocalAgent { executable, .. } = self else {
            return Ok(());
        };
        let bytes = executable.as_bytes();
        let unix_absolute = executable.starts_with('/');
        let windows_absolute = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\');
        let filename = executable.rsplit(['/', '\\']).next().unwrap_or_default();
        if executable.len() > 4096
            || executable.chars().any(char::is_control)
            || !(unix_absolute || windows_absolute)
            || matches!(filename, "" | "." | "..")
            || filename.rsplit_once('.').is_some_and(|(_, ext)| {
                ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat")
            })
        {
            return Err(ValidationError::new(
                "ai.profile.backend.executable",
                "must be a bounded absolute native executable path, not a shell launcher",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_local_limits_preserve_defaults_and_strict_numbers()
    -> Result<(), Box<dyn std::error::Error>> {
        let old =
            serde_json::json!({"kind":"local_agent", "agent":"codex", "executable":"/opt/codex"});
        let AiBackend::LocalAgent { limits, .. } = serde_json::from_value(old.clone())? else {
            panic!("local backend");
        };
        assert_eq!(limits, AiLocalAgentLimits::default());
        let mut partial = old.clone();
        partial["limits"] = serde_json::json!({"timeout_seconds": 30});
        let AiBackend::LocalAgent { limits, .. } = serde_json::from_value(partial)? else {
            panic!("local backend");
        };
        assert_eq!(limits, AiLocalAgentLimits::new(30, 1024, 2048)?);
        for invalid in [
            serde_json::json!(null),
            serde_json::json!({"timeout_seconds":0}),
            serde_json::json!({"timeout_seconds":301}),
            serde_json::json!({"timeout_seconds":1.5}),
            serde_json::json!({"timeout_seconds":"30"}),
            serde_json::json!({"timeout_seconds":-1}),
            serde_json::json!({"answer_kib":1025}),
            serde_json::json!({"answer_kib":0}),
            serde_json::json!({"output_kib":8193}),
            serde_json::json!({"output_kib":0}),
            serde_json::json!({"answer_kib":2,"output_kib":1}),
            serde_json::json!({"frames":512}),
            serde_json::json!({"credential":"private-fixture"}),
        ] {
            let mut wire = old.clone();
            wire["limits"] = invalid;
            assert!(serde_json::from_value::<AiBackend>(wire).is_err());
        }
        assert!(
            serde_json::from_value::<AiBackend>(serde_json::json!({"kind":"api","limits":{}}))
                .is_err()
        );
        for budget in [
            AiLocalAgentLimits::new(1, 1, 1)?,
            AiLocalAgentLimits::new(300, 1024, 8192)?,
        ] {
            assert_eq!(
                serde_json::from_value::<AiLocalAgentLimits>(serde_json::to_value(budget)?)?,
                budget
            );
        }
        Ok(())
    }

    #[test]
    fn credential_identity_excludes_budgets_but_request_equality_includes_them()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = AiBackend::LocalAgent {
            agent: AiLocalAgent::Codex,
            executable: "/opt/codex".into(),
            limits: AiLocalAgentLimits::default(),
        };
        let changed = AiBackend::LocalAgent {
            agent: AiLocalAgent::Codex,
            executable: "/opt/codex".into(),
            limits: AiLocalAgentLimits::new(30, 1, 2)?,
        };
        assert_ne!(original, changed);
        assert!(original.same_credential_destination(&changed));
        for other in [
            AiBackend::Api,
            AiBackend::LocalAgent {
                agent: AiLocalAgent::ClaudeCode,
                executable: "/opt/codex".into(),
                limits: AiLocalAgentLimits::default(),
            },
            AiBackend::LocalAgent {
                agent: AiLocalAgent::Codex,
                executable: "/opt/replacement".into(),
                limits: AiLocalAgentLimits::default(),
            },
        ] {
            assert!(!original.same_credential_destination(&other));
        }
        Ok(())
    }

    #[test]
    fn tagged_backends_reject_secret_and_shell_fields() {
        for value in [
            serde_json::json!({"kind":"api","api_key":"fixture-only"}),
            serde_json::json!({"kind":"local_agent","agent":"codex","executable":"/opt/codex","argv":["--unsafe"]}),
            serde_json::json!({"kind":"local_agent","agent":"codex","executable":"/opt/codex","credential":"fixture-only"}),
            serde_json::json!({"kind":"local_agent","agent":"arbitrary","executable":"/opt/codex"}),
            serde_json::Value::Null,
        ] {
            assert!(serde_json::from_value::<AiBackend>(value).is_err());
        }
    }

    #[test]
    fn native_path_metadata_is_portable_and_debug_hides_it()
    -> Result<(), Box<dyn std::error::Error>> {
        for executable in ["/opt/tools/codex", "C:\\Program Files\\Tools\\codex.exe"] {
            let backend = AiBackend::LocalAgent {
                agent: AiLocalAgent::Codex,
                executable: executable.to_owned(),
                limits: Default::default(),
            };
            backend.validate()?;
            assert_eq!(
                serde_json::from_str::<AiBackend>(&serde_json::to_string(&backend)?)?,
                backend
            );
            assert!(!format!("{backend:?}").contains(executable));
        }
        Ok(())
    }

    #[test]
    fn relative_control_and_batch_paths_fail_before_any_io() {
        for executable in [
            "codex",
            "~/bin/codex",
            "C:codex.exe",
            "/opt/tools/",
            "/opt/codex\n",
            "/opt/codex.cmd",
            "C:\\bin\\CLAUDE.BAT",
        ] {
            assert!(
                AiBackend::LocalAgent {
                    agent: AiLocalAgent::Codex,
                    executable: executable.to_owned(),
                    limits: Default::default(),
                }
                .validate()
                .is_err()
            );
        }
    }
}
