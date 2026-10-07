#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{AssistantEvent, AssistantPanel, PreparedAssistantRequest, shell_blocks};
use crate::{ai_settings::EphemeralCredentials, i18n::set_language};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, InputEvent as _, TestAppContext, WindowBounds,
    WindowOptions, point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_ai::RequestCancellation;
use keelshell_core::{
    AiApiStyle, AiAuthentication, AiPreset, AiProfileCatalog, Language, NamedAiProfile,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

fn profile(name: &str) -> NamedAiProfile {
    let mut profile = NamedAiProfile::draft(AiPreset::OpenAiCompatible);
    profile.name = name.into();
    profile.endpoint = "https://provider.example/v1/chat/completions".into();
    profile.model = "fixture-model".into();
    profile.authentication = AiAuthentication::None;
    profile
}

// Start each owned exchange only after the corresponding control is revealed.
// UI preparation cannot consume its six-second request-arrival budget.
struct ProxyFixture {
    listener: TcpListener,
    origin: std::net::SocketAddr,
}

impl ProxyFixture {
    fn start(&self, step: usize) -> ProxyExchange {
        ProxyExchange::start(
            self.listener
                .try_clone()
                .unwrap_or_else(|error| panic!("proxy clone: {error}")),
            self.origin,
            step,
            Duration::from_secs(6),
        )
    }
}

struct ProxyExchange {
    accepted: Arc<std::sync::atomic::AtomicBool>,
    arrived: Arc<std::sync::atomic::AtomicBool>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
    release: Option<std::sync::mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ProxyExchange {
    fn start(
        listener: TcpListener,
        origin_address: std::net::SocketAddr,
        step: usize,
        limit: Duration,
    ) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};
        let accepted = Arc::new(AtomicBool::new(false));
        let connection_observed = accepted.clone();
        let arrived = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let (release, gated) = std::sync::mpsc::channel();
        let observed = arrived.clone();
        let cancelled = stopped.clone();
        let thread = std::thread::spawn(move || {
            let deadline = Instant::now() + limit;
            let mut stream = loop {
                assert!(
                    !cancelled.load(Ordering::Acquire),
                    "proxy exchange cancelled"
                );
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("proxy accept step {step}: {error}"),
                }
            };
            connection_observed.store(true, Ordering::Release);
            proxy_exchange(
                &mut stream,
                origin_address,
                step,
                &observed,
                &cancelled,
                gated,
            );
        });
        Self {
            accepted,
            arrived,
            stopped,
            release: Some(release),
            thread: Some(thread),
        }
    }

    fn arrived(&self) -> bool {
        self.arrived.load(std::sync::atomic::Ordering::Acquire)
    }

    fn finished(&self) -> bool {
        self.thread
            .as_ref()
            .is_none_or(std::thread::JoinHandle::is_finished)
    }

    fn release_reply(&mut self) {
        self.release
            .take()
            .unwrap_or_else(|| panic!("one reply gate"))
            .send(())
            .unwrap_or_else(|error| panic!("proxy reply: {error}"));
    }

    fn join(&mut self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(4);
        while !self.finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !self.finished() {
            return Err("owned proxy thread exceeded cleanup budget".into());
        }
        self.thread
            .take()
            .ok_or("owned proxy thread already joined")?
            .join()
            .map_err(|_| "owned proxy fixture panicked".into())
    }

    fn finish(mut self) {
        self.join()
            .unwrap_or_else(|error| panic!("proxy fixture: {error}"));
    }
}

impl Drop for ProxyExchange {
    fn drop(&mut self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Release);
        self.release.take();
        if self.thread.is_some() {
            let result = self.join();
            // A failing test must still close its listener and join its worker.
            if let Err(error) = result {
                eprintln!("owned proxy fixture cleanup: {error}");
            }
        }
    }
}

fn proxy_read_exact(
    stream: &mut std::net::TcpStream,
    mut bytes: &mut [u8],
    deadline: Instant,
    stopped: &std::sync::atomic::AtomicBool,
) -> std::io::Result<()> {
    while !bytes.is_empty() {
        if stopped.load(std::sync::atomic::Ordering::Acquire) {
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        stream.set_read_timeout(Some(remaining.min(Duration::from_millis(20))))?;
        match stream.read(bytes) {
            Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => bytes = &mut bytes[count..],
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn proxy_exchange(
    stream: &mut std::net::TcpStream,
    origin_address: std::net::SocketAddr,
    step: usize,
    arrived: &std::sync::atomic::AtomicBool,
    stopped: &std::sync::atomic::AtomicBool,
    gated: std::sync::mpsc::Receiver<()>,
) {
    stream
        .set_nonblocking(false)
        .unwrap_or_else(|error| panic!("stream mode: {error}"));
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap_or_else(|error| panic!("write limit: {error}"));
    let read_deadline = Instant::now() + Duration::from_secs(3);
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        proxy_read_exact(stream, &mut byte, read_deadline, stopped)
            .unwrap_or_else(|error| panic!("proxy header: {error}"));
        headers.push(byte[0]);
        assert!(headers.len() <= 8192);
    }
    let headers = String::from_utf8(headers).unwrap_or_else(|error| panic!("header utf8: {error}"));
    let lower = headers.to_ascii_lowercase();
    assert!(lower.contains("x-project: synthetic-route-header\r\n"));
    assert!(lower.contains("proxy-authorization: basic "));
    assert!(headers.starts_with(&format!(
        "{} http://{origin_address}/v1/{} HTTP/1.1",
        if step == 0 { "GET" } else { "POST" },
        if step == 0 {
            "models"
        } else {
            "chat/completions"
        }
    )));
    let length = lower
        .lines()
        .find_map(|line| {
            line.strip_prefix("content-length: ")
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    assert!(length <= 32768);
    let mut body = vec![0; length];
    proxy_read_exact(stream, &mut body, read_deadline, stopped)
        .unwrap_or_else(|error| panic!("proxy body: {error}"));
    let body = String::from_utf8(body).unwrap_or_else(|error| panic!("body utf8: {error}"));
    for secret in [
        "synthetic-route-header",
        "synthetic-route-user",
        "synthetic-route-password",
    ] {
        assert!(
            !body.contains(secret),
            "credentials never enter request context"
        );
    }
    if step == 1 {
        assert!(body.contains(keelshell_ai::CONNECTIVITY_PROMPT));
        assert!(!body.contains("synthetic-question"));
    } else if step == 2 {
        assert!(body.contains("synthetic-question"));
    }
    // Arrival means the complete real request and all route/body assertions passed.
    arrived.store(true, std::sync::atomic::Ordering::Release);
    gated
        .recv_timeout(Duration::from_secs(6))
        .unwrap_or_else(|error| panic!("reply gate: {error}"));
    assert!(!stopped.load(std::sync::atomic::Ordering::Acquire));
    let response = if step == 0 {
        r#"{"data":[{"id":"fixture-model"}]}"#
    } else if step == 1 {
        r#"{"model":"fixture-model","choices":[{"message":{"content":"OK"}}]}"#
    } else {
        r#"{"choices":[{"message":{"content":"synthetic-answer synthetic-route-header synthetic-route-user synthetic-route-password"}}]}"#
    };
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap_or_else(|error| panic!("proxy response: {error}"));
}

fn assert_proxy_peer_closed(peer: &mut std::net::TcpStream) {
    match peer.read(&mut [0]) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
            ) => {}
        result => panic!("owned peer was not closed: {result:?}"),
    }
}

fn wait_for_proxy_arrival(exchange: &ProxyExchange) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !exchange.arrived() && !exchange.finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(exchange.arrived(), "complete validated request must arrive");
}

fn proxy_fixture_request(address: std::net::SocketAddr) -> std::net::TcpStream {
    let mut stream = std::net::TcpStream::connect(address)
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    stream.write_all(b"GET http://127.0.0.1:1/v1/models HTTP/1.1\r\nx-project: synthetic-route-header\r\nproxy-authorization: basic fixture\r\n\r\n").unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    stream
}

#[test]
fn proxy_fixture_late_preparation_does_not_consume_exchange_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    // Deliberately finish preparation after the original six-second arrival budget.
    // The request still has six seconds once started; no worker exists during setup.
    let setup = Instant::now();
    std::thread::sleep(Duration::from_millis(6050));
    assert!(setup.elapsed() > Duration::from_secs(6));
    let mut exchange = ProxyExchange::start(
        listener,
        "127.0.0.1:1"
            .parse()
            .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}")),
        0,
        Duration::from_secs(6),
    );
    let mut peer = proxy_fixture_request(address);
    wait_for_proxy_arrival(&exchange);
    assert!(
        !exchange.finished(),
        "reply gate holds the actual exchange pending"
    );
    exchange.release_reply();
    let mut response = String::new();
    peer.read_to_string(&mut response)
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    assert!(response.contains("fixture-model"));
    exchange.finish();
}

