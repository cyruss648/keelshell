//! Actual loopback HTTP interoperability, without cloud credentials or services.

use std::{
    error::Error,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use keelshell_ai::{AiClient, AiError, ContextDraft, ProviderConfig};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct RecordedRequest {
    headers: String,
    body: String,
}

struct Server {
    provider: ProviderConfig,
    handle: JoinHandle<io::Result<RecordedRequest>>,
}

impl Server {
    fn start(
        status: u16,
        body: &str,
        declared_length: bool,
        body_delay: Duration,
    ) -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let provider = ProviderConfig::new(
            &format!("http://{address}/v1/chat/completions"),
            "fixture-model",
        )?;
        let body = body.to_owned();
        let handle = thread::spawn(move || {
            let started = Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if started.elapsed() > Duration::from_secs(5) {
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "fixture accept deadline",
                            ));
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => return Err(error),
                }
            };
            // macOS may inherit O_NONBLOCK from the listening socket.
            stream.set_nonblocking(false)?;
            stream.set_read_timeout(Some(Duration::from_secs(3)))?;
            stream.set_write_timeout(Some(Duration::from_secs(3)))?;
            let request = read_request(&mut stream)?;
            let length = if declared_length {
                format!("Content-Length: {}\r\n", body.len())
            } else {
                String::new()
            };
            let redirect = if status == 302 {
                "Location: http://127.0.0.1:9/unreviewed\r\n"
            } else {
                ""
            };
            stream.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\n{length}{redirect}Connection: close\r\n\r\n").as_bytes())?;
            if !body_delay.is_zero() {
                thread::sleep(body_delay);
            }
            // A timed-out or oversized-response client is allowed to close early.
            let _ = stream.write_all(body.as_bytes());
            Ok(request)
        });
        Ok(Self { provider, handle })
    }

    fn finish(self) -> TestResult<RecordedRequest> {
        self.handle
            .join()
            .map_err(|_| io::Error::other("fixture thread panicked"))?
            .map_err(Into::into)
    }
}

fn read_request(stream: &mut TcpStream) -> io::Result<RecordedRequest> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(offset) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
            break offset + 4;
        }
        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer)?;
        if read == 0 || bytes.len() > 1024 * 1024 {
            return Err(io::Error::other("invalid fixture request"));
        }
        bytes.extend_from_slice(&buffer[..read]);
    };
    let headers = String::from_utf8(bytes[..header_end].to_vec()).map_err(io::Error::other)?;
    let length = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .ok_or_else(|| io::Error::other("missing request content length"))?;
    if length > 1024 * 1024 {
        return Err(io::Error::other("fixture request body too large"));
    }
    while bytes.len() - header_end < length {
        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Err(io::Error::other("truncated fixture request"));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    let body = String::from_utf8(bytes[header_end..header_end + length].to_vec())
        .map_err(io::Error::other)?;
    Ok(RecordedRequest { headers, body })
}

fn client(max_bytes: usize) -> Result<AiClient, AiError> {
    AiClient::new(Duration::from_secs(2), max_bytes)
}

fn answer(content: &str) -> String {
    serde_json::json!({"choices":[{"message":{"role":"assistant","content":content}}]}).to_string()
}

#[test]
fn real_request_equals_preview_and_excludes_selected_secrets() -> TestResult {
    let server = Server::start(
        200,
        &answer("Inspect the SSH listener"),
        true,
        Duration::ZERO,
    )?;
    let prepared = ContextDraft::new("Explain failure for custom-private-host")
        .with_host_label("custom-private-host")
        .add_selection(
            "selected log",
            "password='two secret words'\nAuthorization: Bearer context-token\nconnection refused",
        )
        .prepare(
            &server.provider,
            &["custom-private-host", "transport-only-secret"],
            8192,
        )?;
    let preview = prepared.preview_json().to_owned();
    let reply = client(4096)?.send(prepared.approve(), Some("transport-only-secret"))?;
    assert_eq!(reply.text(), "Inspect the SSH listener");
    let recorded = server.finish()?;
    assert_eq!(recorded.body, preview);
    for secret in [
        "custom-private-host",
        "two secret words",
        "context-token",
        "transport-only-secret",
    ] {
        assert!(!recorded.body.contains(secret));
    }
    assert!(
        recorded
            .headers
            .starts_with("POST /v1/chat/completions HTTP/1.1")
    );
    assert!(
        recorded
            .headers
            .to_ascii_lowercase()
            .contains("authorization: bearer transport-only-secret")
    );
    let payload: serde_json::Value = serde_json::from_str(&recorded.body)?;
    assert_eq!(payload["model"], "fixture-model");
    assert_eq!(payload["stream"], false);
    Ok(())
}

