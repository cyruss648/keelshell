//! An isolated, explicitly saved command-template draft. Opening it never reads
//! command history or copies the command bar, and saving never executes commands.

use gpui_kit::{
    component::input::{InputEvent, InputState, TextareaState},
    *,
};
use keelshell_core::{Snippet, SnippetTemplateError, ValidationError, compile_snippet_template};
use uuid::Uuid;

use crate::i18n::{Message, t};

#[cfg(test)]
mod tests;
mod view;

/// The workspace owns persistence and closes the editor only after a successful
/// save. Failed saves must call [`SnippetEditor::set_error`] to retain the draft.
pub enum SnippetEditorEvent {
    Save(Snippet),
    Cancel,
}

pub struct SnippetEditor {
    id: Uuid,
    editing: bool,
    focus: FocusHandle,
    name: Entity<InputState>,
    description: Entity<InputState>,
    tags: Entity<InputState>,
    command: Entity<TextareaState>,
    parameterized: bool,
    template_feedback: Option<TemplateFeedback>,
    _command_subscription: Subscription,
    saving: bool,
    error: Option<Message>,
}

struct TemplateFeedback {
    source: SharedString,
    variables: Vec<String>,
    error: Option<Message>,
}

fn field(value: String, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| {
        let mut input = InputState::new(window, cx);
        input.set_value(value, window, cx);
        input
    })
}

impl SnippetEditor {
    #[cfg(test)]
    pub(crate) fn set_command_text(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command
            .update(cx, |command, cx| command.set_value(text, window, cx));
    }

    /// Create a blank draft or preserve the identity and text of an existing one.
    pub fn new(snippet: Option<Snippet>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editing = snippet.is_some();
        let snippet = snippet.unwrap_or_else(|| Snippet::new("", ""));
        let command = cx.new(|cx| {
            let mut command = TextareaState::new(window, cx).rows(8);
            command.set_value(snippet.command, window, cx);
            command
        });
        let command_subscription = cx.subscribe(&command, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.error = None;
                cx.notify();
            }
        });
        let mut editor = Self {
            id: snippet.id,
            editing,
            focus: cx.focus_handle(),
            name: field(snippet.name, window, cx),
            description: field(snippet.description, window, cx),
            tags: field(format_tags(&snippet.tags), window, cx),
            command,
            parameterized: snippet.parameterized,
            template_feedback: None,
            _command_subscription: command_subscription,
            saving: false,
            error: None,
        };
        editor.refresh_locale(window, cx);
        editor.focus(window, cx);
        editor
    }

    /// Validate the current input values without changing the draft or writing it.
    pub fn draft(&self, cx: &App) -> Result<Snippet, ValidationError> {
        let snippet = Snippet {
            id: self.id,
            name: self.name.read(cx).value().to_string(),
            description: self.description.read(cx).value().to_string(),
            tags: parse_tags(self.tags.read(cx).value().as_ref())?,
            // Command whitespace is operational content, not UI formatting.
            command: self.command.read(cx).value().to_string(),
            parameterized: self.parameterized,
        };
        snippet.validate()?;
        Ok(snippet)
    }

    /// Freeze input immediately while the workspace persists the emitted snapshot.
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.saving = saving;
        for input in [&self.name, &self.description, &self.tags] {
            input.update(cx, |input, cx| input.set_disabled(saving, cx));
        }
        self.command
            .update(cx, |input, cx| input.set_disabled(saving, cx));
        cx.notify();
    }

    /// Whether closing or editing would discard an in-flight save outcome.
    pub fn is_saving(&self) -> bool {
        self.saving
    }

    /// Show a persistence failure and make the exact unsaved draft editable again.
    pub fn set_error(&mut self, message: Message, cx: &mut Context<Self>) {
        self.error = Some(message);
        self.set_saving(false, cx);
    }

    /// Update translated hints without replacing text, identity or keyboard focus.
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (input, zh, en) in [
            (
                &self.name,
                "例如：检查磁盘空间",
                "For example: Check disk space",
            ),
            (
                &self.description,
                "说明用途、执行前提或需要修改的参数",
                "Describe its purpose, prerequisites or parameters to review",
            ),
            (
                &self.tags,
                "例如：磁盘, 诊断",
                "For example: disk, diagnostics",
            ),
        ] {
            let placeholder = t(cx, zh, en);
            input.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
        let placeholder = t(
            cx,
            "输入需要保存的命令，可包含多行",
            "Enter the command to save; multiple lines are supported",
        );
        self.command.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        cx.notify();
    }

    /// Take modal keyboard focus without focusing an obscured terminal.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if self.saving {
            self.focus.focus(window, cx);
        } else {
            self.name.read(cx).focus_handle(cx).focus(window, cx);
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        match self.draft(cx) {
            Ok(snippet) => {
                self.error = None;
                self.set_saving(true, cx);
                self.focus.focus(window, cx);
                cx.emit(SnippetEditorEvent::Save(snippet));
            }
            Err(error) => {
                self.error = Some(
                    if self.parameterized && error.field.starts_with("snippet.command") {
                        let text = self.command.read(cx).value();
                        compile_snippet_template(&text)
                            .err()
                            .map(|error| template_error_message(&error, &text))
                            .unwrap_or_else(|| validation_message(&error))
                    } else {
                        validation_message(&error)
                    },
                );
                match error.field {
                    "snippet.name" => self.name.read(cx).focus_handle(cx),
                    "snippet.description" => self.description.read(cx).focus_handle(cx),
                    "tags" | "tag" => self.tags.read(cx).focus_handle(cx),
                    _ => self.command.read(cx).focus_handle(cx),
                }
                .focus(window, cx);
                cx.notify();
            }
        }
    }

    fn toggle_parameters(&mut self, cx: &mut Context<Self>) {
        if !self.saving {
            self.parameterized = !self.parameterized;
            self.error = None;
            cx.notify();
        }
    }

    fn refresh_template_feedback(&mut self, cx: &App) {
        if !self.parameterized {
            return;
        }
        let source = self.command.read(cx).value();
        if self
            .template_feedback
            .as_ref()
            .is_some_and(|feedback| feedback.source == source)
        {
            return;
        }
        let (variables, error) = match compile_snippet_template(&source) {
            Ok(template) => (template.variables().to_vec(), None),
            Err(error) => (Vec::new(), Some(template_error_message(&error, &source))),
        };
        self.template_feedback = Some(TemplateFeedback {
            source,
            variables,
            error,
        });
    }

    /// Cancel an idle draft. A pending save must finish before it can be closed.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if !self.saving {
            cx.emit(SnippetEditorEvent::Cancel);
        }
    }
}

