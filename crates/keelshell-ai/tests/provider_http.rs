//! Actual loopback HTTP interoperability, without cloud credentials or services.

use std::{
    error::Error,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use keelshell_ai::{AiClient, AiError, ContextDraft, ProviderConfig, ProviderProtocol};

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
        Self::start_at(
            "/v1/chat/completions",
            status,
            body,
            declared_length,
            body_delay,
        )
    }

    fn start_at(
        path: &str,
        status: u16,
        body: &str,
        declared_length: bool,
        body_delay: Duration,
    ) -> TestResult<Self> {
        Self::start_at_protocol(
            path,
            ProviderProtocol::ChatCompletions,
            status,
            body,
            declared_length,
            body_delay,
        )
    }

    fn start_at_protocol(
        path: &str,
        protocol: ProviderProtocol,
        status: u16,
        body: &str,
        declared_length: bool,
        body_delay: Duration,
    ) -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let provider = ProviderConfig::new_with_protocol(
            &format!("http://{address}{path}"),
            "fixture-model",
            protocol,
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

fn responses_answer(content: &str) -> String {
    serde_json::json!({
        "id": "resp_fixture",
        "model": "responses-model",
        "output": [
            {"type": "reasoning", "id": "reasoning_fixture"},
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": content}]
            }
        ]
    })
    .to_string()
}

