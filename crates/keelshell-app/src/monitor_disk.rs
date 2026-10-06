//! Per-device observations only; selecting a row never sends a remote command.
use super::*;
use keelshell_session::monitor::{DiskIoError, DiskRateUnavailable};

fn unavailable(reason: DiskRateUnavailable, cx: &App) -> &'static str {
    use DiskRateUnavailable as E;
    match reason {
        E::FirstSample => t(
            cx,
            "需要下一次有效采样",
            "Waiting for a valid second sample",
        ),
        E::BootChanged => t(
            cx,
            "主机已重启，请等待新采样",
            "Boot changed; wait for new samples",
        ),
        E::InvalidInterval => t(cx, "远端采样时间无效", "Remote sample interval is invalid"),
        E::NewDevice => t(
            cx,
            "新设备，等待下一次采样",
            "New device; waiting for a second sample",
        ),
        E::Disappeared => t(
            cx,
            "设备已消失或身份变化",
            "Device disappeared or identity changed",
        ),
        E::CounterReset => t(
            cx,
            "计数已重置，等待新采样",
            "Counters reset; wait for new samples",
        ),
        E::LayoutChanged => t(
            cx,
            "统计格式变化，等待新采样",
            "Counter layout changed; wait for new samples",
        ),
        E::Overflow => t(cx, "计数或速率超出范围", "Counter or rate overflow"),
    }
}

fn optional_error(error: &DiskIoError, cx: &App) -> &'static str {
    match error {
        DiskIoError::MissingData => t(
            cx,
            "磁盘统计不可用：目标未提供数据",
            "Disk statistics unavailable: no target data",
        ),
        DiskIoError::BootIdentity => t(
            cx,
            "磁盘统计不可用：缺少有效启动标识",
            "Disk statistics unavailable: no valid boot identity",
        ),
        DiskIoError::UnsupportedLayout => t(
            cx,
            "磁盘统计不可用：内核格式不支持",
            "Disk statistics unavailable: unsupported kernel layout",
        ),
        DiskIoError::Limit => t(
            cx,
            "磁盘统计不可用：超过采样上限",
            "Disk statistics unavailable: sample limit exceeded",
        ),
        DiskIoError::InvalidData => t(
            cx,
            "磁盘统计不可用：响应无效",
            "Disk statistics unavailable: invalid response",
        ),
    }
}

fn observation(id: &str, label: &str, value: String, color: u32) -> AnyElement {
    div()
        .id(id.to_owned())
        .test_support()
        .min_w_0()
        .flex()
        .justify_between()
        .gap_2()
        .text_xs()
        .child(div().flex_1().min_w_0().child(label.to_owned()))
        .child(div().flex_shrink_0().text_color(rgb(color)).child(value))
        .into_any_element()
}

// Valid finite rates may still be very large for tiny intervals. Keep the label
// bounded so remote counters cannot push the resource column outside its layout.
fn compact_number(value: f64) -> String {
    if !value.is_finite() || value < 0. {
        "—".into()
    } else if value >= 1_000_000. || (value > 0. && value < 0.01) {
        format!("{value:.2e}")
    } else {
        format!("{value:.2}")
    }
}

fn disk_bytes(value: Option<f64>) -> String {
    let Some(value) = value.filter(|v| v.is_finite() && *v >= 0.) else {
        return "—".into();
    };
    let units = ["B/s", "KiB/s", "MiB/s", "GiB/s", "TiB/s", "PiB/s", "EiB/s"];
    let mut scaled = value;
    let mut index = 0;
    while scaled >= 1024. && index + 1 < units.len() {
        scaled /= 1024.;
        index += 1;
    }
    format!("{} {}", compact_number(scaled), units[index])
}