#[test]
fn proxy_fixture_missing_peer_expires_and_joins_without_arrival() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    let mut exchange = ProxyExchange::start(
        listener,
        "127.0.0.1:1"
            .parse()
            .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}")),
        0,
        Duration::from_millis(30),
    );
    assert!(exchange.join().is_err());
    assert!(!exchange.arrived());
    assert!(exchange.thread.is_none());
    assert!(std::net::TcpStream::connect(address).is_err());
}

#[test]
fn proxy_fixture_partial_peer_drop_cancels_and_joins_worker() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    let exchange = ProxyExchange::start(
        listener,
        "127.0.0.1:1"
            .parse()
            .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}")),
        0,
        Duration::from_secs(6),
    );
    let mut peer = std::net::TcpStream::connect(address)
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    peer.set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    peer.write_all(b"G")
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    let accepted_deadline = Instant::now() + Duration::from_secs(3);
    while !exchange.accepted.load(std::sync::atomic::Ordering::Acquire)
        && !exchange.finished()
        && Instant::now() < accepted_deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(exchange.accepted.load(std::sync::atomic::Ordering::Acquire));
    assert!(!exchange.arrived());
    let start = Instant::now();
    drop(exchange);
    assert!(start.elapsed() < Duration::from_secs(4));
    assert_proxy_peer_closed(&mut peer);
    assert!(std::net::TcpStream::connect(address).is_err());
}

#[test]
fn proxy_fixture_pending_reply_drop_closes_gate_and_joins_worker() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}"));
    let exchange = ProxyExchange::start(
        listener,
        "127.0.0.1:1"
            .parse()
            .unwrap_or_else(|error| panic!("owned proxy fixture operation: {error}")),
        0,
        Duration::from_secs(6),
    );
    let mut peer = proxy_fixture_request(address);
    wait_for_proxy_arrival(&exchange);
    let start = Instant::now();
    drop(exchange);
    assert!(start.elapsed() < Duration::from_secs(4));
    assert_proxy_peer_closed(&mut peer);
    assert!(std::net::TcpStream::connect(address).is_err());
}

#[gpui_kit::test]
async fn request_options_real_settings_apply_uses_same_proxy_for_discovery_test_and_ask(
    cx: &mut TestAppContext,
) {
    use crate::ai_settings::{
        AiSettingsEvent,
        tests::{mount_sized, request_has_finished},
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    let origin = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("origin: {error}"));
    origin
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("origin mode: {error}"));
    let origin_address = origin
        .local_addr()
        .unwrap_or_else(|error| panic!("origin address: {error}"));
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("proxy: {error}"));
    proxy
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("proxy mode: {error}"));
    let proxy_address = proxy
        .local_addr()
        .unwrap_or_else(|error| panic!("proxy address: {error}"));
    let fixture = ProxyFixture {
        listener: proxy,
        origin: origin_address,
    };
    let mut configuration = profile("Proxy fixture");
    configuration.endpoint = format!("http://{origin_address}/v1/chat/completions");
    let (settings_handle, settings) = mount_sized(cx, configuration, 900., 580.);
    // Real controls generate the metadata and ephemeral slots; no fixture inserts them directly.
    cx.update_window(settings_handle, |_, window, cx| {
        window.render_frame(cx);
        window.scroll(
            "ai-profile-form-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        window.click("ai-header-add", cx);
    })
    .unwrap_or_else(|error| panic!("add UI header: {error}"));
    for (id, value) in [
        (
            gpui_kit::ElementId::from(("ai-header-name", 0_usize)),
            "x-project".to_owned(),
        ),
        (
            gpui_kit::ElementId::from(("ai-header-value", 0_usize)),
            "synthetic-route-header".to_owned(),
        ),
    ] {
        input_request_option(settings_handle, id, &value, cx);
    }
    cx.update_window(settings_handle, |_, window, cx| {
        reveal_request_option(window, "ai-proxy-explicit".into(), cx);
        window.click("ai-proxy-explicit", cx);
    })
    .unwrap_or_else(|error| panic!("explicit UI route: {error}"));
    input_request_option(
        settings_handle,
        "ai-proxy-url".into(),
        &format!("http://{proxy_address}"),
        cx,
    );
    cx.update_window(settings_handle, |_, window, cx| {
        reveal_request_option(window, "ai-proxy-auth-toggle".into(), cx);
        window.click("ai-proxy-auth-toggle", cx);
    })
    .unwrap_or_else(|error| panic!("proxy UI auth: {error}"));
    input_request_option(
        settings_handle,
        "ai-proxy-username".into(),
        "synthetic-route-user",
        cx,
    );
    input_request_option(
        settings_handle,
        "ai-proxy-password".into(),
        "synthetic-route-password",
        cx,
    );
    for (step, id) in ["ai-models-discover", "ai-profile-test"]
        .into_iter()
        .enumerate()
    {
        let mut exchange = None;
        cx.update_window(settings_handle, |_, window, cx| {
            reveal_request_option(window, id.into(), cx);
            exchange = Some(fixture.start(step));
            window.click(id, cx);
        })
        .unwrap_or_else(|error| panic!("explicit settings request: {error}"));
        let mut exchange = exchange.unwrap_or_else(|| panic!("settings request gate"));
        cx.run_until_parked();
        cx.wait_for(settings_handle, Duration::from_secs(5), |_, _| {
            exchange.arrived() || exchange.finished()
        })
        .await;
        assert!(
            exchange.arrived(),
            "validated settings request arrived before release"
        );
        assert!(!cx.update(|cx| request_has_finished(&settings, cx)));
        exchange.release_reply();
        cx.wait_for(settings_handle, Duration::from_secs(5), |_, cx| {
            request_has_finished(&settings, cx)
        })
        .await;
        exchange.finish();
    }
    let (assistant_handle, assistant) = mount(cx);
    let applied = Arc::new(AtomicBool::new(false));
    assistant.update(cx, |panel, cx| {
        let applied = applied.clone();
        panel
            ._subscriptions
            .push(cx.subscribe(&settings, move |panel, settings, event, cx| {
                if let AiSettingsEvent::Apply {
                    catalog,
                    credentials,
                    revision,
                } = event
                {
                    panel.set_profiles(catalog, credentials, cx);
                    panel.select_profile(
                        catalog
                            .active_id
                            .unwrap_or_else(|| panic!("applied profile")),
                        cx,
                    );
                    settings.update(cx, |settings, cx| settings.mark_saved(*revision, cx));
                    applied.store(true, Ordering::Release);
                }
            }));
    });
    cx.update_window(settings_handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("ai-settings-apply", cx);
    })
    .unwrap_or_else(|error| panic!("apply UI options: {error}"));
    cx.run_until_parked();
    assert!(applied.load(Ordering::Acquire));
    cx.update_window(assistant_handle, |_, window, cx| {
        assistant.update(cx, |panel, cx| {
            panel.prompt.update(cx, |input, cx| input.set_value("synthetic-question synthetic-route-header synthetic-route-user synthetic-route-password", window, cx));
        });
    }).unwrap_or_else(|error| panic!("question: {error}"));
    cx.run_until_parked();
    cx.update_window(assistant_handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("prepare-request", cx);
    })
    .unwrap_or_else(|error| panic!("review UI options: {error}"));
    assistant.read_with(cx, |panel, cx| {
        let prepared = panel
            .prepared
            .as_ref()
            .unwrap_or_else(|| panic!("prepared routed request"));
        let summary = prepared.request_options_summary(panel.profile.as_ref(), cx);
        assert!(summary.contains(&proxy_address.to_string()));
        assert!(summary.contains("x-project"));
        assert!(summary.contains("临时值"));
        for secret in [
            "synthetic-route-header",
            "synthetic-route-user",
            "synthetic-route-password",
        ] {
            assert!(!summary.contains(secret));
            assert!(!prepared.preview_json().contains(secret));
        }
    });
    let mut exchange = None;
    cx.update_window(assistant_handle, |_, window, cx| {
        window.render_frame(cx);
        window.scroll(
            "assistant-scroll",
            gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
            cx,
        );
        exchange = Some(fixture.start(2));
        window.click("send-approved-request", cx);
    })
    .unwrap_or_else(|error| panic!("send reviewed UI options: {error}"));
    let mut exchange = exchange.unwrap_or_else(|| panic!("Ask request gate"));
    cx.run_until_parked();
    cx.wait_for(assistant_handle, Duration::from_secs(5), |_, _| {
        exchange.arrived() || exchange.finished()
    })
    .await;
    assert!(
        exchange.arrived(),
        "validated Ask request arrived before release"
    );
    assert!(assistant.read_with(cx, |panel, _| panel.busy));
    exchange.release_reply();
    cx.wait_for(assistant_handle, Duration::from_secs(5), |_, cx| {
        !assistant.read(cx).busy
    })
    .await;
    assistant.read_with(cx, |panel, _| {
        assert!(panel.response.contains("synthetic-answer"));
        for secret in [
            "synthetic-route-header",
            "synthetic-route-user",
            "synthetic-route-password",
        ] {
            assert!(!panel.response.contains(secret));
        }
    });
    exchange.finish();
    assert!(
        matches!(origin.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}

fn reveal_request_option(
    window: &mut gpui_kit::Window,
    id: gpui_kit::ElementId,
    cx: &mut gpui_kit::App,
) {
    window.render_frame(cx);
    let viewport = window.find("ai-profile-form-scroll").bounds();
    let bounds = window.find(id.clone()).bounds();
    window.scroll(
        "ai-profile-form-scroll",
        gpui_kit::ScrollDelta::Pixels(point(px(0.), viewport.top() + px(30.) - bounds.top())),
        cx,
    );
    assert!(window.find(id).visible());
}

fn input_request_option(
    handle: AnyWindowHandle,
    id: gpui_kit::ElementId,
    value: &str,
    cx: &mut TestAppContext,
) {
    cx.update_window(handle, |_, window, cx| {
        reveal_request_option(window, id.clone(), cx);
        window.click(id, cx);
        window.input(value, cx);
    })
    .unwrap_or_else(|error| panic!("UI request option: {error}"));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn request_options_credential_change_on_inactive_profile_revokes_review(cx: &mut TestAppContext) {
    use crate::ai_request_options::{RequestSecret, SecretPurpose};
    use keelshell_core::{AiCustomHeader, AiSecretRef};
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        let revision = panel.request_revision;
        let mut profiles = panel.profiles.clone();
        let inactive = &mut profiles.profiles[1];
        let reference = AiSecretRef::Ephemeral {
            id: uuid::Uuid::new_v4(),
        };
        inactive.custom_headers.push(AiCustomHeader {
            name: "x-project".into(),
            value_ref: reference.clone(),
        });
        let mut credentials = panel.credentials.clone();
        credentials.insert_request(
            inactive,
            SecretPurpose::Header("x-project".into()),
            reference,
            RequestSecret::Header(Zeroizing::new("synthetic-inactive-secret".into())),
        );
        panel.set_profiles(&profiles, &credentials, cx);
        assert!(panel.prepared.is_none());
        assert!(panel.request_revision > revision);
        panel.set_context(
            "synthetic-inactive-secret".into(),
            "host".into(),
            "session-A".into(),
            cx,
        );
        panel.prepare(cx);
        assert!(
            !panel
                .prepared
                .as_ref()
                .unwrap_or_else(|| panic!("redacted review"))
                .preview_json()
                .contains("synthetic-inactive-secret")
        );
        let selected = &mut profiles.profiles[0];
        let reference = AiSecretRef::Ephemeral {
            id: uuid::Uuid::new_v4(),
        };
        selected.custom_headers.push(AiCustomHeader {
            name: "x-selected".into(),
            value_ref: reference.clone(),
        });
        credentials.insert_request(
            selected,
            SecretPurpose::Header("x-selected".into()),
            reference,
            RequestSecret::Header(Zeroizing::new("synthetic-selected-secret".into())),
        );
        panel.set_profiles(&profiles, &credentials, cx);
        panel.prepare(cx);
        let request = panel
            .prepared
            .as_ref()
            .unwrap_or_else(|| panic!("options review"));
        let summary = request.request_options_summary(panel.profile.as_ref(), cx);
        assert!(summary.contains("x-selected"));
        assert!(!summary.contains("synthetic-selected-secret"));
        assert!(panel.prepared_key.is_none());
    });
}

