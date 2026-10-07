//! Explicit directory drafts and revision-bound background validation.

use gpui_kit::{
    component::{
        Disableable, IconName, Selectable,
        button::{Button, ButtonVariants},
        input::Input,
        label::Label,
        scroll::{Scrollbar, ScrollbarMode},
    },
    *,
};
use keelshell_ai::{
    LocalAgentError, LocalAgentKind, LocalAgentWorkingDirectory, RequestCancellation,
    ValidatedLocalAgentDirectory,
};
use keelshell_core::{AiBackend, AiLocalAgent, AiLocalAgentWorkingDirectory, NamedAiProfile};

use super::{AiSettingsPanel, OperationKind, local_agent_error};
use crate::i18n::{Message, t};

pub(super) fn path_value(profile: &NamedAiProfile) -> String {
    match &profile.backend {
        AiBackend::LocalAgent {
            working_directory: AiLocalAgentWorkingDirectory::Selected { path },
            ..
        } => path.clone(),
        _ => String::new(),
    }
}

impl AiSettingsPanel {
    fn set_directory_mode(&mut self, selected: bool, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        let Some(id) = self.selected else {
            return;
        };
        let path = self.local_directory.read(cx).value().to_string();
        if let Some(profile) = self
            .catalog
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            && let AiBackend::LocalAgent {
                working_directory, ..
            } = &mut profile.backend
        {
            let new = if selected {
                AiLocalAgentWorkingDirectory::Selected { path }
            } else {
                AiLocalAgentWorkingDirectory::Isolated
            };
            if *working_directory == new {
                return;
            }
            *working_directory = new;
            self.changed(false, cx);
        }
    }

