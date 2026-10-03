//! One explicit, local parameter review against an immutable snippet snapshot.
use std::collections::BTreeMap;

use crate::{
    i18n::{Message, t},
    snippet_editor::template_error_message,
};
use gpui_kit::{
    component::input::{InputEvent, TextareaState},
    *,
};
use keelshell_core::{Snippet, SnippetTemplate};

#[cfg(test)]
mod tests;
mod view;

/// The workspace must verify the source, target, and command revision before insertion.
pub enum SnippetParametersEvent {
    Rendered { snippet: Snippet, text: String },
    Cancel,
}

struct Parameter {
    name: String,
    value: Entity<TextareaState>,
    allow_empty: bool,
}

pub struct SnippetParameters {
    snippet: Snippet,
    target: String,
    template: Option<SnippetTemplate>,
    parameters: Vec<Parameter>,
    preview: Entity<TextareaState>,
    rendered: Option<String>,
    focus: FocusHandle,
    saving: bool,
    error: Option<Message>,
    validation: Option<Message>,
    _subscriptions: Vec<Subscription>,
}

impl SnippetParameters {
    /// Start with no supplied values. The snapshot and target label are immutable.
    pub fn new(
        snippet: Snippet,
        target: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (template, validation) = match snippet.compile_template() {
            Ok(Some(template)) => (Some(template), None),
            Ok(None) => (
                None,
                Some(Message::new(
                    "此片段未启用变量模式。",
                    "This snippet has parameter mode disabled.",
                )),
            ),
            Err(error) => (None, Some(template_error_message(&error, &snippet.command))),
        };
        let mut subscriptions = Vec::new();
        let parameters = template
            .as_ref()
            .map(|template| {
                template
                    .variables()
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        let value = cx.new(|cx| TextareaState::new(window, cx).rows(2));
                        subscriptions.push(cx.subscribe_in(
                            &value,
                            window,
                            move |view, _, event: &InputEvent, window, cx| {
                                if matches!(event, InputEvent::Change) && !view.saving {
                                    if let Some(parameter) = view.parameters.get_mut(index)
                                        && !parameter.value.read(cx).value().is_empty()
                                    {
                                        parameter.allow_empty = false;
                                    }
                                    view.error = None;
                                    view.refresh_preview(window, cx);
                                }
                            },
                        ));
                        Parameter {
                            name: name.clone(),
                            value,
                            allow_empty: false,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let preview = cx.new(|cx| {
            let mut preview = TextareaState::new(window, cx).rows(5);
            preview.set_readonly(true, cx);
            preview
        });
        let mut panel = Self {
            snippet,
            target,
            template,
            parameters,
            preview,
            rendered: None,
            focus: cx.focus_handle(),
            saving: false,
            error: None,
            validation,
            _subscriptions: subscriptions,
        };
        panel.refresh_locale(window, cx);
        panel.refresh_preview(window, cx);
        panel.focus(window, cx);
        panel
    }

    fn values(&self, cx: &App) -> BTreeMap<String, String> {
        self.parameters
            .iter()
            .filter_map(|parameter| {
                let value = parameter.value.read(cx).value();
                (!value.is_empty() || parameter.allow_empty)
                    .then(|| (parameter.name.clone(), value.to_string()))
            })
            .collect()
    }

    fn refresh_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(template) = &self.template else {
            return;
        };
        match template.render(&self.values(cx)) {
            Ok(text) => {
                self.validation = None;
                self.rendered = Some(text.clone());
                self.preview
                    .update(cx, |preview, cx| preview.set_value(text, window, cx));
            }
            Err(error) => {
                self.validation = Some(template_error_message(&error, &self.snippet.command));
                self.rendered = None;
                self.preview
                    .update(cx, |preview, cx| preview.set_value("", window, cx));
            }
        }
        cx.notify();
    }

    fn toggle_empty(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if let Some(parameter) = self.parameters.get_mut(index)
            && parameter.value.read(cx).value().is_empty()
        {
            parameter.allow_empty = !parameter.allow_empty;
            self.error = None;
            self.refresh_preview(window, cx);
        }
    }

    /// Freeze fields during the workspace's final insertion checks.
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.saving = saving;
        for parameter in &self.parameters {
            parameter
                .value
                .update(cx, |input, cx| input.set_disabled(saving, cx));
        }
        cx.notify();
    }
    /// Whether an emitted review is awaiting the workspace's decision.
    pub fn is_saving(&self) -> bool {
        self.saving
    }
    /// Preserve every value after a failed target, source, or draft check.
    pub fn set_error(&mut self, error: Message, cx: &mut Context<Self>) {
        self.error = Some(error);
        self.set_saving(false, cx);
    }

    /// Translate hints without changing parameter values, preview, or current focus.
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for parameter in &self.parameters {
            parameter.value.update(cx, |input, cx| {
                input.set_placeholder(
                    t(
                        cx,
                        "输入本次值，或明确选择空值",
                        "Enter a value, or explicitly choose empty",
                    ),
                    window,
                    cx,
                )
            });
        }
        self.preview.update(cx, |preview, cx| {
            preview.set_placeholder(
                t(
                    cx,
                    "填写全部参数后显示完整命令",
                    "Complete all parameters to preview the full command",
                ),
                window,
                cx,
            )
        });
        cx.notify();
    }
    /// Keep the modal's keyboard focus away from the terminal beneath it.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if !self.saving
            && let Some(parameter) = self.parameters.first()
        {
            parameter.value.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
    }
    /// Cancel without emitting rendered text or persisting values.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if !self.saving {
            cx.emit(SnippetParametersEvent::Cancel);
        }
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        // Read current inputs again: a Change event can still be queued behind a click.
        self.refresh_preview(window, cx);
        let Some(text) = self.rendered.clone() else {
            self.focus(window, cx);
            return;
        };
        self.error = None;
        self.set_saving(true, cx);
        self.focus.focus(window, cx);
        cx.emit(SnippetParametersEvent::Rendered {
            snippet: self.snippet.clone(),
            text,
        });
    }
}
impl EventEmitter<SnippetParametersEvent> for SnippetParameters {}