fn mount(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<AssistantPanel>) {
    cx.update(gpui_kit::init);
    let one = profile("Profile A");
    let catalog = AiProfileCatalog {
        active_id: Some(one.id),
        profiles: vec![one, profile("Profile B")],
    };
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("test runtime: {error}")),
    );
    let fixture = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(720.), px(900.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    AssistantPanel::new(&catalog, &EphemeralCredentials::new(), runtime, window, cx)
                })
            },
        )
        .unwrap_or_else(|error| panic!("assistant window: {error}"))
    });
    cx.update_window(fixture.0, |_, window, cx| {
        fixture.1.update(cx, |panel, cx| {
            panel.prompt.update(cx, |prompt, cx| {
                prompt.set_value("Explain this output", window, cx)
            });
            panel.set_context(
                "selected output".into(),
                "ops@server.example:22".into(),
                "session-A".into(),
                cx,
            );
        });
    })
    .unwrap_or_else(|error| panic!("assistant fixture: {error}"));
    cx.run_until_parked();
    fixture
}

#[gpui_kit::test]
fn token_profile_change_revokes_exact_request_and_stale_completion(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let mut selected = panel.profile.clone().unwrap_or_else(|| panic!("profile"));
        selected.max_output_tokens = Some(512);
        selected.context_window_tokens = Some(8192);
        panel.set_profile(Some(selected.clone()), None, cx);
        panel.prepare(cx);
        let prepared = panel
            .prepared
            .as_ref()
            .unwrap_or_else(|| panic!("prepared"));
        let payload: serde_json::Value = serde_json::from_str(prepared.preview_json())
            .unwrap_or_else(|error| panic!("JSON: {error}"));
        assert_eq!(payload["max_completion_tokens"], 512);
        let old_revision = panel.request_revision;
        let cancellation = RequestCancellation::new();
        panel.cancellation = Some(cancellation.clone());
        panel.busy = true;
        selected.max_output_tokens = Some(1024);
        panel.set_profile(Some(selected), None, cx);
        assert!(cancellation.is_cancelled());
        assert!(panel.prepared.is_none());
        panel.finish_request(
            old_revision,
            ("host".into(), "session-A".into()),
            Ok("stale".into()),
            cx,
        );
        assert!(panel.response.is_empty());
        panel.prepare(cx);
        let prepared = panel
            .prepared
            .as_ref()
            .unwrap_or_else(|| panic!("new prepared"));
        let payload: serde_json::Value = serde_json::from_str(prepared.preview_json())
            .unwrap_or_else(|error| panic!("JSON: {error}"));
        assert_eq!(payload["max_completion_tokens"], 1024);
    });
}

