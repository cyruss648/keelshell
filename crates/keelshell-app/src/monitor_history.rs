//! Session-local CPU/memory history. Rendering reads observations, never SSH.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use gpui_kit::App;
use keelshell_session::monitor::Snapshot;

use crate::i18n::{Message, t};

const WINDOW: Duration = Duration::from_secs(300);
const MAX_POINTS: usize = 60;
// A late process-list response must not make an old resource sample look new.
const MAX_ADOPTION_AGE: Duration = Duration::from_secs(10);
// Beyond one polling interval plus jitter, a missing collection stays a gap.
const MAX_JOIN_INTERVAL: Duration = Duration::from_millis(7500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SampleTimeError {
    Late,
    NonForward,
    InvalidUptime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Series {
    Cpu,
    Memory,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Observation {
    at: Instant,
    cpu: Option<f64>,
    memory: Option<f64>,
    break_before: bool,
}

impl Observation {
    fn value(self, series: Series) -> Option<f64> {
        match series {
            Series::Cpu => self.cpu,
            Series::Memory => self.memory,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Vertex {
    pub x: f32,
    pub y: f32,
    pub connect: bool,
}

#[derive(Default)]
pub(super) struct ResourceHistory {
    points: VecDeque<Observation>,
    last_at: Option<Instant>,
    continuous: bool,
}

impl ResourceHistory {
    /// Invalidate the next CPU interval without inventing a missing observation.
    pub fn interrupt(&mut self) {
        self.continuous = false;
    }

    pub fn prune(&mut self, now: Instant) {
        self.points.retain(|point| {
            now.checked_duration_since(point.at)
                .is_some_and(|age| age <= WINDOW)
        });
    }

    /// Adopt only a fresh, forward local receipt time from the snapshot worker.
    ///
    /// The immutable panel owns the authenticated connection; its previous
    /// snapshot supplies the existing kernel counter calculation. History never
    /// changes the collector, persists values, or extrapolates missing samples.
    pub fn record(
        &mut self,
        snapshot: &Snapshot,
        previous: Option<&Snapshot>,
        at: Instant,
        now: Instant,
    ) -> Result<Option<f64>, SampleTimeError> {
        self.prune(now);
        let rejection = if now.checked_duration_since(at).is_none()
            || self.last_at.is_some_and(|last| at <= last)
        {
            Some(SampleTimeError::NonForward)
        } else if now.duration_since(at) > MAX_ADOPTION_AGE {
            Some(SampleTimeError::Late)
        } else if !snapshot.uptime_seconds.is_finite() || snapshot.uptime_seconds < 0. {
            Some(SampleTimeError::InvalidUptime)
        } else {
            None
        };
        if let Some(error) = rejection {
            self.interrupt();
            return Err(error);
        }
        let continuous = self.continuous
            && self
                .last_at
                .is_some_and(|last| at.duration_since(last) <= MAX_JOIN_INTERVAL)
            && previous.is_some_and(|old| {
                old.boot_time == snapshot.boot_time
                    && old.uptime_seconds.is_finite()
                    && snapshot.uptime_seconds > old.uptime_seconds
                    && self.last_at.is_some_and(|last| {
                        let local = at.duration_since(last).as_secs_f64();
                        (snapshot.uptime_seconds - old.uptime_seconds - local).abs()
                            <= MAX_ADOPTION_AGE.as_secs_f64()
                    })
                    && snapshot
                        .cpu
                        .ticks
                        .iter()
                        .zip(old.cpu.ticks)
                        .all(|(now, old)| *now >= old)
            });
        let cpu = previous
            .filter(|_| continuous)
            .and_then(|old| snapshot.rates_since(old).cpu_busy_percent)
            .and_then(valid_percent);
        let memory = snapshot
            .memory
            .used_bytes()
            .filter(|_| snapshot.memory.total_bytes > 0)
            .map(|used| used as f64 / snapshot.memory.total_bytes as f64 * 100.)
            .and_then(valid_percent);
        self.points.push_back(Observation {
            at,
            cpu,
            memory,
            break_before: !continuous,
        });
        while self.points.len() > MAX_POINTS {
            self.points.pop_front();
        }
        self.last_at = Some(at);
        self.continuous = true;
        Ok(cpu)
    }

    /// Normalize actual sample ages onto a fixed five-minute, 0..100% viewport.
    pub fn vertices(&self, series: Series, now: Instant) -> Vec<Vertex> {
        let mut connected = false;
        self.points
            .iter()
            .filter_map(|sample| {
                let age = now.checked_duration_since(sample.at)?;
                if age > WINDOW {
                    connected = false;
                    return None;
                }
                let Some(value) = sample.value(series).and_then(valid_percent) else {
                    connected = false;
                    return None;
                };
                let connect = connected && !sample.break_before;
                connected = true;
                Some(Vertex {
                    x: 1. - age.as_secs_f32() / WINDOW.as_secs_f32(),
                    y: 1. - value as f32 / 100.,
                    connect,
                })
            })
            .collect()
    }

    pub fn time_range(&self, now: Instant, cx: &App) -> String {
        let mut ages = self.points.iter().filter_map(|sample| {
            now.checked_duration_since(sample.at)
                .filter(|age| *age <= WINDOW)
        });
        let range = ages.next().map_or_else(
            || t(cx, "暂无采样", "No samples").to_owned(),
            |first| {
                let last = ages.next_back().unwrap_or(first);
                Message::new(
                    format!("采样 {} 秒前 → {} 秒前", first.as_secs(), last.as_secs()),
                    format!("Samples {}s ago → {}s ago", first.as_secs(), last.as_secs()),
                )
                .render(cx)
            },
        );
        Message::new(
            format!("近 5 分钟 · 最多 60 点 · 0–100% · 本机接收时间 · {range}"),
            format!("Last 5 min · Up to 60 points · 0–100% · Local receipt time · {range}"),
        )
        .render(cx)
    }

    fn text(&self, series: Series, now: Instant, cx: &App) -> (String, String) {
        let samples: Vec<_> = self
            .points
            .iter()
            .filter_map(|sample| {
                let age = now.checked_duration_since(sample.at)?;
                (age <= WINDOW).then_some((age, sample.value(series)))
            })
            .collect();
        let values: Vec<_> = samples.iter().filter_map(|(_, value)| *value).collect();
        let count = values.len();
        let range = samples.first().zip(samples.last()).map_or_else(
            || t(cx, "暂无采样", "No samples").to_owned(),
            |((first, _), (last, _))| {
                Message::new(
                    format!("{} 秒前 → {} 秒前", first.as_secs(), last.as_secs()),
                    format!("{}s ago → {}s ago", first.as_secs(), last.as_secs()),
                )
                .render(cx)
            },
        );
        let mut summary = if count == 0 {
            t(
                cx,
                "数据不足 · 无有效值",
                "Insufficient data · No valid values",
            )
            .to_owned()
        } else {
            let min = values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            Message::new(
                format!("{count} 点 · {min:.1}–{max:.1}%"),
                format!("{count} points · {min:.1}–{max:.1}%"),
            )
            .render(cx)
        };
        if count == 1 {
            summary.push_str(t(cx, " · 数据不足", " · Insufficient data"));
        }
        let interrupted = self
            .points
            .iter()
            .filter(|sample| {
                now.checked_duration_since(sample.at)
                    .is_some_and(|age| age <= WINDOW)
            })
            .enumerate()
            .any(|(index, sample)| {
                index > 0 && (sample.break_before || sample.value(series).is_none())
            });
        if interrupted {
            summary.push_str(t(cx, " · 断段", " · Gaps"));
        }
        let name = match series {
            Series::Cpu => "CPU",
            Series::Memory => t(cx, "内存", "Memory"),
        };
        let detail = samples
            .iter()
            .map(|(age, value)| {
                let value = value.map_or_else(
                    || t(cx, "未知", "unknown").to_owned(),
                    |value| format!("{value:.1}%"),
                );
                Message::new(
                    format!("{} 秒前 {value}", age.as_secs()),
                    format!("{}s ago {value}", age.as_secs()),
                )
                .render(cx)
            })
            .collect::<Vec<_>>()
            .join("; ");
        let accessible = Message::new(
            format!("{name} 近 5 分钟趋势，0–100%。{range}，{summary}。按本机收到采样的时间定位；未知值与中断不连线。{detail}"),
            format!("{name} trend, last 5 minutes, 0–100%. {range}, {summary}. Local snapshot receipt times; unknown values and interruptions break the line. {detail}"),
        ).render(cx);
        (summary, accessible)
    }
}

fn valid_percent(value: f64) -> Option<f64> {
    (value.is_finite() && (0. ..=100.).contains(&value)).then_some(value)
}

/// Paint cached, bounded geometry only. Single observations remain visible dots.
pub(super) fn view(
    history: &ResourceHistory,
    series: Series,
    now: Instant,
    cx: &App,
) -> impl gpui_kit::IntoElement {
    use gpui_kit::*;
    let visual = crate::design::palette(cx);
    let color = match series {
        Series::Cpu => visual.accent,
        Series::Memory => visual.success,
    };
    let vertices = history.vertices(series, now);
    let (summary, accessible) = history.text(series, now, cx);
    let id = match series {
        Series::Cpu => "monitor-cpu-trend",
        Series::Memory => "monitor-memory-trend",
    };
    div()
        .id(id)
        .test_support()
        .role(accesskit::Role::Image)
        .aria_label(accessible)
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(rgb(visual.muted)).child(summary))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    let width = (bounds.size.width - px(4.)).max(px(0.));
                    let height = (bounds.size.height - px(4.)).max(px(0.));
                    window.paint_quad(fill(bounds, rgb(visual.surface)));
                    let mut path = PathBuilder::stroke(px(1.5));
                    for vertex in &vertices {
                        let position = bounds.origin
                            + point(px(2.) + width * vertex.x, px(2.) + height * vertex.y);
                        if vertex.connect {
                            path.line_to(position);
                        } else {
                            path.move_to(position);
                        }
                        window.paint_quad(fill(
                            Bounds::new(position - point(px(1.5), px(1.5)), size(px(3.), px(3.))),
                            rgb(color),
                        ));
                    }
                    if !vertices.is_empty()
                        && let Ok(path) = path.build()
                    {
                        window.paint_path(path, rgb(color));
                    }
                },
            )
            .w_full()
            .h(px(48.)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use keelshell_session::monitor::{CpuCounters, DiskIoError, MemorySnapshot};

    fn snapshot(n: u64) -> Snapshot {
        Snapshot {
            boot_time: 1,
            uptime_seconds: n as f64,
            load_average: [0.; 3],
            cpu: CpuCounters {
                ticks: [n, 0, 0, n, 0, 0, 0, 0],
            },
            memory: MemorySnapshot {
                total_bytes: 100,
                available_bytes: Some(50),
                swap_total_bytes: None,
                swap_free_bytes: None,
            },
            networks: Vec::new(),
            filesystems: Vec::new(),
            disk_io: Err(DiskIoError::MissingData),
        }
    }

    #[test]
    fn history_evicts_by_real_time_and_by_sixty_samples_including_manual_refresh() {
        let start = Instant::now();
        let mut history = ResourceHistory::default();
        for n in 1..=75 {
            let at = start + Duration::from_secs(n);
            assert!(
                history
                    .record(&snapshot(n), Some(&snapshot(n - 1)), at, at)
                    .is_ok()
            );
        }
        assert_eq!(history.points.len(), 60);
        assert_eq!(history.points[0].at, start + Duration::from_secs(16));
        history.prune(start + Duration::from_secs(375));
        assert_eq!(history.points.len(), 1, "exactly 300 seconds is retained");
        history.prune(start + Duration::from_secs(376));
        assert!(history.points.is_empty());
    }

    #[test]
    fn first_cpu_unknown_and_real_idle_zero_are_different_from_missing_memory() {
        let start = Instant::now();
        let mut history = ResourceHistory::default();
        let first = snapshot(1);
        assert_eq!(history.record(&first, None, start, start), Ok(None));
        let mut second = snapshot(2);
        second.cpu.ticks[0] = 1;
        second.memory.available_bytes = Some(100);
        let at = start + Duration::from_secs(5);
        assert_eq!(history.record(&second, Some(&first), at, at), Ok(Some(0.)));
        assert_eq!(history.points[1].memory, Some(0.));
        let mut third = snapshot(3);
        third.memory.available_bytes = None;
        let at = at + Duration::from_secs(5);
        assert!(history.record(&third, Some(&second), at, at).is_ok());
        assert_eq!(history.points[2].memory, None);
        assert_eq!(history.vertices(Series::Memory, at).len(), 2);
    }

    #[test]
    fn reboot_rollback_and_remote_time_anomalies_start_new_segments() {
        for change in 0..5 {
            let start = Instant::now();
            let first = snapshot(10);
            let mut second = snapshot(20);
            match change {
                0 => second.boot_time = 2,
                1 => second.cpu.ticks[0] = 0,
                2 => second.uptime_seconds = 9.,
                3 => second.uptime_seconds = 10.,
                _ => second.uptime_seconds = 1000.,
            }
            let mut history = ResourceHistory::default();
            assert!(history.record(&first, None, start, start).is_ok());
            let at = start + Duration::from_secs(5);
            assert_eq!(history.record(&second, Some(&first), at, at), Ok(None));
            assert!(history.points[1].break_before);
            assert!(!history.vertices(Series::Memory, at)[1].connect);
        }
    }

    #[test]
    fn errors_pause_and_long_gaps_never_bridge_a_cpu_interval() {
        let start = Instant::now();
        let mut history = ResourceHistory::default();
        assert!(history.record(&snapshot(1), None, start, start).is_ok());
        history.interrupt();
        let at = start + Duration::from_secs(5);
        assert_eq!(
            history.record(&snapshot(2), Some(&snapshot(1)), at, at),
            Ok(None)
        );
        let at = at + Duration::from_secs(10);
        assert_eq!(
            history.record(&snapshot(3), Some(&snapshot(2)), at, at),
            Ok(None)
        );
        assert!(history.points.iter().all(|sample| sample.break_before));
    }

    #[test]
    fn late_future_duplicate_and_nonfinite_samples_add_no_old_value() {
        let start = Instant::now();
        let mut history = ResourceHistory::default();
        assert!(history.record(&snapshot(1), None, start, start).is_ok());
        for (at, now, uptime) in [
            (start, start, 2.),
            (start + Duration::from_secs(5), start, 2.),
            (
                start + Duration::from_secs(5),
                start + Duration::from_secs(16),
                2.,
            ),
            (
                start + Duration::from_secs(5),
                start + Duration::from_secs(5),
                f64::NAN,
            ),
        ] {
            let mut sample = snapshot(2);
            sample.uptime_seconds = uptime;
            assert!(
                history
                    .record(&sample, Some(&snapshot(1)), at, now)
                    .is_err()
            );
            assert_eq!(history.points.len(), 1);
            assert!(!history.continuous);
        }
        for value in [f64::NAN, f64::INFINITY, -1., 100.1] {
            assert_eq!(valid_percent(value), None);
        }
    }

    #[test]
    fn geometry_uses_real_spacing_and_missing_values_break_only_their_series() {
        let start = Instant::now();
        let mut history = ResourceHistory::default();
        let mut previous = None;
        for (n, seconds) in [(1, 0), (2, 2), (3, 7), (4, 9)] {
            let mut sample = snapshot(n);
            if n == 3 {
                sample.memory.available_bytes = None;
            }
            let at = start + Duration::from_secs(seconds);
            assert!(history.record(&sample, previous.as_ref(), at, at).is_ok());
            previous = Some(sample);
        }
        let now = start + Duration::from_secs(9);
        let cpu = history.vertices(Series::Cpu, now);
        let memory = history.vertices(Series::Memory, now);
        assert_eq!(cpu.len(), 3);
        assert!(cpu[1].connect && cpu[2].connect);
        assert!((cpu[1].x - cpu[0].x - 5. / 300.).abs() < 0.00001);
        assert!(!memory[2].connect);
        assert!(history.vertices(Series::Cpu, now + WINDOW).len() <= 1);
    }
}
