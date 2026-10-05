//! Known inactive credentials must never become provider-controlled outbound context.

use keelshell_ai::{
    AiError, ContextDraft, ProviderClient, ProviderConfig, ProviderEndpoint, ProviderProtocol,
    RequestCancellation, RequestOptions,
};
use std::{error::Error, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;
const LIMIT: Duration = Duration::from_secs(3);
const KNOWN: &str = "aW5hY3RpdmUtdXNlcjppbmFjdGl2ZS1wYXNz";
const STYLES: [ProviderProtocol; 3] = [
    ProviderProtocol::ChatCompletions,
    ProviderProtocol::Responses,
    ProviderProtocol::AnthropicMessages,
];
fn suffix(style: ProviderProtocol) -> &'static str {
    match style {
        ProviderProtocol::ChatCompletions => "chat/completions",
        ProviderProtocol::Responses => "responses",
        ProviderProtocol::AnthropicMessages => "messages",
    }
}
fn answer(style: ProviderProtocol) -> String {
    match style {
        ProviderProtocol::ChatCompletions => serde_json::json!({"model":KNOWN,"choices":[{"message":{"content":KNOWN}}]}),
        ProviderProtocol::Responses => serde_json::json!({"model":KNOWN,"output":[{"type":"message","content":[{"type":"output_text","text":KNOWN}]}]}),
        ProviderProtocol::AnthropicMessages => serde_json::json!({"role":"assistant","model":KNOWN,"content":[{"type":"text","text":KNOWN}]}),
    }.to_string()
}
fn options() -> Result<RequestOptions, AiError> {
    RequestOptions::new(
        vec![(
            "x-fixture".into(),
            Zeroizing::new("explicit-active-header".into()),
        )],
        Default::default(),
    )?
    .with_context_secrets(&[KNOWN])
}
async fn read(stream: &mut TcpStream) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut data = Vec::new();
    while !data.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).await?;
        data.push(byte[0]);
        if data.len() > 65536 {
            return Err("fixture header limit".into());
        }
    }
    let headers = String::from_utf8(data)?;
    let length: usize = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(str::trim)
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(0);
    if length > 65536 {
        return Err("fixture body limit".into());
    }
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await?;
    Ok(format!("{headers}{}", String::from_utf8(body)?))
}
async fn reply(stream: &mut TcpStream, body: &str) -> std::io::Result<()> {
    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await
}

#[tokio::test]
async fn all_protocols_catalog_test_and_ask_share_known_secrets_without_delivering_them()
-> TestResult {
    for style in STYLES {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/v1/{}", listener.local_addr()?, suffix(style));
        let task = tokio::spawn(async move {
            let mut count = 0;
            for index in 0..3 {
                let (mut stream, _) = timeout(LIMIT, listener.accept()).await??;
                let wire = timeout(LIMIT, read(&mut stream)).await??;
                assert!(
                    !wire.contains(KNOWN),
                    "known inactive credential reached the wire"
                );
                assert!(wire.contains("x-fixture: explicit-active-header"));
                assert!(
                    wire.contains(if style == ProviderProtocol::AnthropicMessages {
                        "x-api-key: explicit-active-key"
                    } else {
                        "authorization: Bearer explicit-active-key"
                    })
                );
                assert!(wire.starts_with(if index == 0 { "GET " } else { "POST " }));
                let body = if index == 0 {
                    serde_json::json!({"data":[{"id":KNOWN}]}).to_string()
                } else {
                    answer(style)
                };
                reply(&mut stream, &body).await?;
                count += 1;
            }
            Ok::<_, Box<dyn Error + Send + Sync>>(count)
        });
        let options = options()?;
        let provider = ProviderConfig::new_with_protocol(&endpoint, "ordinary-model", style)?
            .with_request_options(options.clone());
        let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
        let cancellation = RequestCancellation::new();
        assert!(matches!(
            client
                .discover_models(
                    &ProviderEndpoint::new_with_protocol(&endpoint, style)?,
                    Some("explicit-active-key"),
                    &cancellation
                )
                .await,
            Err(AiError::InvalidModelCatalog)
        ));
        assert!(
            client
                .test_connection(&provider, Some("explicit-active-key"), &cancellation)
                .await?
                .actual_model()
                .is_none()
        );
        let prepared =
            ContextDraft::new(format!("Explain {KNOWN}")).prepare(&provider, &[], 4096)?;
        assert!(!prepared.preview_json().contains(KNOWN));
        let text = client
            .send_approved(
                prepared.approve(),
                Some("explicit-active-key"),
                &cancellation,
            )
            .await?;
        assert!(!text.text().contains(KNOWN));
        let count = timeout(LIMIT, task)
            .await?
            .map_err(|_| "fixture panic")?
            .map_err(|_| "fixture failed")?;
        assert_eq!(count, 3, "joined GET/Test/Ask fixture count");
    }
    Ok(())
}