#[gpui_kit::test]
fn native_question_edit_revokes_approved_payload_and_old_reply(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx);
    let (revision, cancellation) = panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        let token = RequestCancellation::new();
        panel.cancellation = Some(token.clone());
        panel.busy = true;
        (panel.request_revision, token)
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("input", panel.read(cx).prompt.entity_id()), cx);
        window.input("x", cx);
    })
    .unwrap_or_else(|error| panic!("edit question: {error}"));
    cx.run_until_parked();
    panel.update(cx, |panel, cx| {
        assert!(cancellation.is_cancelled());
        assert!(panel.request_revision > revision);
        assert!(panel.prepared.is_none());
        panel.finish_request(
            revision,
            ("oldhost".into(), "session-A".into()),
            Ok("```sh\necho stale\n```".into()),
            cx,
        );
        assert!(panel.response.is_empty());
        assert!(panel.response_target.is_none());
    });
}

#[gpui_kit::test]
fn reconnect_revokes_only_the_retired_context_and_keeps_question(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        let revision = panel.request_revision;
        let cancellation = RequestCancellation::new();
        panel.cancellation = Some(cancellation.clone());
        panel.busy = true;
        panel.invalidate_session("different-session", cx);
        assert!(panel.prepared.is_some());
        assert!(!cancellation.is_cancelled());
        panel.invalidate_session("session-A", cx);
        assert!(cancellation.is_cancelled());
        assert_eq!(
            panel.prompt.read(cx).value().as_ref(),
            "Explain this output"
        );
        assert!(panel.context.is_empty());
        assert!(panel.host.is_empty());
        assert!(panel.session_id.is_empty());
        assert!(panel.prepared.is_none());
        panel.finish_request(
            revision,
            ("old-host".into(), "session-A".into()),
            Ok("```sh\necho stale\n```".into()),
            cx,
        );
        assert!(panel.response.is_empty());
        assert!(panel.suggestions.is_empty());
        assert!(panel.response_target.is_none());
    });
}

#[gpui_kit::test]
fn changing_profile_preserves_host_question_and_context_but_revokes_old_result(
    cx: &mut TestAppContext,
) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        let revision = panel.request_revision;
        let second = panel.profiles.profiles[1].id;
        panel.select_profile(second, cx);
        assert_eq!(panel.profile.as_ref().map(|p| p.id), Some(second));
        assert_eq!(panel.host, "ops@server.example:22");
        assert_eq!(panel.session_id, "session-A");
        assert_eq!(panel.context, "selected output");
        assert_eq!(panel.prompt.read(cx).value(), "Explain this output");
        assert!(panel.prepared.is_none());
        panel.busy = true; // Simulate a new request starting before the old one resolves.
        panel.finish_request(
            revision,
            ("oldhost".into(), "session-A".into()),
            Ok("```sh\necho stale\n```".into()),
            cx,
        );
        assert!(
            panel.busy,
            "old response must not clear a newer request's busy state"
        );
        assert!(panel.response.is_empty());
        assert!(panel.response_target.is_none());
    });
}

#[gpui_kit::test]
fn credential_rotation_and_profile_removal_revoke_preview(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        let profile = panel.profile.clone();
        panel.set_profile(profile, Some(Zeroizing::new("temporary-key".into())), cx);
        assert!(panel.prepared.is_none());
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        panel.set_profiles(
            &AiProfileCatalog::default(),
            &EphemeralCredentials::new(),
            cx,
        );
        assert!(panel.profile.is_none());
        assert!(panel.prepared.is_none());
        assert_eq!(panel.session_id, "session-A");
    });
}

#[gpui_kit::test]
fn changing_selected_session_discards_previous_target_and_reply(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let revision = panel.request_revision;
        panel.finish_request(
            revision,
            ("ops@server.example:22".into(), "session-A".into()),
            Ok("```sh\necho reviewed\n```".into()),
            cx,
        );
        assert_eq!(panel.suggestions, ["echo reviewed"]);
        assert_eq!(
            panel
                .response_target
                .as_ref()
                .map(|target| target.1.as_str()),
            Some("session-A")
        );
        panel.set_context(
            "new output".into(),
            "otherhost".into(),
            "session-B".into(),
            cx,
        );
        panel.finish_request(
            revision,
            ("oldhost".into(), "session-A".into()),
            Ok("```sh\necho stale\n```".into()),
            cx,
        );
        assert!(panel.suggestions.is_empty());
        assert!(panel.response_target.is_none());
        assert_eq!(panel.session_id, "session-B");
    });
}

#[gpui_kit::test]
fn focus_and_language_changes_preserve_the_exact_preview(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx);
    let (revision, payload, selected, status_zh) = panel.update(cx, |panel, cx| {
        panel.prepare(cx);
        let payload = panel
            .prepared
            .as_ref()
            .map(PreparedAssistantRequest::preview_json)
            .unwrap_or_else(|| panic!("fixture prepares"))
            .to_owned();
        (
            panel.request_revision,
            payload,
            panel.profile.clone(),
            panel.status.render(cx),
        )
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("input", panel.read(cx).prompt.entity_id()), cx);
        set_language(Language::En, cx);
        panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
        window.render_frame(cx);
    })
    .unwrap_or_else(|error| panic!("change locale: {error}"));
    cx.run_until_parked();
    panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.request_revision, revision);
        assert_eq!(
            panel
                .prepared
                .as_ref()
                .map(PreparedAssistantRequest::preview_json),
            Some(payload.as_str())
        );
        assert_eq!(panel.profile, selected);
        assert_eq!(panel.prompt.read(cx).value(), "Explain this output");
        assert_eq!(panel.session_id, "session-A");
        assert_ne!(panel.status.render(cx), status_zh);
    });
}

#[gpui_kit::test]
fn unsupported_configuration_and_missing_bearer_key_never_prepare(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let mut needs_key = profile("Needs key");
        needs_key.authentication = AiAuthentication::Bearer { credential: None };
        panel.set_profile(Some(needs_key.clone()), None, cx);
        panel.prepare(cx);
        assert!(panel.prepared.is_none());
        needs_key.proxy = keelshell_core::AiProxy::Explicit {
            url: "http://127.0.0.1:9080/invalid-path".into(),
            credentials: None,
        };
        panel.set_profile(
            Some(needs_key),
            Some(Zeroizing::new("temporary".into())),
            cx,
        );
        panel.prepare(cx);
        assert!(panel.prepared.is_none());

        let mut anthropic = profile("Anthropic");
        anthropic.api_style = AiApiStyle::AnthropicMessages;
        anthropic.endpoint = "http://127.0.0.1:9911/v1/messages".into();
        anthropic.authentication = AiAuthentication::Header {
            name: "x-api-key".into(),
            credential: None,
        };
        panel.set_profile(
            Some(anthropic),
            Some(Zeroizing::new("temporary-anthropic".into())),
            cx,
        );
        panel.prepare(cx);
        let preview = panel
            .prepared
            .as_ref()
            .map(PreparedAssistantRequest::preview_json)
            .unwrap_or_else(|| panic!("Anthropic profile should prepare"));
        assert!(preview.contains("\"max_tokens\": 4096"));
        assert!(preview.contains("\"system\""));
    });
}