#[test]
fn http_failure_never_echoes_provider_body_or_key() -> TestResult {
    let server = Server::start(
        401,
        "provider echoed sensitive-test-key and request details",
        true,
        Duration::ZERO,
    )?;
    let request = ContextDraft::new("hello").prepare(&server.provider, &[], 100)?;
    let result = client(4096)?.send(request.approve(), Some("sensitive-test-key"));
    assert!(matches!(result, Err(AiError::HttpStatus(401))));
    assert!(!format!("{result:?}").contains("sensitive-test-key"));
    server.finish()?;
    Ok(())
}

#[test]
fn redirects_cannot_change_the_reviewed_destination() -> TestResult {
    let server = Server::start(302, "", true, Duration::ZERO)?;
    let request = ContextDraft::new("hello").prepare(&server.provider, &[], 100)?;
    assert!(matches!(
        client(1024)?.send(request.approve(), None),
        Err(AiError::HttpStatus(302))
    ));
    server.finish()?;
    Ok(())
}

#[test]
fn oversized_declared_and_streamed_bodies_are_rejected() -> TestResult {
    for declared in [true, false] {
        let server = Server::start(200, &"x".repeat(8192), declared, Duration::ZERO)?;
        let request = ContextDraft::new("hello").prepare(&server.provider, &[], 100)?;
        assert!(matches!(
            client(1024)?.send(request.approve(), None),
            Err(AiError::ResponseTooLarge)
        ));
        server.finish()?;
    }
    Ok(())
}

#[test]
fn malformed_and_empty_replies_are_reported_without_raw_content() -> TestResult {
    for (body, expected) in [
        ("not json".to_owned(), AiError::InvalidResponse),
        (answer(" \n "), AiError::EmptyReply),
        ("{\"choices\":[]}".to_owned(), AiError::EmptyReply),
        (
            "{\"choices\":[{\"message\":{\"content\":null}}]}".to_owned(),
            AiError::EmptyReply,
        ),
    ] {
        let server = Server::start(200, &body, true, Duration::ZERO)?;
        let request = ContextDraft::new("hello").prepare(&server.provider, &[], 100)?;
        let result = client(4096)?.send(request.approve(), None);
        assert!(matches!(result, Err(error) if error == expected));
        server.finish()?;
    }
    Ok(())
}

#[test]
fn body_read_timeout_is_bounded() -> TestResult {
    let server = Server::start(200, &answer("late reply"), true, Duration::from_millis(180))?;
    let request = ContextDraft::new("hello").prepare(&server.provider, &[], 100)?;
    let client = AiClient::new(Duration::from_millis(50), 4096)?;
    let started = Instant::now();
    assert!(matches!(
        client.send(request.approve(), None),
        Err(AiError::Timeout)
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
    server.finish()?;
    Ok(())
}

#[test]
fn authentication_key_found_in_context_is_rejected_before_network() -> TestResult {
    let provider = ProviderConfig::new("http://127.0.0.1:9/chat", "fixture")?;
    let request = ContextDraft::new("my unmarked-transport-key").prepare(&provider, &[], 1024)?;
    assert!(matches!(
        client(4096)?.send(request.approve(), Some("unmarked-transport-key")),
        Err(AiError::CredentialInContext)
    ));
    Ok(())
}

#[test]
fn provider_cannot_reflect_authentication_key_into_reply() -> TestResult {
    let server = Server::start(
        200,
        &answer("echo response-secret-key"),
        true,
        Duration::ZERO,
    )?;
    let request = ContextDraft::new("hello").prepare(&server.provider, &[], 100)?;
    let reply = client(4096)?.send(request.approve(), Some("response-secret-key"))?;
    assert!(!reply.text().contains("response-secret-key"));
    server.finish()?;
    Ok(())
}
