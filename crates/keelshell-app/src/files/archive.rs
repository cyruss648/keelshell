//! Archive preparation binds the full manifest to the current browser and drafts.
use super::*;
use keelshell_core::{ArchiveEntryKind, ArchiveFormat};
use keelshell_session::sftp::{ArchiveTransferPlan, ArchiveTransferRequest};

#[derive(Clone, PartialEq, Eq)]
struct ArchiveBinding {
    browser: batch::BatchBinding,
    format: ArchiveFormat,
    name: String,
    local: String,
}

#[derive(Clone)]
pub(super) struct ArchiveRequest {
    binding: ArchiveBinding,
    request: ArchiveTransferRequest,
}

#[derive(Clone)]
pub(super) struct ArchivePlan {
    binding: ArchiveBinding,
    pub(super) transfer: ArchiveTransferPlan,
}

pub(super) fn expired_message() -> Message {
    Message::new(
        "归档审核已失效：会话、目录、选择、格式或路径已变化，请重新准备。",
        "Archive review expired: the session, folder, selection, format or path changed. Prepare again.",
    )
}

pub(super) fn retains_close_hold(result: &Result<Outcome, FileFailure>) -> bool {
    match result {
        Ok(_) => false,
        Err(error) => {
            error.is_outcome_unknown()
                || matches!(error, FileFailure::Cleanup | FileFailure::IsolationUnknown)
        }
    }
}

fn format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Zip => "ZIP",
        ArchiveFormat::TarGz => "tar.gz",
    }
}

impl ArchivePlan {
    pub(super) fn isolation_target(&self) -> IsolationTarget {
        match self.transfer.request() {
            ArchiveTransferRequest::PackUpload { remote, .. } => {
                IsolationTarget::Remote(remote.clone())
            }
            ArchiveTransferRequest::DownloadExtract { local, .. } => {
                IsolationTarget::Local(local.clone())
            }
        }
    }

    pub(super) fn review_message(&self) -> Message {
        let mut zh = format!(
            "归档格式：{}\n归档字节：{}\n展开字节：{}\n成员数：{}\n",
            format_name(self.transfer.format()),
            self.transfer.archive_bytes(),
            self.transfer.expanded_bytes(),
            self.transfer.members().len()
        );
        let mut en = format!(
            "Format: {}\nArchive bytes: {}\nExpanded bytes: {}\nMembers: {}\n",
            format_name(self.transfer.format()),
            self.transfer.archive_bytes(),
            self.transfer.expanded_bytes(),
            self.transfer.members().len()
        );
        match self.transfer.request() {
            ArchiveTransferRequest::PackUpload {
                sources, remote, ..
            } => {
                zh.push_str(&format!("\n新远端归档：{remote}\n本地源：\n"));
                en.push_str(&format!("\nNew remote archive: {remote}\nLocal sources:\n"));
                for source in sources {
                    zh.push_str(&format!("{}\n", source.display()));
                    en.push_str(&format!("{}\n", source.display()));
                }
            }
            ArchiveTransferRequest::DownloadExtract { remote, local, .. } => {
                zh.push_str(&format!(
                    "\n远端归档：{remote}\n新本地目录：{}\n",
                    local.display()
                ));
                en.push_str(&format!(
                    "\nRemote archive: {remote}\nNew local folder: {}\n",
                    local.display()
                ));
            }
        }
        zh.push_str("\n完整成员清单：\n");
        en.push_str("\nComplete member manifest:\n");
        for member in self.transfer.members() {
            let (zh_kind, en_kind) = match member.kind() {
                ArchiveEntryKind::File => ("文件", "File"),
                ArchiveEntryKind::Directory => ("目录", "Folder"),
            };
            zh.push_str(&format!(
                "{zh_kind}\t{} 字节\t{}\n",
                member.bytes().len(),
                member.path()
            ));
            en.push_str(&format!(
                "{en_kind}\t{} bytes\t{}\n",
                member.bytes().len(),
                member.path()
            ));
        }
        zh.push_str("\n确认后重新校验源。仅发布新目标，不覆盖已有文件或目录。解包不恢复归档权限和所有者。中断或未知结果需检查目标，不自动重试；重启后需重新准备。");
        en.push_str("\nSources are revalidated after confirmation. Publishes a new destination and preserves existing files or folders. Extraction does not restore archived permissions or ownership. Inspect the destination after interruption or an unknown result; no automatic retry. Prepare again after restart.");
        Message::new(zh, en)
    }
}

pub(super) async fn prepare(
    sftp: &SftpSession,
    request: ArchiveRequest,
    stop: &AtomicBool,
) -> Result<ArchivePlan, FileFailure> {
    let transfer = sftp
        .plan_archive_transfer(request.request, &|| !stop.load(Ordering::Acquire))
        .await?;
    Ok(ArchivePlan {
        binding: request.binding,
        transfer,
    })
}

impl FilesPanel {
    fn archive_binding(&self, cx: &App) -> Option<ArchiveBinding> {
        Some(ArchiveBinding {
            browser: self.batch_binding()?,
            format: self.archive_format,
            name: self.archive_name.read(cx).value().to_string(),
            local: self.local.read(cx).value().to_string(),
        })
    }

    pub(super) fn archive_plan_is_current(&self, plan: &ArchivePlan, cx: &App) -> bool {
        !self.suspended
            && self.session.is_some()
            && self.archive_binding(cx).as_ref() == Some(&plan.binding)
    }

