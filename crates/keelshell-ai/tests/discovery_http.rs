//! Real loopback HTTP tests. No cloud accounts, user context, or API keys are used.

use std::{
    error::Error,
    io,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use keelshell_ai::{
    AiError, AiErrorCategory, CONNECTIVITY_PROMPT, ContextDraft, ProviderClient, ProviderConfig,
    ProviderEndpoint, RequestCancellation,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
    time::{sleep, timeout},
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const FIXTURE_LIMIT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy)]
enum Framing {
    Length,
    Eof,
    Chunked,
}

struct Reply {
    status: u16,
    body: String,
    framing: Framing,
    header_delay: Duration,
    body_delay: Duration,
    location: Option<String>,
}

impl Reply {
    fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            framing: Framing::Length,
            header_delay: Duration::ZERO,
            body_delay: Duration::ZERO,
            location: None,
        }
    }
}

struct Recorded {
    headers: String,
    body: String,
}

struct Fixture {
    endpoint: ProviderEndpoint,
    recorded: oneshot::Receiver<Recorded>,
    headers_sent: oneshot::Receiver<()>,
    task: JoinHandle<io::Result<()>>,
}

impl Fixture {
    async fn start(reply: Reply) -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = ProviderEndpoint::new(&format!(
            "http://{}/tenant/v1/chat/completions",
            listener.local_addr()?
        ))?;
        let (send, recorded) = oneshot::channel();
        let (sent, headers_sent) = oneshot::channel();
        let task = tokio::spawn(async move {
            timeout(FIXTURE_LIMIT, async move {
                let (mut stream, _) = listener.accept().await?;
                let request = read_request(&mut stream).await?;
                let _ = send.send(request);
                sleep(reply.header_delay).await;
                let framing = match reply.framing {
                    Framing::Length => format!("Content-Length: {}\r\n", reply.body.len()),
                    Framing::Eof => String::new(),
                    Framing::Chunked => "Transfer-Encoding: chunked\r\n".into(),
                };
                let location = reply.location.map_or(String::new(), |url| format!("Location: {url}\r\n"));
                let headers = format!("HTTP/1.1 {} Fixture\r\nContent-Type: application/json\r\n{framing}{location}Connection: close\r\n\r\n", reply.status);
                // Timed-out/cancelled clients may close before either write.
                if stream.write_all(headers.as_bytes()).await.is_err() { return Ok(()); }
                let _ = sent.send(());
                sleep(reply.body_delay).await;
                let body = match reply.framing {
                    Framing::Chunked => format!("{:x}\r\n{}\r\n0\r\n\r\n", reply.body.len(), reply.body),
                    _ => reply.body,
                };
                let _ = stream.write_all(body.as_bytes()).await;
                Ok(())
            }).await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "HTTP fixture deadline"))?
        });
        Ok(Self {
            endpoint,
            recorded,
            headers_sent,
            task,
        })
    }

    async fn observed(&mut self) -> TestResult<Recorded> {
        Ok(timeout(FIXTURE_LIMIT, &mut self.recorded).await??)
    }

    async fn headers_sent(&mut self) -> TestResult {
        timeout(FIXTURE_LIMIT, &mut self.headers_sent).await??;
        Ok(())
    }

    async fn finish(self) -> TestResult {
        timeout(FIXTURE_LIMIT, self.task).await???;
        Ok(())
    }
}

async fn read_request(stream: &mut TcpStream) -> io::Result<Recorded> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(offset) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            break offset + 4;
        }
        let mut chunk = [0; 1024];
        let count = stream.read(&mut chunk).await?;
        if count == 0 || bytes.len() > 1024 * 1024 {
            return Err(io::Error::other("invalid request headers"));
        }
        bytes.extend_from_slice(&chunk[..count]);
    };
    let headers = String::from_utf8(bytes[..header_end].to_vec()).map_err(io::Error::other)?;
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
        .map(str::parse::<usize>)
        .transpose()
        .map_err(io::Error::other)?
        .unwrap_or(0);
    if length > 1024 * 1024 {
        return Err(io::Error::other("request body exceeds fixture limit"));
    }
    while bytes.len() - header_end < length {
        let mut chunk = [0; 1024];
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Err(io::Error::other("truncated request body"));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    let body = String::from_utf8(bytes[header_end..header_end + length].to_vec())
        .map_err(io::Error::other)?;
    Ok(Recorded { headers, body })
}

