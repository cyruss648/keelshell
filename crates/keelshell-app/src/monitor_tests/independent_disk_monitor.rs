//! Fresh reviewer interactions on the real monitor with owned SSH fixture samples.
use super::*;

#[gpui_kit::test]
async fn independent_disk_samples_survive_production_theme_buttons_and_focused_input(
    cx: &mut TestAppContext,
) {
    let mut h = Harness::new(cx);
    h.panel.update(cx, |panel, _| panel.paused = true);
    h.idle(cx).await;
    cx.update_window(h.window, |_, window, cx| {
        window.click("refresh-monitor", cx)
    })
    .checked("explicit second owned SSH disk sample");
    h.idle(cx).await;
    let before = h.panel.read_with(cx, |panel, _| {
        (
            panel.snapshot.clone(),
            panel.rates.clone(),
            panel.mcp_sample_id,
            panel.last_sample,
        )
    });
    let commands = h.control.commands.load(Ordering::Acquire);
    crate::workspace::tests::independent_disk_monitor::exercise(h.panel.clone(), cx).await;
    let after = h.panel.read_with(cx, |panel, _| {
        assert_eq!(
            panel.selected_disk.as_ref().map(|id| id.name.as_str()),
            Some("fixture11")
        );
        assert!(panel.paused && !panel.busy && panel.pending.is_none());
        (
            panel.snapshot.clone(),
            panel.rates.clone(),
            panel.mcp_sample_id,
            panel.last_sample,
        )
    });
    assert_eq!(
        before, after,
        "appearance and text input do not replace sampled data"
    );
    assert_eq!(h.control.commands.load(Ordering::Acquire), commands);
    assert_eq!(h.control.terminating.load(Ordering::Acquire), 0);
    assert_eq!(h.control.probes.load(Ordering::Acquire), 0);
    h._runtime.block_on(async {
        h.session
            .close()
            .await
            .checked("close owned SSH monitor connection");
        h.server.abort();
        let joined = tokio::time::timeout(Duration::from_secs(2), &mut h.server)
            .await
            .checked("join owned monitor server task");
        assert!(joined.is_ok() || joined.as_ref().is_err_and(|error| error.is_cancelled()));
    });
}
