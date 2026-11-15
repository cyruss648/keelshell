//! Server packing has separate read-only inspection and complete write reviews.
use super::*;
use keelshell_core::server_archive::ServerArchiveRequest;
use keelshell_core::{ArchiveEntryKind, ArchiveFormat};
use keelshell_session::sftp::ServerArchivePlan;

#[derive(Clone, PartialEq, Eq)]
struct Binding {
    browser: batch::BatchBinding,
    format: ArchiveFormat,
    name: String,
    host: String,
}

#[derive(Clone)]
pub(super) struct Request {
    binding: Binding,
    connection: SshSession,
    request: ServerArchiveRequest,
}

impl Request {
    pub(super) fn matches_session(&self, connection: &SshSession) -> bool {
        self.connection.same_connection(connection)
    }
}

#[derive(Clone)]
pub(super) struct Plan {
    request: Request,
    input: String,
    pub(super) transfer: ServerArchivePlan,
}

impl Plan {
    pub(super) fn matches_session(&self, connection: &SshSession) -> bool {
        self.request.matches_session(connection)
    }

    pub(super) fn isolation_target(&self) -> IsolationTarget {
        IsolationTarget::Remote(self.transfer.request().destination().to_owned())
    }

    pub(super) fn review_message(&self) -> Message {
        let request = self.transfer.request();
        let format = match request.format() {
            ArchiveFormat::Zip => "ZIP",
            ArchiveFormat::TarGz => "tar.gz",
        };
        let manifest = self.transfer.manifest();
        let mut zh = format!(
            "当前 SSH 主机：{}\n服务器打包格式：{format}\n新远端归档：{}\n私有临时目录：{}\n成员数：{}\n源文件总字节：{}\n\n完整成员清单：\n",
            self.request.binding.host,
            request.destination(),
            self.transfer.staging_path(),
            manifest.members().len(),
            manifest.expanded_bytes(),
        );
        let mut en = format!(
            "Current SSH host: {}\nServer archive format: {format}\nNew remote archive: {}\nPrivate staging folder: {}\nMembers: {}\nSource file bytes: {}\n\nComplete member manifest:\n",
            self.request.binding.host,
            request.destination(),
            self.transfer.staging_path(),
            manifest.members().len(),
            manifest.expanded_bytes(),
        );
        for member in manifest.members() {
            let (zh_kind, en_kind) = match member.kind() {
                ArchiveEntryKind::File => ("文件", "File"),
                ArchiveEntryKind::Directory => ("目录", "Folder"),
            };
            zh.push_str(&format!(
                "{zh_kind}\t{} 字节\t{}\n",
                member.bytes(),
                member.path()
            ));
            en.push_str(&format!(
                "{en_kind}\t{} bytes\t{}\n",
                member.bytes(),
                member.path()
            ));
            if let Some(digest) = member.sha256() {
                zh.push_str(&format!("SHA-256: {digest}\n"));
                en.push_str(&format!("SHA-256: {digest}\n"));
            }
        }
        zh.push_str(&format!(
            "\n确认后在服务器重新核验源并压缩，仅发布新归档，不覆盖目标。需要远端 macOS/Linux、Python 3.8+ 和可核验的私有文件权限。成功前必须完整读回归档并核对成员；中断或未知结果保留目标和临时目录隔离，不自动重试或删除。完成后可刷新列表，另行选择下载或解包。\n\n完整执行命令：\n{}\n\n完整标准输入（审核快照）：\n{}",
            self.transfer.command(),
            self.input,
        ));
        en.push_str(&format!(
            "\nConfirmation revalidates and compresses the sources on the server, publishing only a new archive. Existing targets are preserved. Requires remote macOS/Linux, Python 3.8+ and verifiable private file permissions. Success requires complete archive readback and member verification. Interruption or an unknown result retains destination and staging isolation, without automatic retry or deletion. Refresh the list after completion, then separately choose download or extraction.\n\nComplete execution command:\n{}\n\nComplete standard input (reviewed snapshot):\n{}",
            self.transfer.command(),
            self.input,
        ));
        Message::new(zh, en)
    }
}

