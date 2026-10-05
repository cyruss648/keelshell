//! Known metadata names must fail before connecting; fixtures own and join every listener.

use keelshell_ai::{
    AiClient, AiError, ContextDraft, ProviderClient, ProviderConfig, ProviderEndpoint,
    ProviderProtocol, RequestCancellation, RequestOptions,
};
use std::{error::Error, net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
    time::timeout,
};
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;
type FixtureResult = Result<Option<String>, Box<dyn Error + Send + Sync>>;
const LIMIT: Duration = Duration::from_secs(3);
const KNOWN: &str = "retained-metadata-secret";
const ACTIVE: &str = "approved-header-value";
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

fn reply(style: ProviderProtocol, operation: usize) -> String {
    if operation == 0 {
        return serde_json::json!({"data":[{"id":"ordinary-model"}]}).to_string();
    }
    match style {
        ProviderProtocol::ChatCompletions => serde_json::json!({"model":"ordinary-model","choices":[{"message":{"content":"ordinary reply"}}]}),
        ProviderProtocol::Responses => serde_json::json!({"model":"ordinary-model","output":[{"type":"message","content":[{"type":"output_text","text":"ordinary reply"}]}]}),
        ProviderProtocol::AnthropicMessages => serde_json::json!({"role":"assistant","model":"ordinary-model","content":[{"type":"text","text":"ordinary reply"}]}),
    }.to_string()
}

async fn read(stream: &mut TcpStream) -> FixtureResult {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).await?;
        bytes.push(byte[0]);
        if bytes.len() > 16384 {
            return Err("fixture header bound".into());
        }
    }
    let headers = String::from_utf8(bytes)?;
    let length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")?
                .trim()
                .parse::<usize>()
                .ok()
        })
        .unwrap_or(0);
    if length > 65536 {
        return Err("fixture body bound".into());
    }
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await?;
    Ok(Some(format!("{headers}{}", String::from_utf8(body)?)))
}

struct Fixture {
    address: SocketAddr,
    stop: oneshot::Sender<()>,
    task: JoinHandle<FixtureResult>,
}

impl Fixture {
    async fn start(style: ProviderProtocol, operation: usize) -> Result<Self, Box<dyn Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            // Check a queued connection before acknowledging the caller's stop.
            // If the guard regresses, reply normally so rejection assertions fail
            // after the owned task has joined, rather than stranding a client.
            let accepted = tokio::select! {
                biased;
                result = listener.accept() => Some(result?),
                _ = &mut stopped => None,
            };
            if let Some((mut stream, _)) = accepted {
                let wire = timeout(LIMIT, read(&mut stream)).await??;
                let body = reply(style, operation);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                timeout(LIMIT, stream.write_all(response.as_bytes())).await??;
                Ok(wire)
            } else {
                Ok(None)
            }
        });
        Ok(Self {
            address,
            stop,
            task,
        })
    }

    fn endpoint(&self, style: ProviderProtocol) -> String {
        format!("http://{}/v1/{}", self.address, suffix(style))
    }

    async fn finish(self) -> Result<Option<String>, Box<dyn Error>> {
        let _ = self.stop.send(());
        let mut task = self.task;
        let joined = match timeout(Duration::from_secs(4), &mut task).await {
            Ok(result) => result.map_err(|_| "fixture task panic")?,
            Err(_) => {
                task.abort();
                let _ = task.await;
                return Err("fixture task bound".into());
            }
        };
        let rebound = TcpListener::bind(self.address).await?;
        drop(rebound);
        joined.map_err(|_| "fixture read/write failed".into())
    }
}