fn client(limit: usize) -> Result<ProviderClient, AiError> {
    ProviderClient::new(Duration::from_secs(2), limit)
}

fn answer(model: Option<&str>) -> String {
    let mut json = serde_json::json!({"choices": [{"message": {"content": "OK"}}]});
    if let Some(model) = model {
        json["model"] = model.into();
    }
    json.to_string()
}

#[tokio::test]
async fn discovery_get_preserves_prefix_and_sends_only_explicit_authentication() -> TestResult {
    let mut fixture = Fixture::start(Reply::json(
        r#"{"data":[{"id":"z-model"},{"id":"模型:7b"},{"id":"a-model"},{"id":"z-model"}]}"#,
    ))
    .await?;
    let catalog = client(4096)?
        .discover_models(
            &fixture.endpoint,
            Some("fixture-only-key"),
            &RequestCancellation::new(),
        )
        .await?;
    assert_eq!(catalog.models(), &["a-model", "z-model", "模型:7b"]);
    let request = fixture.observed().await?;
    assert!(
        request
            .headers
            .starts_with("GET /tenant/v1/models HTTP/1.1\r\n")
    );
    assert!(
        request
            .headers
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-only-key\r\n")
    );
    assert!(request.body.is_empty());
    fixture.finish().await
}

#[tokio::test]
async fn fixed_probe_reports_actual_model_without_retaining_provider_text() -> TestResult {
    for actual_model in [None, Some("actual-provider-model")] {
        let mut fixture = Fixture::start(Reply::json(answer(actual_model))).await?;
        let provider = ProviderConfig::new(fixture.endpoint.as_str(), "requested-model")?;
        let report = client(4096)?
            .test_connection(&provider, None, &RequestCancellation::new())
            .await?;
        assert_eq!(report.actual_model(), actual_model);
        assert!(report.elapsed() < Duration::from_secs(2));
        let request = fixture.observed().await?;
        assert!(
            request
                .headers
                .starts_with("POST /tenant/v1/chat/completions HTTP/1.1\r\n")
        );
        assert!(
            !request
                .headers
                .to_ascii_lowercase()
                .contains("authorization:")
        );
        let json: serde_json::Value = serde_json::from_str(&request.body)?;
        assert_eq!(
            json,
            serde_json::json!({
                "model": "requested-model", "stream": false,
                "messages": [{"role": "user", "content": CONNECTIVITY_PROMPT}]
            })
        );
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn async_approved_request_keeps_original_destination_model_and_exact_body() -> TestResult {
    let mut fixture = Fixture::start(Reply::json(answer(Some("actual-model")))).await?;
    let mut configuration = ProviderConfig::new(fixture.endpoint.as_str(), "reviewed-model")?;
    let request = ContextDraft::new("Explain the selected failure")
        .with_host_label("reviewed-host")
        .add_selection("explicit selection", "connection refused")
        .prepare(&configuration, &[], 4096)?;
    let preview = request.preview_json().to_owned();
    configuration = ProviderConfig::new("http://127.0.0.1:9/unreviewed", "different-model")?;
    assert_ne!(configuration.endpoint(), request.provider().endpoint());
    let reply = client(4096)?
        .send_approved(
            request.approve(),
            Some("transport-key"),
            &RequestCancellation::new(),
        )
        .await?;
    assert_eq!(reply.text(), "OK");
    let recorded = fixture.observed().await?;
    assert_eq!(recorded.body, preview);
    assert!(recorded.body.contains("reviewed-model"));
    assert!(!recorded.body.contains("different-model"));
    fixture.finish().await
}

#[tokio::test]
async fn status_categories_hide_sensitive_error_bodies_and_do_not_retry() -> TestResult {
    for (status, category) in [
        (401, AiErrorCategory::Authentication),
        (403, AiErrorCategory::PermissionDenied),
        (404, AiErrorCategory::UnsupportedEndpoint),
        (405, AiErrorCategory::UnsupportedEndpoint),
        (429, AiErrorCategory::RateLimited),
        (503, AiErrorCategory::ProviderUnavailable),
        (422, AiErrorCategory::RequestRejected),
    ] {
        let mut reply = Reply::json("echoed fixture-private-key and private body");
        reply.status = status;
        let mut fixture = Fixture::start(reply).await?;
        let result = client(4096)?
            .discover_models(
                &fixture.endpoint,
                Some("fixture-private-key"),
                &RequestCancellation::new(),
            )
            .await;
        let error = match result {
            Err(error) => error,
            Ok(_) => return Err("unsuccessful HTTP status was accepted".into()),
        };
        assert_eq!(error, AiError::HttpStatus(status));
        assert_eq!(error.category(), category);
        assert!(!format!("{error:?} {error}").contains("fixture-private-key"));
        fixture.observed().await?;
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn cross_origin_redirect_is_refused_before_credentials_reach_target() -> TestResult {
    let trap = TcpListener::bind("127.0.0.1:0").await?;
    let mut reply = Reply::json("");
    reply.status = 307;
    reply.location = Some(format!("http://{}/secret-target", trap.local_addr()?));
    let fixture = Fixture::start(reply).await?;
    let result = client(4096)?
        .discover_models(
            &fixture.endpoint,
            Some("not-for-redirect"),
            &RequestCancellation::new(),
        )
        .await;
    assert_eq!(result, Err(AiError::HttpStatus(307)));
    assert!(
        timeout(Duration::from_millis(100), trap.accept())
            .await
            .is_err()
    );
    fixture.finish().await
}

#[tokio::test]
async fn catalog_byte_limit_applies_to_declared_eof_and_chunked_bodies() -> TestResult {
    for framing in [Framing::Length, Framing::Eof, Framing::Chunked] {
        let mut reply = Reply::json("x".repeat(8192));
        reply.framing = framing;
        let fixture = Fixture::start(reply).await?;
        assert_eq!(
            client(1024)?
                .discover_models(&fixture.endpoint, None, &RequestCancellation::new())
                .await,
            Err(AiError::ResponseTooLarge)
        );
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn malformed_catalogs_and_reflected_keys_are_never_presented_as_models() -> TestResult {
    let oversized = serde_json::json!({"data": (0..4097).map(|n| serde_json::json!({"id":format!("model-{n}")})).collect::<Vec<_>>()}).to_string();
    for body in [
        "not json".to_owned(),
        r#"{"data":null}"#.into(),
        r#"{"data":[{}]}"#.into(),
        r#"{"data":[{"id":"bad model"}]}"#.into(),
        r#"{"data":[{"id":"\u001b[2J"}]}"#.into(),
        r#"{"data":[{"id":"echo-private-key"}]}"#.into(),
        oversized,
    ] {
        let fixture = Fixture::start(Reply::json(body)).await?;
        let result = client(256 * 1024)?
            .discover_models(
                &fixture.endpoint,
                Some("private-key"),
                &RequestCancellation::new(),
            )
            .await;
        assert_eq!(result, Err(AiError::InvalidModelCatalog));
        assert!(!format!("{result:?}").contains("private-key"));
        fixture.finish().await?;
    }
    let fixture = Fixture::start(Reply::json(r#"{"data":[]}"#)).await?;
    assert!(
        client(4096)?
            .discover_models(&fixture.endpoint, None, &RequestCancellation::new())
            .await?
            .models()
            .is_empty()
    );
    fixture.finish().await
}

#[tokio::test]
async fn deadline_covers_both_response_headers_and_body() -> TestResult {
    for delay_headers in [true, false] {
        let mut reply = Reply::json(r#"{"data":[]}"#);
        if delay_headers {
            reply.header_delay = Duration::from_millis(180);
        } else {
            reply.body_delay = Duration::from_millis(180);
        }
        let fixture = Fixture::start(reply).await?;
        let client = ProviderClient::new(Duration::from_millis(60), 4096)?;
        let started = Instant::now();
        assert_eq!(
            client
                .discover_models(&fixture.endpoint, None, &RequestCancellation::new())
                .await,
            Err(AiError::Timeout)
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn cancellation_interrupts_header_and_body_waits_without_waiting_for_deadline() -> TestResult
{
    for delay_headers in [true, false] {
        let mut reply = Reply::json(r#"{"data":[]}"#);
        if delay_headers {
            reply.header_delay = Duration::from_millis(400);
        } else {
            reply.body_delay = Duration::from_millis(400);
        }
        let mut fixture = Fixture::start(reply).await?;
        let endpoint = fixture.endpoint.clone();
        let client = client(4096)?;
        let cancel = RequestCancellation::new();
        let operation = client.discover_models(&endpoint, None, &cancel);
        tokio::pin!(operation);
        tokio::select! {
            _ = &mut operation => return Err("request completed before cancellation point".into()),
            observed = fixture.observed() => { observed?; },
        }
        if !delay_headers {
            tokio::select! {
                _ = &mut operation => return Err("request completed before headers were written".into()),
                sent = fixture.headers_sent() => { sent?; },
            }
        }
        cancel.clone().cancel();
        assert!(cancel.is_cancelled());
        assert_eq!(
            timeout(Duration::from_millis(150), operation).await?,
            Err(AiError::Cancelled)
        );
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn cancelled_or_invalid_operations_do_not_open_any_socket() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let endpoint = ProviderEndpoint::new(&format!("{base}/v1/chat/completions"))?;
    let client = client(4096)?;
    let cancel = RequestCancellation::new();
    cancel.cancel();
    assert_eq!(
        client.discover_models(&endpoint, None, &cancel).await,
        Err(AiError::Cancelled)
    );
    let fresh = RequestCancellation::new();
    let unknown = ProviderEndpoint::new(&format!("{base}/unknown"))?;
    assert_eq!(
        client.discover_models(&unknown, None, &fresh).await,
        Err(AiError::UnsupportedDiscoveryEndpoint)
    );
    assert_eq!(
        client
            .discover_models(&endpoint, Some("bad\nkey"), &fresh)
            .await,
        Err(AiError::InvalidApiKey)
    );
    let provider = ProviderConfig::new(endpoint.as_str(), "fixture")?;
    let request = ContextDraft::new("contains transport-key").prepare(&provider, &[], 1024)?;
    assert!(matches!(
        client
            .send_approved(request.approve(), Some("transport-key"), &fresh)
            .await,
        Err(AiError::CredentialInContext)
    ));
    assert!(
        timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn probe_and_approved_post_are_cancellable_after_server_receives_the_body() -> TestResult {
    for probe in [true, false] {
        let mut reply = Reply::json(answer(None));
        reply.body_delay = Duration::from_millis(250);
        let mut fixture = Fixture::start(reply).await?;
        let provider = ProviderConfig::new(fixture.endpoint.as_str(), "model")?;
        let client = client(4096)?;
        let cancel = RequestCancellation::new();
        let operation = async {
            if probe {
                client
                    .test_connection(&provider, None, &cancel)
                    .await
                    .map(|_| ())
            } else {
                let request =
                    ContextDraft::new("explicit question").prepare(&provider, &[], 1024)?;
                client
                    .send_approved(request.approve(), None, &cancel)
                    .await
                    .map(|_| ())
            }
        };
        tokio::pin!(operation);
        let request = tokio::select! {
            _ = &mut operation => return Err("POST completed before cancellation point".into()),
            request = fixture.observed() => request?,
        };
        assert!(request.headers.starts_with("POST "));
        assert_eq!(request.body.contains("explicit question"), !probe);
        cancel.cancel();
        assert_eq!(
            timeout(Duration::from_millis(150), operation).await?,
            Err(AiError::Cancelled)
        );
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn probe_rejects_missing_text_or_untrustworthy_returned_model_metadata() -> TestResult {
    for (body, expected) in [
        ("not json".to_owned(), AiError::InvalidResponse),
        (r#"{"choices":[]}"#.into(), AiError::EmptyReply),
        (
            r#"{"choices":[{"message":{"content":" "}}]}"#.into(),
            AiError::EmptyReply,
        ),
        (answer(Some("bad\nmodel")), AiError::InvalidResponse),
        (answer(Some("echo-fixture-key")), AiError::InvalidResponse),
    ] {
        let fixture = Fixture::start(Reply::json(body)).await?;
        let provider = ProviderConfig::new(fixture.endpoint.as_str(), "model")?;
        let result = client(4096)?
            .test_connection(&provider, Some("fixture-key"), &RequestCancellation::new())
            .await;
        assert_eq!(result, Err(expected));
        assert!(!format!("{result:?}").contains("fixture-key"));
        fixture.finish().await?;
    }
    Ok(())
}

#[tokio::test]
async fn async_answer_redacts_reflected_transport_credentials() -> TestResult {
    let body =
        serde_json::json!({"choices":[{"message":{"content":"echo fixture-key"}}]}).to_string();
    let fixture = Fixture::start(Reply::json(body)).await?;
    let provider = ProviderConfig::new(fixture.endpoint.as_str(), "model")?;
    let request = ContextDraft::new("question").prepare(&provider, &[], 1024)?;
    let reply = client(4096)?
        .send_approved(
            request.approve(),
            Some("fixture-key"),
            &RequestCancellation::new(),
        )
        .await?;
    assert!(!reply.text().contains("fixture-key"));
    fixture.finish().await
}

// Isolate environment changes in a subprocess. Rust 2024 process-wide set_var
// is unsafe in a parallel test executable and is intentionally not used.
#[test]
fn environment_proxies_are_ignored_in_an_isolated_process() -> TestResult {
    if std::env::var_os("KEELSHELL_AI_PROXY_FIXTURE").is_some() {
        return Ok(());
    }
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--exact", "environment_proxy_child", "--nocapture"])
        .env("KEELSHELL_AI_PROXY_FIXTURE", "1")
        .env("HTTP_PROXY", "http://127.0.0.1:9")
        .env("http_proxy", "http://127.0.0.1:9")
        .env("HTTPS_PROXY", "http://127.0.0.1:9")
        .env("https_proxy", "http://127.0.0.1:9")
        .env("ALL_PROXY", "http://127.0.0.1:9")
        .env("all_proxy", "http://127.0.0.1:9")
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            assert!(status.success(), "isolated proxy test failed: {status}");
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(8) {
            child.kill()?;
            child.wait()?;
            return Err("isolated proxy test exceeded deadline".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test]
async fn environment_proxy_child() -> TestResult {
    if std::env::var_os("KEELSHELL_AI_PROXY_FIXTURE").is_none() {
        return Ok(());
    }
    let fixture = Fixture::start(Reply::json(r#"{"data":[{"id":"direct-model"}]}"#)).await?;
    assert_eq!(
        client(4096)?
            .discover_models(&fixture.endpoint, None, &RequestCancellation::new())
            .await?
            .models(),
        &["direct-model"]
    );
    fixture.finish().await
}
