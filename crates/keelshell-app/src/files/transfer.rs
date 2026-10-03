//! Transfer presentation follows worker acknowledgements, never optimistic I/O state.
use super::*;
use gpui_kit::component::popover::Popover;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransferPhase {
    Preparing,
    Running,
    Pausing,
    Paused,
    Resuming,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

impl TransferPhase {
    fn label(self, cx: &App) -> &'static str {
        match self {
            Self::Preparing => t(cx, "准备传输", "Preparing"),
            Self::Running => t(cx, "传输中", "Transferring"),
            Self::Pausing => t(cx, "暂停中…", "Pausing…"),
            Self::Paused => t(cx, "已暂停", "Paused"),
            Self::Resuming => t(cx, "继续中…", "Resuming…"),
            Self::Cancelling => t(cx, "取消中…", "Cancelling…"),
            Self::Completed => t(cx, "已完成", "Completed"),
            Self::Cancelled => t(cx, "已取消", "Cancelled"),
            Self::Failed => t(cx, "未完成", "Incomplete"),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum TransferUpdate {
    Started(Option<u64>),
    Progress(u64, Option<u64>),
    Paused(u64, Option<u64>),
    Resumed(u64, Option<u64>),
    Finished(u64),
}

pub(super) struct TransferStatus {
    pub(super) phase: TransferPhase,
    pub(super) transferred: u64,
    pub(super) total: Option<u64>,
    source: String,
    destination: String,
    direction: TransferDirection,
    directory: bool,
    continuation: bool,
    pub(super) reviewed_existing: u64,
}

impl TransferStatus {
    pub(super) fn new(spec: TransferSpec, directory: bool, continuation: bool) -> Self {
        let (source, destination) = match spec.direction {
            TransferDirection::Upload => (spec.local.display().to_string(), spec.remote),
            TransferDirection::Download => (spec.remote, spec.local.display().to_string()),
        };
        Self {
            phase: TransferPhase::Preparing,
            transferred: 0,
            total: None,
            source,
            destination,
            direction: spec.direction,
            directory,
            continuation,
            reviewed_existing: 0,
        }
    }

    pub(super) fn update(&mut self, event: TransferUpdate) {
        match event {
            TransferUpdate::Started(total) => {
                self.total = total;
                if self.phase == TransferPhase::Preparing {
                    self.phase = TransferPhase::Running;
                }
            }
            TransferUpdate::Progress(bytes, total) => {
                self.transferred = bytes;
                self.total = total;
            }
            TransferUpdate::Paused(bytes, total) => {
                self.transferred = bytes;
                self.total = total;
                if self.phase != TransferPhase::Cancelling {
                    self.phase = TransferPhase::Paused;
                }
            }
            TransferUpdate::Resumed(bytes, total) => {
                self.transferred = bytes;
                self.total = total;
                if self.phase != TransferPhase::Cancelling {
                    self.phase = TransferPhase::Running;
                }
            }
            TransferUpdate::Finished(bytes) => self.transferred = bytes,
        }
    }

    pub(super) fn bytes_label(&self, cx: &App) -> String {
        let mut bytes = self.total.map_or_else(
            || format!("{} B", self.transferred),
            |total| format!("{} / {total} B", self.transferred),
        );
        if self.continuation && self.directory {
            bytes = format!(
                "{} {} · {} {} B",
                t(cx, "已确认", "Confirmed"),
                bytes,
                t(cx, "审核时已有", "Existing at review"),
                self.reviewed_existing
            );
        } else if self.continuation {
            bytes = format!(
                "{} · {} {} B",
                bytes,
                t(cx, "本次新增", "New"),
                self.transferred.saturating_sub(self.reviewed_existing)
            );
        }
        bytes
    }

    fn details(&self, cx: &App) -> String {
        format!(
            "{}\n{}\n\n{}\n{}",
            t(cx, "源", "Source"),
            self.source,
            t(cx, "目标", "Destination"),
            self.destination,
        )
    }
}

impl FilesPanel {
    pub(super) fn request_pause(&mut self, paused: bool, cx: &mut Context<Self>) {
        if self.suspended {
            return;
        }
        let Some(transfer) = &mut self.transfer else {
            return;
        };
        let required = if paused {
            TransferPhase::Running
        } else {
            TransferPhase::Paused
        };
        if !self.busy || transfer.phase != required {
            return;
        }
        let Some(control) = &self.transfer_pause else {
            return;
        };
        control.send_replace(paused);
        transfer.phase = if paused {
            TransferPhase::Pausing
        } else {
            TransferPhase::Resuming
        };
        cx.notify();
    }

    pub(super) fn cancel_active(&mut self, cx: &mut Context<Self>) {
        if !self.busy {
            return;
        }
        if let Some(stop) = &self.operation_stop {
            stop.store(true, Ordering::Release);
        }
        if let Some(transfer) = &mut self.transfer {
            transfer.phase = TransferPhase::Cancelling;
        }
        self.status = Message::new(
            "正在取消，等待传输清理…",
            "Cancelling; waiting for transport cleanup…",
        );
        cx.notify();
    }

    pub(super) fn transfer_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(transfer) = &self.transfer else {
            return div().into_any_element();
        };
        let bytes = transfer.bytes_label(cx);
        let kind = match (
            transfer.direction,
            transfer.directory,
            transfer.continuation,
        ) {
            (TransferDirection::Upload, false, false) => t(cx, "文件上传", "File upload"),
            (TransferDirection::Download, false, false) => t(cx, "文件下载", "File download"),
            (TransferDirection::Upload, true, false) => t(cx, "目录上传", "Folder upload"),
            (TransferDirection::Download, true, false) => t(cx, "目录下载", "Folder download"),
            (_, false, true) => t(cx, "文件断点续传", "File continuation"),
            (_, true, true) => t(cx, "目录断点续传", "Folder continuation"),
        };
        let details = transfer.details(cx);
        let fraction = transfer
            .total
            .filter(|total| *total > 0)
            .map_or(0., |total| {
                (transfer.transferred as f32 / total as f32).clamp(0., 1.)
            });
        div()
            .id("file-transfer-card")
            .test_support()
            .flex_shrink_0()
            .w_full()
            .min_w_0()
            .px_3()
            .py_2()
            .bg(rgb(CANVAS))
            .border_t_1()
            .border_color(rgb(BORDER))
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(kind),
                    )
                    .child(
                        div()
                            .id("file-transfer-phase")
                            .test_support()
                            .text_color(rgb(ACCENT))
                            .child(transfer.phase.label(cx)),
                    )
                    .child(
                        Popover::new("file-transfer-details-popover")
                            .anchor(Anchor::BottomRight)
                            .trigger(
                                Button::new("file-transfer-details")
                                    .ghost()
                                    .compact()
                                    .label(t(cx, "路径", "Paths")),
                            )
                            .content(move |_, window, _| {
                                div()
                                    .id("file-transfer-paths")
                                    .w(px(500.).min(
                                        (window.viewport_size().width - px(48.)).max(px(100.)),
                                    ))
                                    .max_h(px(200.))
                                    .overflow_scroll()
                                    .child(details.clone())
                            }),
                    )
                    .when(self.busy, |row| {
                        row.when(
                            matches!(
                                transfer.phase,
                                TransferPhase::Running | TransferPhase::Pausing
                            ),
                            |row| {
                                row.child(
                                    Button::new("pause-file-transfer")
                                        .ghost()
                                        .compact()
                                        .disabled(
                                            self.suspended
                                                || transfer.phase != TransferPhase::Running,
                                        )
                                        .label(t(cx, "暂停", "Pause"))
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.request_pause(true, cx)
                                        })),
                                )
                            },
                        )
                        .when(
                            matches!(
                                transfer.phase,
                                TransferPhase::Paused | TransferPhase::Resuming
                            ),
                            |row| {
                                row.child(
                                    Button::new("resume-file-transfer")
                                        .primary()
                                        .compact()
                                        .disabled(
                                            self.suspended
                                                || transfer.phase != TransferPhase::Paused,
                                        )
                                        .label(t(cx, "继续", "Continue"))
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.request_pause(false, cx)
                                        })),
                                )
                            },
                        )
                        .child(
                            Button::new("cancel-active-file-operation")
                                .ghost()
                                .compact()
                                .disabled(transfer.phase == TransferPhase::Cancelling)
                                .label(t(cx, "取消", "Cancel"))
                                .on_click(cx.listener(|view, _, _, cx| view.cancel_active(cx))),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_color(rgb(MUTED))
                            .child(transfer.destination.clone()),
                    )
                    .child(
                        div()
                            .id("file-transfer-bytes")
                            .test_support()
                            .max_w(relative(0.72))
                            .whitespace_normal()
                            .text_right()
                            .flex_shrink_0()
                            .child(bytes),
                    ),
            )
            .child(
                div()
                    .h(px(3.))
                    .w_full()
                    .rounded_full()
                    .bg(rgb(BORDER))
                    .overflow_hidden()
                    .child(div().h_full().w(relative(fraction)).bg(rgb(ACCENT))),
            )
            .into_any_element()
    }
}