impl EventEmitter<SnippetEditorEvent> for SnippetEditor {}

fn validation_message(error: &ValidationError) -> Message {
    match error.field {
        "snippet.name" => Message::new(
            "请输入名称，最多 120 个字符，不含首尾空白或控制字符。",
            "Enter a name of at most 120 characters, without surrounding whitespace or control characters.",
        ),
        "snippet.description" => Message::new(
            "说明最多 2048 个字符，不含首尾空白或控制字符。",
            "The description allows at most 2048 characters, without surrounding whitespace or control characters.",
        ),
        "tags" | "tag" => Message::new(
            "最多 32 个标签，每个最多 64 个字符。用逗号分隔；含逗号的标签用双引号包裹，内部双引号写两次。",
            "Use up to 32 tags of 64 characters each. Separate with commas; quote tags containing commas and double any embedded quotes.",
        ),
        _ => Message::new(
            "请输入命令，最多 64 KiB；仅允许换行和 Tab 控制字符。",
            "Enter a command of at most 64 KiB; newline and Tab are the only allowed control characters.",
        ),
    }
}

fn format_tags(tags: &[String]) -> String {
    tags.iter()
        .map(|tag| {
            if tag.contains([',', '"']) {
                format!("\"{}\"", tag.replace('"', "\"\""))
            } else {
                tag.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_tags(text: &str) -> Result<Vec<String>, ValidationError> {
    let invalid = || ValidationError {
        field: "tags",
        reason: "must use comma-separated text with balanced CSV quotes",
    };
    let mut tags = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        if let Some(quoted) = rest.strip_prefix('"') {
            let mut tag = String::new();
            let mut chars = quoted.char_indices().peekable();
            let mut closing = None;
            while let Some((offset, ch)) = chars.next() {
                if ch == '"' {
                    if chars.peek().is_some_and(|(_, ch)| *ch == '"') {
                        chars.next();
                        tag.push('"');
                    } else {
                        closing = Some(offset + ch.len_utf8());
                        break;
                    }
                } else {
                    tag.push(ch);
                }
            }
            let tail = quoted[closing.ok_or_else(invalid)?..].trim_start();
            if !tail.is_empty() && !tail.starts_with(',') {
                return Err(invalid());
            }
            tags.push(tag);
            rest = tail.strip_prefix(',').unwrap_or(tail).trim_start();
        } else {
            let (tag, tail) = rest.split_once(',').unwrap_or((rest, ""));
            let tag = tag.trim();
            if tag.contains('"') {
                return Err(invalid());
            }
            if !tag.is_empty() {
                tags.push(tag.to_owned());
            }
            rest = tail.trim_start();
        }
        if tags.len() > 32 {
            return Err(invalid());
        }
    }
    Ok(tags)
}

/// Shared bilingual template diagnostics, without echoing parameter values.
pub(crate) fn template_error_message(error: &SnippetTemplateError, source: &str) -> Message {
    use SnippetTemplateError as E;
    let at = |offset: usize| {
        let prefix = source.get(..offset).unwrap_or("");
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        (line, column)
    };
    match error {
        E::EmptyCommand => Message::new("请输入模板命令。", "Enter a template command."),
        E::CommandTooLong => Message::new(
            "模板命令不能超过 64 KiB。",
            "The template command must not exceed 64 KiB.",
        ),
        E::InvalidName { offset } => {
            let (line, column) = at(*offset);
            Message::new(
                format!(
                    "第 {line} 行、第 {column} 列的变量名无效。使用字母或下划线开头，之后可用数字，最多 64 字节。"
                ),
                format!(
                    "Invalid variable name at line {line}, column {column}. Use an ASCII letter or underscore first, then letters, digits or underscores; up to 64 bytes."
                ),
            )
        }
        E::MalformedPlaceholder { offset } => {
            let (line, column) = at(*offset);
            Message::new(
                format!("第 {line} 行、第 {column} 列的变量标记不完整。使用 {{{{name}}}}。"),
                format!("Malformed placeholder at line {line}, column {column}. Use {{{{name}}}}."),
            )
        }
        E::UnsupportedContext { offset, .. } => {
            let (line, column) = at(*offset);
            Message::new(
                format!(
                    "第 {line} 行、第 {column} 列不支持变量。请将 {{{{name}}}} 放在未加引号的完整参数，或 VAR= / --option= 后的完整值中。"
                ),
                format!(
                    "Unsupported variable context at line {line}, column {column}. Place {{{{name}}}} as an unquoted whole word, or the entire value after VAR= / --option=."
                ),
            )
        }
        E::TooManyVariables => Message::new(
            "每个模板最多使用 32 个不同变量。",
            "A template supports up to 32 distinct variables.",
        ),
        E::MissingValue { name } => Message::detail(
            "尚未填写参数（空值须明确选择）",
            "Parameter not filled (choose empty explicitly)",
            name,
        ),
        E::UnexpectedValue { name } => Message::detail(
            "模板不包含此参数",
            "Parameter is not part of the template",
            name,
        ),
        E::InvalidValueKey => Message::new(
            "参数名称无效，请重新打开片段。",
            "Invalid parameter name. Reopen the snippet.",
        ),
        E::ValueTooLong { name } => Message::detail(
            "参数值超过 4096 字节",
            "Parameter value exceeds 4096 bytes",
            name,
        ),
        E::InvalidValue { name } => Message::detail(
            "参数值包含不支持的控制字符",
            "Parameter contains unsupported control characters",
            name,
        ),
        E::RenderedTooLong => Message::new(
            "生成的完整命令超过 64 KiB，请缩短参数值。",
            "The rendered command exceeds 64 KiB. Shorten the values.",
        ),
    }
}
