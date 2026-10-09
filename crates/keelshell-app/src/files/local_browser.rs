//! Explicit local navigation has its own read owner, separate from remote writes.

use super::browser::BrowserSort;
use super::local_catalog::{LocalBrowseError, LocalEntryKind, LocalListing};
use super::*;

pub(super) struct LocalBrowser {
    pub(super) path: Entity<InputState>,
    // A native picker/entry may carry a non-UTF-8 path. The displayed text is
    // never parsed back into that path unless the user edits it explicitly.
    native_draft: Option<PathBuf>,
    pub(super) listing: Option<LocalListing>,
    pub(super) selected: Option<PathBuf>,
    pub(super) selection: selection::Selection<PathBuf>,
    pub(super) sort: BrowserSort,
    pub(super) show_hidden: bool,
    pub(super) status: Message,
    revision: uuid::Uuid,
    active: Option<LocalReadOwner>,
    queued: Option<(uuid::Uuid, PathBuf)>,
    picker_pending: bool,
}

struct LocalReadOwner {
    id: uuid::Uuid,
    revision: uuid::Uuid,
    stop: Arc<AtomicBool>,
}

impl LocalBrowser {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            path: field(
                t(cx, "本地绝对目录", "Absolute local folder"),
                "",
                window,
                cx,
            ),
            native_draft: None,
            listing: None,
            selected: None,
            selection: selection::Selection::default(),
            sort: BrowserSort::default(),
            show_hidden: false,
            status: Message::new(
                "选择本地文件夹后浏览；不会自动扫描。",
                "Choose a local folder to browse; nothing is scanned automatically.",
            ),
            revision: uuid::Uuid::new_v4(),
            active: None,
            queued: None,
            picker_pending: false,
        }
    }

    pub(super) fn cancel_read(&self) {
        if let Some(owner) = &self.active {
            owner.stop.store(true, Ordering::Release);
        }
    }

    pub(super) fn retire_navigation(&mut self) {
        self.invalidate_navigation();
    }

    fn invalidate_navigation(&mut self) {
        self.revision = uuid::Uuid::new_v4();
        self.queued = None;
        self.selected = None;
        self.selection.clear();
        self.cancel_read();
    }

    #[cfg(test)]
    pub(super) fn reading(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn choosing(&self) -> bool {
        self.picker_pending
    }

    pub(super) fn selected_entry(&self) -> Option<&local_catalog::LocalEntry> {
        let path = self.selected.as_ref()?;
        self.listing
            .as_ref()?
            .entries
            .iter()
            .find(|entry| &entry.path == path)
    }
}

impl FilesPanel {
    // Only an unexecuted proposal is withdrawn. An issued remote operation owns
    // its captured path and result until the original transport completes.
    pub(super) fn withdraw_browser_review(&mut self) {
        self.pending = None;
        self.confirmation_expanded = false;
        self.browser_review_revision = uuid::Uuid::new_v4();
        if let Some(comparison) = &mut self.comparison {
            comparison.sync_plan = None;
        }
    }

