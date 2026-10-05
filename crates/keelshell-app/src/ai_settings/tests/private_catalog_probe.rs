#[gpui_kit::test]
async fn reviewer_b_inactive_basic_catalog_must_not_become_test_request_model(
    cx: &mut TestAppContext,
) {
    use crate::ai_request_options::{RequestSecret, SecretPurpose};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use keelshell_core::AiProxy;
    use zeroize::Zeroizing;
    let bare = STANDARD.encode("reviewer-inactive-user:reviewer-inactive-pass");
    let listener =
        TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("owned listener: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("listener mode: {error}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("listener address: {error}"));
    let mut configuration = fixture_profile();
    configuration.endpoint = format!("http://{address}/v1/chat/completions");
    // Window/runtime setup must not consume the server's three-second accept budget.
    let (handle, panel) = mount_sized(cx, configuration, 1000., 900.);
    let server_bare = bare.clone();
    let mut server = CatalogProbeServer::start(listener, server_bare);
    panel.update(cx, |panel, cx| {
        let mut inactive = fixture_profile();
        inactive.name = "Inactive proxy fixture".into();
        let reference = AiSecretRef::Ephemeral {
            id: uuid::Uuid::new_v4(),
        };
        inactive.proxy = AiProxy::Explicit {
            url: "http://127.0.0.1:9".into(),
            credentials: Some(reference.clone()),
        };
        panel.catalog.profiles.push(inactive.clone());
        panel.credentials.insert_request(
            &inactive,
            SecretPurpose::Proxy,
            reference,
            RequestSecret::Proxy {
                username: Zeroizing::new("reviewer-inactive-user".into()),
                password: Zeroizing::new("reviewer-inactive-pass".into()),
            },
        );
        assert!(panel.credentials.all_secrets().contains(&bare.as_str()));
        panel.start_operation(OperationKind::Models, cx);
    });
    cx.wait_for(handle, Duration::from_secs(5), |_, cx| {
        request_has_finished(&panel, cx)
    })
    .await;
    let catalog_admitted_secret = panel.read_with(cx, |p, _| p.models.iter().any(|m| m == &bare));
    // Use the discovered value through the same InputState/sync path as the model button.
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            if let Some(discovered) = panel.models.first().cloned() {
                panel
                    .model
                    .update(cx, |field, cx| field.set_value(discovered, window, cx));
                panel.sync_editor(cx);
                panel.start_operation(OperationKind::Test, cx);
            }
        });
    })
    .unwrap_or_else(|error| panic!("owned model selection: {error}"));
    cx.wait_for(handle, Duration::from_secs(5), |_, cx| {
        request_has_finished(&panel, cx)
    })
    .await;
    let (requests, posted_model_secret) = server
        .finish()
        .unwrap_or_else(|error| panic!("owned catalog observation: {error}"));
    eprintln!(
        "reviewer inactive Basic catalog admitted={catalog_admitted_secret}; HTTP requests={requests}; posted model contains known secret={posted_model_secret}; owned server joined=true"
    );
    assert_eq!(
        requests, 1,
        "discovery must reach the fixture exactly once without a connectivity POST"
    );
    assert!(
        !catalog_admitted_secret && !posted_model_secret,
        "known inactive credential must not enter discovered catalog or another provider's connectivity payload"
    );
}

/// Test-only ownership: cancellation also closes the current accepted socket.
/// Drop joins during unwinding, so a failed GPUI assertion cannot detach a worker.
struct CatalogProbeServer {
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    active: Arc<std::sync::Mutex<Option<TcpStream>>>,
    observed: std::sync::mpsc::Receiver<std::io::Result<(usize, bool)>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl CatalogProbeServer {
    fn start(listener: TcpListener, bare: String) -> Self {
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let active = Arc::new(std::sync::Mutex::new(None));
        let worker_cancelled = Arc::clone(&cancelled);
        let worker_active = Arc::clone(&active);
        let (observed_tx, observed) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = catalog_probe_serve(listener, &bare, &worker_cancelled, &worker_active);
            // The receiver may disappear when the test unwinds; cleanup still finishes.
            let _ = observed_tx.send(result);
        });
        Self {
            cancelled,
            active,
            observed,
            worker: Some(worker),
        }
    }

    fn finish(&mut self) -> std::io::Result<(usize, bool)> {
        let result = self.observed.recv_timeout(Duration::from_secs(5));
        self.stop_and_join()?;
        result.map_err(|error| std::io::Error::other(format!("bounded observation: {error}")))?
    }

    fn stop_and_join(&mut self) -> std::io::Result<()> {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        // Registration and cancellation use the same short lock; there is no
        // gap in which an accepted socket can escape cancellation.
        if let Some(stream) = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| std::io::Error::other("owned catalog worker panicked"))?;
        }
        Ok(())
    }
}

impl Drop for CatalogProbeServer {
    fn drop(&mut self) {
        // Never panic a second time while unwinding a UI or fixture assertion.
        let _ = self.stop_and_join();
    }
}