#[gpui_kit::test]
async fn approved_send_delivers_delayed_tokio_reply_after_gpui_waits(cx: &mut TestAppContext) {
    let listener =
        TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("loopback bind: {error}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("loopback address: {error}"));
    let (release_reply, reply_gate) = std::sync::mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|error| panic!("nonblocking listener: {error}"));
        let deadline = Instant::now() + Duration::from_secs(3);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("bounded loopback accept: {error}"),
            }
        };
        // Some platforms inherit nonblocking mode from the listening socket.
        // Explicit blocking mode makes the bounded read/write timeouts apply.
        stream
            .set_nonblocking(false)
            .unwrap_or_else(|error| panic!("blocking HTTP stream: {error}"));
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap_or_else(|error| panic!("read timeout: {error}"));
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap_or_else(|error| panic!("write timeout: {error}"));
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream
                .read_exact(&mut byte)
                .unwrap_or_else(|error| panic!("read HTTP: {error}"));
            header.push(byte[0]);
            assert!(header.len() <= 4096, "request header bound");
        }
        let header = String::from_utf8_lossy(&header);
        assert!(header.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(!header.to_ascii_lowercase().contains("authorization"));
        let length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or_else(|| panic!("content length"));
        assert!(length <= 64 * 1024, "request body bound");
        let mut body = vec![0; length];
        stream
            .read_exact(&mut body)
            .unwrap_or_else(|error| panic!("read reviewed payload: {error}"));
        reply_gate
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|error| panic!("reply gate: {error}"));
        let response = r#"{"choices":[{"message":{"content":"```sh\necho reviewed\n```"}}]}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response)
            .unwrap_or_else(|error| panic!("reply HTTP: {error}"));
        String::from_utf8(body).unwrap_or_else(|error| panic!("UTF-8 payload: {error}"))
    });
    let (window, panel) = mount(cx);
    let preview = panel.update(cx, |panel, cx| {
        let mut profile = profile("Loopback");
        profile.endpoint = format!("http://{address}/v1/chat/completions");
        panel.set_profile(Some(profile), None, cx);
        panel.prepare(cx);
        let preview = panel
            .prepared
            .as_ref()
            .map(PreparedAssistantRequest::preview_json)
            .unwrap_or_else(|| panic!("reviewed request"))
            .to_owned();
        panel.send(cx);
        preview
    });
    // The server cannot reply until GPUI has suspended its completion task.
    cx.run_until_parked();
    assert!(panel.read_with(cx, |panel, _| panel.busy));
    release_reply
        .send(())
        .unwrap_or_else(|error| panic!("release reply: {error}"));
    cx.wait_for(window, Duration::from_secs(3), |_, cx| !panel.read(cx).busy)
        .await;
    panel.read_with(cx, |panel, _| {
        assert_eq!(panel.response, "```sh\necho reviewed\n```");
        assert_eq!(panel.suggestions, ["echo reviewed"]);
        assert_eq!(
            panel.response_target,
            Some(("ops@server.example:22".into(), "session-A".into()))
        );
    });
    assert_eq!(
        server.join().unwrap_or_else(|_| panic!("loopback server")),
        preview,
        "only the exact approved payload reaches the provider"
    );
}

#[test]
fn only_extracts_closed_explicit_shell_blocks() {
    assert_eq!(
        shell_blocks("```bash\necho hi\n```\n```json\n{}\n```"),
        vec!["echo hi"]
    );
    assert!(shell_blocks("```sh\nrm -rf /").is_empty());
}

#[gpui_kit::test]
fn diagnostic_plan_requires_explicit_build_and_keeps_session_binding(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.finish_request(
            panel.request_revision,
            ("ops@server.example:22".into(), "session-A".into()),
            Ok("The load is high.\n```sh\nuptime\n```\n```bash\nrm -rf /tmp/unknown\n```".into()),
            cx,
        );
        assert!(panel.diagnostic_plan.is_none());
        panel.build_diagnostic_plan(cx);
        let plan = panel
            .diagnostic_plan
            .as_ref()
            .unwrap_or_else(|| panic!("explicit build creates a plan"));
        assert_eq!(plan.steps().len(), 2);
        assert_eq!(plan.session_id(), "session-A");
        assert_eq!(plan.steps()[0].command(), "uptime");
        assert_eq!(
            plan.steps()[1].risk(),
            keelshell_ai::DiagnosticRisk::ReviewRequired
        );

        let review = plan
            .review_step(0, Duration::from_secs(60))
            .unwrap_or_else(|error| panic!("review step: {error}"));
        let proposal = review
            .into_proposal(plan, "session-A")
            .unwrap_or_else(|error| panic!("proposal: {error}"));
        assert_eq!(proposal.command, "uptime");
        assert!(panel.diagnostic_plan.is_some());
        panel.invalidate_session("session-A", cx);
        assert!(panel.diagnostic_plan.is_none());
    });
}

#[gpui_kit::test]
fn diagnostic_plan_buttons_only_emit_reviewable_text(cx: &mut TestAppContext) {
    let (window, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.finish_request(
            panel.request_revision,
            ("ops@server.example:22".into(), "session-A".into()),
            Ok("```sh\nss -ltn\n```".into()),
            cx,
        );
    });
    let observed = Arc::new(std::sync::Mutex::new(None::<(String, String)>));
    let copy = observed.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event: &AssistantEvent, _| {
            if let AssistantEvent::Suggestion {
                command,
                session_id,
            } = event
            {
                let Ok(mut value) = copy.lock() else {
                    return;
                };
                *value = Some((command.clone(), session_id.clone()));
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("build-diagnostic-plan", cx);
        window.render_frame(cx);
        window.click(("review-diagnostic-step", 0_usize), cx);
    })
    .unwrap_or_else(|error| panic!("diagnostic plan buttons: {error}"));
    cx.run_until_parked();
    let observed = observed
        .lock()
        .ok()
        .and_then(|value| value.clone())
        .unwrap_or_else(|| panic!("step emits a review suggestion"));
    assert_eq!(observed, ("ss -ltn".into(), "session-A".into()));
}

#[test]
fn rejects_terminal_control_in_suggestion() {
    assert!(shell_blocks("```bash\necho \u{1b}[2J\n``` ").is_empty());
}

#[gpui_kit::test]
fn locked_reference_blocks_preview_and_key_or_reference_changes_revoke_it(cx: &mut TestAppContext) {
    use keelshell_core::AiSecretRef;
    use uuid::Uuid;
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let mut selected = panel
            .profile
            .clone()
            .unwrap_or_else(|| panic!("selected profile"));
        selected.authentication = AiAuthentication::Bearer {
            credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
        };
        panel.set_profile(Some(selected.clone()), None, cx);
        panel.prepare(cx);
        assert!(panel.prepared.is_none());
        assert!(!panel.busy);
        panel.set_profile(
            Some(selected.clone()),
            Some(Zeroizing::new("fixture-key".into())),
            cx,
        );
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        selected.authentication = AiAuthentication::Bearer {
            credential: Some(AiSecretRef::SecretStore { id: Uuid::new_v4() }),
        };
        panel.set_profile(Some(selected.clone()), None, cx);
        assert!(panel.prepared.is_none());
        panel.prepare(cx);
        assert!(panel.prepared.is_none());
        panel.set_profile(
            Some(selected.clone()),
            Some(Zeroizing::new("fixture-key".into())),
            cx,
        );
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        selected.endpoint = "https://other.example/v1/chat/completions".into();
        panel.set_profile(Some(selected), None, cx);
        assert!(panel.prepared.is_none());
    });
}

