use std::collections::HashSet;

use serde_json::Value;
use zeroize::Zeroizing;

use super::{LocalAgentError, LocalAgentKind, LocalAgentLimits};

// The two exact checked Codex patches emit this frame with all tools disabled.
// The disabled provider cannot spawn the host. Only the selected-directory
// admission path recognizes it; supplier errors remain refusals everywhere else.
const DISABLED_HOST_WARNING: &str = "Code Mode is unavailable because code-mode host is disabled. Code mode will fail closed; enable `features.code_mode_host` and install `codex-code-mode-host`.";

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StartupWarning {
    #[serde(rename = "type")]
    kind: String,
    item: StartupWarningItem,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StartupWarningItem {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    message: String,
}

pub(super) struct AnswerStream {
    kind: LocalAgentKind,
    limits: LocalAgentLimits,
    frames: usize,
    initialized: bool,
    started: bool,
    completed: bool,
    item_ids: HashSet<String>,
    answer: Zeroizing<String>,
    selected_codex_warning: bool,
    warning_observed: bool,
}

impl AnswerStream {
    pub(super) fn new(kind: LocalAgentKind, limits: LocalAgentLimits) -> Self {
        Self {
            kind,
            limits,
            frames: 0,
            initialized: false,
            started: false,
            completed: false,
            item_ids: HashSet::new(),
            answer: Zeroizing::new(String::new()),
            selected_codex_warning: false,
            warning_observed: false,
        }
    }

    /// Internal admission has already verified the exact CLI and disabled host.
    pub(super) fn selected(kind: LocalAgentKind, limits: LocalAgentLimits) -> Self {
        let mut stream = Self::new(kind, limits);
        stream.selected_codex_warning = kind == LocalAgentKind::Codex;
        stream
    }

    pub(super) fn frame(&mut self, bytes: &[u8]) -> Result<(), LocalAgentError> {
        if self.completed || bytes.is_empty() {
            return Err(LocalAgentError::InvalidProtocol);
        }
        if bytes.len() > self.limits.line_bytes || self.frames >= self.limits.frames {
            return Err(LocalAgentError::OutputTooLarge);
        }
        let value: Value =
            serde_json::from_slice(bytes).map_err(|_| LocalAgentError::InvalidProtocol)?;
        self.frames += 1;
        match self.kind {
            LocalAgentKind::Codex => self.codex(&value, bytes),
            LocalAgentKind::ClaudeCode => self.claude(&value),
        }
    }

    pub(super) fn finish(self) -> Result<(Zeroizing<String>, usize), LocalAgentError> {
        if !self.completed {
            return Err(LocalAgentError::InvalidProtocol);
        }
        if self.answer.trim().is_empty() {
            return Err(LocalAgentError::EmptyReply);
        }
        Ok((self.answer, self.frames))
    }

    pub(super) fn started(&self) -> bool {
        self.started
    }

    pub(super) fn completed(&self) -> bool {
        self.completed
    }

    fn replace_answer(&mut self, text: &str) -> Result<(), LocalAgentError> {
        if text.len() > self.limits.answer_bytes {
            return Err(LocalAgentError::OutputTooLarge);
        }
        self.answer = Zeroizing::new(text.to_owned());
        Ok(())
    }

    fn codex(&mut self, value: &Value, bytes: &[u8]) -> Result<(), LocalAgentError> {
        match string(value, "type")? {
            "thread.started" if !self.initialized => {
                nonempty_string(value, "thread_id")?;
                self.initialized = true;
            }
            "item.completed"
                if self.selected_codex_warning
                    && self.initialized
                    && !self.started
                    && !self.warning_observed =>
            {
                // Decode the original bytes again with a strict shape. Value
                // alone would lose duplicate keys before this narrow admission.
                let warning: StartupWarning =
                    serde_json::from_slice(bytes).map_err(|_| LocalAgentError::InvalidProtocol)?;
                if warning.kind != "item.completed"
                    || warning.item.id != "item_0"
                    || warning.item.kind != "error"
                    || warning.item.message != DISABLED_HOST_WARNING
                    || !self.item_ids.insert(warning.item.id)
                {
                    return Err(LocalAgentError::InvalidProtocol);
                }
                self.warning_observed = true;
            }
            "turn.started" if self.initialized && !self.started => self.started = true,
            "item.started" | "item.updated" | "item.completed" if self.started => {
                let item = value
                    .get("item")
                    .filter(|item| item.is_object())
                    .ok_or(LocalAgentError::InvalidProtocol)?;
                let id = nonempty_string(item, "id")?;
                if id.len() > 256 {
                    return Err(LocalAgentError::InvalidProtocol);
                }
                match string(item, "type")? {
                    "agent_message" if string(value, "type")? == "item.completed" => {
                        if !self.item_ids.insert(id.to_owned()) {
                            return Err(LocalAgentError::InvalidProtocol);
                        }
                        // Final complete messages replace preliminary commentary;
                        // neither reasoning nor arbitrary tool output is an answer.
                        self.replace_answer(string(item, "text")?)?;
                    }
                    "agent_message" | "reasoning" => {}
                    _ => return Err(LocalAgentError::UnexpectedOperation),
                }
            }
            "turn.completed" if self.started => {
                if !value.get("usage").is_some_and(Value::is_object) {
                    return Err(LocalAgentError::InvalidProtocol);
                }
                self.completed = true;
            }
            "turn.failed" | "error" => return Err(LocalAgentError::InferenceFailed),
            _ => return Err(LocalAgentError::InvalidProtocol),
        }
        Ok(())
    }

    fn claude(&mut self, value: &Value) -> Result<(), LocalAgentError> {
        match string(value, "type")? {
            "system" if string(value, "subtype")? == "init" && !self.initialized => {
                for key in ["tools", "mcp_servers", "slash_commands", "skills"] {
                    if !value
                        .get(key)
                        .and_then(Value::as_array)
                        .is_some_and(Vec::is_empty)
                    {
                        return Err(LocalAgentError::UnexpectedOperation);
                    }
                }
                for key in ["plugin_errors", "mcp_server_errors"] {
                    if value
                        .get(key)
                        .is_some_and(|entries| !entries.as_array().is_some_and(Vec::is_empty))
                    {
                        return Err(LocalAgentError::UnexpectedOperation);
                    }
                }
                validate_claude_builtins(value)?;
                if value.get("analytics_disabled").and_then(Value::as_bool) != Some(true)
                    || value
                        .get("product_feedback_disabled")
                        .and_then(Value::as_bool)
                        != Some(true)
                    || string(value, "permissionMode")? != "default"
                    || string(value, "claude_code_version")? != "2.1.285"
                    || string(value, "apiKeySource")? != "ANTHROPIC_API_KEY"
                {
                    return Err(LocalAgentError::UnexpectedOperation);
                }
                self.initialized = true;
                self.started = true;
            }
            "system" if self.started && string(value, "subtype")? == "commands_changed" => {
                if !value
                    .get("commands")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
                {
                    return Err(LocalAgentError::UnexpectedOperation);
                }
            }
            "assistant" if self.started => {
                if !value.get("parent_tool_use_id").is_some_and(Value::is_null) {
                    return Err(LocalAgentError::UnexpectedOperation);
                }
                let message = value
                    .get("message")
                    .filter(|message| message.is_object())
                    .ok_or(LocalAgentError::InvalidProtocol)?;
                if string(message, "role")? != "assistant" {
                    return Err(LocalAgentError::InvalidProtocol);
                }
                let id = nonempty_string(message, "id")?;
                if id.len() > 256 || !self.item_ids.insert(id.to_owned()) {
                    return Err(LocalAgentError::InvalidProtocol);
                }
                let content = message
                    .get("content")
                    .and_then(Value::as_array)
                    .ok_or(LocalAgentError::InvalidProtocol)?;
                let mut answer = Zeroizing::new(String::new());
                for block in content {
                    match string(block, "type")? {
                        "text" => {
                            let text = string(block, "text")?;
                            if answer.len().saturating_add(text.len()) > self.limits.answer_bytes {
                                return Err(LocalAgentError::OutputTooLarge);
                            }
                            answer.push_str(text);
                        }
                        "thinking" | "redacted_thinking" => {}
                        _ => return Err(LocalAgentError::UnexpectedOperation),
                    }
                }
                self.replace_answer(&answer)?;
            }
            "result" if self.started => {
                if value.get("is_error").and_then(Value::as_bool) != Some(false)
                    || string(value, "subtype")? != "success"
                {
                    return Err(LocalAgentError::InferenceFailed);
                }
                if value.get("num_turns").and_then(Value::as_u64) != Some(1) {
                    return Err(LocalAgentError::UnexpectedOperation);
                }
                if value
                    .get("permission_denials")
                    .is_some_and(|denials| !denials.as_array().is_some_and(Vec::is_empty))
                {
                    return Err(LocalAgentError::UnexpectedOperation);
                }
                let result = string(value, "result")?;
                // Both a complete assistant message and final success receipt
                // must agree; never present a truncated stream as completed.
                if self.item_ids.len() != 1 || result != self.answer.as_str() {
                    return Err(LocalAgentError::InvalidProtocol);
                }
                self.completed = true;
            }
            "system" if string(value, "subtype")? == "api_retry" => {
                return Err(LocalAgentError::InferenceFailed);
            }
            "system" => return Err(LocalAgentError::UnexpectedOperation),
            "user" | "stream_event" | "tool_progress" | "tool_use_summary" => {
                return Err(LocalAgentError::UnexpectedOperation);
            }
            _ => return Err(LocalAgentError::InvalidProtocol),
        }
        Ok(())
    }
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, LocalAgentError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(LocalAgentError::InvalidProtocol)
}

// --bare still reports built-in metadata in 2.1.285. Only these packaged
// entries are admitted; installed plugins and additional agents are rejected.
// Their existence grants no tools: init and actual wire tests require tools=[].
fn validate_claude_builtins(value: &Value) -> Result<(), LocalAgentError> {
    let mut seen = HashSet::new();
    for plugin in value
        .get("plugins")
        .and_then(Value::as_array)
        .ok_or(LocalAgentError::InvalidProtocol)?
    {
        let name = string(plugin, "name")?;
        if !matches!(name, "cc-plugin-agents-md" | "cc-plugin-telemetry")
            || string(plugin, "path")? != "builtin"
            || string(plugin, "source")? != format!("{name}@builtin")
            || !seen.insert(name)
        {
            return Err(LocalAgentError::UnexpectedOperation);
        }
    }
    let mut seen = HashSet::new();
    for agent in value
        .get("agents")
        .and_then(Value::as_array)
        .ok_or(LocalAgentError::InvalidProtocol)?
    {
        let name = agent.as_str().ok_or(LocalAgentError::InvalidProtocol)?;
        if !matches!(
            name,
            "claude" | "Explore" | "general-purpose" | "Plan" | "statusline-setup"
        ) || !seen.insert(name)
        {
            return Err(LocalAgentError::UnexpectedOperation);
        }
    }
    Ok(())
}

fn nonempty_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, LocalAgentError> {
    let text = string(value, key)?;
    if text.trim().is_empty() {
        Err(LocalAgentError::InvalidProtocol)
    } else {
        Ok(text)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn codex_started(stream: &mut AnswerStream) {
        stream
            .frame(br#"{"type":"thread.started","thread_id":"thread"}"#)
            .unwrap();
        stream.frame(br#"{"type":"turn.started"}"#).unwrap();
    }

    fn claude_started(stream: &mut AnswerStream) {
        stream
            .frame(br#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[],"plugins":[],"agents":[],"skills":[],"slash_commands":[],"analytics_disabled":true,"product_feedback_disabled":true,"permissionMode":"default","claude_code_version":"2.1.285","apiKeySource":"ANTHROPIC_API_KEY"}"#)
            .unwrap();
    }

    #[test]
    fn codex_requires_success_and_classifies_only_complete_assistant_text() {
        let mut stream = AnswerStream::new(LocalAgentKind::Codex, LocalAgentLimits::default());
        codex_started(&mut stream);
        stream
            .frame(br#"{"type":"item.completed","item":{"id":"thinking","type":"reasoning","text":"hidden reasoning"}}"#)
            .unwrap();
        stream
            .frame("{\"type\":\"item.completed\",\"item\":{\"id\":\"answer\",\"type\":\"agent_message\",\"text\":\"完整中文回答\"}}".as_bytes())
            .unwrap();
        stream
            .frame(br#"{"type":"turn.completed","usage":{}}"#)
            .unwrap();
        assert_eq!(stream.finish().unwrap().0.as_str(), "完整中文回答");
    }

    fn disabled_host_warning() -> Value {
        serde_json::json!({"type":"item.completed","item":{"id":"item_0","type":"error","message":DISABLED_HOST_WARNING}})
    }

    fn selected_initialized() -> AnswerStream {
        let mut stream = AnswerStream::selected(LocalAgentKind::Codex, LocalAgentLimits::default());
        stream
            .frame(br#"{"type":"thread.started","thread_id":"owned"}"#)
            .unwrap();
        stream
    }

    #[test]
    fn selected_exact_disabled_host_warning_is_neither_an_answer_nor_success() {
        let warning = disabled_host_warning().to_string();
        let mut stream = selected_initialized();
        stream.frame(warning.as_bytes()).unwrap();
        assert!(!stream.started());
        assert!(!stream.completed());
        assert_eq!(
            stream.finish().unwrap_err(),
            LocalAgentError::InvalidProtocol
        );
        let mut stream = selected_initialized();
        stream.frame(warning.as_bytes()).unwrap();
        stream.frame(br#"{"type":"turn.started"}"#).unwrap();
        stream.frame(br#"{"type":"item.completed","item":{"id":"answer","type":"agent_message","text":"reviewable answer"}}"#).unwrap();
        stream
            .frame(br#"{"type":"turn.completed","usage":{}}"#)
            .unwrap();
        assert_eq!(stream.finish().unwrap().0.as_str(), "reviewable answer");
    }

    #[test]
    fn selected_warning_rejects_fields_duplicates_order_and_general_errors() {
        let exact = disabled_host_warning().to_string();
        let mut variations = Vec::new();
        for field in ["tool_calls", "unknown"] {
            let mut value = disabled_host_warning();
            value["item"][field] = serde_json::json!([]);
            variations.push(value.to_string());
        }
        let mut value = disabled_host_warning();
        value["extra"] = serde_json::json!(true);
        variations.push(value.to_string());
        let mut value = disabled_host_warning();
        value["item"]["message"] = serde_json::json!("Unexpected configuration or inference error");
        variations.push(value.to_string());
        for id in ["", " ", "item_1", "other-warning"] {
            let mut value = disabled_host_warning();
            value["item"]["id"] = serde_json::json!(id);
            variations.push(value.to_string());
        }
        variations.push(exact.replacen(
            "\"message\":",
            "\"message\":\"ignored duplicate\",\"message\":",
            1,
        ));
        variations.push(exact.replacen("\"item\":", "\"type\":\"item.completed\",\"item\":", 1));
        for frame in variations {
            assert!(
                selected_initialized().frame(frame.as_bytes()).is_err(),
                "malformed warning shape must refuse"
            );
        }
        let mut before = AnswerStream::selected(LocalAgentKind::Codex, LocalAgentLimits::default());
        assert!(before.frame(exact.as_bytes()).is_err());
        let mut duplicate = selected_initialized();
        duplicate.frame(exact.as_bytes()).unwrap();
        assert!(duplicate.frame(exact.as_bytes()).is_err());
        let mut late = selected_initialized();
        late.frame(br#"{"type":"turn.started"}"#).unwrap();
        assert!(late.frame(exact.as_bytes()).is_err());
        let mut isolated = AnswerStream::new(LocalAgentKind::Codex, LocalAgentLimits::default());
        isolated
            .frame(br#"{"type":"thread.started","thread_id":"owned"}"#)
            .unwrap();
        assert!(isolated.frame(exact.as_bytes()).is_err());
        let mut selected = selected_initialized();
        selected.frame(exact.as_bytes()).unwrap();
        selected.frame(br#"{"type":"turn.started"}"#).unwrap();
        assert_eq!(
            selected.frame(br#"{"type":"error","message":"inference failure"}"#),
            Err(LocalAgentError::InferenceFailed)
        );
    }

    #[test]
    fn codex_tool_and_duplicate_or_late_frames_fail_closed() {
        for kind in [
            "command_execution",
            "file_change",
            "mcp_tool_call",
            "web_search",
        ] {
            let mut stream = AnswerStream::new(LocalAgentKind::Codex, LocalAgentLimits::default());
            codex_started(&mut stream);
            let frame = serde_json::json!({"type":"item.started","item":{"id":"tool","type":kind}})
                .to_string();
            assert_eq!(
                stream.frame(frame.as_bytes()),
                Err(LocalAgentError::UnexpectedOperation)
            );
        }
        let mut stream = AnswerStream::new(LocalAgentKind::Codex, LocalAgentLimits::default());
        codex_started(&mut stream);
        let frame = br#"{"type":"item.completed","item":{"id":"answer","type":"agent_message","text":"answer"}}"#;
        stream.frame(frame).unwrap();
        assert_eq!(stream.frame(frame), Err(LocalAgentError::InvalidProtocol));
        stream
            .frame(br#"{"type":"turn.completed","usage":{}}"#)
            .unwrap();
        assert_eq!(stream.frame(frame), Err(LocalAgentError::InvalidProtocol));
    }

    #[test]
    fn claude_text_blocks_require_matching_final_success() {
        let mut stream = AnswerStream::new(LocalAgentKind::ClaudeCode, LocalAgentLimits::default());
        claude_started(&mut stream);
        stream.frame(br#"{"type":"assistant","parent_tool_use_id":null,"message":{"id":"message","role":"assistant","content":[{"type":"thinking","thinking":"private"},{"type":"text","text":"complete "},{"type":"text","text":"answer"}]}}"#).unwrap();
        stream.frame(br#"{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"complete answer","permission_denials":[]}"#).unwrap();
        assert_eq!(stream.finish().unwrap().0.as_str(), "complete answer");
    }

    #[test]
    fn claude_rejects_unreviewed_tools_hooks_parent_messages_and_partial_results() {
        let mut stream = AnswerStream::new(LocalAgentKind::ClaudeCode, LocalAgentLimits::default());
        assert_eq!(stream.frame(br#"{"type":"system","subtype":"init","tools":["Read"],"mcp_servers":[],"plugins":[],"agents":[],"skills":[],"slash_commands":[],"analytics_disabled":true,"product_feedback_disabled":true,"permissionMode":"default","claude_code_version":"2.1.285","apiKeySource":"ANTHROPIC_API_KEY"}"#), Err(LocalAgentError::UnexpectedOperation));
        let mut stream = AnswerStream::new(LocalAgentKind::ClaudeCode, LocalAgentLimits::default());
        assert_eq!(
            stream.frame(br#"{"type":"system","subtype":"hook_started"}"#),
            Err(LocalAgentError::UnexpectedOperation)
        );
        claude_started(&mut stream);
        assert_eq!(
            stream.frame(
                br#"{"type":"assistant","parent_tool_use_id":"unreviewed-child","message":{}}"#
            ),
            Err(LocalAgentError::UnexpectedOperation)
        );
        assert_eq!(stream.frame(br#"{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"missing assistant message"}"#), Err(LocalAgentError::InvalidProtocol));
    }

    #[test]
    fn claude_builtin_metadata_is_version_bound_and_nonessential_traffic_is_off() {
        let init = serde_json::json!({"type":"system","subtype":"init","tools":[],"mcp_servers":[],"slash_commands":[],"skills":[],"plugins":[{"name":"cc-plugin-agents-md","path":"builtin","source":"cc-plugin-agents-md@builtin"}],"agents":["claude","Explore","general-purpose","Plan","statusline-setup"],"analytics_disabled":true,"product_feedback_disabled":true,"permissionMode":"default","claude_code_version":"2.1.285","apiKeySource":"ANTHROPIC_API_KEY"});
        let mut stream = AnswerStream::new(LocalAgentKind::ClaudeCode, LocalAgentLimits::default());
        stream.frame(init.to_string().as_bytes()).unwrap();
        stream
            .frame(br#"{"type":"system","subtype":"commands_changed","commands":[]}"#)
            .unwrap();
        for (field, replacement) in [
            ("analytics_disabled", serde_json::json!(false)),
            ("product_feedback_disabled", serde_json::json!(false)),
            ("apiKeySource", serde_json::json!("subscription")),
            ("agents", serde_json::json!(["unreviewed-agent"])),
            (
                "plugins",
                serde_json::json!([{"name":"cc-plugin-agents-md","path":"installed","source":"cc-plugin-agents-md@builtin"}]),
            ),
        ] {
            let mut changed = init.clone();
            changed[field] = replacement;
            let mut stream =
                AnswerStream::new(LocalAgentKind::ClaudeCode, LocalAgentLimits::default());
            assert_eq!(
                stream.frame(changed.to_string().as_bytes()),
                Err(LocalAgentError::UnexpectedOperation)
            );
        }
        assert_eq!(
            stream.frame(
                br#"{"type":"system","subtype":"commands_changed","commands":["unreviewed"]}"#
            ),
            Err(LocalAgentError::UnexpectedOperation)
        );
        assert_eq!(
            stream.frame(br#"{"type":"system","subtype":"api_retry"}"#),
            Err(LocalAgentError::InferenceFailed)
        );
    }

    #[test]
    fn malformed_out_of_order_excessive_and_incomplete_streams_are_rejected() {
        let limits =
            LocalAgentLimits::new(std::time::Duration::from_secs(1), 1024, 512, 64, 2).unwrap();
        let mut stream = AnswerStream::new(LocalAgentKind::Codex, limits);
        assert_eq!(
            stream.frame(b"not-json-private-content"),
            Err(LocalAgentError::InvalidProtocol)
        );
        assert_eq!(
            stream.frame(br#"{"type":"turn.completed","usage":{}}"#),
            Err(LocalAgentError::InvalidProtocol)
        );
        let mut stream = AnswerStream::new(LocalAgentKind::Codex, limits);
        codex_started(&mut stream);
        assert_eq!(
            stream.frame(br#"{"type":"turn.completed","usage":{}}"#),
            Err(LocalAgentError::OutputTooLarge)
        );
        assert_eq!(
            stream.finish().err(),
            Some(LocalAgentError::InvalidProtocol)
        );
    }
}