    pub(super) fn install_browser_subscriptions(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for input in [&self.path, &self.name, &self.mode, &self.local] {
            self._browser_subscriptions.push(cx.subscribe_in(
                input,
                window,
                |panel, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        panel.withdraw_browser_review();
                        cx.notify();
                    }
                },
            ));
        }
        self._browser_subscriptions.push(cx.subscribe_in(
            &self.path,
            window,
            |panel, _, event: &InputEvent, window, cx| {
                if matches!(
                    event,
                    InputEvent::PressEnter {
                        secondary: false,
                        shift: false
                    }
                ) {
                    panel.run(
                        Operation::List(panel.path.read(cx).value().to_string()),
                        window,
                        cx,
                    );
                }
            },
        ));
        self._browser_subscriptions.push(cx.subscribe_in(
            &self.local_browser.path,
            window,
            |panel, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    panel.local_browser.invalidate_navigation();
                    panel.local_browser.native_draft = None;
                    panel.local_browser.listing = None;
                    panel.withdraw_browser_review();
                    panel.local_browser.status = Message::new(
                        "目录草稿已修改；按刷新浏览。",
                        "Folder draft changed; refresh to browse.",
                    );
                    cx.notify();
                } else if matches!(
                    event,
                    InputEvent::PressEnter {
                        secondary: false,
                        shift: false
                    }
                ) {
                    panel.refresh_local_folder(window, cx);
                }
            },
        ));
    }

    pub(super) fn choose_local_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended || self.local_browser.picker_pending {
            return;
        }
        self.local_browser.invalidate_navigation();
        self.withdraw_browser_review();
        self.local_browser.picker_pending = true;
        let revision = self.local_browser.revision;
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(
                t(
                    cx,
                    "选择 SFTP 传输的本地文件夹",
                    "Choose a local folder for SFTP transfers",
                )
                .into(),
            ),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = choice.await;
            let _ = this.update_in(cx, |panel, window, cx| {
                panel.local_browser.picker_pending = false;
                if panel.suspended || panel.local_browser.revision != revision {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(Some(paths))) if paths.len() == 1 => {
                        if let Some(path) = paths.into_iter().next() {
                            panel.browse_local(path, window, cx);
                        }
                    }
                    Ok(Ok(None)) => panel.local_browser.status = Message::new("已取消文件夹选择。", "Folder selection cancelled."),
                    _ => panel.local_browser.status = Message::new("无法完成系统文件夹选择；可手动填写绝对目录后刷新。", "The system folder picker could not complete; enter an absolute folder and refresh."),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    pub(super) fn refresh_local_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path =
            self.local_browser.native_draft.clone().unwrap_or_else(|| {
                PathBuf::from(self.local_browser.path.read(cx).value().to_string())
            });
        self.browse_local(path, window, cx);
    }

    pub(super) fn parent_local_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parent = self
            .local_browser
            .listing
            .as_ref()
            .and_then(|listing| listing.directory.parent())
            .map(PathBuf::from);
        if let Some(parent) = parent {
            self.browse_local(parent, window, cx);
        }
    }

    pub(super) fn browse_local(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suspended {
            return;
        }
        self.local_browser.invalidate_navigation();
        self.withdraw_browser_review();
        self.local_browser.native_draft = Some(path.clone());
        self.local_browser.listing = None;
        self.local_browser.path.update(cx, |input, cx| {
            input.set_value(path.to_string_lossy().into_owned(), window, cx);
        });
        let revision = self.local_browser.revision;
        self.local_browser.queued = Some((revision, path));
        self.local_browser.status =
            Message::new("正在读取本地目录元数据…", "Reading local folder metadata…");
        self.start_local_read(window, cx);
        cx.notify();
    }

    fn start_local_read(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.suspended || self.local_browser.active.is_some() {
            return;
        }
        let Some((revision, path)) = self.local_browser.queued.take() else {
            return;
        };
        if revision != self.local_browser.revision {
            return;
        }
        let id = uuid::Uuid::new_v4();
        let stop = Arc::new(AtomicBool::new(false));
        self.local_browser.active = Some(LocalReadOwner {
            id,
            revision,
            stop: stop.clone(),
        });
        let worker_stop = stop.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        #[cfg(test)]
        let fixture_group = self.fixture_group.clone();
        if crate::terminal::spawn_transport_worker("keelshell-local-browser", stop, move || {
            #[cfg(test)]
            let _fixture_group = fixture_group;
            let result = local_catalog::list_directory(&path, &worker_stop);
            // A display read's failure is delivered to its owner without logging
            // paths; the registry still observes a normal completed worker.
            let _ = sender.send(result);
            Ok(())
        })
        .is_err()
        {
            self.local_browser.active = None;
            self.local_browser.status = Message::new(
                "无法启动本地目录工作线程。",
                "Unable to start the local folder worker.",
            );
            return;
        }
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = loop {
                match receiver.try_recv() {
                    Ok(result) => break Some(result),
                    Err(mpsc::TryRecvError::Disconnected) => break None,
                    Err(mpsc::TryRecvError::Empty) => {
                        executor.timer(Duration::from_millis(16)).await
                    }
                }
                if this.update_in(cx, |_, _, _| ()).is_err() {
                    return;
                }
            };
            let _ = this.update_in(cx, |panel, window, cx| {
                let matches = panel
                    .local_browser
                    .active
                    .as_ref()
                    .is_some_and(|owner| owner.id == id && owner.revision == revision);
                if !matches {
                    return;
                }
                panel.local_browser.active = None;
                if !panel.suspended && panel.local_browser.revision == revision {
                    match result {
                        Some(Ok(listing)) => {
                            let count = listing.entries.len();
                            panel.local_browser.listing = Some(listing);
                            panel.local_browser.selected = None;
                            panel.local_browser.selection.clear();
                            panel.local_browser.status = Message::new(
                                format!("已读取 {count} 项（仅元数据）"),
                                format!("Loaded {count} entries (metadata only)"),
                            );
                        }
                        Some(Err(error)) => panel.local_browser.status = local_browse_error(error),
                        None => {
                            panel.local_browser.status = Message::new(
                                "本地目录工作线程已停止。",
                                "The local folder worker stopped.",
                            )
                        }
                    }
                }
                // A cancelled blocking syscall cannot be hard-killed. Wait for
                // its real terminal result before launching the latest request.
                panel.start_local_read(window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn use_local_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suspended {
            return;
        }
        let Some(value) = path.to_str() else {
            self.local_browser.status = Message::new(
                "该路径不是 UTF-8；可浏览，但不能写入当前传输路径输入。",
                "This path is not UTF-8; it can be browsed but cannot populate the current transfer input.",
            );
            cx.notify();
            return;
        };
        self.withdraw_browser_review();
        self.local.update(cx, |input, cx| {
            input.set_value(value.to_owned(), window, cx)
        });
        cx.notify();
    }

    pub(super) fn use_local_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self
            .local_browser
            .selected_entry()
            .filter(|entry| matches!(entry.kind, LocalEntryKind::Directory | LocalEntryKind::File))
            .map(|entry| entry.path.clone());
        if let Some(path) = path {
            self.use_local_path(path, window, cx);
        }
    }

    pub(super) fn use_local_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self
            .local_browser
            .listing
            .as_ref()
            .map(|listing| listing.directory.clone())
        {
            self.use_local_path(path, window, cx);
        }
    }

    pub(super) fn prepare_local_destination(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(folder) = self
            .local_browser
            .listing
            .as_ref()
            .map(|listing| listing.directory.clone())
        else {
            return;
        };
        let Some(entry) = &self.selected else {
            return;
        };
        let Some(destination) = browser::local_destination(&folder, &entry.name) else {
            self.local_browser.status = Message::new(
                "远程名称不能安全用于本地目标；请手动填写目标路径。",
                "The remote name cannot safely form a local destination; enter the destination manually.",
            );
            cx.notify();
            return;
        };
        self.use_local_path(destination, window, cx);
    }

    pub(super) fn select_local_entry(
        &mut self,
        path: &PathBuf,
        directory: &PathBuf,
        modifiers: Modifiers,
        checkbox: bool,
        cx: &mut Context<Self>,
    ) {
        if self.suspended {
            return;
        }
        let Some(listing) = &self.local_browser.listing else {
            return;
        };
        if &listing.directory != directory {
            return;
        }
        let visible: Vec<_> = browser::local_indices(
            &listing.entries,
            self.local_browser.show_hidden,
            self.local_browser.sort,
        )
        .into_iter()
        .map(|index| listing.entries[index].path.clone())
        .collect();
        if !self.local_browser.selection.click(
            path,
            &visible,
            checkbox || modifiers.control || modifiers.platform,
            modifiers.shift,
        ) {
            self.local_browser.status = Message::new(
                "最多选择32项；隐藏或过期项目不能加入选择。",
                "Select at most 32 entries; hidden or stale rows cannot join the selection.",
            );
        } else {
            self.local_browser.selected = self.local_browser.selection.single().cloned();
            self.withdraw_browser_review();
        }
        cx.notify();
    }

    pub(super) fn select_remote_entry(
        &mut self,
        path: &str,
        directory: &Option<String>,
        modifiers: Modifiers,
        checkbox: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suspended || &self.directory != directory {
            return;
        }
        let visible: Vec<_> =
            browser::remote_indices(&self.entries, self.remote_show_hidden, self.remote_sort)
                .into_iter()
                .map(|index| self.entries[index].path.clone())
                .collect();
        if !self.remote_selection.click(
            &path.to_owned(),
            &visible,
            checkbox || modifiers.control || modifiers.platform,
            modifiers.shift,
        ) {
            self.status = Message::new(
                "最多选择32项；隐藏或过期项目不能加入选择。",
                "Select at most 32 entries; hidden or stale rows cannot join the selection.",
            );
        } else {
            self.selected = self
                .remote_selection
                .single()
                .and_then(|path| self.entries.iter().find(|entry| &entry.path == path))
                .cloned();
            self.withdraw_browser_review();
            if let Some(mode) = self.selected.as_ref().and_then(|entry| entry.permissions) {
                self.mode.update(cx, |input, cx| {
                    input.set_value(format!("{:04o}", mode & 0o7777), window, cx)
                });
            }
        }
        cx.notify();
    }

    pub(super) fn toggle_remote_hidden(&mut self, cx: &mut Context<Self>) {
        self.remote_show_hidden = !self.remote_show_hidden;
        let visible: Vec<_> =
            browser::remote_indices(&self.entries, self.remote_show_hidden, self.remote_sort)
                .into_iter()
                .map(|index| self.entries[index].path.clone())
                .collect();
        self.remote_selection.retain_visible(&visible);
        self.selected = self
            .remote_selection
            .single()
            .and_then(|path| self.entries.iter().find(|entry| &entry.path == path))
            .cloned();
        self.withdraw_browser_review();
        cx.notify();
    }

    pub(super) fn toggle_local_hidden(&mut self, cx: &mut Context<Self>) {
        self.local_browser.show_hidden = !self.local_browser.show_hidden;
        let visible: Vec<_> = self
            .local_browser
            .listing
            .as_ref()
            .map(|listing| {
                browser::local_indices(
                    &listing.entries,
                    self.local_browser.show_hidden,
                    self.local_browser.sort,
                )
                .into_iter()
                .map(|index| listing.entries[index].path.clone())
                .collect()
            })
            .unwrap_or_default();
        self.local_browser.selection.retain_visible(&visible);
        self.local_browser.selected = self.local_browser.selection.single().cloned();
        self.withdraw_browser_review();
        cx.notify();
    }
}

