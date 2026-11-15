//! Assert the text actually painted by the production component. Protocol
//! results below are explicitly synthetic presentation inputs.
use super::*;

fn painted_text(window: &Window, id: &str) -> String {
    gpui_kit::base::test_support::texts(window)
        .into_iter()
        .filter(|text| text.path().contains(&ElementId::from(id.to_owned())))
        .map(|text| text.text().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

#[gpui_kit::test]
fn path_comparison_localizes_painted_review_failures_and_observed_differences(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.preview(cx);
    let panel = h.panel.clone();
    let (window, _) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(400.), px(900.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, _| panel,
        )
        .checked("localized comparison view")
    });
    for language in [keelshell_core::Language::ZhCn, keelshell_core::Language::En] {
        cx.update_window(window, |_, window, cx| {
            crate::i18n::set_language(language, cx);
            h.panel.update(cx, |panel, cx| {
                panel.preview(cx);
                cx.notify();
            });
            window.render_frame(cx);
            let text = painted_text(window, "path-comparison-review-body");
            assert_eq!(
                window.find("path-comparison-review-text").role(),
                Some(gpui_kit::accesskit::Role::Label)
            );
            assert_eq!(
                window.find("path-comparison-review-text").label(),
                Some(text.as_str()),
                "the exact painted routes, commands and limits must be readable through AX"
            );
            if language == keelshell_core::Language::ZhCn {
                assert!(text.contains("单样本绝对期限八秒"));
                assert!(text.contains("已核验密钥"));
                assert!(text.contains("未确认清理"));
            } else {
                assert!(text.contains("eight seconds per sample"));
                assert!(text.contains("verified key"));
            }
            for kind in [
                PathProbeKind::Exec,
                PathProbeKind::Upload,
                PathProbeKind::Download,
            ] {
                assert!(
                    text.contains(kind.command()),
                    "exact command must survive localization"
                );
            }
            h.panel.update(cx, |panel, cx| {
                panel.review = None;
                let mut partial = report(PathComparisonStatus::Incomplete);
                partial.samples.truncate(1);
                partial.samples[0].outcome =
                    PathSampleOutcome::Failed(PathProbeFailure::OutputLimit);
                panel.result = Some(partial);
                panel.show_samples = true;
                cx.notify();
            });
            window.render_frame(cx);
            let text = painted_text(window, "path-comparison-raw-samples");
            let expected_failure = if language == keelshell_core::Language::ZhCn {
                "响应超过样本输出上限"
            } else {
                "Sample output limit exceeded"
            };
            assert!(text.contains(expected_failure), "{text}");
            assert_eq!(
                window.find(("path-comparison-sample", 0_usize)).label(),
                Some(text.as_str()),
                "a failed raw sample must retain its reason in AX"
            );
            h.panel.update(cx, |panel, cx| {
                panel.result = Some(report(PathComparisonStatus::Complete));
                panel.show_samples = false;
                cx.notify();
            });
            window.render_frame(cx);
            let id = ("path-comparison-difference", PathProbeKind::Exec as usize);
            assert!(window.find(id).bounds().size.height > px(0.));
            let difference = window.find(("path-comparison-difference-text", 0_usize));
            assert_eq!(difference.role(), Some(gpui_kit::accesskit::Role::Label));
            assert!(difference.label().is_some());
            assert!(
                window
                    .find(("path-comparison-median", PathProbeKind::Exec as usize))
                    .label()
                    .unwrap_or_default()
                    .contains("ms")
            );
            let text = gpui_kit::base::test_support::texts(window)
                .into_iter()
                .map(|text| text.text().to_owned())
                .collect::<Vec<_>>()
                .join("\n");
            let equal_text = if language == keelshell_core::Language::ZhCn {
                "观察差异不足，无法判断哪条路径更快。"
            } else {
                "Insufficient observed difference to determine which path is faster."
            };
            assert!(text.contains(equal_text), "{text}");
            for micros in [20_009, 19_991, 20_010, 19_990] {
                h.panel.update(cx, |panel, cx| {
                    if let Some(report) = &mut panel.result {
                        for sample in &mut report.samples {
                            if sample.path == PathSide::B {
                                sample.elapsed = Duration::from_micros(micros);
                            }
                        }
                    }
                    cx.notify();
                });
                window.render_frame(cx);
                let text = gpui_kit::base::test_support::texts(window)
                    .into_iter()
                    .map(|text| text.text().to_owned())
                    .collect::<Vec<_>>()
                    .join("\n");
                let expected = if micros == 20_009 || micros == 19_991 {
                    if language == keelshell_core::Language::ZhCn {
                        "差值低于 0.01 ms 显示精度，无法判断哪条路径更快。"
                    } else {
                        "Difference is below the 0.01 ms display precision; no faster path determined."
                    }
                } else if micros == 20_010 {
                    "B − A: +0.01 ms"
                } else {
                    "B − A: -0.01 ms"
                };
                assert!(text.contains(expected), "{micros}: {text}");
            }
            h.panel.update(cx, |panel, cx| {
                if let Some(report) = &mut panel.result {
                    for sample in &mut report.samples {
                        if sample.path == PathSide::B {
                            sample.elapsed = Duration::from_millis(10);
                        }
                    }
                }
                cx.notify();
            });
            window.render_frame(cx);
            let text = gpui_kit::base::test_support::texts(window)
                .into_iter()
                .map(|text| text.text().to_owned())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("B − A: -10.00 ms"));
            assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
        })
        .checked("painted bilingual review, exact commands, typed failure and signed differences");
    }
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("close localized view");
    h.finish(cx);
}
