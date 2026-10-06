use gpui_kit::{
    component::{
        Disableable, Selectable,
        button::{Button, ButtonVariants},
        input::{Input, InputState},
    },
    *,
};
use keelshell_core::{
    AiApiStyle, AiMessagesEffort, AiMessagesInference, AiMessagesThinking, AiModelReasoning,
    AiModelSampling, AiReasoningCapability, AiReasoningSelection, AiSamplingValue, NamedAiProfile,
};
use uuid::Uuid;

use super::{AiSettingsPanel, field, view::label};
use crate::i18n::{Message, t};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct InferenceDraft(pub [String; 3]);
impl InferenceDraft {
    fn from_profile(profile: &NamedAiProfile, model: &str) -> Self {
        let budget = profile
            .reasoning_by_model
            .get(model)
            .and_then(|r| budget_tokens(&r.selection))
            .map_or_else(String::new, |n| n.to_string());
        let sampling = profile.sampling_by_model.get(model);
        Self([
            budget,
            sampling
                .and_then(|s| s.temperature)
                .map_or_else(String::new, AiSamplingValue::decimal),
            sampling
                .and_then(|s| s.top_p)
                .map_or_else(String::new, AiSamplingValue::decimal),
        ])
    }
    fn parse(
        &self,
        profile: &NamedAiProfile,
        model: &str,
    ) -> Result<(Option<u32>, AiModelSampling), ()> {
        let decimal = |s: &str| {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                AiSamplingValue::parse(s).map(Some).map_err(|_| ())
            }
        };
        let mut sampling = profile
            .sampling_by_model
            .get(model)
            .cloned()
            .unwrap_or_default();
        sampling.temperature = decimal(&self.0[1])?;
        sampling.top_p = decimal(&self.0[2])?;
        sampling.validate().map_err(|_| ())?;
        let budget = if profile
            .reasoning_by_model
            .get(model)
            .is_some_and(|r| budget_tokens(&r.selection).is_some())
        {
            let text = self.0[0].trim();
            if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
                return Err(());
            }
            let value: u32 = text.parse().map_err(|_| ())?;
            if !(1024..1_000_000).contains(&value) {
                return Err(());
            }
            Some(value)
        } else {
            None
        };
        Ok((budget, sampling))
    }
}

pub(super) fn inputs(window: &mut Window, cx: &mut App) -> [Entity<InputState>; 3] {
    [
        field("", "1024–999999", window, cx),
        field("", "0–2; 0.001", window, cx),
        field("", "0–1; 0.001", window, cx),
    ]
}