#[tokio::test]
async fn snapshot_known_names_reject_all_nine_protocol_operations_without_http() -> TestResult {
    for style in STYLES {
        for operation in 0..3 {
            let fixture = Fixture::start(style, operation).await?;
            let endpoint = fixture.endpoint(style);
            // Different ASCII case and enclosing text exercise wire normalization.
            let options = RequestOptions::new(
                vec![(
                    format!("x-{}-suffix", KNOWN.to_ascii_uppercase()),
                    Zeroizing::new(ACTIVE.into()),
                )],
                Default::default(),
            )?
            .with_context_secrets(&[KNOWN])?;
            let provider = ProviderConfig::new_with_protocol(&endpoint, "ordinary-model", style)?
                .with_request_options(options.clone());
            let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
            let cancel = RequestCancellation::new();
            let result = match operation {
                0 => client
                    .discover_models(
                        &ProviderEndpoint::new_with_protocol(&endpoint, style)?,
                        None,
                        &cancel,
                    )
                    .await
                    .map(|_| ()),
                1 => client
                    .test_connection(&provider, None, &cancel)
                    .await
                    .map(|_| ()),
                _ => ContextDraft::new("ordinary prompt")
                    .prepare(&provider, &[], 4096)
                    .map(|_| ()),
            };
            let observed = fixture.finish().await?;
            assert!(
                matches!(result, Err(AiError::CredentialInContext)),
                "{style:?}/{operation}: {result:?}"
            );
            assert!(
                observed.is_none(),
                "{style:?}/{operation} connected before rejection"
            );
            println!(
                "snapshot_name style={style:?} operation={operation} typed_rejection=true http=0 joined=true listener_released=true"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn late_api_key_names_reject_all_nine_async_operations_without_http() -> TestResult {
    for style in STYLES {
        for operation in 0..3 {
            let fixture = Fixture::start(style, operation).await?;
            let endpoint = fixture.endpoint(style);
            let options = RequestOptions::new(
                vec![(KNOWN.into(), Zeroizing::new(ACTIVE.into()))],
                Default::default(),
            )?;
            let provider = ProviderConfig::new_with_protocol(&endpoint, "ordinary-model", style)?
                .with_request_options(options.clone());
            let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
            let cancel = RequestCancellation::new();
            let result = match operation {
                0 => client
                    .discover_models(
                        &ProviderEndpoint::new_with_protocol(&endpoint, style)?,
                        Some(KNOWN),
                        &cancel,
                    )
                    .await
                    .map(|_| ()),
                1 => client
                    .test_connection(&provider, Some(KNOWN), &cancel)
                    .await
                    .map(|_| ()),
                _ => {
                    let request =
                        ContextDraft::new("ordinary prompt").prepare(&provider, &[], 4096)?;
                    client
                        .send_approved(request.approve(), Some(KNOWN), &cancel)
                        .await
                        .map(|_| ())
                }
            };
            let observed = fixture.finish().await?;
            assert!(
                matches!(result, Err(AiError::CredentialInContext)),
                "{style:?}/{operation}: {result:?}"
            );
            assert!(observed.is_none());
        }
    }
    Ok(())
}

#[tokio::test]
async fn late_api_key_names_reject_all_blocking_protocols_without_http() -> TestResult {
    for style in STYLES {
        let fixture = Fixture::start(style, 2).await?;
        let endpoint = fixture.endpoint(style);
        let result = timeout(
            LIMIT,
            tokio::task::spawn_blocking(move || {
                let options = RequestOptions::new(
                    vec![(KNOWN.into(), Zeroizing::new(ACTIVE.into()))],
                    Default::default(),
                )?;
                let provider =
                    ProviderConfig::new_with_protocol(&endpoint, "ordinary-model", style)?
                        .with_request_options(options.clone());
                let request = ContextDraft::new("ordinary prompt").prepare(&provider, &[], 4096)?;
                AiClient::new_with_options(LIMIT, 65536, options)?
                    .send(request.approve(), Some(KNOWN))
                    .map(|_| ())
            }),
        )
        .await??;
        let observed = fixture.finish().await?;
        assert!(matches!(result, Err(AiError::CredentialInContext)));
        assert!(observed.is_none());
    }
    Ok(())
}

#[tokio::test]
async fn approved_values_still_deliver_with_safe_normalized_names_and_authentication() -> TestResult
{
    for style in STYLES {
        let fixture = Fixture::start(style, 2).await?;
        let endpoint = fixture.endpoint(style);
        let options = RequestOptions::new(
            vec![("X-Approved-Key".into(), Zeroizing::new(KNOWN.into()))],
            Default::default(),
        )?
        .with_context_secrets(&[KNOWN])?;
        let provider = ProviderConfig::new_with_protocol(&endpoint, "ordinary-model", style)?
            .with_request_options(options.clone());
        let request = ContextDraft::new("ordinary prompt").prepare(&provider, &[ACTIVE], 4096)?;
        let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
        let result = client
            .send_approved(request.approve(), Some(ACTIVE), &RequestCancellation::new())
            .await;
        let observed = fixture
            .finish()
            .await?
            .ok_or("approved request did not connect")?;
        result?;
        assert!(
            observed
                .lines()
                .any(|line| line == format!("x-approved-key: {KNOWN}"))
        );
        let expected = if style == ProviderProtocol::AnthropicMessages {
            format!("x-api-key: {ACTIVE}")
        } else {
            format!("authorization: Bearer {ACTIVE}")
        };
        assert!(observed.lines().any(|line| line == expected));
        assert!(
            !observed
                .split_once("\r\n\r\n")
                .ok_or("body separator")?
                .1
                .contains(KNOWN)
        );
    }
    Ok(())
}