pub(super) async fn prepare(
    sftp: &SftpSession,
    request: Request,
    stop: &AtomicBool,
) -> Result<Plan, FileFailure> {
    let transfer = sftp
        .plan_server_archive(request.request.clone(), stop)
        .await?;
    // Display the exact bytes that the transport will send, without lossy
    // replacement or omission of a field the user has not actually reviewed.
    let input = std::str::from_utf8(transfer.input())
        .map_err(|_| {
            keelshell_session::SessionError::Invalid("server archive review is not UTF-8")
        })?
        .to_owned();
    Ok(Plan {
        request,
        input,
        transfer,
    })
}

impl FilesPanel {
    fn server_archive_binding(&self, cx: &App) -> Option<Binding> {
        Some(Binding {
            browser: self.batch_binding()?,
            format: self.archive_format,
            name: self.archive_name.read(cx).value().to_string(),
            host: self.host.clone(),
        })
    }

    pub(super) fn server_archive_request_is_current(&self, request: &Request, cx: &App) -> bool {
        !self.suspended
            && self.server_archive_binding(cx).as_ref() == Some(&request.binding)
            && self
                .session
                .as_ref()
                .is_some_and(|session| request.matches_session(session))
    }

    pub(super) fn server_archive_plan_is_current(&self, plan: &Plan, cx: &App) -> bool {
        self.server_archive_request_is_current(&plan.request, cx)
    }

    fn review_server_archive(&mut self, cx: &mut Context<Self>) {
        if !self.drop_upload_available() {
            return;
        }
        let Some(binding) = self.server_archive_binding(cx) else {
            return;
        };
        let Some(connection) = self.session.clone() else {
            return;
        };
        let sources: Vec<_> = self.remote_selection.paths().cloned().collect();
        if sources.is_empty()
            || sources.iter().any(|source| {
                self.entries
                    .iter()
                    .all(|entry| &entry.path != source || entry.is_symlink)
            })
        {
            self.status = Message::new(
                "请在远端列表选择普通文件或目录。",
                "Select ordinary files or folders in the remote list.",
            );
            cx.notify();
            return;
        }
        let Some(directory) = &self.directory else {
            return;
        };
        let destination = format!("{}/{}", directory.trim_end_matches('/'), binding.name);
        let request = match ServerArchiveRequest::new(sources, destination, binding.format) {
            Ok(request) => request,
            Err(error) => {
                self.status = Message::detail(
                    "无法准备服务器归档，请检查选择、名称和格式",
                    "Cannot prepare the server archive; check the selection, name and format",
                    error,
                );
                cx.notify();
                return;
            }
        };
        let command = request.inspection_command();
        self.confirm(
            Message::new(
                format!(
                    "当前 SSH 主机：{}\n新归档候选：{}\n\n先在服务器只读扫描所选文件和目录，计算完整成员与摘要；这一步不会打包或创建归档。需要远端 macOS/Linux、Python 3.8+ 和可核验的私有文件权限。扫描后必须另行审核完整清单及写入命令，取消不会授权打包。\n\n完整只读准备命令：\n{command}",
                    binding.host,
                    request.destination(),
                ),
                format!(
                    "Current SSH host: {}\nProposed new archive: {}\n\nFirst inspect the selected files and folders on the server and compute the complete manifest and digests, without packing or creating an archive. Requires remote macOS/Linux, Python 3.8+ and verifiable private file permissions. Afterwards, separately review the complete manifest and write command. Cancelling does not authorize packing.\n\nComplete read-only preparation command:\n{command}",
                    binding.host,
                    request.destination(),
                ),
            ),
            Operation::PlanServerArchive(Request {
                binding,
                connection,
                request,
            }),
            cx,
        );
    }

    pub(super) fn server_archive_button(&self, cx: &Context<Self>) -> Button {
        let expected = self.server_archive_binding(cx);
        Button::new("review-server-archive")
            .ghost()
            .compact()
            .small()
            .rounded(px(crate::design::components(cx).control_radius))
            .label(t(cx, "审核远端打包", "Review server packing"))
            .accessibility_label(t(
                cx,
                "审核在服务器打包远端所选文件和目录",
                "Review server packing of the selected remote files and folders",
            ))
            .disabled(!self.drop_upload_available() || self.remote_selection.len() == 0)
            .on_click(cx.listener(move |view, _, _, cx| {
                if expected.is_none() || view.server_archive_binding(cx) != expected {
                    view.status = archive::expired_message();
                    cx.notify();
                    return;
                }
                view.review_server_archive(cx);
            }))
    }
}
