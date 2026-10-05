#[gpui_kit::test]
async fn reviewer_b_inactive_basic_catalog_must_not_become_test_request_model(cx: &mut TestAppContext) {
    use crate::ai_request_options::{RequestSecret, SecretPurpose};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use keelshell_core::AiProxy;
    use zeroize::Zeroizing;
    let bare = STANDARD.encode("reviewer-inactive-user:reviewer-inactive-pass");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| panic!("owned listener: {error}"));
    listener.set_nonblocking(true).unwrap_or_else(|error| panic!("listener mode: {error}"));
    let address = listener.local_addr().unwrap_or_else(|error| panic!("listener address: {error}"));
    let server_bare = bare.clone();
    let (observed_tx, observed_rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut requests = 0; let mut posted_model_secret = false;
        for step in 0..2 {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break Some(stream),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break None,
                    Err(error) => panic!("owned accept: {error}"),
                }
            };
            let Some(mut stream) = stream.take() else { break; };
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap_or_else(|error| panic!("read limit: {error}"));
            stream.set_write_timeout(Some(Duration::from_secs(3))).unwrap_or_else(|error| panic!("write limit: {error}"));
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") { let mut byte=[0];stream.read_exact(&mut byte).unwrap_or_else(|error| panic!("bounded headers: {error}"));headers.push(byte[0]);assert!(headers.len()<=16384); }
            let head = String::from_utf8(headers).unwrap_or_else(|_| panic!("HTTP headers UTF8"));
            let length = head.lines().find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(str::trim).and_then(|s| s.parse::<usize>().ok())).unwrap_or(0);
            assert!(length <= 16384); let mut body = vec![0;length];stream.read_exact(&mut body).unwrap_or_else(|error| panic!("bounded body: {error}"));
            requests += 1;
            let answer = if step == 0 {
                assert!(head.starts_with("GET "));
                serde_json::json!({"data":[{"id":server_bare}]}).to_string()
            } else {
                assert!(head.starts_with("POST "));
                let value: serde_json::Value = serde_json::from_slice(&body).unwrap_or_else(|_| panic!("JSON request"));
                posted_model_secret = value.get("model").and_then(|v| v.as_str()) == Some(server_bare.as_str());
                serde_json::json!({"model":"ordinary-model","choices":[{"message":{"role":"assistant","content":"OK"}}]}).to_string()
            };
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",answer.len()).unwrap_or_else(|error| panic!("owned response: {error}"));
        }
        observed_tx.send((requests, posted_model_secret)).unwrap_or_else(|_| panic!("observation send"));
    });
    let mut configuration = fixture_profile();
    configuration.endpoint = format!("http://{address}/v1/chat/completions");
    let (handle, panel) = mount_sized(cx, configuration, 1000., 900.);
    panel.update(cx, |panel,cx| {
        let mut inactive = fixture_profile(); inactive.name = "Inactive proxy fixture".into();
        let reference = AiSecretRef::Ephemeral { id: uuid::Uuid::new_v4() };
        inactive.proxy = AiProxy::Explicit { url:"http://127.0.0.1:9".into(),credentials:Some(reference.clone()) };
        panel.catalog.profiles.push(inactive.clone());
        panel.credentials.insert_request(&inactive,SecretPurpose::Proxy,reference,RequestSecret::Proxy {username:Zeroizing::new("reviewer-inactive-user".into()),password:Zeroizing::new("reviewer-inactive-pass".into())});
        assert!(panel.credentials.all_secrets().contains(&bare.as_str()));
        panel.start_operation(OperationKind::Models,cx);
    });
    cx.wait_for(handle,Duration::from_secs(5),|_,cx| request_has_finished(&panel,cx)).await;
    let catalog_admitted_secret = panel.read_with(cx,|p,_| p.models.iter().any(|m|m==&bare));
    // Use the discovered value through the same InputState/sync path as the model button.
    cx.update_window(handle,|_,window,cx| {
        panel.update(cx,|panel,cx| {
            if let Some(discovered) = panel.models.first().cloned() {
                panel.model.update(cx,|field,cx|field.set_value(discovered,window,cx));
                panel.sync_editor(cx);
                panel.start_operation(OperationKind::Test,cx);
            }
        });
    }).unwrap_or_else(|error|panic!("owned model selection: {error}"));
    cx.wait_for(handle,Duration::from_secs(5),|_,cx| request_has_finished(&panel,cx)).await;
    let (requests,posted_model_secret) = observed_rx.recv_timeout(Duration::from_secs(5)).unwrap_or_else(|_|panic!("bounded observation"));
    server.join().unwrap_or_else(|_|panic!("owned server join"));
    eprintln!("reviewer inactive Basic catalog admitted={catalog_admitted_secret}; HTTP requests={requests}; posted model contains known secret={posted_model_secret}; owned server joined=true");
    assert!(!catalog_admitted_secret && !posted_model_secret,"known inactive credential must not enter discovered catalog or another provider's connectivity payload");
}