    fn choose_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        let Some(owner) = self.selected else {
            return;
        };
        if self.vault_prompt.is_some() {
            return;
        }
        self.cancel_operation(false, cx);
        self.operation = Some(OperationKind::DirectoryCheck);
        let revision = self.revision;
        let operation_revision = self.operation_revision;
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(
                t(
                    cx,
                    "选择本地智能体的工作目录",
                    "Choose the local agent working directory",
                )
                .into(),
            ),
        });
        self._job = Some(cx.spawn_in(window, async move |this, cx| {
            let result = choice.await;
            let _ = this.update_in(cx, |panel, window, cx| {
                // A late native chooser cannot overwrite another profile/draft.
                if panel.selected != Some(owner) || panel.revision != revision
                    || panel.operation_revision != operation_revision { return; }
                panel.operation = None;
                match result {
                    Ok(Ok(Some(paths))) if paths.len() == 1 => {
                        if let Some(path) = paths[0].to_str() {
                            panel.local_directory.update(cx, |field, cx| field.set_value(path.to_owned(), window, cx));
                            panel.sync_editor(cx);
                            panel.set_directory_mode(true, cx);
                            panel.start_directory_check(cx);
                        } else { panel.status = local_agent_error(LocalAgentError::DirectoryInvalid); }
                    }
                    Ok(Ok(None)) => panel.status = Message::new("已取消目录选择。", "Folder selection cancelled."),
                    _ => panel.status = Message::new("无法打开或完成目录选择；可手动填写绝对路径。", "Folder selection could not be opened or completed; enter an absolute path instead."),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn start_directory_check(&mut self, cx: &mut Context<Self>) {
        self.sync_editor(cx);
        if self.vault_prompt.is_some() {
            return;
        }
        if crate::ai_request_options::validate_catalog_secrets(&self.catalog, &self.credentials)
            .is_err()
        {
            self.status = Message::new(
                "目录或配置包含已知秘密，不能检查；请修正后重试。",
                "The directory or configuration contains a known secret; correct it before checking.",
            );
            cx.notify();
            return;
        }
        let Some(profile) = self.profile() else {
            return;
        };
        let AiBackend::LocalAgent {
            agent,
            working_directory: AiLocalAgentWorkingDirectory::Selected { path },
            ..
        } = &profile.backend
        else {
            return;
        };
        let owner = profile.id;
        let directory = LocalAgentWorkingDirectory::Selected(path.into());
        let kind = match agent {
            AiLocalAgent::Codex => LocalAgentKind::Codex,
            AiLocalAgent::ClaudeCode => LocalAgentKind::ClaudeCode,
        };
        self.cancel_operation(false, cx);
        self.local_directory_checked = None;
        let revision = self.revision;
        let operation_revision = self.operation_revision;
        let cancellation = RequestCancellation::new();
        self.cancellation = Some(cancellation.clone());
        self.operation = Some(OperationKind::DirectoryCheck);
        self.status = Message::new(
            "后台检查目录与项目元数据；不启动 CLI、不发送模型请求…",
            "Checking directory and project metadata in the background; no CLI or inference request…",
        );
        let job = crate::runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move { directory.validate_directory(kind, &cancellation).await },
        );
        self._job = Some(cx.spawn(async move |this, cx| {
            let result = job
                .await
                .unwrap_or(Err(LocalAgentError::DirectoryUnavailable));
            let _ = this.update(cx, |panel, cx| {
                panel.finish_directory_check(owner, revision, operation_revision, result, cx)
            });
        }));
        cx.notify();
    }

    fn finish_directory_check(
        &mut self,
        owner: uuid::Uuid,
        revision: u64,
        operation_revision: u64,
        result: Result<Option<ValidatedLocalAgentDirectory>, LocalAgentError>,
        cx: &mut Context<Self>,
    ) {
        if self.selected != Some(owner)
            || self.revision != revision
            || self.operation_revision != operation_revision
        {
            return;
        }
        self.operation = None;
        self.cancellation = None;
        self.status = match result {
            Ok(Some(directory)) => {
                let known = self.credentials.all_secrets();
                if keelshell_ai::RequestOptions::default()
                    .with_context_secrets(&known)
                    .and_then(|guard| {
                        guard.validate_metadata_text(
                            directory.canonical_path().to_string_lossy().as_ref(),
                        )
                    })
                    .is_err()
                {
                    self.status = local_agent_error(LocalAgentError::CredentialInContext);
                    cx.notify();
                    return;
                }
                self.local_directory_checked = Some((
                    owner,
                    revision,
                    directory.canonical_path().to_string_lossy().into_owned(),
                ));
                Message::new(
                    "目录校验通过；发送前仍需重新校验并审阅完整目录和 SSH 片段。",
                    "Directory validated; sending still requires fresh validation and review of the full directory and SSH selection.",
                )
            }
            Ok(None) => Message::new(
                "默认使用新建的空隔离目录。",
                "The default uses a newly created empty isolated directory.",
            ),
            Err(error) => local_agent_error(error),
        };
        cx.notify();
    }

    pub(super) fn working_directory_view(
        &self,
        profile: &NamedAiProfile,
        cx: &mut Context<Self>,
    ) -> Div {
        let visual = crate::design::palette(cx);
        let selected = matches!(
            &profile.backend,
            AiBackend::LocalAgent {
                working_directory: AiLocalAgentWorkingDirectory::Selected { .. },
                ..
            }
        );
        let mut view = div().flex().flex_col().gap_2()
            .child(super::view::label(cx,"工作目录", "Working directory"))
            .child(div().flex().flex_wrap().gap_2()
                .child(Button::new("ai-local-directory-isolated").selected(!selected).label(t(cx,"新建空隔离目录（默认）", "Fresh empty isolated directory (default)"))
                    .on_click(cx.listener(|panel,_,_,cx| panel.set_directory_mode(false,cx))))
                .child(Button::new("ai-local-directory-selected").selected(selected).label(t(cx,"使用我选择的目录", "Use my selected directory"))
                    .on_click(cx.listener(|panel,_,_,cx| panel.set_directory_mode(true,cx)))))
            .child(super::view::label(cx,"工作目录不会转交现有登录、环境变量或工具权限；仍仅运行受审核问答。", "The working directory does not grant existing logins, environment variables or tool permissions; execution remains reviewed Ask only."));
        if selected {
            let entered = self.local_directory.read(cx).value().to_string();
            let canonical = self
                .local_directory_checked
                .as_ref()
                .filter(|(owner, revision, _)| *owner == profile.id && *revision == self.revision)
                .map(|(_, _, path)| path.as_str());
            let full = format!(
                "{}\n{}\n\n{}\n{}",
                t(cx, "所选完整路径", "Full selected path"),
                entered,
                t(cx, "规范目录路径", "Canonical directory path"),
                canonical.unwrap_or(t(
                    cx,
                    "待后台校验；发送预览时会重新检查",
                    "Awaiting background validation; checked again when preparing the send preview"
                ))
            );
            view = view.child(Input::new(&self.local_directory).id("ai-local-directory-path").aria_label(t(cx,"工作目录完整绝对路径", "Full absolute working directory path")))
                .child(div().flex().flex_wrap().gap_2()
                    .child(Button::new("ai-local-directory-choose").icon(IconName::Folder).ghost().label(t(cx,"选择目录…", "Choose folder…")).disabled(self.operation.is_some() || self.vault_prompt.is_some())
                        .on_click(cx.listener(|panel,_,window,cx| panel.choose_directory(window,cx))))
                    .child(Button::new("ai-local-directory-check").icon(IconName::Check).ghost().label(t(cx,"检查目录", "Check directory")).disabled(self.operation.is_some() || self.vault_prompt.is_some())
                        .on_click(cx.listener(|panel,_,_,cx| panel.start_directory_check(cx)))))
                .child(div().relative().w_full().h(px(112.)).bg(rgb(visual.canvas)).border_1().border_color(rgb(visual.border)).rounded(px(6.))
                    .child(div().id("ai-local-directory-full-preview").test_support().size_full().overflow_scroll().track_scroll(&self.local_directory_scroll).p_2().pr_5().pb_5()
                        .child(Label::new(full).text_xs()))
                    .child(div().id("ai-local-directory-preview-scrollbar").test_support()
                        .aria_label(t(cx,"完整工作目录预览滚动条", "Full working directory preview scrollbars"))
                        .absolute().inset_0()
                        .child(Scrollbar::new(&self.local_directory_scroll).id("ai-local-directory-preview-scrollbar-control").mode(ScrollbarMode::Always))))
                .child(super::view::label(cx,"选择目录允许 CLI 检查其项目元数据，但不会自动发送项目文件、说明或技能。目录路径及 CLI 生成的目录元数据可能随请求发送给推理服务；请在发送前完整审阅。", "Selecting a directory permits CLI inspection of its project metadata, without automatically sending project files, instructions or skills. The directory path and CLI-generated directory metadata may reach the inference service; review them in full before sending."));
        }
        view
    }
}

#[cfg(test)]
mod tests;
