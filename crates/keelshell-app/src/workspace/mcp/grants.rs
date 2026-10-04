use super::*;

impl Workspace {
    pub(super) fn mcp_capture_selection(&mut self, cx: &mut Context<Self>) {
        if self.mcp.busy {
            return;
        }
        let text = self
            .tabs
            .iter()
            .find(|tab| Some(tab.entity_id()) == self.mcp.draft_entity)
            .map(|tab| tab.read(cx).selected_text())
            .unwrap_or_default();
        if text.is_empty() || text.len() > 16 * 1024 {
            self.mcp.status = Message::new(
                "先在当前终端选择 1–16 KiB 的文本；不会读取整屏或历史。",
                "Select 1–16 KiB of terminal text first; the screen and history are not shared.",
            );
        } else {
            self.mcp.draft_selection = Some((uuid::Uuid::new_v4(), text));
            self.mcp.status = Message::new(
                "已捕获所选文本；点击授权才会对外共享此快照。",
                "Selection captured; only granting access shares this snapshot.",
            );
        }
        cx.notify();
    }

    pub(super) fn grant_mcp(&mut self, cx: &mut Context<Self>) {
        if self.mcp.busy {
            return;
        }
        let Some(entity) = self
            .mcp
            .draft_entity
            .filter(|entity| self.mcp_target_current(*entity, cx))
        else {
            self.mcp.status = Message::new(
                "需要一个已就绪的真实 SSH 会话。",
                "A ready SSH session is required.",
            );
            cx.notify();
            return;
        };
        if self.mcp.targets.len() >= 32
            && !self
                .mcp
                .targets
                .iter()
                .any(|target| target.entity == entity)
        {
            self.mcp.status = Message::new(
                "最多授权 32 个会话；请先撤销一个现有授权。",
                "At most 32 sessions may be granted; revoke an existing grant first.",
            );
            cx.notify();
            return;
        }
        if self.mcp.draft_tools.is_empty() {
            return;
        }
        let roots = if self.mcp.draft_tools.contains(&ToolKind::SftpList)
            || self.mcp.draft_tools.contains(&ToolKind::SftpRead)
        {
            vec![self.mcp.root.read(cx).value().to_string()]
        } else {
            Vec::new()
        };
        let selection = self
            .mcp
            .draft_tools
            .contains(&ToolKind::ReadSelection)
            .then(|| self.mcp.draft_selection.clone())
            .flatten();
        if self.mcp.draft_tools.contains(&ToolKind::ReadSelection) && selection.is_none() {
            self.mcp.status = Message::new(
                "读取所选文本需要先明确捕获一个片段。",
                "Reading a selection requires explicitly capturing a fragment first.",
            );
            cx.notify();
            return;
        }
        let identity = SessionIdentity {
            connection_id: self
                .batch_profile_id(entity)
                .unwrap_or_else(uuid::Uuid::new_v4),
            session_id: uuid::Uuid::new_v4(),
            route_revision: uuid::Uuid::new_v4(),
        };
        if SessionGrant::new(
            identity,
            self.mcp.draft_tools.iter().copied(),
            roots.clone(),
            selection.iter().map(|(id, _)| *id),
        )
        .is_err()
        {
            self.mcp.status = Message::new(
                "远程根目录必须是绝对 canonical POSIX 路径。",
                "The remote root must be an absolute canonical POSIX path.",
            );
            cx.notify();
            return;
        }
        let label = self
            .tabs
            .iter()
            .find(|tab| tab.entity_id() == entity)
            .map(|tab| tab.read(cx).title.clone())
            .unwrap_or_default();
        let target = Target {
            entity,
            identity,
            label,
            roots,
            tools: self.mcp.draft_tools.clone(),
            selection,
        };
        let Some(session) = self.remote_sessions.get(&entity).cloned() else {
            return;
        };
        self.mcp.busy = true;
        self.mcp.revision = uuid::Uuid::new_v4();
        let revision = self.mcp.revision;
        let sender = self.mcp.result_sender.clone();
        self.mcp.workers.push(self.runtime.spawn(async move {
            let result = if let Some(path) = target.roots.first() {
                tokio::time::timeout(
                    Duration::from_secs(5),
                    crate::mcp_bridge::validate_root(session, path),
                )
                .await
                .unwrap_or(Err(McpFailure::Timeout))
            } else {
                Ok(())
            };
            let _ = sender
                .send(Completion::Granted {
                    revision,
                    target,
                    result,
                })
                .await;
        }));
        cx.notify();
    }

    pub(super) fn apply_mcp_grants(&mut self, cx: &mut Context<Self>) {
        // Synchronously cancel the old transport before installing replacement
        // authority. Its capability must never admit a request to the new scope.
        self.mcp.host = None;
        let grants = self
            .mcp
            .targets
            .iter()
            .map(|target| {
                SessionGrant::new(
                    target.identity,
                    target.tools.iter().copied(),
                    target.roots.clone(),
                    target.selection.iter().map(|(id, _)| *id),
                )
            })
            .collect::<Result<Vec<_>, _>>();
        let result = grants
            .and_then(AccessPolicy::enabled)
            .and_then(|policy| self.mcp.authority.replace(policy));
        if result.is_err() {
            self.mcp.stop();
            return;
        }
        // Replacing any grant invalidates all previous review leases. A new
        // listener/secret also prevents an old client from using replacement scope.
        for action in &mut self.mcp.actions {
            if matches!(action.state, ActionState::PendingReview) {
                action.state = ActionState::Cancelled;
            }
        }
        match KeelShellMcpServer::new(
            self.mcp.backend.clone(),
            self.mcp.authority.clone(),
            8,
            Duration::from_secs(5),
        ) {
            Ok(server) => {
                self.mcp.busy = true;
                self.mcp.revision = uuid::Uuid::new_v4();
                let revision = self.mcp.revision;
                let sender = self.mcp.result_sender.clone();
                self.mcp.workers.push(self.runtime.spawn(async move {
                    let result = keelshell_mcp::DesktopIpcHost::bind(server)
                        .await
                        .map_err(|_| McpFailure::BackendFailure);
                    let candidate = std::env::current_exe().ok().and_then(|path| {
                        path.parent().map(|parent| {
                            parent.join(if cfg!(target_os = "windows") {
                                "keelshell-mcp.exe"
                            } else {
                                "keelshell-mcp"
                            })
                        })
                    });
                    let executable = if let Some(path) = candidate {
                        tokio::fs::symlink_metadata(&path)
                            .await
                            .ok()
                            .filter(|metadata| {
                                metadata.is_file() && !metadata.file_type().is_symlink()
                            })
                            .map(|_| path)
                    } else {
                        None
                    };
                    let _ = sender
                        .send(Completion::Started {
                            revision,
                            result,
                            executable,
                        })
                        .await;
                }));
            }
            Err(_) => self.mcp.stop(),
        }
        cx.notify();
    }
}