    fn request_archive(&mut self, upload: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.drop_upload_available() {
            return;
        }
        let Some(binding) = self.archive_binding(cx) else {
            return;
        };
        let request = if upload {
            let Some(listing) = &self.local_browser.listing else {
                return;
            };
            let sources: Vec<_> = self.local_browser.selection.paths().cloned().collect();
            if sources.is_empty()
                || sources.iter().any(|path| {
                    listing.entries.iter().all(|entry| {
                        &entry.path != path
                            || !matches!(
                                entry.kind,
                                local_catalog::LocalEntryKind::File
                                    | local_catalog::LocalEntryKind::Directory
                            )
                    })
                })
            {
                self.status = Message::new(
                    "请在本地列表选择普通文件或目录。",
                    "Select ordinary files or folders in the local list.",
                );
                cx.notify();
                return;
            }
            let suffix = match binding.format {
                ArchiveFormat::Zip => ".zip",
                ArchiveFormat::TarGz => ".tar.gz",
            };
            let name = &binding.name;
            let remote = self
                .directory
                .as_ref()
                .filter(|_| name.ends_with(suffix))
                .filter(|_| browser::local_destination(&listing.directory, name).is_some())
                .map(|directory| format!("{}/{name}", directory.trim_end_matches('/')));
            let Some(remote) = remote else {
                self.status = Message::new(
                    format!("请输入可移植的新归档名称，以 {suffix} 结尾。"),
                    format!("Enter a portable new archive name ending in {suffix}."),
                );
                cx.notify();
                return;
            };
            ArchiveTransferRequest::PackUpload {
                sources,
                remote,
                format: binding.format,
            }
        } else {
            let Some(entry) = &self.selected else {
                return;
            };
            let local = PathBuf::from(&binding.local);
            if entry.is_directory || entry.is_symlink || !local.is_absolute() {
                self.status = Message::new(
                    "请选择单个远端归档，并在本地传输路径填写新目录的绝对路径。",
                    "Select one remote archive and enter an absolute new folder in the local transfer path.",
                );
                cx.notify();
                return;
            }
            ArchiveTransferRequest::DownloadExtract {
                remote: entry.path.clone(),
                local,
                format: binding.format,
            }
        };
        self.run(
            Operation::PlanArchive(ArchiveRequest { binding, request }),
            window,
            cx,
        );
    }

    pub(super) fn archive_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let disabled = !self.drop_upload_available();
        let painted_binding = self.archive_binding(cx);
        let mut formats = div().flex().items_center().gap_1();
        for (format, id) in [
            (ArchiveFormat::Zip, "archive-format-zip"),
            (ArchiveFormat::TarGz, "archive-format-tar-gz"),
        ] {
            let expected_session = self.session_token;
            formats = formats.child(
                Button::new(id)
                    .ghost()
                    .compact()
                    .small()
                    .rounded(px(crate::design::components(cx).control_radius))
                    .label(format_name(format))
                    .accessibility_label(match format {
                        ArchiveFormat::Zip => t(cx, "归档格式 ZIP", "Archive format ZIP"),
                        ArchiveFormat::TarGz => t(cx, "归档格式 tar.gz", "Archive format tar.gz"),
                    })
                    .selected(self.archive_format == format)
                    .toggled(self.archive_format == format)
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.session_token != expected_session || !view.drop_upload_available() {
                            return;
                        }
                        view.withdraw_browser_review();
                        view.archive_format = format;
                        cx.notify();
                    })),
            );
        }
        let mut actions = div().flex().flex_wrap().items_center().gap_2();
        for (upload, id, zh, en) in [
            (
                true,
                "review-archive-upload",
                "审核打包上传",
                "Review packed upload",
            ),
            (
                false,
                "review-archive-extract",
                "审核下载解包",
                "Review extraction",
            ),
        ] {
            let expected = painted_binding.clone();
            let unavailable = if upload {
                self.local_browser.selection.len() == 0
            } else {
                !cfg!(any(target_os = "macos", target_os = "linux"))
                    || self
                        .selected
                        .as_ref()
                        .is_none_or(|entry| entry.is_directory || entry.is_symlink)
            };
            actions = actions.child(
                Button::new(id)
                    .ghost()
                    .compact()
                    .small()
                    .rounded(px(crate::design::components(cx).control_radius))
                    .label(t(cx, zh, en))
                    .accessibility_label(t(cx, zh, en))
                    .disabled(disabled || unavailable)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if expected.is_none() || view.archive_binding(cx) != expected {
                            view.status = expired_message();
                            cx.notify();
                            return;
                        }
                        view.request_archive(upload, window, cx);
                    })),
            );
        }
        div()
            .id("file-archive-tools")
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .py_2()
            .border_t_1()
            .border_color(rgb(visual.border))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t(cx, "归档传输", "Archive transfer")),
                    )
                    .child(formats)
                    .child(
                        div()
                            .id("archive-name-draft")
                            .min_w(px(180.))
                            .flex_1()
                            .child(
                                Input::new(&self.archive_name)
                                    .small()
                                    .rounded(px(crate::design::components(cx).control_radius))
                                    .accessibility_id("archive-name")
                                    .aria_label(t(cx, "远端新归档名称", "New remote archive name")),
                            )
                            .test_support(),
                    ),
            )
            .child(
                div().text_color(rgb(visual.muted)).child(t(
                    cx,
                    "上传使用本地多选；解包使用远端所选和上方本地路径。目标必须不存在。",
                    "Pack the local selection, or extract the remote selection to the local path above. Destination must be new.",
                )),
            )
            .child(actions)
            .when(!cfg!(any(target_os = "macos", target_os = "linux")), |tools| {
                tools.child(div().text_color(rgb(visual.muted)).child(t(
                    cx,
                    "当前平台的安全新目录发布仍待实现，下载解包暂不可用；可继续打包上传。",
                    "Safe new-folder publication is pending on this platform. Extraction is unavailable; packed uploads remain available.",
                )))
            })
            .test_support()
            .into_any_element()
    }
}