fn local_browse_error(error: LocalBrowseError) -> Message {
    match error {
        LocalBrowseError::Cancelled => {
            Message::new("本地目录读取已取消。", "Local folder reading cancelled.")
        }
        LocalBrowseError::RelativePath | LocalBrowseError::UnsupportedPath => Message::new(
            "请输入不含 .. 的本地绝对目录。",
            "Enter an absolute local folder without parent (..) components.",
        ),
        LocalBrowseError::RootLink => Message::new(
            "本地浏览根不能是符号链接或重解析点；请选择实际目录。",
            "The local browsing root cannot be a symbolic link or reparse point; choose the actual folder.",
        ),
        LocalBrowseError::NotDirectory => Message::new(
            "所选路径不是普通目录。",
            "The selected path is not an ordinary folder.",
        ),
        LocalBrowseError::Changed => Message::new(
            "本地目录在读取期间变化；请刷新重试。",
            "The local folder changed while reading; refresh to retry.",
        ),
        LocalBrowseError::EntryLimit | LocalBrowseError::NameLimit => Message::new(
            "本地目录超过单次浏览预算（4096 项 / 1 MiB 文件名）；未采用部分列表。",
            "The local folder exceeds the browsing budget (4096 entries / 1 MiB of names); no partial list was adopted.",
        ),
        LocalBrowseError::TimedOut => Message::new(
            "本地读取超过协作时间预算；阻塞文件系统调用无法硬取消。",
            "The local read exceeded its cooperative time budget; blocking filesystem calls cannot be forcibly cancelled.",
        ),
        LocalBrowseError::RootMetadata(_)
        | LocalBrowseError::DirectoryRead(_)
        | LocalBrowseError::EntryRead(_)
        | LocalBrowseError::EntryMetadata(_) => Message::new(
            "无法读取本地目录元数据；请检查路径及权限后刷新。",
            "Local folder metadata could not be read; check the path and permissions, then refresh.",
        ),
    }
}