fn anthropic_answer(content: &str) -> String {
    serde_json::json!({
        "id": "msg_fixture",
        "type": "message",
        "role": "assistant",
        "model": "fixture-model",
        "content": [{"type": "text", "text": content}],
        "stop_reason": "end_turn"
    })
    .to_string()
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
fn protocol_token_limits_reach_http_as_the_exact_reviewed_body() -> TestResult {
    for (protocol, path, field, response) in [
        (
            ProviderProtocol::ChatCompletions,
            "/v1/chat/completions",
            "max_completion_tokens",
            answer("OK"),
        ),
        (
            ProviderProtocol::Responses,
            "/v1/responses",
            "max_output_tokens",
            responses_answer("OK"),
        ),
        (
            ProviderProtocol::AnthropicMessages,
            "/v1/messages",
            "max_tokens",
            anthropic_answer("OK"),
        ),
    ] {
        let server =
            Server::start_at_protocol(path, protocol, 200, &response, true, Duration::ZERO)?;
        let prepared = ContextDraft::new("Explain this failure")
            .add_selection("output", "diagnostic line\n".repeat(3000))
            .prepare_with_limits(&server.provider, &[], 16 * 1024, Some(512), Some(8192))?;
        assert!(prepared.redaction_report().truncated_bytes > 0);
        let preview = prepared.preview_json().to_owned();
        assert_eq!(client(4096)?.send(prepared.approve(), None)?.text(), "OK");
        let recorded = server.finish()?;
        assert_eq!(recorded.body, preview);
        let json: serde_json::Value = serde_json::from_str(&recorded.body)?;
        assert_eq!(json[field], 512);
    }
    Ok(())
}

#[test]
fn responses_request_equals_preview_and_extracts_output_text() -> TestResult {
    let server = Server::start_at(
        "/v1/responses",
        200,
        &responses_answer("Review the SSH listener"),
        true,
        Duration::ZERO,
    )?;
    let provider = ProviderConfig::new_with_protocol(
        server.provider.endpoint(),
        "responses-model",
        ProviderProtocol::Responses,
    )?;
    let prepared = ContextDraft::new("Explain the selected failure")
        .add_selection("selected output", "connection refused")
        .prepare(&provider, &[], 8192)?;
    let preview = prepared.preview_json().to_owned();
    let reply = client(4096)?.send(prepared.approve(), None)?;
    assert_eq!(reply.text(), "Review the SSH listener");
    let recorded = server.finish()?;
    assert_eq!(recorded.body, preview);
    let payload: serde_json::Value = serde_json::from_str(&recorded.body)?;
    assert_eq!(payload["model"], "responses-model");
    assert_eq!(payload["stream"], false);
    assert!(payload["instructions"].is_string());
    assert!(
        payload["input"]
            .as_str()
            .is_some_and(|value| value.contains("connection refused"))
    );
    assert!(recorded.headers.starts_with("POST /v1/responses HTTP/1.1"));
    Ok(())
}

#[test]
fn anthropic_request_equals_preview_uses_x_api_key_and_exact_messages_fields() -> TestResult {
    let server = Server::start_at_protocol(
        "/v1/messages",
        ProviderProtocol::AnthropicMessages,
        200,
        &anthropic_answer("Review the SSH listener"),
        true,
        Duration::ZERO,
    )?;
    let prepared = ContextDraft::new("Explain the selected failure")
        .add_selection("selected output", "connection refused")
        .prepare_with_max_tokens(&server.provider, &[], 8192, 8192)?;
    let preview = prepared.preview_json().to_owned();
    let reply = client(4096)?.send(prepared.approve(), Some("anthropic-fixture-key"))?;
    assert_eq!(reply.text(), "Review the SSH listener");
    let recorded = server.finish()?;
    assert_eq!(recorded.body, preview);
    let lower = recorded.headers.to_ascii_lowercase();
    assert!(lower.contains("x-api-key: anthropic-fixture-key\r\n"));
    assert!(lower.contains("anthropic-version: 2023-06-01\r\n"));
    assert!(!lower.contains("authorization:"));
    let payload: serde_json::Value = serde_json::from_str(&recorded.body)?;
    assert_eq!(payload["model"], "fixture-model");
    assert_eq!(payload["max_tokens"], 8192);
    assert_eq!(payload["stream"], false);
    assert!(payload["system"].is_string());
    assert_eq!(payload["messages"][0]["role"], "user");
    assert!(payload.get("tools").is_none());
    Ok(())
}

#[test]
fn anthropic_version_is_sent_without_an_api_key_and_non_text_blocks_are_never_text() -> TestResult {
    let server = Server::start_at_protocol(
        "/v1/messages",
        ProviderProtocol::AnthropicMessages,
        200,
        &anthropic_answer("OK"),
        true,
        Duration::ZERO,
    )?;
    let request = ContextDraft::new("hello").prepare(&server.provider, &[], 100)?;
    let reply = client(4096)?.send(request.approve(), None)?;
    assert_eq!(reply.text(), "OK");
    let recorded = server.finish()?;
    let lower = recorded.headers.to_ascii_lowercase();
    assert!(lower.contains("anthropic-version: 2023-06-01\r\n"));
    assert!(!lower.contains("x-api-key:"));

    for (body, expected) in [
        (
            serde_json::json!({
                "role":"assistant", "model":"fixture-model",
                "content":[{"type":"thinking","thinking":"private reasoning"},{"type":"text","text":"visible"}]
            }).to_string(),
            Ok("visible"),
        ),
        (
            serde_json::json!({
                "role":"assistant", "content":[{"type":"tool_use","id":"tool","name":"x","input":{}}]
            }).to_string(),
            Err(AiError::EmptyReply),
        ),
        (
            serde_json::json!({
                "role":"assistant", "content":[{"type":"thinking","thinking":"private reasoning"}]
            }).to_string(),
            Err(AiError::EmptyReply),
        ),
        (
            serde_json::json!({
                "role":"user", "content":[{"type":"text","text":"spoof"}]
            }).to_string(),
            Err(AiError::InvalidResponse),
        ),
        (
            serde_json::json!({
                "role":"assistant", "content":[{"type":"text"}]
            }).to_string(),
            Err(AiError::InvalidResponse),
        ),
    ] {
        let server = Server::start_at_protocol(
            "/v1/messages",
            ProviderProtocol::AnthropicMessages,
            200,
            &body,
            true,
            Duration::ZERO,
        )?;
        let request = ContextDraft::new("hello").prepare(&server.provider, &[], 100)?;
        let result = client(4096)?.send(request.approve(), None);
        match expected {
            Ok(text) => assert_eq!(result?.text(), text),
            Err(error) => assert!(matches!(result, Err(actual) if actual == error)),
        }
        server.finish()?;
    }
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

#[test]
fn inference_exact_reviewed_body_is_sent_for_all_protocol_fields() -> TestResult {
    use keelshell_ai::{InferenceOptions, ReasoningEffort, ReasoningOption, SamplingOption};
    use serde_json::json;
    for protocol in [
        ProviderProtocol::ChatCompletions,
        ProviderProtocol::Responses,
        ProviderProtocol::AnthropicMessages,
    ] {
        let (path, answer) = match protocol {
            ProviderProtocol::ChatCompletions => ("/v1/chat/completions", answer("OK")),
            ProviderProtocol::Responses => ("/v1/responses", responses_answer("OK")),
            ProviderProtocol::AnthropicMessages => ("/v1/messages", anthropic_answer("OK")),
        };
        let mut cases = vec![
            (InferenceOptions::default(), None, serde_json::Value::Null),
            (
                InferenceOptions {
                    reasoning: ReasoningOption::Effort(ReasoningEffort::Low),
                    sampling: SamplingOption::ProviderDefault,
                },
                Some(match protocol {
                    ProviderProtocol::ChatCompletions => "reasoning_effort",
                    ProviderProtocol::Responses => "reasoning",
                    ProviderProtocol::AnthropicMessages => "output_config",
                }),
                if protocol == ProviderProtocol::ChatCompletions {
                    json!("low")
                } else {
                    json!({"effort":"low"})
                },
            ),
            (
                InferenceOptions {
                    reasoning: ReasoningOption::ProviderDefault,
                    sampling: SamplingOption::Temperature(0),
                },
                Some("temperature"),
                json!(0.0),
            ),
            (
                InferenceOptions {
                    reasoning: ReasoningOption::ProviderDefault,
                    sampling: SamplingOption::TopP(999),
                },
                Some("top_p"),
                json!(0.999),
            ),
        ];
        if protocol == ProviderProtocol::AnthropicMessages {
            for (mode, expected) in [
                (ReasoningOption::Thinking(true), json!({"type":"adaptive"})),
                (ReasoningOption::Thinking(false), json!({"type":"disabled"})),
                (
                    ReasoningOption::TokenBudget(8192),
                    json!({"type":"enabled","budget_tokens":8192}),
                ),
            ] {
                cases.push((
                    InferenceOptions {
                        reasoning: mode,
                        sampling: SamplingOption::ProviderDefault,
                    },
                    Some("thinking"),
                    expected,
                ));
            }
        }
        for (options, field, expected) in cases {
            let server =
                Server::start_at_protocol(path, protocol, 200, &answer, true, Duration::ZERO)?;
            let provider = server.provider.clone().with_inference_options(options)?;
            let request = ContextDraft::new("Explicit inference fixture").prepare_with_max_tokens(
                &provider,
                &[],
                8192,
                16000,
            )?;
            let preview = request.preview_json().to_owned();
            let response = client(4096)?.send(request.approve(), None);
            let recorded = server.finish()?;
            assert_eq!(response?.text(), "OK");
            assert_eq!(recorded.body, preview);
            let body: serde_json::Value = serde_json::from_str(&recorded.body)?;
            if let Some(field) = field {
                assert_eq!(body[field], expected);
            } else {
                for name in [
                    "reasoning_effort",
                    "reasoning",
                    "output_config",
                    "thinking",
                    "temperature",
                    "top_p",
                ] {
                    assert!(body.get(name).is_none(), "{name}");
                }
            }
            assert!(body.get("tools").is_none());
        }
    }
    Ok(())
}

#[test]
fn inference_invalid_protocol_budget_and_secret_metadata_fail_before_network() -> TestResult {
    use keelshell_ai::{InferenceOptions, ReasoningEffort, ReasoningOption, SamplingOption};
    for protocol in [
        ProviderProtocol::ChatCompletions,
        ProviderProtocol::Responses,
        ProviderProtocol::AnthropicMessages,
    ] {
        let provider =
            ProviderConfig::new_with_protocol("http://127.0.0.1:9/unused", "fixture", protocol)?;
        for options in [
            InferenceOptions {
                reasoning: ReasoningOption::ProviderDefault,
                sampling: SamplingOption::Temperature(2001),
            },
            InferenceOptions {
                reasoning: ReasoningOption::ProviderDefault,
                sampling: SamplingOption::TopP(1001),
            },
            InferenceOptions {
                reasoning: ReasoningOption::Effort(ReasoningEffort::High),
                sampling: SamplingOption::Temperature(1000),
            },
        ] {
            assert_eq!(
                provider.clone().with_inference_options(options),
                Err(AiError::InvalidInferenceOptions)
            );
        }
        let options = InferenceOptions {
            reasoning: ReasoningOption::Effort(ReasoningEffort::High),
            sampling: SamplingOption::ProviderDefault,
        };
        let provider = provider.with_inference_options(options)?;
        assert!(matches!(
            ContextDraft::new("ordinary").prepare(&provider, &["high"], 8192),
            Err(AiError::CredentialInContext)
        ));
    }
    let provider = ProviderConfig::new_with_protocol(
        "http://127.0.0.1:9/unused",
        "fixture",
        ProviderProtocol::AnthropicMessages,
    )?
    .with_inference_options(InferenceOptions {
        reasoning: ReasoningOption::TokenBudget(4096),
        sampling: SamplingOption::ProviderDefault,
    })?;
    for output in [1024, 4096] {
        assert!(matches!(
            ContextDraft::new("ordinary").prepare_with_max_tokens(&provider, &[], 8192, output),
            Err(AiError::InvalidInferenceOptions)
        ));
    }
    assert!(
        ContextDraft::new("ordinary")
            .prepare_with_max_tokens(&provider, &[], 8192, 4097)
            .is_ok()
    );
    Ok(())
}

#[tokio::test]
async fn inference_connectivity_test_uses_same_protocol_options() -> TestResult {
    use keelshell_ai::{
        InferenceOptions, ProviderClient, ReasoningEffort, ReasoningOption, RequestCancellation,
        SamplingOption,
    };
    for protocol in [
        ProviderProtocol::ChatCompletions,
        ProviderProtocol::Responses,
        ProviderProtocol::AnthropicMessages,
    ] {
        let (path, answer, field) = match protocol {
            ProviderProtocol::ChatCompletions => {
                ("/v1/chat/completions", answer("OK"), "reasoning_effort")
            }
            ProviderProtocol::Responses => ("/v1/responses", responses_answer("OK"), "reasoning"),
            ProviderProtocol::AnthropicMessages => {
                ("/v1/messages", anthropic_answer("OK"), "output_config")
            }
        };
        let server = Server::start_at_protocol(path, protocol, 200, &answer, true, Duration::ZERO)?;
        let provider = server
            .provider
            .clone()
            .with_inference_options(InferenceOptions {
                reasoning: ReasoningOption::Effort(ReasoningEffort::Medium),
                sampling: SamplingOption::ProviderDefault,
            })?;
        let result = ProviderClient::new(Duration::from_secs(2), 4096)?
            .test_connection_with_limits(
                &provider,
                None,
                &RequestCancellation::new(),
                Some(16000),
                None,
            )
            .await;
        let recorded = server.finish()?;
        result?;
        let body: serde_json::Value = serde_json::from_str(&recorded.body)?;
        assert_eq!(
            body[field],
            if protocol == ProviderProtocol::ChatCompletions {
                serde_json::json!("medium")
            } else {
                serde_json::json!({"effort":"medium"})
            }
        );
    }
    Ok(())
}

#[test]
fn inference_remote_rejection_keeps_the_original_reviewed_payload() -> TestResult {
    use keelshell_ai::{
        InferenceOptions, MessagesThinking, ReasoningEffort, ReasoningOption, SamplingOption,
    };
    let server = Server::start_at_protocol(
        "/v1/messages",
        ProviderProtocol::AnthropicMessages,
        400,
        r#"{"error":{"message":"unsupported effort"}}"#,
        true,
        Duration::ZERO,
    )?;
    let provider = server
        .provider
        .clone()
        .with_inference_options(InferenceOptions {
            reasoning: ReasoningOption::Messages {
                effort: Some(ReasoningEffort::High),
                thinking: MessagesThinking::Adaptive,
            },
            sampling: SamplingOption::ProviderDefault,
        })?;
    let request = ContextDraft::new("fixture rejection").prepare(&provider, &[], 8192)?;
    let preview = request.preview_json().to_owned();
    let result = client(4096)?.send(request.approve(), None);
    let recorded = server.finish()?;
    assert!(matches!(result, Err(AiError::HttpStatus(400))));
    assert_eq!(recorded.body, preview);
    assert!(recorded.body.contains("high"));
    assert!(recorded.body.contains("adaptive"));
    Ok(())
}

#[test]
fn inference_messages_composes_effort_and_thinking_for_ask_and_connectivity() -> TestResult {
    use keelshell_ai::{
        InferenceOptions, MessagesThinking, ProviderClient, ReasoningEffort, ReasoningOption,
        SamplingOption,
    };
    use serde_json::json;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let protocol = ProviderProtocol::AnthropicMessages;
    for (thinking, expected) in [
        (MessagesThinking::ProviderDefault, serde_json::Value::Null),
        (MessagesThinking::Adaptive, json!({"type":"adaptive"})),
        (MessagesThinking::Disabled, json!({"type":"disabled"})),
        (
            MessagesThinking::LegacyBudget(8192),
            json!({"type":"enabled","budget_tokens":8192}),
        ),
    ] {
        let options = InferenceOptions {
            reasoning: ReasoningOption::Messages {
                effort: Some(ReasoningEffort::Medium),
                thinking,
            },
            sampling: SamplingOption::ProviderDefault,
        };
        for connection_test in [false, true] {
            let server = Server::start_at_protocol(
                "/v1/messages",
                protocol,
                200,
                &anthropic_answer("OK"),
                true,
                Duration::ZERO,
            )?;
            let provider = server.provider.clone().with_inference_options(options)?;
            let preview = if connection_test {
                runtime.block_on(
                    ProviderClient::new(Duration::from_secs(2), 4096)?.test_connection_with_limits(
                        &provider,
                        None,
                        &keelshell_ai::RequestCancellation::new(),
                        Some(16000),
                        None,
                    ),
                )?;
                None
            } else {
                let request = ContextDraft::new("composed fixture").prepare_with_max_tokens(
                    &provider,
                    &[],
                    8192,
                    16000,
                )?;
                let preview = request.preview_json().to_owned();
                assert_eq!(client(4096)?.send(request.approve(), None)?.text(), "OK");
                Some(preview)
            };
            let recorded = server.finish()?;
            if let Some(preview) = preview {
                assert_eq!(recorded.body, preview);
            }
            let body: serde_json::Value = serde_json::from_str(&recorded.body)?;
            assert_eq!(body["output_config"], json!({"effort":"medium"}));
            assert_eq!(body.get("thinking").cloned().unwrap_or_default(), expected);
            assert!(body.get("tools").is_none());
        }
    }
    let provider =
        ProviderConfig::new_with_protocol("http://127.0.0.1:9/unused", "fixture", protocol)?;
    let empty = provider.clone().with_inference_options(InferenceOptions {
        reasoning: ReasoningOption::Messages {
            effort: None,
            thinking: MessagesThinking::ProviderDefault,
        },
        sampling: SamplingOption::ProviderDefault,
    })?;
    assert_eq!(
        ContextDraft::new("omitted")
            .prepare(&provider, &[], 8192)?
            .preview_json(),
        ContextDraft::new("omitted")
            .prepare(&empty, &[], 8192)?
            .preview_json()
    );
    let budget = provider.clone().with_inference_options(InferenceOptions {
        reasoning: ReasoningOption::Messages {
            effort: Some(ReasoningEffort::Medium),
            thinking: MessagesThinking::LegacyBudget(4096),
        },
        sampling: SamplingOption::ProviderDefault,
    })?;
    assert!(matches!(
        ContextDraft::new("boundary").prepare(&budget, &[], 8192),
        Err(AiError::InvalidInferenceOptions)
    ));
    assert!(
        provider
            .with_inference_options(InferenceOptions {
                reasoning: ReasoningOption::Messages {
                    effort: Some(ReasoningEffort::High),
                    thinking: MessagesThinking::Adaptive
                },
                sampling: SamplingOption::Temperature(0),
            })
            .is_err()
    );
    Ok(())
}
