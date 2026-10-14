//! Bounded literal search and replacement for an unsaved remote editor draft.
use super::*;

const SEARCH_LIMIT: usize = 1024;
pub(super) const DRAFT_LIMIT: usize = 1024 * 1024;
const MATCH_LIMIT: usize = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EditorMatch {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EditorSearchError {
    Empty,
    Limit,
    Invalid,
}

impl EditorSearchError {
    fn message(self) -> Message {
        match self {
            Self::Empty => Message::new("请输入要查找的文本。", "Enter text to find."),
            Self::Limit => Message::new(
                "查找、替换或草稿超过编辑器上限。",
                "The query, replacement, or draft exceeds the editor limit.",
            ),
            Self::Invalid => Message::new(
                "编辑器只接受不含 NUL 的 UTF-8 文本。",
                "The editor accepts only UTF-8 text without NUL.",
            ),
        }
    }
}

fn valid_piece(value: &str, allow_empty: bool) -> Result<(), EditorSearchError> {
    if value.contains('\0') {
        return Err(EditorSearchError::Invalid);
    }
    if value.len() > SEARCH_LIMIT || (!allow_empty && value.is_empty()) {
        return Err(if value.is_empty() {
            EditorSearchError::Empty
        } else {
            EditorSearchError::Limit
        });
    }
    Ok(())
}

pub(super) fn editor_matches(
    draft: &str,
    query: &str,
) -> Result<Vec<EditorMatch>, EditorSearchError> {
    valid_piece(query, false)?;
    if draft.contains('\0') || draft.len() > DRAFT_LIMIT {
        return Err(if draft.contains('\0') {
            EditorSearchError::Invalid
        } else {
            EditorSearchError::Limit
        });
    }
    let mut matches = Vec::new();
    let mut start = 0;
    while let Some(offset) = draft[start..].find(query) {
        if matches.len() == MATCH_LIMIT {
            return Err(EditorSearchError::Limit);
        }
        let found = start + offset;
        matches.push(EditorMatch {
            start: found,
            end: found + query.len(),
        });
        start = found + query.len();
    }
    Ok(matches)
}

pub(super) fn replace_editor_draft(
    draft: &str,
    query: &str,
    replacement: &str,
    one: bool,
) -> Result<String, EditorSearchError> {
    valid_piece(replacement, true)?;
    let matches = editor_matches(draft, query)?;
    if matches.is_empty() {
        return Ok(draft.to_owned());
    }
    let selected = if one { &matches[..1] } else { &matches[..] };
    let mut next = String::with_capacity(draft.len());
    let mut copied = 0;
    for item in selected {
        next.push_str(&draft[copied..item.start]);
        next.push_str(replacement);
        copied = item.end;
    }
    next.push_str(&draft[copied..]);
    if next.len() > DRAFT_LIMIT || next.contains('\0') {
        return Err(if next.contains('\0') {
            EditorSearchError::Invalid
        } else {
            EditorSearchError::Limit
        });
    }
    Ok(next)
}

impl FilesPanel {
    pub(super) fn editor_search_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("editor-search-controls")
            .min_w_0()
            .flex()
            .flex_shrink(1.)
            .items_center()
            .gap_1()
            .child(
                div()
                    .w(px(96.))
                    .min_w_0()
                    .flex_shrink(1.)
                    .child(Input::new(&self.editor_find).h(px(28.)).text_xs()),
            )
            .child(
                div()
                    .w(px(96.))
                    .min_w_0()
                    .flex_shrink(1.)
                    .child(Input::new(&self.editor_replace).h(px(28.)).text_xs()),
            )
            .child(
                Button::new("find-editor-text")
                    .ghost()
                    .compact()
                    .disabled(self.suspended)
                    .label(t(cx, "查找下一个", "Find next"))
                    .on_click(cx.listener(|view, _, _, cx| view.find_in_editor(cx))),
            )
            .child(
                Button::new("replace-editor-match")
                    .ghost()
                    .compact()
                    .disabled(self.suspended || self.busy || self.pending.is_some())
                    .label(t(cx, "替换当前", "Replace"))
                    .on_click(
                        cx.listener(|view, _, window, cx| view.replace_in_editor(true, window, cx)),
                    ),
            )
            .child(
                Button::new("replace-editor-matches")
                    .ghost()
                    .compact()
                    .disabled(self.suspended || self.busy || self.pending.is_some())
                    .label(t(cx, "全部替换", "Replace all"))
                    .on_click(
                        cx.listener(|view, _, window, cx| {
                            view.replace_in_editor(false, window, cx)
                        }),
                    ),
            )
            .test_support()
            .into_any_element()
    }

    pub(super) fn find_in_editor(&mut self, cx: &mut Context<Self>) {
        let draft = self.editor.read(cx).value().to_string();
        let query = self.editor_find.read(cx).value().to_string();
        match editor_matches(&draft, &query) {
            Ok(matches) => {
                self.editor_match = if matches.is_empty() {
                    None
                } else {
                    Some(
                        self.editor_match
                            .map_or(0, |index| (index + 1) % matches.len()),
                    )
                };
                let count = matches.len();
                self.status = if count == 0 {
                    Message::new(
                        "没有匹配。远端文件未改变。",
                        "No matches. Remote file unchanged.",
                    )
                } else {
                    let shown = self.editor_match.unwrap_or(0) + 1;
                    Message::new(
                        format!("第 {shown}/{count} 处匹配。远端文件未改变。"),
                        format!("Match {shown}/{count}. Remote file unchanged."),
                    )
                };
            }
            Err(error) => self.status = error.message(),
        }
        cx.notify();
    }

    pub(super) fn replace_in_editor(
        &mut self,
        one: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending.is_some() || self.suspended || self.editing.is_none() {
            return;
        }
        let draft = self.editor.read(cx).value().to_string();
        let query = self.editor_find.read(cx).value().to_string();
        let replacement = self.editor_replace.read(cx).value().to_string();
        match replace_editor_draft(&draft, &query, &replacement, one) {
            Ok(next) if next == draft => {
                self.status = Message::new("没有可替换的匹配。", "No match to replace.");
            }
            Ok(next) => {
                self.editor
                    .update(cx, |editor, cx| editor.set_value(next, window, cx));
                self.diff_preview = None;
                self.status = Message::new(
                    "已替换本地草稿；远端文件未改变，保存前仍需审核。",
                    "Draft replaced locally; remote unchanged and save still requires review.",
                );
            }
            Err(error) => self.status = error.message(),
        }
        cx.notify();
    }
}
