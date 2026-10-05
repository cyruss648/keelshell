//! Isolated real TCP routing fixtures; every credential is synthetic.

use keelshell_ai::{
    AiError, ContextDraft, ProviderClient, ProviderConfig, ProviderEndpoint, ProviderProtocol,
    ProxyCredentials, ProxyRoute, RequestCancellation, RequestOptions,
};
use std::{error::Error, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use zeroize::Zeroizing;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const LIMIT: Duration = Duration::from_secs(3);
const VALUE: &str = "synthetic-header-secret";

const CHILD_RECEIPT: &str = "KEELSHELL_ISOLATED_PROXY_REAL_HTTP_COMPLETED";
const CHILD_OUTPUT_LIMIT: usize = 32 * 1024;

#[derive(Debug)]
struct IsolatedChildReceipt {
    pid: Option<u32>,
    io_error: Option<String>,
    status: Option<std::process::ExitStatus>,
    failure: Option<&'static str>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    reaped: bool,
    stdout_closed: bool,
    stderr_closed: bool,
}

impl IsolatedChildReceipt {
    fn success(&self) -> bool {
        self.failure.is_none()
            && self.status.is_some_and(|status| status.success())
            && self.reaped
            && self.stdout_closed
            && self.stderr_closed
            && String::from_utf8_lossy(&self.stdout).contains(CHILD_RECEIPT)
    }

    fn diagnostic(&self) -> String {
        format!(
            "pid={:?}; status={:?}; failure={:?}; io_error={:?}; reaped={}; pipes_closed={}/{}; stdout_bytes={}; stderr_bytes={}\nstdout: {}\nstderr: {}",
            self.pid,
            self.status,
            self.failure,
            self.io_error,
            self.reaped,
            self.stdout_closed,
            self.stderr_closed,
            self.stdout.len(),
            self.stderr.len(),
            String::from_utf8_lossy(&self.stdout),
            String::from_utf8_lossy(&self.stderr)
        )
    }
}

fn isolated_child(mode: Option<&str>, limit: Duration) -> TestResult<IsolatedChildReceipt> {
    isolated_child_environment(mode, limit, true)
}

fn isolated_child_environment(
    mode: Option<&str>,
    limit: Duration,
    include_system_root: bool,
) -> TestResult<IsolatedChildReceipt> {
    tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
        use process_wrap::tokio::{CommandWrap, KillOnDrop};
        use std::process::Stdio;
        let deadline = tokio::time::Instant::now() + limit;
        // Keep the original eight-second outer limit, including stop, reap and EOF.
        let execution_deadline = deadline - (limit / 4).min(Duration::from_secs(1));
        let mut command = tokio::process::Command::new(std::env::current_exe()?);
        command.args(["--exact", "explicit_proxy_environment_child", "--nocapture"])
            .env_clear()
            .env("KEELSHELL_REQUEST_OPTIONS_CHILD", "1")
            .env("NO_PROXY", "*")
            .env("no_proxy", "*")
            .env("HTTP_PROXY", "http://127.0.0.1:9")
            .env("HTTPS_PROXY", "http://127.0.0.1:9")
            .env("ALL_PROXY", "http://127.0.0.1:9")
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        // Winsock provider DLL paths may contain %SystemRoot%; keep this OS value
        // without inheriting PATH, credentials, proxy exclusions or user settings.
        if cfg!(windows) && include_system_root
            && let Some(system_root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system_root);
        }
        if let Some(mode) = mode {
            command.env("KEELSHELL_REQUEST_OPTIONS_DIAGNOSTIC_MODE", mode);
        }
        let mut command = CommandWrap::from(command);
        command.wrap(KillOnDrop);
        #[cfg(unix)]
        command.wrap(process_wrap::tokio::ProcessGroup::leader());
        #[cfg(windows)]
        command.wrap(process_wrap::tokio::JobObject);
        let mut child = command.spawn()?;
        let streams = (child.stdout().take(), child.stderr().take());
        let (Some(mut stdout), Some(mut stderr)) = streams else {
            let _ = child.start_kill();
            let status = tokio::time::timeout_at(deadline, child.wait()).await;
            return Err(format!("owned child pipes missing; reap={status:?}").into());
        };
        let mut receipt = IsolatedChildReceipt {
            pid: child.inner_mut().id(), io_error: None,
            status: None, failure: None, stdout: Vec::new(), stderr: Vec::new(),
            reaped: false, stdout_closed: false, stderr_closed: false,
        };
        let mut out = [0; 4096];
        let mut err = [0; 4096];
        loop {
            if receipt.status.is_none() {
                match child.inner_mut().try_wait() {
                    Ok(status) => { receipt.status = status; receipt.reaped = status.is_some(); },
                    Err(error) => {
                        receipt.failure = Some("child status observation failed");
                        receipt.io_error = Some(error.to_string()); break;
                    },
                }
            }
            if receipt.reaped && receipt.stdout_closed && receipt.stderr_closed { break; }
            tokio::select! {
                _ = tokio::time::sleep_until(execution_deadline) => {
                    receipt.failure = Some(if receipt.reaped { "pipe EOF deadline" } else { "child execution deadline" });
                    break;
                },
                count = stdout.read(&mut out), if !receipt.stdout_closed => {
                    let count = match count {
                        Ok(count) => count,
                        Err(error) => { receipt.failure = Some("stdout read failed"); receipt.io_error = Some(error.to_string()); break; },
                    };
                    receipt.stdout_closed = count == 0;
                    if receipt.stdout.len().saturating_add(count) > CHILD_OUTPUT_LIMIT {
                        receipt.failure = Some("stdout quota"); break;
                    }
                    receipt.stdout.extend_from_slice(&out[..count]);
                },
                count = stderr.read(&mut err), if !receipt.stderr_closed => {
                    let count = match count {
                        Ok(count) => count,
                        Err(error) => { receipt.failure = Some("stderr read failed"); receipt.io_error = Some(error.to_string()); break; },
                    };
                    receipt.stderr_closed = count == 0;
                    if receipt.stderr.len().saturating_add(count) > CHILD_OUTPUT_LIMIT {
                        receipt.failure = Some("stderr quota"); break;
                    }
                    receipt.stderr.extend_from_slice(&err[..count]);
                },
                _ = tokio::time::sleep(Duration::from_millis(10)) => {},
            }
        }
        // Reap before signalling: a macOS zombie-only process group can return EPERM.
        if receipt.status.is_none() {
            match child.inner_mut().try_wait() {
                Ok(status) => { receipt.status = status; receipt.reaped = status.is_some(); },
                Err(error) => { receipt.failure = Some("child status observation failed"); receipt.io_error = Some(error.to_string()); },
            }
        }
        if let Err(error) = child.start_kill() {
            #[cfg(unix)]
            let absent = error.raw_os_error() == Some(nix::errno::Errno::ESRCH as i32);
            #[cfg(not(unix))]
            let absent = error.kind() == std::io::ErrorKind::NotFound;
            if !absent { receipt.failure = Some("owned child stop failed"); receipt.io_error = Some(error.to_string()); }
        }
        match tokio::time::timeout_at(deadline, child.wait()).await {
            Ok(Ok(status)) => { receipt.status = Some(status); receipt.reaped = true; },
            Ok(Err(error)) => { receipt.failure = Some("owned child reap failed"); receipt.io_error = Some(error.to_string()); },
            Err(_) => receipt.failure = Some("owned child reap deadline"),
        }
        // No detached reader threads. Held inherited pipes stay under this same deadline.
        while !(receipt.stdout_closed && receipt.stderr_closed) {
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => {
                    receipt.failure = Some("owned pipe cleanup deadline"); break;
                },
                count = stdout.read(&mut out), if !receipt.stdout_closed => {
                    let count = match count {
                        Ok(count) => count,
                        Err(error) => { receipt.failure = Some("stdout cleanup read failed"); receipt.io_error = Some(error.to_string()); break; },
                    }; receipt.stdout_closed = count == 0;
                    let room = CHILD_OUTPUT_LIMIT - receipt.stdout.len();
                    receipt.stdout.extend_from_slice(&out[..count.min(room)]);
                },
                count = stderr.read(&mut err), if !receipt.stderr_closed => {
                    let count = match count {
                        Ok(count) => count,
                        Err(error) => { receipt.failure = Some("stderr cleanup read failed"); receipt.io_error = Some(error.to_string()); break; },
                    }; receipt.stderr_closed = count == 0;
                    let room = CHILD_OUTPUT_LIMIT - receipt.stderr.len();
                    receipt.stderr.extend_from_slice(&err[..count.min(room)]);
                },
            }
        }
        if receipt.failure.is_none() && !receipt.status.is_some_and(|status| status.success()) {
            receipt.failure = Some("child nonzero exit");
        }
        if receipt.failure.is_none() && !String::from_utf8_lossy(&receipt.stdout).contains(CHILD_RECEIPT) {
            receipt.failure = Some("real HTTP completion receipt missing");
        }
        Ok(receipt)
    })
}