impl AiSettingsPanel {
    pub(super) fn load_inference_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let temperature_hint = if self
            .profile()
            .is_some_and(|profile| profile.api_style == AiApiStyle::AnthropicMessages)
        {
            "0–1; 0.001"
        } else {
            "0–2; 0.001"
        };
        let active = self.profile().map(|p| (p.id, p.model.clone()));
        if self.inference_active == active {
            return;
        }
        // Protocol/profile changes clear the editor identity. Updating only on
        // a cache miss avoids notifying the input from every render frame.
        self.inference_inputs[1].update(cx, |input, cx| {
            input.set_placeholder(temperature_hint, window, cx);
        });
        let draft = active
            .as_ref()
            .and_then(|key| self.inference_drafts.get(key).cloned())
            .unwrap_or_else(|| {
                self.profile().map_or_else(
                    || InferenceDraft(Default::default()),
                    |p| InferenceDraft::from_profile(p, &p.model),
                )
            });
        for (input, value) in self.inference_inputs.iter().zip(&draft.0) {
            input.update(cx, |input, cx| input.set_value(value.clone(), window, cx));
        }
        self.inference_values = draft;
        self.inference_active = active;
    }

    pub(super) fn sync_inference_editor(&mut self, cx: &mut Context<Self>) -> bool {
        let values = InferenceDraft(
            self.inference_inputs
                .each_ref()
                .map(|input| input.read(cx).value().to_string()),
        );
        if values == self.inference_values {
            return false;
        }
        self.inference_values = values;
        self.apply_inference_draft();
        true
    }

    fn apply_inference_draft(&mut self) {
        if let Some((id, model)) = self.inference_active.clone() {
            self.inference_drafts
                .insert((id, model.clone()), self.inference_values.clone());
            if let Some(profile) = self.catalog.profiles.iter_mut().find(|p| p.id == id)
                && let Ok((budget, sampling)) = self.inference_values.parse(profile, &model)
            {
                if let Some(budget) = budget
                    && let Some(reasoning) = profile.reasoning_by_model.get_mut(&model)
                {
                    match &mut reasoning.selection {
                        AiReasoningSelection::Messages(options) => {
                            options.thinking = AiMessagesThinking::LegacyBudget(budget)
                        }
                        selection => *selection = AiReasoningSelection::Budget(budget),
                    }
                }
                if sampling != AiModelSampling::default() {
                    profile.sampling_by_model.insert(model, sampling);
                } else {
                    profile.sampling_by_model.remove(&model);
                }
            }
        }
    }

    pub(super) fn inference_draft_valid(&self, id: Uuid) -> bool {
        let Some(profile) = self.catalog.profiles.iter().find(|p| p.id == id) else {
            return false;
        };
        self.inference_drafts
            .iter()
            .filter(|((owner, _), _)| *owner == id)
            .all(|((_, model), draft)| draft.parse(profile, model).is_ok())
    }

    fn choose_reasoning(
        &mut self,
        selection: AiReasoningSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_editor(cx);
        self.load_inference_editor(window, cx);
        if let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        {
            let preserve_budget_draft = profile.api_style == AiApiStyle::AnthropicMessages
                && matches!(
                    selection,
                    AiReasoningSelection::ProviderDefault | AiReasoningSelection::Effort(_)
                );
            let selection = if profile.api_style == AiApiStyle::AnthropicMessages {
                let mut options = profile
                    .reasoning_by_model
                    .get(&profile.model)
                    .and_then(|r| AiMessagesInference::from_selection(&r.selection).ok())
                    .unwrap_or_default();
                match selection {
                    AiReasoningSelection::ProviderDefault => options.effort = None,
                    AiReasoningSelection::Effort(value) => {
                        let Ok(effort) = AiMessagesEffort::parse(&value) else {
                            return;
                        };
                        options.effort = Some(effort);
                    }
                    AiReasoningSelection::Thinking(true) => {
                        options.thinking = AiMessagesThinking::Adaptive
                    }
                    AiReasoningSelection::Thinking(false) => {
                        options.thinking = AiMessagesThinking::Disabled
                    }
                    AiReasoningSelection::Budget(n) => {
                        options.thinking = AiMessagesThinking::LegacyBudget(n)
                    }
                    AiReasoningSelection::Messages(value) => options = value,
                    AiReasoningSelection::Text(_) => return,
                }
                if options == AiMessagesInference::default() {
                    AiReasoningSelection::ProviderDefault
                } else {
                    AiReasoningSelection::Messages(options)
                }
            } else {
                selection
            };
            let capability = match &selection {
                AiReasoningSelection::Effort(v) => AiReasoningCapability::Effort {
                    values: vec![v.clone()],
                },
                AiReasoningSelection::Messages(options) => AiReasoningCapability::Messages {
                    efforts: options.effort.into_iter().collect(),
                    adaptive: options.thinking == AiMessagesThinking::Adaptive,
                    disabled: options.thinking == AiMessagesThinking::Disabled,
                    manual_budget: matches!(options.thinking, AiMessagesThinking::LegacyBudget(_)),
                },
                AiReasoningSelection::Thinking(_) => AiReasoningCapability::ThinkingToggle,
                AiReasoningSelection::Budget(_) => AiReasoningCapability::TokenBudget {
                    min: 1024,
                    max: 999999,
                },
                _ => AiReasoningCapability::Unknown,
            };
            profile.reasoning_by_model.insert(
                profile.model.clone(),
                AiModelReasoning {
                    capability,
                    selection,
                },
            );
            let mut draft = self.inference_values.clone();
            if !preserve_budget_draft {
                draft.0[0] = profile
                    .reasoning_by_model
                    .get(&profile.model)
                    .and_then(|r| budget_tokens(&r.selection))
                    .map_or_else(String::new, |n| n.to_string());
            }
            self.inference_drafts
                .insert((profile.id, profile.model.clone()), draft);
        }
        self.inference_active = None;
        self.load_inference_editor(window, cx);
        self.changed(false, cx);
    }

    fn choose_messages_thinking_default(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        let mut options = self
            .profile()
            .and_then(|p| p.reasoning_by_model.get(&p.model))
            .and_then(|r| AiMessagesInference::from_selection(&r.selection).ok())
            .unwrap_or_default();
        options.thinking = AiMessagesThinking::ProviderDefault;
        self.choose_reasoning(AiReasoningSelection::Messages(options), window, cx);
    }

    fn toggle_sampling_support(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        self.load_inference_editor(window, cx);
        if let Some(profile) = self
            .selected
            .and_then(|id| self.catalog.profiles.iter_mut().find(|p| p.id == id))
        {
            let sampling = profile
                .sampling_by_model
                .entry(profile.model.clone())
                .or_default();
            sampling.declared_supported = !sampling.declared_supported;
        }
        // Retained raw values must be rechecked under the new declaration.
        self.apply_inference_draft();
        self.changed(false, cx);
    }

    pub(super) fn inference_error() -> Message {
        Message::new(
            "请修正模型推理/采样设置：先声明模型支持，采用一个采样参数；数值最多三位小数。",
            "Correct model inference/sampling settings: declare model support, use one sampling parameter and at most three decimal places.",
        )
    }

    pub(super) fn inference_view(
        &self,
        profile: &NamedAiProfile,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selection = profile
            .reasoning_by_model
            .get(&profile.model)
            .map(|r| &r.selection);
        let messages = (profile.api_style == AiApiStyle::AnthropicMessages).then(|| {
            selection
                .and_then(|s| AiMessagesInference::from_selection(s).ok())
                .unwrap_or_default()
        });
        let mut choices = div().flex().flex_wrap().gap_1().child(
            Button::new("ai-reasoning-default")
                .ghost()
                .label(if messages.is_some() {
                    t(cx, "努力值默认（省略）", "Default effort (omit)")
                } else {
                    t(cx, "服务端默认（省略）", "Provider default (omit)")
                })
                .selected(messages.as_ref().map_or_else(
                    || {
                        matches!(
                            selection,
                            None | Some(AiReasoningSelection::ProviderDefault)
                        )
                    },
                    |v| v.effort.is_none(),
                ))
                .on_click(cx.listener(|p, _, w, cx| {
                    p.choose_reasoning(AiReasoningSelection::ProviderDefault, w, cx)
                })),
        );
        let efforts: &[&str] = if profile.api_style == AiApiStyle::AnthropicMessages {
            &["low", "medium", "high", "xhigh", "max"]
        } else {
            &["none", "minimal", "low", "medium", "high", "xhigh", "max"]
        };
        for (index, effort) in efforts.iter().enumerate() {
            let effort = *effort;
            choices = choices.child(
                Button::new(("ai-reasoning-effort", index))
                    .ghost()
                    .label(effort)
                    .selected(
                        messages.as_ref().map_or_else(|| matches!(selection, Some(AiReasoningSelection::Effort(v)) if v == effort), |v| v.effort.is_some_and(|v| v.as_str() == effort)),
                    )
                    .on_click(cx.listener(move |p, _, w, cx| {
                        p.choose_reasoning(AiReasoningSelection::Effort(effort.into()), w, cx)
                    })),
            );
        }
        if let Some(messages) = &messages {
            choices = choices.child(
                Button::new("ai-thinking-default")
                    .ghost()
                    .label(t(cx, "思考模式默认（省略）", "Default thinking (omit)"))
                    .selected(messages.thinking == AiMessagesThinking::ProviderDefault)
                    .on_click(cx.listener(|p, _, w, cx| p.choose_messages_thinking_default(w, cx))),
            );
            for (id, zh, en, value) in [
                (
                    "ai-thinking-adaptive",
                    "自适应思考",
                    "Adaptive thinking",
                    AiReasoningSelection::Thinking(true),
                ),
                (
                    "ai-thinking-disabled",
                    "明确关闭思考",
                    "Disable thinking explicitly",
                    AiReasoningSelection::Thinking(false),
                ),
                (
                    "ai-thinking-budget",
                    "旧版思考预算",
                    "Legacy thinking budget",
                    AiReasoningSelection::Budget(1024),
                ),
            ] {
                let selected = match value {
                    AiReasoningSelection::Thinking(true) => {
                        messages.thinking == AiMessagesThinking::Adaptive
                    }
                    AiReasoningSelection::Thinking(false) => {
                        messages.thinking == AiMessagesThinking::Disabled
                    }
                    AiReasoningSelection::Budget(_) => {
                        matches!(messages.thinking, AiMessagesThinking::LegacyBudget(_))
                    }
                    _ => false,
                };
                choices =
                    choices.child(
                        Button::new(id)
                            .ghost()
                            .label(t(cx, zh, en))
                            .selected(selected)
                            .on_click(cx.listener(move |p, _, w, cx| {
                                p.choose_reasoning(value.clone(), w, cx)
                            })),
                    );
            }
        }
        let mut body = div().id("ai-inference-settings").test_support().flex().flex_col().gap_2().child(label(cx, "当前模型的推理与采样", "Inference and sampling for this model")).child(label(cx, "选择显式设置即声明已核对当前模型支持该选项。模型目录不保证能力；更换地址会清除声明。", "Selecting an explicit setting declares you checked this model's support. Model discovery does not verify capabilities; changing the endpoint clears declarations.")).child(choices);
        if selection.is_some_and(|s| budget_tokens(s).is_some()) {
            body = body
                .child(label(cx, "思考预算（Token）", "Thinking budget (tokens)"))
                .child(
                    Input::new(&self.inference_inputs[0])
                        .id("ai-inference-budget")
                        .aria_label(t(cx, "思考预算（Token）", "Thinking budget (tokens)")),
                );
        }
        if profile.api_style == AiApiStyle::AnthropicMessages {
            body = body.child(label(cx, "努力值与思考模式独立，可组合使用：output_config.effort 与 thinking.type。请核对模型支持所选组合；某些新模型拒绝关闭思考。旧版预算只用于明确支持的旧模型，必须至少 1024 且小于输出上限；新模型可能拒绝它。采样也只适用于明确支持的旧模型。", "Effort and thinking are independent and compose as output_config.effort and thinking.type. Check model support for the combination; some newer models reject disabled thinking. Legacy budgets need model support, at least 1024 and less than output limit; newer models may reject them. Sampling also requires a supporting legacy model."));
        } else {
            body = body.child(label(cx, "Chat 使用 reasoning_effort；Responses 使用 reasoning.effort。支持的努力值因模型而异。", "Chat uses reasoning_effort; Responses uses reasoning.effort. Supported efforts vary by model."));
        }
        body.child(Button::new("ai-sampling-support").ghost().label(t(cx, "已核对当前模型支持采样", "I checked this model supports sampling")).selected(profile.sampling_by_model.get(&profile.model).is_some_and(|s| s.declared_supported)).disabled(profile.model.is_empty()).on_click(cx.listener(|p, _, w, cx| p.toggle_sampling_support(w, cx))))
            .child(label(cx, "温度（空白省略）", "Temperature (blank omits)"))
            .child(Input::new(&self.inference_inputs[1]).id("ai-inference-temperature").aria_label(t(cx, "温度（空白省略）", "Temperature (blank omits)")))
            .child(label(cx, "Top P（空白省略）", "Top P (blank omits)"))
            .child(Input::new(&self.inference_inputs[2]).id("ai-inference-top-p").aria_label(t(cx, "Top P（空白省略）", "Top P (blank omits)")))
            .child(label(cx, "温度 0–2（Messages 0–1），Top P 0–1；最多三位小数。只填一个；不能与启用的推理混用。空白与显式 0 不同。请求被服务端拒绝时不会改参数重试。", "Temperature 0–2 (Messages 0–1), Top P 0–1; up to three decimals. Set only one; do not mix with active reasoning. Blank differs from explicit 0. Rejected requests are not retried with different parameters."))
            .into_any_element()
    }
}

fn budget_tokens(selection: &AiReasoningSelection) -> Option<u32> {
    match selection {
        AiReasoningSelection::Budget(n) => Some(*n),
        AiReasoningSelection::Messages(options) => options.budget_tokens(),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