fn local_profile(agent: keelshell_core::AiLocalAgent) -> NamedAiProfile {
    use keelshell_core::{AiBackend, AiLocalAgent};
    let mut profile = profile("Local CLI");
    profile.backend = AiBackend::LocalAgent {
        working_directory: Default::default(),
        agent,
        limits: Default::default(),
        executable: std::env::temp_dir()
            .join(format!("keelshell-nonexistent-{}", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into_owned(),
    };
    match agent {
        AiLocalAgent::Codex => {
            profile.api_style = AiApiStyle::Responses;
            profile.endpoint = "https://api.openai.com/v1".into();
            profile.authentication = AiAuthentication::Bearer { credential: None };
        }
        AiLocalAgent::ClaudeCode => {
            profile.api_style = AiApiStyle::AnthropicMessages;
            profile.endpoint = "https://api.anthropic.com".into();
            profile.authentication = AiAuthentication::Header {
                name: "x-api-key".into(),
                credential: None,
            };
        }
    }
    profile
}

fn owned_selected_directory_path(root: &std::path::Path) -> std::path::PathBuf {
    // Unix resolves aliases such as /var; Windows admission expects an ordinary
    // drive path and intentionally rejects canonicalize's verbatim namespace.
    #[cfg(unix)]
    let path = root.canonicalize().expect("canonical owned root");
    #[cfg(not(unix))]
    let path = root.to_path_buf();
    path
}

#[gpui_kit::test]
fn local_review_requires_explicit_key_redacts_context_and_survives_language(
    cx: &mut TestAppContext,
) {
    let (handle, panel) = mount(cx);
    cx.update_window(handle, |_, window, cx| {
        for agent in [
            keelshell_core::AiLocalAgent::Codex,
            keelshell_core::AiLocalAgent::ClaudeCode,
        ] {
            panel.update(cx, |panel, cx| {
                let profile = local_profile(agent);
                panel.set_profile(Some(profile.clone()), None, cx);
                panel.prepare(cx);
                assert!(
                    panel.prepared.is_none(),
                    "subscription/environment login never substitutes for an explicit key"
                );
                panel.set_profile(
                    Some(profile),
                    Some(Zeroizing::new("fixture-quote-\"-slash-\\-key".into())),
                    cx,
                );
                panel.set_context(
                    "selected fixture-quote-\"-slash-\\-key text".into(),
                    "fixture-host".into(),
                    "fixture-session".into(),
                    cx,
                );
                panel.prepare(cx);
                let PreparedAssistantRequest::Local(request) = panel
                    .prepared
                    .as_ref()
                    .unwrap_or_else(|| panic!("CLI review: {}", panel.status.render(cx)))
                else {
                    panic!("must not prepare HTTP");
                };
                assert!(!request.preview_stdin().contains("fixture-quote-"));
                let payload: serde_json::Value = serde_json::from_str(request.preview_json())
                    .unwrap_or_else(|_| panic!("review JSON"));
                assert_eq!(payload["model"], "fixture-model");
                assert_eq!(
                    payload["inference_endpoint"],
                    panel
                        .profile
                        .as_ref()
                        .unwrap_or_else(|| panic!("profile"))
                        .endpoint
                );
                assert!(
                    payload["credential"]
                        .as_str()
                        .unwrap_or_else(|| panic!("policy"))
                        .contains("subscription login not reused")
                );
                assert!(request.redaction_report().total_redactions() > 0);
                let before = request.preview_json().to_owned();
                assert!(!panel.busy);
                assert!(
                    panel._job.is_none(),
                    "preparation performs no filesystem/process lookup"
                );
                set_language(Language::En, cx);
                panel.refresh_locale(window, cx);
                assert_eq!(
                    panel
                        .prepared
                        .as_ref()
                        .unwrap_or_else(|| panic!("same review"))
                        .preview_json(),
                    before
                );
                set_language(Language::ZhCn, cx);
            });
        }
    })
    .unwrap_or_else(|_| panic!("CLI review languages"));
}

#[gpui_kit::test]
async fn selected_directory_review_runs_in_background_and_preserves_scrollable_payload(
    cx: &mut TestAppContext,
) {
    use keelshell_core::{AiBackend, AiLocalAgent, AiLocalAgentWorkingDirectory};
    let root = tempfile::tempdir().expect("owned selected directory");
    let path = owned_selected_directory_path(root.path()).join("完整-directory-".repeat(12));
    std::fs::create_dir(&path).expect("owned selected directory");
    let (handle, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let mut profile = local_profile(AiLocalAgent::Codex);
        if let AiBackend::LocalAgent {
            working_directory, ..
        } = &mut profile.backend
        {
            *working_directory = AiLocalAgentWorkingDirectory::Selected {
                path: path.to_string_lossy().into_owned(),
            };
        }
        panel.set_profile(
            Some(profile),
            Some(Zeroizing::new("owned-ephemeral-key".into())),
            cx,
        );
        panel.set_context(
            "unbroken-explicit-selected-output-".repeat(140),
            "owned-host".into(),
            "owned-session".into(),
            cx,
        );
        panel.prepare(cx);
        assert!(panel.busy);
        assert!(
            panel.prepared.is_none(),
            "no approval before background validation"
        );
        assert!(panel._job.is_some());
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .expect("responsive frame while validating");
    cx.wait_for(handle, Duration::from_secs(5), |_, cx| !panel.read(cx).busy)
        .await;
    let exact = panel.read_with(cx, |panel, _| {
        let Some(PreparedAssistantRequest::Local(request)) = &panel.prepared else {
            panic!("selected review");
        };
        let preview: serde_json::Value =
            serde_json::from_str(request.preview_json()).expect("complete review JSON");
        assert_eq!(
            preview["workspace"]["selected_path"].as_str(),
            path.to_str(),
            "JSON escaping does not change the selected native path"
        );
        assert_eq!(
            preview["workspace"]["canonical_path"].as_str(),
            path.canonicalize()
                .expect("canonical selected directory")
                .to_str(),
            "the independently resolved identity is included in the review"
        );
        assert!(
            !request.preview_json().contains("owned-session"),
            "request identity remains local"
        );
        assert!(
            !request
                .preview_stdin()
                .contains(path.to_str().expect("path"))
        );
        request.preview_json().to_owned()
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let viewport = window.find("assistant-request-full-preview").bounds();
        let scroll = panel.read(cx).preview_scroll.clone();
        assert!(
            scroll.max_offset().x > px(0.),
            "long original JSON line has a horizontal range"
        );
        assert!(
            scroll.max_offset().y > px(0.),
            "complete policy JSON has a vertical range"
        );
        let position = viewport.center();
        window.dispatch_event(
            gpui_kit::MouseMoveEvent {
                position,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        for delta in [point(px(-10000.), px(0.)), point(px(0.), px(-10000.))] {
            window.dispatch_event(
                gpui_kit::ScrollWheelEvent {
                    position,
                    delta: gpui_kit::ScrollDelta::Pixels(delta),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
        }
        assert!(scroll.offset().x < px(0.));
        assert!(scroll.offset().y < px(0.));
        assert!(window.find("assistant-request-preview-scrollbar").visible());
        for language in [Language::En, Language::ZhCn] {
            set_language(language, cx);
            panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
            assert_eq!(
                panel
                    .read(cx)
                    .prepared
                    .as_ref()
                    .expect("immutable review")
                    .preview_json(),
                exact
            );
        }
        panel.update(cx, |panel, cx| {
            panel.set_context(
                "changed".into(),
                "new-host".into(),
                "new-session".into(),
                cx,
            )
        });
        assert_eq!(scroll.offset(), point(px(0.), px(0.)));
        assert!(panel.read(cx).prepared.is_none());
    })
    .expect("two-axis original preview");
}

#[gpui_kit::test]
async fn changing_selected_directory_cancels_pending_prepare_without_replacing_new_review(
    cx: &mut TestAppContext,
) {
    use keelshell_core::{AiBackend, AiLocalAgent, AiLocalAgentWorkingDirectory};
    let root = tempfile::tempdir().expect("owned selected directory");
    let (handle, panel) = mount(cx);
    let cancellation = panel.update(cx, |panel, cx| {
        let mut profile = local_profile(AiLocalAgent::ClaudeCode);
        if let AiBackend::LocalAgent {
            working_directory, ..
        } = &mut profile.backend
        {
            *working_directory = AiLocalAgentWorkingDirectory::Selected {
                path: owned_selected_directory_path(root.path())
                    .to_string_lossy()
                    .into_owned(),
            };
        }
        panel.set_profile(
            Some(profile.clone()),
            Some(Zeroizing::new("owned-key".into())),
            cx,
        );
        panel.prepare(cx);
        let cancellation = panel
            .cancellation
            .as_ref()
            .expect("background owner")
            .clone();
        if let AiBackend::LocalAgent {
            working_directory, ..
        } = &mut profile.backend
        {
            *working_directory = AiLocalAgentWorkingDirectory::Isolated;
        }
        panel.set_profile(Some(profile), Some(Zeroizing::new("owned-key".into())), cx);
        assert!(cancellation.is_cancelled());
        panel.set_context(
            "new SSH selection".into(),
            "new host".into(),
            "new session".into(),
            cx,
        );
        panel.prepare(cx);
        assert!(
            !panel.busy,
            "default empty mode keeps historical synchronous preview"
        );
        cancellation
    });
    let exact = panel.read_with(cx, |panel, _| {
        panel
            .prepared
            .as_ref()
            .expect("new default review")
            .preview_json()
            .to_owned()
    });
    let deadline = Instant::now() + Duration::from_millis(60);
    cx.wait_for(handle, Duration::from_secs(2), |_, _| {
        Instant::now() >= deadline
    })
    .await;
    panel.read_with(cx, |panel, _| {
        assert!(cancellation.is_cancelled());
        assert_eq!(
            panel
                .prepared
                .as_ref()
                .expect("new review retained")
                .preview_json(),
            exact
        );
        assert!(panel.prepared_key.is_some());
        assert!(!panel.busy);
        assert!(panel.cancellation.is_none());
    });
}

#[gpui_kit::test]
fn local_executable_change_cancels_inflight_and_rejects_old_completion(cx: &mut TestAppContext) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let mut profile = local_profile(keelshell_core::AiLocalAgent::Codex);
        let key = Some(Zeroizing::new("fixture-key".into()));
        panel.set_profile(Some(profile.clone()), key.clone(), cx);
        panel.prepare(cx);
        assert!(matches!(
            panel.prepared,
            Some(PreparedAssistantRequest::Local(_))
        ));
        let revision = panel.request_revision;
        let cancellation = RequestCancellation::new();
        panel.cancellation = Some(cancellation.clone());
        panel.busy = true;
        if let keelshell_core::AiBackend::LocalAgent { executable, .. } = &mut profile.backend {
            *executable = std::env::temp_dir()
                .join("new-cli")
                .to_string_lossy()
                .into_owned();
        }
        panel.set_profile(Some(profile), key, cx);
        assert!(cancellation.is_cancelled());
        assert!(panel.prepared.is_none());
        panel.finish_reply(
            revision,
            ("old-target".into(), "old-session".into()),
            Ok("```sh\nwrong-target\n```".into()),
            cx,
        );
        assert!(panel.response.is_empty());
        assert!(panel.suggestions.is_empty());
        assert!(panel.response_target.is_none());
    });
}

#[gpui_kit::test]
async fn approved_local_failure_returns_on_gpui_without_http_fallback(cx: &mut TestAppContext) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| panic!("owned endpoint"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|_| panic!("nonblocking endpoint"));
    let (handle, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let mut profile = local_profile(keelshell_core::AiLocalAgent::Codex);
        profile.endpoint = format!(
            "http://{}",
            listener.local_addr().unwrap_or_else(|_| panic!("endpoint"))
        );
        panel.set_profile(
            Some(profile),
            Some(Zeroizing::new("fixture-key".into())),
            cx,
        );
        panel.prepare(cx);
        assert!(matches!(
            panel.prepared,
            Some(PreparedAssistantRequest::Local(_))
        ));
        panel.send(cx);
        assert!(panel.busy);
    });
    cx.wait_for(handle, Duration::from_secs(5), |_, cx| !panel.read(cx).busy)
        .await;
    panel.read_with(cx, |panel, cx| {
        assert!(panel.status.render(cx).contains("无法启动 CLI"));
        assert!(panel.response.is_empty());
        assert!(panel.suggestions.is_empty());
        assert!(panel.response_target.is_none());
        assert!(panel.prepared.is_none(), "send consumes its review once");
    });
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}

#[gpui_kit::test]
fn native_local_profile_choice_revokes_http_review_and_selects_cli(cx: &mut TestAppContext) {
    let (handle, panel) = mount(cx);
    let local = local_profile(keelshell_core::AiLocalAgent::ClaudeCode);
    let id = local.id;
    panel.update(cx, |panel, cx| {
        let mut catalog = panel.profiles.clone();
        catalog.profiles.push(local);
        let mut keys = EphemeralCredentials::new();
        keys.insert(id, Zeroizing::new("fixture-key".into()));
        panel.set_profiles(&catalog, &keys, cx);
        panel.prepare(cx);
        assert!(matches!(
            panel.prepared,
            Some(PreparedAssistantRequest::Api(_))
        ));
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("assistant-profile", cx);
        window.click(("assistant-profile-choice", 2_usize), cx);
    })
    .unwrap_or_else(|error| panic!("select local profile: {error}"));
    panel.update(cx, |panel, cx| {
        assert_eq!(panel.profile.as_ref().map(|p| p.id), Some(id));
        assert!(
            panel.prepared.is_none(),
            "selecting CLI revokes the old HTTP review"
        );
        panel.prepare(cx);
        assert!(matches!(
            panel.prepared,
            Some(PreparedAssistantRequest::Local(_))
        ));
        assert!(!panel.busy);
    });
}

#[gpui_kit::test]
fn long_cli_review_scrolls_to_visible_send_in_both_languages(cx: &mut TestAppContext) {
    let (handle, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        panel.set_profile(
            Some(local_profile(keelshell_core::AiLocalAgent::Codex)),
            Some(Zeroizing::new("fixture-key".into())),
            cx,
        );
        panel.set_context(
            "controlled remote context\n".repeat(500),
            "fixture-host".into(),
            "fixture-session".into(),
            cx,
        );
        panel.prepare(cx);
        assert!(matches!(
            panel.prepared,
            Some(PreparedAssistantRequest::Local(_))
        ));
    });
    cx.update_window(handle, |_, window, cx| {
        for language in [Language::ZhCn, Language::En] {
            set_language(language, cx);
            panel.update(cx, |panel, cx| panel.refresh_locale(window, cx));
            window.render_frame(cx);
            let viewport = window.find("assistant-scroll").bounds();
            let preview = window.find("assistant-request-full-preview").bounds();
            let scroll = panel.read(cx).preview_scroll.clone();
            assert!(
                scroll.max_offset().x > px(0.),
                "original long JSON line is readable horizontally"
            );
            assert!(
                scroll.max_offset().y > px(0.),
                "complete policy remains readable vertically"
            );
            assert!(preview.size.width <= viewport.size.width);
            window.scroll(
                "assistant-scroll",
                gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
                cx,
            );
            window.render_frame(cx);
            let position = window
                .find("assistant-request-full-preview")
                .bounds()
                .center();
            window.dispatch_event(
                gpui_kit::MouseMoveEvent {
                    position,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            for delta in [point(px(-10000.), px(0.)), point(px(0.), px(-10000.))] {
                window.dispatch_event(
                    gpui_kit::ScrollWheelEvent {
                        position,
                        delta: gpui_kit::ScrollDelta::Pixels(delta),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
                window.render_frame(cx);
            }
            assert!(
                scroll.offset().x < px(0.),
                "horizontal event at {position:?}: max {:?}, offset {:?}",
                scroll.max_offset(),
                scroll.offset()
            );
            assert!(scroll.offset().y < px(0.));
            let send = window.find("send-approved-request");
            let footer = window.find("assistant-confirmation-footer").bounds();
            assert!(
                send.visible(),
                "send visible after scrolling with {language:?}"
            );
            assert!(
                send.bounds().bottom() <= footer.bottom(),
                "send stays inside the fixed confirmation footer"
            );
            assert!(send.bounds().origin.y >= footer.origin.y);
            assert!(footer.top() >= viewport.bottom());
            assert!(send.bounds().size.height >= px(28.));
        }
        set_language(Language::ZhCn, cx);
    })
    .unwrap_or_else(|error| panic!("scroll CLI review: {error}"));
    panel.read_with(cx, |panel, _| {
        assert!(!panel.busy, "scroll and locale refresh do not send");
        assert!(panel.prepared.is_some());
        assert!(panel._job.is_none());
    });
}

#[gpui_kit::test]
fn local_budget_change_revokes_exact_review_and_old_result_without_changing_key(
    cx: &mut TestAppContext,
) {
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        for agent in [
            keelshell_core::AiLocalAgent::Codex,
            keelshell_core::AiLocalAgent::ClaudeCode,
        ] {
            let mut profile = local_profile(agent);
            let key = Some(Zeroizing::new("fixture-key".into()));
            panel.set_profile(Some(profile.clone()), key.clone(), cx);
            panel.prepare(cx);
            let Some(PreparedAssistantRequest::Local(request)) = panel.prepared.as_ref() else {
                panic!("local review");
            };
            let original = request.config().clone();
            let old_revision = panel.request_revision;
            let cancellation = RequestCancellation::new();
            panel.cancellation = Some(cancellation.clone());
            panel.busy = true;
            if let keelshell_core::AiBackend::LocalAgent { limits, .. } = &mut profile.backend {
                *limits = keelshell_core::AiLocalAgentLimits::new(27, 3, 19)
                    .unwrap_or_else(|_| panic!("valid budgets"));
            }
            panel.set_profile(Some(profile), key.clone(), cx);
            assert!(cancellation.is_cancelled());
            assert!(panel.prepared.is_none());
            assert!(panel.request_revision > old_revision);
            assert_eq!(panel.key, key);
            assert_eq!(
                original.limits(),
                keelshell_ai::LocalAgentLimits::default(),
                "captured review remains immutable"
            );
            panel.finish_reply(
                old_revision,
                ("old-host".into(), "old-session".into()),
                Ok("untrusted late answer".into()),
                cx,
            );
            assert!(panel.response.is_empty());
            assert!(panel.response_target.is_none());
            panel.prepare(cx);
            let Some(PreparedAssistantRequest::Local(request)) = panel.prepared.as_ref() else {
                panic!("updated review");
            };
            assert_eq!(request.config().limits().timeout(), Duration::from_secs(27));
            let preview: serde_json::Value = serde_json::from_str(request.preview_json())
                .unwrap_or_else(|_| panic!("valid review"));
            assert_eq!(preview["timeout_ms"], 27000);
            assert_eq!(preview["answer_bytes"], 3 * 1024);
            assert_eq!(preview["combined_output_bytes"], 19 * 1024);
        }
        panel.set_profile(Some(profile("API remains independent")), None, cx);
        panel.prepare(cx);
        let Some(PreparedAssistantRequest::Api(request)) = panel.prepared.as_ref() else {
            panic!("API request");
        };
        let preview: serde_json::Value =
            serde_json::from_str(request.preview_json()).unwrap_or_else(|_| panic!("API JSON"));
        for field in [
            "timeout_ms",
            "answer_bytes",
            "combined_output_bytes",
            "limits",
        ] {
            assert!(
                preview.get(field).is_none(),
                "local budget must not alter API payload"
            );
        }
    });
}

// Keep the independent five-path counterexample byte-for-byte for regression replay.
include!("assistant_tests/inactive_basic_probe.rs");

#[path = "assistant_tests/retained_drafts.rs"]
mod retained_drafts;

#[path = "assistant_tests/local_progress.rs"]
mod local_progress;

#[path = "assistant_tests/redaction_boundaries.rs"]
mod redaction_boundaries;

#[path = "assistant_tests/command_review_target.rs"]
mod command_review_target;

#[gpui_kit::test]
fn inactive_proxy_basic_reply_is_redacted_and_late_reply_after_clear_is_discarded(
    cx: &mut TestAppContext,
) {
    use crate::ai_request_options::{RequestSecret, SecretPurpose};
    use keelshell_core::{AiProxy, AiSecretRef};
    let (_, panel) = mount(cx);
    panel.update(cx, |panel, cx| {
        let active = profile("Active");
        let mut inactive = profile("Inactive");
        let reference = AiSecretRef::Ephemeral {
            id: uuid::Uuid::new_v4(),
        };
        inactive.proxy = AiProxy::Explicit {
            url: "http://127.0.0.1:9".into(),
            credentials: Some(reference.clone()),
        };
        let mut credentials = EphemeralCredentials::new();
        credentials.insert_request(
            &inactive,
            SecretPurpose::Proxy,
            reference,
            RequestSecret::Proxy {
                username: Zeroizing::new("synthetic-user".into()),
                password: Zeroizing::new("synthetic-pass".into()),
            },
        );
        let catalog = AiProfileCatalog {
            active_id: Some(active.id),
            profiles: vec![active, inactive.clone()],
        };
        panel.set_profiles(&catalog, &credentials, cx);
        let basic = "c3ludGhldGljLXVzZXI6c3ludGhldGljLXBhc3M=";
        panel.prepare(cx);
        assert!(panel.prepared.is_some());
        let revision = panel.request_revision;
        panel.finish_reply(
            revision,
            ("ops@server.example:22".into(), "session-A".into()),
            Ok(format!(
                "ordinary reply {basic}; Basic {basic}\n```shell\nprintf '{basic}'\n```"
            )),
            cx,
        );
        assert!(panel.response.contains("ordinary reply"));
        assert!(!panel.response.contains(basic));
        assert!(
            panel
                .suggestions
                .iter()
                .all(|command| !command.contains(basic))
        );
        let cancellation = RequestCancellation::new();
        panel.cancellation = Some(cancellation.clone());
        panel.busy = true;
        credentials.clear_requests(inactive.id);
        panel.set_profiles(&catalog, &credentials, cx);
        assert!(cancellation.is_cancelled());
        assert!(panel.prepared.is_none());
        assert!(panel.response.is_empty());
        assert!(panel.credentials.all_secrets().is_empty());
        panel.finish_reply(
            revision,
            ("host".into(), "session-A".into()),
            Ok(format!("late {basic}")),
            cx,
        );
        assert!(panel.response.is_empty());
        assert!(panel.response_target.is_none());
        assert!(!panel.busy);
        assert!(panel._job.is_none());
    });
}

#[gpui_kit::test]
#[allow(clippy::expect_used)]
fn local_environment_key_requires_explicit_binding_and_freezes_review_snapshot(
    cx: &mut TestAppContext,
) {
    let (_, panel) = mount(cx);
    for agent in [
        keelshell_core::AiLocalAgent::Codex,
        keelshell_core::AiLocalAgent::ClaudeCode,
    ] {
        panel.update(cx, |panel, cx| {
            let mut profile = local_profile(agent);
            let source = keelshell_core::AiSecretRef::Environment {
                name: "KEELSHELL_IMPORT_ONLY_KEY".into(),
            };
            match &mut profile.authentication {
                AiAuthentication::Bearer { credential }
                | AiAuthentication::Header { credential, .. } => *credential = Some(source),
                AiAuthentication::None => panic!("fixed key auth"),
            }
            let catalog = AiProfileCatalog {
                active_id: Some(profile.id),
                profiles: vec![profile.clone()],
            };
            let mut credentials = EphemeralCredentials::new();
            // A generic manually assigned key cannot impersonate a successful
            // environment import. No environment lookup is attempted here.
            credentials.insert(profile.id, Zeroizing::new("unbound-manual-key".into()));
            panel.set_profiles(&catalog, &credentials, cx);
            panel.prepare(cx);
            assert!(panel.prepared.is_none());
            credentials
                .retain_local_environment(profile.id, Zeroizing::new("frozen-import-key".into()));
            credentials
                .bind_local_environment(&profile, Zeroizing::new("frozen-import-key".into()));
            panel.set_profiles(&catalog, &credentials, cx);
            panel.prepare(cx);
            let request = panel
                .prepared
                .as_ref()
                .expect("prepared with explicit import");
            assert!(request.preview_json().contains("KEELSHELL_IMPORT_ONLY_KEY"));
            assert!(!request.preview_json().contains("frozen-import-key"));
            assert_eq!(
                panel.prepared_key.as_ref().expect("snapshot").as_str(),
                "frozen-import-key"
            );
            // The caller's later cache edit cannot mutate an already reviewed
            // request. Applying that edit revokes the old review and operation.
            credentials
                .bind_local_environment(&profile, Zeroizing::new("rotated-import-key".into()));
            assert_eq!(
                panel.prepared_key.as_ref().expect("still frozen").as_str(),
                "frozen-import-key"
            );
            let cancellation = RequestCancellation::new();
            panel.cancellation = Some(cancellation.clone());
            panel.set_profiles(&catalog, &credentials, cx);
            assert!(cancellation.is_cancelled());
            assert!(panel.prepared.is_none());
            let mut changed = profile.clone();
            changed.endpoint = "https://changed.example/v1".into();
            let changed_catalog = AiProfileCatalog {
                active_id: Some(changed.id),
                profiles: vec![changed],
            };
            panel.set_profiles(&changed_catalog, &credentials, cx);
            panel.prepare(cx);
            assert!(
                panel.prepared.is_none(),
                "old import is not admitted for new receiver"
            );
            assert!(!panel.busy);
            assert!(panel._job.is_none());
        });
    }
}