#[test]
fn explicit_proxy_ignores_environment_exclusions_in_isolated_process() -> TestResult {
    let receipt = isolated_child(None, Duration::from_secs(8))?;
    assert!(
        receipt.success(),
        "isolated proxy fixture failed: {}",
        receipt.diagnostic()
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn windows_proxy_child_compares_cleared_and_explicit_system_root() -> TestResult {
    use std::io::Write;

    let cleared = isolated_child_environment(None, Duration::from_secs(8), false)?;
    assert!(cleared.reaped && cleared.stdout_closed && cleared.stderr_closed);
    let cleared_result = if cleared.success() {
        // An absolute provider catalog can work without this variable. Report the
        // observed control, rather than assuming every Windows installation fails.
        "real-http-completed"
    } else {
        assert_eq!(cleared.failure, Some("child nonzero exit"));
        assert!(
            String::from_utf8_lossy(&cleared.stderr).contains("code: 10106"),
            "unexpected cleared-environment control: {}",
            cleared.diagnostic()
        );
        "provider-init-10106"
    };
    let supported = isolated_child(None, Duration::from_secs(8))?;
    assert!(supported.success(), "{}", supported.diagnostic());
    // A bounded, static receipt bypasses libtest's success-output capture so CI
    // records which control actually occurred; no OS path or environment value.
    writeln!(
        std::io::stderr(),
        "Windows proxy environment control: cleared={cleared_result}; explicit-SystemRoot=real-http-completed; both-reaped-and-EOF"
    )?;
    Ok(())
}

#[test]
fn explicit_proxy_environment_child() -> TestResult {
    if std::env::var_os("KEELSHELL_REQUEST_OPTIONS_CHILD").is_none() {
        return Ok(());
    }
    if let Ok(mode) = std::env::var("KEELSHELL_REQUEST_OPTIONS_DIAGNOSTIC_MODE") {
        match mode.as_str() {
            "stub" => return Ok(()),
            "failure" => {
                eprintln!("synthetic owned child failure detail");
                return Err("controlled child failure".into());
            }
            "park" | "pipe-holder" => {
                println!("synthetic owned child holding");
                loop {
                    std::thread::park_timeout(Duration::from_secs(1));
                }
            }
            "flood" => {
                use std::io::Write;
                let bytes = [b'x'; 4096];
                loop {
                    std::io::stdout().write_all(&bytes)?;
                    std::io::stderr().write_all(&bytes)?;
                }
            }
            "held-pipes" => {
                std::process::Command::new(std::env::current_exe()?)
                    .args(["--exact", "explicit_proxy_environment_child", "--nocapture"])
                    .env_clear()
                    .env("KEELSHELL_REQUEST_OPTIONS_CHILD", "1")
                    .env("KEELSHELL_REQUEST_OPTIONS_DIAGNOSTIC_MODE", "pipe-holder")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .spawn()?;
                return Ok(());
            }
            _ => return Err("unknown controlled child mode".into()),
        }
    }
    assert_eq!(std::env::var("KEELSHELL_REQUEST_OPTIONS_CHILD")?, "1");
    for name in ["NO_PROXY", "no_proxy"] {
        assert_eq!(std::env::var(name)?, "*");
    }
    for name in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"] {
        assert_eq!(std::env::var(name)?, "http://127.0.0.1:9");
    }
    explicit_http_proxy_receives_authentication_and_origin_is_never_contacted()?;
    println!("{CHILD_RECEIPT}");
    Ok(())
}

#[test]
fn isolated_proxy_failure_diagnostics_preserve_child_status_and_reap() -> TestResult {
    let receipt = isolated_child(Some("failure"), Duration::from_secs(8))?;
    assert!(!receipt.success());
    assert!(receipt.status.is_some_and(|status| !status.success()));
    assert_eq!(receipt.failure, Some("child nonzero exit"));
    assert!(
        String::from_utf8_lossy(&receipt.stderr).contains("synthetic owned child failure detail")
    );
    assert!(receipt.reaped && receipt.stdout_closed && receipt.stderr_closed);
    Ok(())
}

#[test]
fn isolated_proxy_timeout_stops_reaps_and_closes_owned_pipes() -> TestResult {
    let receipt = isolated_child(Some("park"), Duration::from_secs(8))?;
    assert_eq!(receipt.failure, Some("child execution deadline"));
    assert!(receipt.reaped && receipt.stdout_closed && receipt.stderr_closed);
    Ok(())
}

#[test]
fn isolated_proxy_output_quota_is_bounded_and_child_is_reaped() -> TestResult {
    let receipt = isolated_child(Some("flood"), Duration::from_secs(8))?;
    assert!(matches!(
        receipt.failure,
        Some("stdout quota" | "stderr quota")
    ));
    assert!(
        receipt.stdout.len() <= CHILD_OUTPUT_LIMIT && receipt.stderr.len() <= CHILD_OUTPUT_LIMIT
    );
    assert!(receipt.reaped && receipt.stdout_closed && receipt.stderr_closed);
    Ok(())
}

#[test]
fn isolated_proxy_inherited_pipes_are_closed_by_owned_cleanup() -> TestResult {
    let receipt = isolated_child(Some("held-pipes"), Duration::from_secs(8))?;
    assert_eq!(receipt.failure, Some("pipe EOF deadline"));
    assert!(receipt.status.is_some_and(|status| status.success()));
    assert!(receipt.reaped && receipt.stdout_closed && receipt.stderr_closed);
    Ok(())
}

#[test]
fn isolated_proxy_stub_success_does_not_count_as_real_fixture_completion() -> TestResult {
    let receipt = isolated_child(Some("stub"), Duration::from_secs(8))?;
    assert!(receipt.status.is_some_and(|status| status.success()));
    assert_eq!(
        receipt.failure,
        Some("real HTTP completion receipt missing")
    );
    assert!(!receipt.success());
    Ok(())
}

fn options(route: ProxyRoute) -> Result<RequestOptions, AiError> {
    RequestOptions::new(
        vec![("x-project-key".into(), Zeroizing::new(VALUE.into()))],
        route,
    )
}

async fn read(stream: &mut TcpStream) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut data = Vec::new();
    let mut byte = [0];
    while !data.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).await?;
        data.push(byte[0]);
        if data.len() > 65536 {
            return Err("fixture header limit".into());
        }
    }
    let header = String::from_utf8(data)?;
    let length: usize = header
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(str::trim)
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0);
    if length > 65536 {
        return Err("fixture body limit".into());
    }
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await?;
    Ok(format!("{header}{}", String::from_utf8(body)?))
}

