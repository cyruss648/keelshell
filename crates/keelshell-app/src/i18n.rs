//! Explicit bilingual UI messages. Remote content is kept outside translations.

use std::fmt::Display;

use gpui_kit::{App, Global};
use keelshell_core::Language;

#[derive(Default)]
struct Locale(Language);

impl Global for Locale {}

/// Current interface language, defaulting to Chinese before application setup.
pub fn language(cx: &App) -> Language {
    cx.try_global::<Locale>()
        .map_or(Language::ZhCn, |locale| locale.0)
}

/// Select a static message. Both supported languages are required at each call.
pub fn t(cx: &App, zh: &'static str, en: &'static str) -> &'static str {
    match language(cx) {
        Language::ZhCn => zh,
        Language::En => en,
    }
}

/// An owned status retaining both translations so it can change language later.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    zh: String,
    en: String,
}

impl Message {
    /// An empty status in both languages.
    pub fn empty() -> Self {
        Self::default()
    }
    /// Keep both complete translations, including any already formatted arguments.
    pub fn new(zh: impl Into<String>, en: impl Into<String>) -> Self {
        Self {
            zh: zh.into(),
            en: en.into(),
        }
    }

    /// Add the same uninterpreted technical detail to translated labels.
    ///
    /// Details may be a remote path, host or diagnostic. This does not translate
    /// or execute the detail, and callers must exclude credentials beforehand.
    pub fn detail(zh_label: &str, en_label: &str, detail: impl Display) -> Self {
        Self::new(
            format!("{zh_label}：{detail}"),
            format!("{en_label}: {detail}"),
        )
    }

    /// Render according to the current application language, without mutation.
    pub fn render(&self, cx: &App) -> String {
        match language(cx) {
            Language::ZhCn => self.zh.clone(),
            Language::En => self.en.clone(),
        }
    }
}

/// Update application and GPUI component translations, then redraw every window.
pub fn set_language(language: Language, cx: &mut App) {
    cx.set_global(Locale(language));
    gpui_kit::component::set_locale(language.code());
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::{Message, language, set_language, t};
    use gpui_kit::TestAppContext;
    use keelshell_core::Language;

    #[gpui_kit::test]
    fn defaults_to_chinese_and_retranslates_existing_messages(cx: &mut TestAppContext) {
        let message = Message::detail("主机", "Host", "ops@example:22");
        cx.update(|cx| {
            assert_eq!(language(cx), Language::ZhCn);
            assert_eq!(t(cx, "连接", "Connect"), "连接");
            assert_eq!(message.render(cx), "主机：ops@example:22");
            set_language(Language::En, cx);
            assert_eq!(t(cx, "连接", "Connect"), "Connect");
            assert_eq!(message.render(cx), "Host: ops@example:22");
            set_language(Language::ZhCn, cx);
            assert_eq!(message.render(cx), "主机：ops@example:22");
        });
    }
}