impl MonitorPanel {
    pub(super) fn disk_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let mut view = section(cx, t(cx, "磁盘 I/O", "Disk I/O"), IconName::HardDrive)
            .id("monitor-disk-io")
            .test_support()
            .min_w_0()
            .flex_shrink_0()
            .child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                cx,
                "按设备/分区查看；父盘、分区和逻辑设备不汇总。",
                "Individual devices/partitions; parent, partition and logical rows are not summed.",
            )));
        let Some(snapshot) = self.snapshot.as_ref() else {
            return view
                .child(
                    div()
                        .id("monitor-disk-unavailable")
                        .test_support()
                        .text_xs()
                        .text_color(rgb(visual.muted))
                        .child(t(cx, "暂无磁盘 I/O 采样", "No disk I/O sample")),
                )
                .into_any_element();
        };
        let disks = match &snapshot.disk_io {
            Ok(disks) => disks,
            Err(error) => {
                return view
                    .child(
                        div()
                            .id("monitor-disk-unavailable")
                            .test_support()
                            .text_xs()
                            .text_color(rgb(visual.warning))
                            .child(optional_error(error, cx)),
                    )
                    .into_any_element();
            }
        };
        let mut choices = div()
            .id("monitor-disk-devices")
            .test_support()
            .min_w_0()
            .max_h(px(144.))
            .overflow_y_scroll()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1();
        for (index, disk) in disks.devices.iter().enumerate() {
            let identity = disk.device.clone();
            let label = if identity.name.len() > 22 {
                format!(
                    "{}… · {}:{}",
                    identity.name.chars().take(22).collect::<String>(),
                    identity.major,
                    identity.minor
                )
            } else {
                format!("{} · {}:{}", identity.name, identity.major, identity.minor)
            };
            choices = choices.child(
                Button::new(("monitor-disk-device", index))
                    .ghost()
                    .compact()
                    .w_full()
                    .label(label)
                    .bg(rgb(if self.selected_disk.as_ref() == Some(&identity) {
                        visual.selected
                    } else {
                        visual.surface
                    }))
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        panel.selected_disk = Some(identity.clone());
                        cx.notify();
                    })),
            );
        }
        view = view.child(choices);
        let Some(selected) = &self.selected_disk else {
            return view
                .child(
                    div()
                        .id("monitor-disk-select-hint")
                        .test_support()
                        .text_xs()
                        .text_color(rgb(visual.muted))
                        .child(t(
                            cx,
                            "选择设备查看区间速率",
                            "Select a device to inspect interval rates",
                        )),
                )
                .into_any_element();
        };
        view = view.child(
            div()
                .id("monitor-disk-identity")
                .test_support()
                .min_w_0()
                .overflow_x_scroll()
                .font_weight(FontWeight::SEMIBOLD)
                .text_xs()
                .child(format!(
                    "{} · {}:{}",
                    selected.name, selected.major, selected.minor
                )),
        );
        let Some(current) = disks.devices.iter().find(|disk| disk.device == *selected) else {
            return view
                .child(
                    div()
                        .id("monitor-disk-unavailable")
                        .test_support()
                        .text_xs()
                        .text_color(rgb(visual.warning))
                        .child(unavailable(DiskRateUnavailable::Disappeared, cx)),
                )
                .into_any_element();
        };
        let sample = self
            .rates
            .disks
            .iter()
            .find(|sample| sample.device == *selected);
        let valid = sample.and_then(|sample| sample.observation.as_ref().ok());
        if valid.is_none() {
            let reason = sample
                .and_then(|sample| sample.observation.as_ref().err())
                .copied()
                .unwrap_or(DiskRateUnavailable::FirstSample);
            view = view.child(
                div()
                    .id("monitor-disk-unavailable")
                    .test_support()
                    .text_xs()
                    .text_color(rgb(visual.warning))
                    .child(unavailable(reason, cx)),
            );
        }
        let mut metrics = div()
            .id("monitor-disk-metrics")
            .test_support()
            .min_w_0()
            .p_2()
            .rounded_md()
            .bg(rgb(visual.canvas))
            .flex()
            .flex_col()
            .gap_2()
            .child(observation(
                "monitor-disk-read-rate",
                t(cx, "读取", "Read"),
                disk_bytes(valid.map(|s| s.read_bytes_per_second)),
                visual.success,
            ))
            .child(observation(
                "monitor-disk-write-rate",
                t(cx, "写入", "Write"),
                disk_bytes(valid.map(|s| s.written_bytes_per_second)),
                visual.accent,
            ))
            .child(observation(
                "monitor-disk-read-iops",
                t(cx, "读取次数", "Read requests"),
                valid
                    .map(|s| format!("{}/s", compact_number(s.reads_per_second)))
                    .unwrap_or_else(|| "—".into()),
                visual.text,
            ))
            .child(observation(
                "monitor-disk-write-iops",
                t(cx, "写入次数", "Write requests"),
                valid
                    .map(|s| format!("{}/s", compact_number(s.writes_per_second)))
                    .unwrap_or_else(|| "—".into()),
                visual.text,
            ))
            .child(observation(
                "monitor-disk-read-duration",
                t(cx, "平均读取耗时", "Mean read time"),
                valid
                    .and_then(|s| s.read_milliseconds_per_request)
                    .map(|v| format!("{} ms", compact_number(v)))
                    .unwrap_or_else(|| "—".into()),
                visual.muted,
            ))
            .child(observation(
                "monitor-disk-write-duration",
                t(cx, "平均写入耗时", "Mean write time"),
                valid
                    .and_then(|s| s.write_milliseconds_per_request)
                    .map(|v| format!("{} ms", compact_number(v)))
                    .unwrap_or_else(|| "—".into()),
                visual.muted,
            ))
            .child(observation(
                "monitor-disk-inflight",
                t(cx, "当前未完成 I/O", "Current in-flight I/O"),
                current.in_flight().to_string(),
                visual.text,
            ));
        if let Some(valid) = valid {
            metrics = metrics.child(
                div().text_xs().text_color(rgb(visual.muted)).child(
                    Message::new(
                        format!(
                            "远端区间 {}s · 活动时间增量 {}ms（非利用率）",
                            compact_number(valid.interval_seconds),
                            valid.activity_milliseconds
                        ),
                        format!(
                            "Remote interval {}s · activity delta {}ms (not utilization)",
                            compact_number(valid.interval_seconds),
                            valid.activity_milliseconds
                        ),
                    )
                    .render(cx),
                ),
            );
        }
        view.child(metrics).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn disk_labels_preserve_zero_absence_and_bound_large_or_tiny_rates() {
        assert_eq!(disk_bytes(None), "—");
        assert_eq!(disk_bytes(Some(0.)), "0.00 B/s");
        assert_eq!(disk_bytes(Some(2048.)), "2.00 KiB/s");
        for value in [f64::MAX, 1e-300, 1_000_000., 123.4] {
            assert!(compact_number(value).len() <= 14);
            assert!(disk_bytes(Some(value)).len() <= 20);
        }
        for invalid in [f64::NAN, f64::INFINITY, -1.] {
            assert_eq!(disk_bytes(Some(invalid)), "—");
        }
    }
}