async fn reply(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
    location: &str,
) -> std::io::Result<()> {
    stream.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{location}\r\n{body}", body.len()).as_bytes()).await
}

#[tokio::test]
async fn direct_discovery_test_and_ask_send_same_headers_and_redact_values() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/chat/completions", listener.local_addr()?);
    let task = tokio::spawn(async move {
        for index in 0..3 {
            let (mut stream, _) = listener.accept().await?;
            let request = read(&mut stream).await?;
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains(&format!("x-project-key: {VALUE}"))
            );
            assert!(!request.contains("proxy-authorization"));
            let body = if index == 0 {
                r#"{"data":[{"id":"fixture-model"}]}"#.to_owned()
            } else {
                format!(
                    r#"{{"model":"{VALUE}","choices":[{{"message":{{"content":"OK {VALUE}"}}}}]}}"#
                )
            };
            reply(&mut stream, 200, &body, "").await?;
        }
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    let options = options(ProxyRoute::Direct)?;
    let provider =
        ProviderConfig::new(&endpoint, "fixture-model")?.with_request_options(options.clone());
    let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
    let cancel = RequestCancellation::new();
    assert_eq!(
        client
            .discover_models(&ProviderEndpoint::new(&endpoint)?, None, &cancel)
            .await?
            .models(),
        ["fixture-model"]
    );
    assert!(
        client
            .test_connection(&provider, None, &cancel)
            .await?
            .actual_model()
            .is_none()
    );
    let request = ContextDraft::new(format!("Explain {VALUE}")).prepare(&provider, &[], 1024)?;
    assert!(!request.preview_json().contains(VALUE));
    assert!(
        !client
            .send_approved(request.approve(), None, &cancel)
            .await?
            .text()
            .contains(VALUE)
    );
    timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    Ok(())
}