#[tokio::test]
async fn inactive_anthropic_cursor_is_rejected_before_a_second_get() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/messages", listener.local_addr()?);
    let task = tokio::spawn(async move {
        let (mut stream, _) = timeout(LIMIT, listener.accept()).await??;
        let request = timeout(LIMIT, read(&mut stream)).await??;
        assert!(request.starts_with("GET /v1/models?limit=1000 "));
        assert!(!request.contains(KNOWN));
        reply(
            &mut stream,
            &serde_json::json!({"data":[{"id":"ordinary-model"}],"has_more":true,"last_id":KNOWN})
                .to_string(),
        )
        .await?;
        let extra = timeout(Duration::from_millis(150), listener.accept()).await;
        Ok::<_, Box<dyn Error + Send + Sync>>(1 + usize::from(extra.is_ok()))
    });
    let client = ProviderClient::new_with_options(LIMIT, 65536, options()?)?;
    assert!(matches!(
        client
            .discover_models(
                &ProviderEndpoint::new_with_protocol(
                    &endpoint,
                    ProviderProtocol::AnthropicMessages
                )?,
                None,
                &RequestCancellation::new()
            )
            .await,
        Err(AiError::InvalidModelCatalog)
    ));
    let count = timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    assert_eq!(
        count, 1,
        "joined cursor fixture must observe only the first GET"
    );
    Ok(())
}

#[tokio::test]
async fn known_model_and_percent_encoded_endpoint_are_rejected_before_any_http() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let encoded: String = KNOWN.bytes().map(|byte| format!("%{byte:02X}")).collect();
    for style in STYLES {
        let options = options()?;
        let client = ProviderClient::new_with_options(LIMIT, 65536, options.clone())?;
        let endpoint = format!("http://{address}/v1/{}", suffix(style));
        let provider = ProviderConfig::new_with_protocol(&endpoint, KNOWN, style)?
            .with_request_options(options.clone());
        assert!(matches!(
            client
                .test_connection(&provider, None, &RequestCancellation::new())
                .await,
            Err(AiError::CredentialInContext)
        ));
        assert!(matches!(
            ContextDraft::new("Explain").prepare(&provider, &[], 4096),
            Err(AiError::CredentialInContext)
        ));
        let endpoint = format!("http://{address}/{encoded}/v1/{}", suffix(style));
        let provider = ProviderConfig::new_with_protocol(&endpoint, "ordinary-model", style)?
            .with_request_options(options);
        assert!(matches!(
            client
                .test_connection(&provider, None, &RequestCancellation::new())
                .await,
            Err(AiError::CredentialInContext)
        ));
        assert!(matches!(
            client
                .discover_models(
                    &ProviderEndpoint::new_with_protocol(&endpoint, style)?,
                    None,
                    &RequestCancellation::new()
                )
                .await,
            Err(AiError::CredentialInContext)
        ));
        assert!(matches!(
            ContextDraft::new("Explain").prepare(&provider, &[], 4096),
            Err(AiError::CredentialInContext)
        ));
    }
    assert!(
        timeout(Duration::from_millis(150), listener.accept())
            .await
            .is_err(),
        "zero-network destination rejection"
    );
    Ok(())
}

#[tokio::test]
async fn changed_known_secrets_invalidate_exact_client_and_approval_matching() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/chat/completions", listener.local_addr()?);
    let old = RequestOptions::default().with_context_secrets(&["old-known-secret"])?;
    let new = RequestOptions::default().with_context_secrets(&["new-known-secret"])?;
    let provider = ProviderConfig::new(&endpoint, "ordinary-model")?.with_request_options(new);
    let client = ProviderClient::new_with_options(LIMIT, 65536, old)?;
    assert!(matches!(
        client
            .test_connection(&provider, None, &RequestCancellation::new())
            .await,
        Err(AiError::RequestOptionsMismatch)
    ));
    let prepared = ContextDraft::new("Explain").prepare(&provider, &[], 4096)?;
    assert!(matches!(
        client
            .send_approved(prepared.approve(), None, &RequestCancellation::new())
            .await,
        Err(AiError::RequestOptionsMismatch)
    ));
    assert!(
        timeout(Duration::from_millis(150), listener.accept())
            .await
            .is_err(),
        "known-secret mismatch must make zero requests"
    );
    Ok(())
}

#[test]
fn known_secret_snapshot_is_bounded_private_and_canonical() -> TestResult {
    let first =
        RequestOptions::default().with_context_secrets(&[KNOWN, "second-known", "", KNOWN])?;
    let reordered = RequestOptions::default().with_context_secrets(&["second-known", KNOWN])?;
    assert_eq!(first, reordered);
    assert!(!format!("{first:?}").contains(KNOWN));
    assert!(
        !first
            .redact_for_review(&format!("Context {KNOWN}"))
            .contains(KNOWN)
    );
    assert!(first.header_names().next().is_none());
    assert!(first.proxy_url().is_none());
    assert!(matches!(
        RequestOptions::default().with_context_secrets(&vec![KNOWN; 4097]),
        Err(AiError::ContextTooLarge)
    ));
    let oversized = "x".repeat(1024 * 1024 + 1);
    assert!(matches!(
        RequestOptions::default().with_context_secrets(&[&oversized]),
        Err(AiError::ContextTooLarge)
    ));
    let large = "x".repeat(8192);
    assert!(matches!(
        RequestOptions::default().with_context_secrets(&vec![large.as_str(); 1025]),
        Err(AiError::ContextTooLarge)
    ));
    Ok(())
}
