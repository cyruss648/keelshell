//! Shared transient fields; no serialization, history, audit or transport access.
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Textarea, TextareaState},
    },
    *,
};
use keelshell_core::{BatchParameterError, BatchParameterValues, BatchParameterizedTemplate};

use crate::i18n::{Message, t};

pub(crate) struct ParameterField {
    pub name: String,
    pub value: Entity<TextareaState>,
    pub subscription: Option<Subscription>,
    pub allow_empty: bool,
}

pub(crate) fn names(source: &str) -> Result<Vec<String>, Message> {
    BatchParameterizedTemplate::compile(source)
        .map(|template| template.map_or_else(Vec::new, |template| template.parameters().to_vec()))
        .map_err(|error| error_message(&error))
}

/// Explicit synchronization retains same-name values and discards removed values.
/// Empty fields remain missing until the user explicitly enables an empty literal.
pub(crate) fn synchronize(
    fields: &mut Vec<ParameterField>,
    names: &[String],
    window: &mut Window,
    cx: &mut App,
) {
    fields.retain(|field| names.contains(&field.name));
    for name in names {
        if fields.iter().any(|field| field.name == *name) {
            continue;
        }
        fields.push(ParameterField {
            name: name.clone(),
            value: cx.new(|cx| TextareaState::new(window, cx).rows(2)),
            subscription: None,
            allow_empty: false,
        });
    }
}

pub(crate) fn values(fields: &[ParameterField], cx: &App) -> Result<BatchParameterValues, Message> {
    BatchParameterValues::new(
        fields
            .iter()
            .filter_map(|field| {
                let value = field.value.read(cx).value();
                (!value.is_empty() || field.allow_empty)
                    .then(|| (field.name.clone(), value.to_string()))
            })
            .collect(),
    )
    .map_err(|error| error_message(&error))
}

pub(crate) fn error_message(error: &BatchParameterError) -> Message {
    use keelshell_core::SnippetTemplateError as E;
    match error {
        BatchParameterError::Template(E::MissingValue { name }) => Message::new(
            format!("缺少参数 {name}：先同步字段并填写此目标的值。"),
            format!("Missing parameter {name}: sync fields and enter this target's value."),
        ),
        BatchParameterError::Template(E::UnexpectedValue { name }) => Message::new(
            format!("参数 {name} 已不使用：同步字段以移除旧值。"),
            format!("Parameter {name} is unused: sync fields to remove its old value."),
        ),
        BatchParameterError::DuplicateValue { name } => Message::new(
            format!("参数名称重复：{name}。"),
            format!("Duplicate parameter name: {name}."),
        ),
        BatchParameterError::ReservedName { name } => Message::new(
            format!("{name} 是连接元数据，不能覆盖。"),
            format!("{name} is connection metadata and cannot be overridden."),
        ),
        BatchParameterError::Template(E::ValueTooLong { name }) => Message::new(
            format!("参数 {name} 的值超过 4 KiB。"),
            format!("The value for {name} exceeds 4 KiB."),
        ),
        BatchParameterError::Template(E::InvalidValue { name }) => Message::new(
            format!("参数 {name} 含不支持的控制字符。"),
            format!("Parameter {name} contains unsupported controls."),
        ),
        BatchParameterError::Template(E::InvalidValueKey | E::InvalidName { .. }) => Message::new(
            "参数名称须为 1–64 个 ASCII 字母、数字或下划线，且不能以数字开头。",
            "Names require 1–64 ASCII letters, digits or underscores and cannot start with a digit.",
        ),
        BatchParameterError::TooManyValues | BatchParameterError::Template(E::TooManyVariables) => {
            Message::new(
                "每个目标最多 32 个参数；每条模板的元数据与用户名称合计也最多 32 个。",
                "At most 32 parameters per target and 32 combined metadata/user names per template.",
            )
        }
        _ => Message::detail(
            "参数模板或展开无效",
            "Invalid parameter template or rendering",
            error,
        ),
    }
}

pub(crate) fn view<P: 'static>(
    fields: &[ParameterField],
    target: &str,
    prefix: &str,
    target_id: uuid::Uuid,
    cx: &mut Context<P>,
    toggle: fn(&mut P, uuid::Uuid, usize, &mut Context<P>),
) -> Div {
    let visual = crate::design::palette(cx);
    let mut view = div().min_w_0().flex_shrink_0().flex().flex_col().gap_2();
    for (index, field) in fields.iter().enumerate() {
        let label = format!("{} · {{{{{}}}}}", target, field.name);
        view = view.child(
            div()
                .flex_shrink_0()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(visual.muted))
                        .child(field.name.clone()),
                )
                .child(
                    div()
                        .id(format!("{prefix}-{}", field.name))
                        .test_support()
                        .h(px(70.))
                        .flex_shrink_0()
                        .min_w_0()
                        .child(Textarea::new(&field.value).aria_label(label).h_full()),
                )
                .child(
                    Button::new(format!("{prefix}-empty-{}", field.name))
                        .ghost()
                        .small()
                        .label(format!(
                            "{} {}",
                            if field.allow_empty { "☑" } else { "☐" },
                            t(cx, "明确使用空字符串", "Use an explicit empty string")
                        ))
                        .disabled(!field.value.read(cx).value().is_empty())
                        .on_click(
                            cx.listener(move |panel, _, _, cx| toggle(panel, target_id, index, cx)),
                        ),
                ),
        );
    }
    view
}

pub(crate) fn hint(cx: &App) -> Div {
    div().flex_shrink_0().text_xs().text_color(rgb(crate::design::palette(cx).muted))
        .child(t(cx,
            "在命令中用 {{path}} 等用户名称，选择目标后同步字段。同名值仅属于该会话，空值需勾选明确空字符串；每值最多 4 KiB。值不入配置、历史或审计。",
            "Use user names such as {{path}} in commands, select targets, then sync fields. Same-name values belong only to that session; empty values require an explicit selection. Each value allows 4 KiB. Values never enter configuration, history or audit."))
}
