//! Explicit read intentions remain separate from an open document's byte envelope.
use super::*;
use gpui_kit::component::popover::Popover;

const CHOICES: [Option<TextEncoding>; 6] = [
    None,
    Some(TextEncoding::Gbk),
    Some(TextEncoding::Gb18030),
    Some(TextEncoding::Big5),
    Some(TextEncoding::ShiftJis),
    Some(TextEncoding::Windows1252),
];

/// A queued read cannot move onto a replacement connection or a new read choice.
pub(super) struct ReadBinding {
    token: uuid::Uuid,
    connection: SshSession,
    revision: uuid::Uuid,
    choice: Option<TextEncoding>,
}

impl ReadBinding {
    pub(super) fn capture(panel: &FilesPanel) -> Option<Self> {
        let connection = panel.session.clone().filter(|_| !panel.suspended)?;
        Some(Self {
            token: panel.session_token,
            connection,
            revision: panel.editor_read_revision,
            choice: panel.editor_read_encoding,
        })
    }

    pub(super) fn is_current(&self, panel: &FilesPanel) -> bool {
        !panel.suspended
            && panel.session_token == self.token
            && panel.editor_read_revision == self.revision
            && panel.editor_read_encoding == self.choice
            && panel
                .session
                .as_ref()
                .is_some_and(|session| session.same_connection(&self.connection))
    }
}

/// Explicit rereads bind the document being discarded, not only its visible text.
pub(super) struct ReadContext {
    binding: ReadBinding,
    editing: Option<(String, Vec<u8>)>,
    snapshot: Option<RegularFileSnapshot>,
    format: Option<TextFormat>,
    reload_required: bool,
    draft: String,
}

impl ReadContext {
    pub(super) fn capture(panel: &FilesPanel, cx: &App) -> Option<Self> {
        Some(Self {
            binding: ReadBinding::capture(panel)?,
            editing: panel.editing.clone(),
            snapshot: panel.editor_snapshot.clone(),
            format: panel.editor_format,
            reload_required: panel.editor_reload_required,
            draft: panel.editor.read(cx).value().to_string(),
        })
    }

    pub(super) fn is_current(&self, panel: &FilesPanel, cx: &App) -> bool {
        self.binding.is_current(panel)
            && panel.editing == self.editing
            && same_snapshot(panel.editor_snapshot.as_ref(), self.snapshot.as_ref())
            && panel.editor_format == self.format
            && panel.editor_reload_required == self.reload_required
            && panel.editor.read(cx).value().as_ref() == self.draft
    }
}

fn same_snapshot(left: Option<&RegularFileSnapshot>, right: Option<&RegularFileSnapshot>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.content == right.content
                && left.entry.path == right.entry.path
                && left.entry.name == right.entry.name
                && left.entry.size == right.entry.size
                && left.entry.permissions == right.entry.permissions
                && left.entry.modified == right.entry.modified
                && left.entry.is_directory == right.entry.is_directory
                && left.entry.is_symlink == right.entry.is_symlink
        }
        _ => false,
    }
}

pub(super) fn choice_name(choice: Option<TextEncoding>, cx: &App) -> &'static str {
    choice.map_or_else(|| t(cx, "Unicode 自动", "Unicode auto"), TextEncoding::name)
}

pub(super) fn decode_read(
    raw: &[u8],
    choice: Option<TextEncoding>,
) -> Result<TextDocument, FileFailure> {
    choice
        .map_or_else(
            || TextDocument::decode(raw),
            |encoding| TextDocument::decode_with_encoding(raw, encoding),
        )
        .map_err(FileFailure::TextDocument)
}

pub(super) fn decode_observation(
    raw: &[u8],
    format: TextFormat,
) -> Result<TextDocument, keelshell_core::TextDocumentError> {
    if format.encoding().is_legacy() {
        TextDocument::decode_with_encoding(raw, format.encoding())
    } else {
        TextDocument::decode(raw)
    }
}

impl FilesPanel {
    fn encoding_choice_available(&self) -> bool {
        !self.suspended && self.operation_id.is_none() && self.pending.is_none()
    }