#[tokio::test]
async fn explicit_http_proxy_receives_authentication_and_origin_is_never_contacted() -> TestResult {
    let origin = TcpListener::bind("127.0.0.1:0").await?;
    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/chat/completions", origin.local_addr()?);
    let proxy_url = format!("http://{}", proxy.local_addr()?);
    let options = options(ProxyRoute::explicit(
        &proxy_url,
        Some(ProxyCredentials::new(
            Zeroizing::new("fixture-user".into()),
            Zeroizing::new("fixture-password".into()),
        )?),
    )?)?;
    let task = tokio::spawn(async move {
        for index in 0..3 {
            let (mut stream, _) = proxy.accept().await?;
            let request = read(&mut stream).await?;
            assert!(request.starts_with(if index == 0 {
                "GET http://127.0.0.1:"
            } else {
                "POST http://127.0.0.1:"
            }));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("proxy-authorization: basic ")
            );
            assert!(request.contains(VALUE));
            let body = if index == 0 {
                r#"{"data":[{"id":"fixture-model"}]}"#
            } else {
                r#"{"choices":[{"message":{"content":"fixture-user fixture-password synthetic-header-secret Zml4dHVyZS11c2VyOmZpeHR1cmUtcGFzc3dvcmQ="}}]}"#
            };
            reply(&mut stream, 200, body, "").await?;
        }
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    let provider =
        ProviderConfig::new(&endpoint, "fixture-model")?.with_request_options(options.clone());
    let prepared = ContextDraft::new("Explain").prepare(&provider, &[], 1024)?;
    let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
    let cancellation = RequestCancellation::new();
    assert_eq!(
        client
            .discover_models(&ProviderEndpoint::new(&endpoint)?, None, &cancellation)
            .await?
            .models(),
        ["fixture-model"]
    );
    client
        .test_connection(&provider, None, &cancellation)
        .await?;
    let answer = client
        .send_approved(prepared.approve(), None, &RequestCancellation::new())
        .await?;
    assert!(!answer.text().contains("fixture-user"));
    assert!(!answer.text().contains("fixture-password"));
    assert!(!answer.text().contains(VALUE));
    assert!(
        !answer
            .text()
            .contains("Zml4dHVyZS11c2VyOmZpeHR1cmUtcGFzc3dvcmQ=")
    );
    assert!(
        timeout(Duration::from_millis(80), origin.accept())
            .await
            .is_err()
    );
    timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    Ok(())
}

#[tokio::test]
async fn failed_proxy_and_changed_options_never_fall_back_to_origin() -> TestResult {
    let origin = TcpListener::bind("127.0.0.1:0").await?;
    let closed = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("http://{}", closed.local_addr()?);
    drop(closed);
    let endpoint = format!("http://{}/v1/chat/completions", origin.local_addr()?);
    let options = options(ProxyRoute::explicit(&proxy_url, None)?)?;
    let provider = ProviderConfig::new(&endpoint, "model")?.with_request_options(options.clone());
    let prepared = ContextDraft::new("Explain").prepare(&provider, &[], 1024)?;
    let direct = ProviderClient::new(LIMIT, 65536)?;
    assert_eq!(
        direct
            .send_approved(prepared.approve(), None, &RequestCancellation::new())
            .await
            .err(),
        Some(AiError::RequestOptionsMismatch)
    );
    let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
    assert!(matches!(
        client
            .test_connection(&provider, None, &RequestCancellation::new())
            .await,
        Err(AiError::Transport)
    ));
    assert!(
        timeout(Duration::from_millis(80), origin.accept())
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn proxy_redirect_rejected_and_cancelled_before_network() -> TestResult {
    let origin = TcpListener::bind("127.0.0.1:0").await?;
    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/chat/completions", origin.local_addr()?);
    let options = options(ProxyRoute::explicit(
        &format!("http://{}", proxy.local_addr()?),
        None,
    )?)?;
    let location = format!("Location: {endpoint}\r\n");
    let task = tokio::spawn(async move {
        let (mut stream, _) = proxy.accept().await?;
        read(&mut stream).await?;
        reply(&mut stream, 307, "hidden body", &location).await?;
        assert!(
            timeout(Duration::from_millis(150), proxy.accept())
                .await
                .is_err()
        );
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    let provider = ProviderConfig::new(&endpoint, "model")?.with_request_options(options.clone());
    let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
    let cancel = RequestCancellation::new();
    cancel.cancel();
    assert_eq!(
        client.test_connection(&provider, None, &cancel).await.err(),
        Some(AiError::Cancelled)
    );
    assert_eq!(
        client
            .test_connection(&provider, None, &RequestCancellation::new())
            .await
            .err(),
        Some(AiError::HttpStatus(307))
    );
    assert!(
        timeout(Duration::from_millis(80), origin.accept())
            .await
            .is_err()
    );
    timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    Ok(())
}

#[tokio::test]
async fn socks5_route_is_used_for_real_http_bytes() -> TestResult {
    let origin = TcpListener::bind("127.0.0.1:0").await?;
    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/chat/completions", origin.local_addr()?);
    let options = options(ProxyRoute::explicit(
        &format!("socks5://{}", proxy.local_addr()?),
        None,
    )?)?;
    let task = tokio::spawn(async move {
        let (mut stream, _) = proxy.accept().await?;
        let mut greeting = [0; 2];
        stream.read_exact(&mut greeting).await?;
        assert_eq!(greeting[0], 5);
        let mut methods = vec![0; greeting[1] as usize];
        stream.read_exact(&mut methods).await?;
        stream.write_all(&[5, 0]).await?;
        let mut command = [0; 4];
        stream.read_exact(&mut command).await?;
        assert_eq!(command, [5, 1, 0, 1]);
        let mut address = [0; 6];
        stream.read_exact(&mut address).await?;
        assert_eq!(&address[..4], &[127, 0, 0, 1]);
        stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).await?;
        let request = read(&mut stream).await?;
        assert!(request.starts_with("POST /v1/chat/completions"));
        assert!(request.contains(VALUE));
        reply(
            &mut stream,
            200,
            r#"{"choices":[{"message":{"content":"OK"}}]}"#,
            "",
        )
        .await?;
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    let provider = ProviderConfig::new(&endpoint, "model")?.with_request_options(options.clone());
    let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
    client
        .test_connection(&provider, None, &RequestCancellation::new())
        .await?;
    assert!(
        timeout(Duration::from_millis(80), origin.accept())
            .await
            .is_err()
    );
    timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    Ok(())
}

#[test]
fn values_controls_bounds_reserved_names_and_debug_are_fail_closed() -> TestResult {
    for name in [
        "Authorization",
        "x-api-key",
        "anthropic-version",
        "Host",
        "Content-Type",
        "Content-Length",
        "Connection",
        "Proxy-Authorization",
        "x-forwarded-for",
    ] {
        assert!(
            RequestOptions::new(
                vec![(name.into(), Zeroizing::new(VALUE.into()))],
                ProxyRoute::Direct
            )
            .is_err()
        );
    }
    for value in ["value\r\nHost: injected", "nul\0value", "tab\tvalue"] {
        assert!(
            RequestOptions::new(
                vec![("x-project".into(), Zeroizing::new(value.into()))],
                ProxyRoute::Direct
            )
            .is_err()
        );
    }
    assert!(
        RequestOptions::new(
            vec![("x-project".into(), Zeroizing::new("x".repeat(8193)))],
            ProxyRoute::Direct
        )
        .is_err()
    );
    assert!(
        RequestOptions::new(
            vec![
                ("x-project".into(), Zeroizing::new(VALUE.into())),
                ("X-Project".into(), Zeroizing::new(VALUE.into()))
            ],
            ProxyRoute::Direct
        )
        .is_err()
    );
    for route in [
        "http://user:pass@localhost:8888",
        "http://localhost:8888/path",
        "http://localhost:8888?x=y",
        "http://localhost:8888#frag",
    ] {
        assert!(ProxyRoute::explicit(route, None).is_err());
    }
    assert!(!format!("{:?}", options(ProxyRoute::Direct)?).contains(VALUE));
    let _ = ProviderProtocol::Responses;
    Ok(())
}

#[tokio::test]
async fn cancellation_after_proxy_admission_discards_late_response_without_origin_fallback()
-> TestResult {
    let origin = TcpListener::bind("127.0.0.1:0").await?;
    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/chat/completions", origin.local_addr()?);
    let options = options(ProxyRoute::explicit(
        &format!("http://{}", proxy.local_addr()?),
        None,
    )?)?;
    let (sent, admitted) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut stream, _) = proxy.accept().await?;
        let request = read(&mut stream).await?;
        assert!(request.contains(VALUE));
        stream
            .write_all(b"HTTP/1.1 200 Fixture\r\nContent-Length: 1000\r\n\r\n")
            .await?;
        let _ = sent.send(());
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = stream.write_all(VALUE.as_bytes()).await;
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    let provider = ProviderConfig::new(&endpoint, "model")?.with_request_options(options.clone());
    let client = ProviderClient::new_with_options(LIMIT, 65536, options)?;
    let cancellation = RequestCancellation::new();
    let worker_cancel = cancellation.clone();
    let worker = tokio::spawn(async move {
        client
            .test_connection(&provider, None, &worker_cancel)
            .await
    });
    timeout(LIMIT, admitted).await??;
    cancellation.cancel();
    assert_eq!(
        timeout(LIMIT, worker).await??.err(),
        Some(AiError::Cancelled)
    );
    assert!(
        timeout(Duration::from_millis(80), origin.accept())
            .await
            .is_err()
    );
    timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    Ok(())
}

#[tokio::test]
async fn discovery_response_budget_is_cumulative_across_pages() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/messages", listener.local_addr()?);
    let first = r#"{"data":[{"id":"first"}],"has_more":true,"last_id":"first"}"#;
    let limit = first.len() + 10;
    let task = tokio::spawn(async move {
        for page in 0..2 {
            let (mut stream, _) = listener.accept().await?;
            read(&mut stream).await?;
            let body = if page == 0 {
                first
            } else {
                r#"{"data":[{"id":"second"}],"has_more":false}"#
            };
            reply(&mut stream, 200, body, "").await?;
        }
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    let client = ProviderClient::new_with_options(LIMIT, limit, options(ProxyRoute::Direct)?)?;
    assert_eq!(
        client
            .discover_models(
                &ProviderEndpoint::new_with_protocol(
                    &endpoint,
                    ProviderProtocol::AnthropicMessages
                )?,
                None,
                &RequestCancellation::new()
            )
            .await
            .err(),
        Some(AiError::ResponseTooLarge)
    );
    timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    Ok(())
}

#[tokio::test]
async fn anthropic_pages_keep_options_and_reject_secret_cursor() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/messages", listener.local_addr()?);
    let task = tokio::spawn(async move {
        for page in 0..2 {
            let (mut stream, _) = listener.accept().await?;
            let request = read(&mut stream).await?;
            assert!(request.contains(VALUE));
            assert!(request.contains("anthropic-version: 2023-06-01"));
            assert!(request.contains("x-api-key: synthetic-auth-key"));
            if page == 1 {
                assert!(request.contains("after_id=first-model"));
            }
            let body = if page == 0 {
                r#"{"data":[{"id":"first-model"}],"has_more":true,"last_id":"first-model"}"#
                    .to_owned()
            } else {
                format!(r#"{{"data":[{{"id":"last-model"}}],"has_more":true,"last_id":"{VALUE}"}}"#)
            };
            reply(&mut stream, 200, &body, "").await?;
        }
        assert!(
            timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    let client = ProviderClient::new_with_options(LIMIT, 65536, options(ProxyRoute::Direct)?)?;
    let error = client
        .discover_models(
            &ProviderEndpoint::new_with_protocol(&endpoint, ProviderProtocol::AnthropicMessages)?,
            Some("synthetic-auth-key"),
            &RequestCancellation::new(),
        )
        .await
        .err();
    assert_eq!(error, Some(AiError::InvalidModelCatalog));
    timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    Ok(())
}

#[tokio::test]
async fn socks5h_authentication_and_proxy_dns_are_explicit() -> TestResult {
    let origin = TcpListener::bind("127.0.0.1:0").await?;
    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!(
        "http://localhost:{}/v1/responses",
        origin.local_addr()?.port()
    );
    let credentials = ProxyCredentials::new(
        Zeroizing::new("fixture-user".into()),
        Zeroizing::new("fixture-password".into()),
    )?;
    let options = options(ProxyRoute::explicit(
        &format!("socks5h://{}", proxy.local_addr()?),
        Some(credentials),
    )?)?;
    let task = tokio::spawn(async move {
        let (mut stream, _) = proxy.accept().await?;
        let mut greeting = [0; 2];
        stream.read_exact(&mut greeting).await?;
        let mut methods = vec![0; greeting[1] as usize];
        stream.read_exact(&mut methods).await?;
        assert!(methods.contains(&2));
        stream.write_all(&[5, 2]).await?;
        let mut auth = [0; 2];
        stream.read_exact(&mut auth).await?;
        assert_eq!(auth[0], 1);
        let mut username = vec![0; auth[1] as usize];
        stream.read_exact(&mut username).await?;
        assert_eq!(username, b"fixture-user");
        let password_len = stream.read_u8().await?;
        let mut password = vec![0; password_len as usize];
        stream.read_exact(&mut password).await?;
        assert_eq!(password, b"fixture-password");
        stream.write_all(&[1, 0]).await?;
        let mut connect = [0; 4];
        stream.read_exact(&mut connect).await?;
        assert_eq!(connect, [5, 1, 0, 3]);
        let length = stream.read_u8().await?;
        let mut name = vec![0; length as usize];
        stream.read_exact(&mut name).await?;
        assert_eq!(name, b"localhost");
        let _port = stream.read_u16().await?;
        stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).await?;
        let request = read(&mut stream).await?;
        assert!(request.starts_with("POST /v1/responses"));
        assert!(request.contains(VALUE));
        assert!(!request.contains("fixture-password"));
        reply(
            &mut stream,
            200,
            r#"{"output":[{"type":"message","content":[{"type":"output_text","text":"OK"}]}]}"#,
            "",
        )
        .await?;
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    let provider =
        ProviderConfig::new_with_protocol(&endpoint, "model", ProviderProtocol::Responses)?
            .with_request_options(options.clone());
    ProviderClient::new_with_options(LIMIT, 65536, options)?
        .test_connection(&provider, None, &RequestCancellation::new())
        .await?;
    assert!(
        timeout(Duration::from_millis(80), origin.accept())
            .await
            .is_err()
    );
    timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    Ok(())
}

#[tokio::test]
async fn https_proxy_starts_tls_and_never_exposes_credentials_in_plaintext_or_falls_back()
-> TestResult {
    let origin = TcpListener::bind("127.0.0.1:0").await?;
    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/v1/chat/completions", origin.local_addr()?);
    let credentials = ProxyCredentials::new(
        Zeroizing::new("fixture-user".into()),
        Zeroizing::new("fixture-password".into()),
    )?;
    let options = options(ProxyRoute::explicit(
        &format!("https://{}", proxy.local_addr()?),
        Some(credentials),
    )?)?;
    let task = tokio::spawn(async move {
        let (mut stream, _) = proxy.accept().await?;
        let mut record = [0; 5];
        stream.read_exact(&mut record).await?;
        assert_eq!(record[0], 0x16);
        assert_eq!(record[1], 0x03);
        let length = u16::from_be_bytes([record[3], record[4]]) as usize;
        assert!(length < 65536);
        let mut hello = vec![0; length];
        stream.read_exact(&mut hello).await?;
        assert!(
            !hello
                .windows(b"fixture-password".len())
                .any(|bytes| bytes == b"fixture-password")
        );
        stream.shutdown().await?;
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });
    let provider = ProviderConfig::new(&endpoint, "model")?.with_request_options(options.clone());
    assert_eq!(
        ProviderClient::new_with_options(LIMIT, 65536, options)?
            .test_connection(&provider, None, &RequestCancellation::new())
            .await
            .err(),
        Some(AiError::Transport)
    );
    assert!(
        timeout(Duration::from_millis(80), origin.accept())
            .await
            .is_err()
    );
    timeout(LIMIT, task)
        .await?
        .map_err(|_| "fixture panic")?
        .map_err(|_| "fixture failed")?;
    Ok(())
}