fn catalog_probe_serve(
    listener: TcpListener,
    bare: &str,
    cancelled: &std::sync::atomic::AtomicBool,
    active: &std::sync::Mutex<Option<TcpStream>>,
) -> std::io::Result<(usize, bool)> {
    let mut requests = 0;
    let mut posted_model_secret = false;
    for step in 0..2 {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut stream = loop {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return Ok((requests, posted_model_secret));
            }
            match listener.accept() {
                Ok((stream, _)) => break Some(stream),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break None,
                Err(error) => return Err(error),
            }
        };
        let Some(mut stream) = stream.take() else {
            break;
        };
        // Restore blocking mode on platforms where accept inherits it.
        configure_http_stream(&stream);
        {
            let mut current = active.lock().unwrap_or_else(|error| error.into_inner());
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return Ok((requests, posted_model_secret));
            }
            *current = Some(stream.try_clone()?);
        }
        // One total header+body deadline; each partial read uses only its remainder.
        let read_deadline = Instant::now() + Duration::from_secs(3);
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            catalog_probe_read_exact(&mut stream, &mut byte, read_deadline)?;
            headers.push(byte[0]);
            if headers.len() > 16384 {
                return Err(std::io::Error::other("request header bound"));
            }
        }
        let head =
            String::from_utf8(headers).map_err(|_| std::io::Error::other("HTTP headers UTF8"))?;
        let length = head
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(str::trim)
                    .and_then(|s| s.parse::<usize>().ok())
            })
            .unwrap_or(0);
        if length > 16384 {
            return Err(std::io::Error::other("request body bound"));
        }
        let mut body = vec![0; length];
        catalog_probe_read_exact(&mut stream, &mut body, read_deadline)?;
        requests += 1;
        let answer = if step == 0 {
            assert!(head.starts_with("GET "));
            serde_json::json!({"data":[{"id":bare}]}).to_string()
        } else {
            assert!(head.starts_with("POST "));
            let value: serde_json::Value =
                serde_json::from_slice(&body).map_err(|_| std::io::Error::other("JSON request"))?;
            posted_model_secret = value.get("model").and_then(|v| v.as_str()) == Some(bare);
            serde_json::json!({"model":"ordinary-model","choices":[{"message":{"role":"assistant","content":"OK"}}]}).to_string()
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
            answer.len()
        );
        let mut remaining = response.as_bytes();
        let write_deadline = Instant::now() + Duration::from_secs(3);
        while !remaining.is_empty() {
            stream.set_write_timeout(Some(catalog_probe_remaining(write_deadline)?))?;
            match stream.write(remaining) {
                Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
                Ok(written) => remaining = &remaining[written..],
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        active
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
    Ok((requests, posted_model_secret))
}

fn catalog_probe_remaining(deadline: Instant) -> std::io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|time| !time.is_zero())
        .ok_or_else(|| std::io::ErrorKind::TimedOut.into())
}

fn catalog_probe_read_exact(
    stream: &mut TcpStream,
    mut output: &mut [u8],
    deadline: Instant,
) -> std::io::Result<()> {
    while !output.is_empty() {
        stream.set_read_timeout(Some(catalog_probe_remaining(deadline)?))?;
        match stream.read(output) {
            Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            Ok(read) => output = &mut output[read..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[test]
fn catalog_probe_partial_reads_cannot_extend_the_total_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    let mut client = TcpStream::connect(
        listener
            .local_addr()
            .unwrap_or_else(|error| panic!("catalog fixture: {error:?}")),
    )
    .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    client
        .set_write_timeout(Some(Duration::from_millis(100)))
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    let (mut stream, _) = listener
        .accept()
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    stream
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    configure_http_stream(&stream);
    let writer = std::thread::spawn(move || {
        for _ in 0..6 {
            if client.write_all(b"x").is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    });
    let result = catalog_probe_read_exact(
        &mut stream,
        &mut [0; 6],
        Instant::now() + Duration::from_millis(120),
    );
    let _ = stream.shutdown(std::net::Shutdown::Both);
    writer
        .join()
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    assert!(matches!(
        result
            .err()
            .unwrap_or_else(|| panic!("drip must exceed the total deadline"))
            .kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ));
}

#[test]
fn catalog_probe_unwind_cancels_accept_and_joins_worker() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    let server = CatalogProbeServer::start(listener, "fixture-model".into());
    let weak_cancelled = Arc::downgrade(&server.cancelled);
    let weak_active = Arc::downgrade(&server.active);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _owned = server;
        panic!("controlled UI failure");
    }));
    assert!(result.is_err());
    assert!(
        weak_cancelled.upgrade().is_none() && weak_active.upgrade().is_none(),
        "guard and worker must release both owners before unwind returns"
    );
}

#[test]
fn catalog_probe_drop_closes_active_stream_and_joins_worker() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    let server = CatalogProbeServer::start(listener, "fixture-model".into());
    let mut client =
        TcpStream::connect(address).unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"));
    let deadline = Instant::now() + Duration::from_secs(3);
    while server
        .active
        .lock()
        .unwrap_or_else(|error| panic!("catalog fixture: {error:?}"))
        .is_none()
    {
        assert!(Instant::now() < deadline, "accepted socket registration");
        std::thread::sleep(Duration::from_millis(5));
    }
    let weak_cancelled = Arc::downgrade(&server.cancelled);
    let weak_active = Arc::downgrade(&server.active);
    drop(server);
    assert!(weak_cancelled.upgrade().is_none() && weak_active.upgrade().is_none());
    match client.read(&mut [0]) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ) => {}
        other => panic!("cancelled peer must be closed: {other:?}"),
    }
}