    pub(super) fn select_editor_read_encoding(
        &mut self,
        choice: Option<TextEncoding>,
        binding: &ReadBinding,
        cx: &mut Context<Self>,
    ) {
        if !self.encoding_choice_available()
            || !binding.is_current(self)
            || !CHOICES.contains(&choice)
        {
            return;
        }
        self.editor_read_encoding = choice;
        self.editor_read_revision = uuid::Uuid::new_v4();
        self.editor_encoding_open = false;
        self.status = Message::new(
            "读取编码只用于下一次明确打开；当前草稿和保存编码保持。重新打开脏草稿仍需确认。",
            "The read encoding applies to the next explicit open. The current draft and save encoding are retained; reopening a dirty draft still needs confirmation.",
        );
        cx.notify();
    }

    pub(super) fn editor_encoding_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let binding = ReadBinding::capture(self);
        let panel = cx.entity().downgrade();
        let trigger = Button::new("file-read-encoding")
            .ghost()
            .small()
            .compact()
            .flex_shrink_0()
            .rounded(px(crate::design::components(cx).control_radius))
            .selected(self.editor_encoding_open)
            .toggled(self.editor_encoding_open)
            .label(choice_name(self.editor_read_encoding, cx))
            .accessibility_label(format!(
                "{}: {}",
                t(cx, "下一次读取编码", "Next read encoding"),
                choice_name(self.editor_read_encoding, cx)
            ))
            .icon(if self.editor_encoding_open {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            })
            .disabled(!self.encoding_choice_available());
        Popover::new("file-read-encoding-popover")
            .anchor(Anchor::BottomRight)
            .open(self.editor_encoding_open && self.encoding_choice_available())
            .trigger(trigger)
            .on_open_change(cx.listener(move |panel, open: &bool, _, cx| {
                if *open
                    && (!panel.encoding_choice_available()
                        || binding
                            .as_ref()
                            .is_none_or(|binding| !binding.is_current(panel)))
                {
                    return;
                }
                panel.editor_encoding_open = *open;
                cx.notify();
            }))
            .content(move |_, window, cx| {
                let content = panel
                    .update(cx, |panel, cx| panel.editor_encoding_options(cx))
                    .unwrap_or_else(|_| div().into_any_element());
                div()
                    .id("file-read-encoding-scroll")
                    .w(px(360.).min((window.viewport_size().width - px(48.)).max(px(100.))))
                    .max_h(px(240.).min((window.viewport_size().height - px(80.)).max(px(100.))))
                    .overflow_y_scroll()
                    .child(content)
            })
            .into_any_element()
    }

    pub(super) fn editor_encoding_options(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut choices = div().flex().flex_wrap().items_center().gap_1().child(t(
            cx,
            "读取编码",
            "Read encoding",
        ));
        for (index, choice) in CHOICES.into_iter().enumerate() {
            let binding = ReadBinding::capture(self);
            choices = choices.child(
                Button::new(("file-read-encoding-choice", index))
                    .ghost()
                    .small()
                    .compact()
                    .rounded(px(crate::design::components(cx).control_radius))
                    .label(choice_name(choice, cx))
                    .accessibility_label(format!(
                        "{} {}",
                        t(cx, "下一次读取使用", "Use for next read"),
                        choice_name(choice, cx)
                    ))
                    .selected(self.editor_read_encoding == choice)
                    .toggled(self.editor_read_encoding == choice)
                    .disabled(!self.encoding_choice_available())
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        if let Some(binding) = &binding {
                            panel.select_editor_read_encoding(choice, binding, cx);
                        }
                    })),
            );
        }
        let current = self.editor_format.map(|format| format.encoding().name());
        div()
            .id("file-read-encoding-options")
            .flex_shrink_0()
            .min_w_0()
            .px_2()
            .py_1()
            .flex()
            .flex_col()
            .gap_1()
            .border_b_1()
            .border_color(rgb(visual.border))
            .child(choices)
            .child(
                div()
                    .min_w_0()
                    .text_color(rgb(visual.muted))
                    .child(match current {
                        Some(name) => match crate::i18n::language(cx) {
                            keelshell_core::Language::ZhCn => format!("当前文档保持 {name}；选项仅用于重新读取。"),
                            keelshell_core::Language::En => format!("Current document retains {name}. The choice applies only to a new read."),
                        },
                        None => t(
                            cx,
                            "Unicode 自动不猜测旧编码；旧编码必须明确选择，无损往返验证失败时拒绝打开。",
                            "Unicode auto does not guess legacy encodings. Explicit legacy reads require a lossless round trip.",
                        )
                        .to_owned(),
                    }),
            )
            .test_support()
            .into_any_element()
    }
}
