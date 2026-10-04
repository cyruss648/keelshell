//! Opt-in interoperability with installed native CLIs and owned loopback services.
//!
//! Ignored in normal CI. Set the absolute executable variable shown on each test
//! and select `--ignored`; no cloud service, user login or real API key is used.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use keelshell_ai::{
    ContextDraft, LocalAgentClient, LocalAgentConfig, LocalAgentCredential, LocalAgentKind,
    LocalAgentLimits, RequestCancellation,
};
use serde_json::{Value, json};

const ANSWER: &str = "controlled fixture answer";
const QUESTION: &str = "explicit-loopback-question";
const UNREVIEWED_CANARY: &str = "keelshell-unreviewed-ancestor-rule-canary";

struct Server {
    endpoint: String,
    requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<io::Result<()>>>,
}

impl Server {
    fn start(kind: LocalAgentKind) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "http://{}{}",
            listener.local_addr().unwrap(),
            if kind == LocalAgentKind::Codex {
                "/v1"
            } else {
                ""
            }
        );
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let ending = stop.clone();
        let recorded = requests.clone();
        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(40);
            while !ending.load(Ordering::Relaxed) && Instant::now() < deadline {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(Duration::from_secs(3)))?;
                stream.set_write_timeout(Some(Duration::from_secs(3)))?;
                let (path, request) = read_request(&mut stream)?;
                if request.is_null() {
                    recorded
                        .lock()
                        .unwrap()
                        .push(json!({"path":path,"method":"HEAD","inference":false}));
                    stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )?;
                    continue;
                }
                // No headers or credentials are recorded, even dummy ones.
                recorded.lock().unwrap().push(json!({"path":path,"tools":request.get("tools"),"model":request.get("model"),"explicit_context_present":request.to_string().contains(QUESTION),"inference":true,"unreviewed_file_canary_present":request.to_string().contains(UNREVIEWED_CANARY)}));
                let body = sse(kind);
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )?;
                stream.flush()?;
            }
            Ok(())
        });
        Self {
            endpoint,
            requests,
            stop,
            handle: Some(handle),
        }
    }

    fn close(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap().unwrap();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> io::Result<(String, Value)> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    let header_end = loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > 512 * 1024 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers =
        std::str::from_utf8(&bytes[..header_end]).map_err(|_| io::ErrorKind::InvalidData)?;
    let path = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or(io::ErrorKind::InvalidData)?
        .to_owned();
    if headers.starts_with("HEAD /api/hello ") {
        return Ok((path, Value::Null));
    }
    let length: usize = headers
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, length)| length.trim().parse().ok())
        })
        .ok_or(io::ErrorKind::InvalidData)?;
    if length > 512 * 1024 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    while bytes.len() < header_end + length {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    let value = serde_json::from_slice(&bytes[header_end..header_end + length])
        .map_err(|_| io::ErrorKind::InvalidData)?;
    Ok((path, value))
}

fn sse(kind: LocalAgentKind) -> String {
    let events = match kind {
        LocalAgentKind::Codex => {
            let item = json!({"id":"msg_fixture","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":ANSWER,"annotations":[]}]});
            vec![
                json!({"type":"response.created","response":{"id":"resp_fixture","object":"response","status":"in_progress","model":"gpt-6-sol","output":[]}}),
                json!({"type":"response.output_item.added","output_index":0,"item":{"id":"msg_fixture","type":"message","role":"assistant","status":"in_progress","content":[]}}),
                json!({"type":"response.content_part.added","item_id":"msg_fixture","output_index":0,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}),
                json!({"type":"response.output_text.delta","item_id":"msg_fixture","output_index":0,"content_index":0,"delta":ANSWER}),
                json!({"type":"response.output_text.done","item_id":"msg_fixture","output_index":0,"content_index":0,"text":ANSWER}),
                json!({"type":"response.output_item.done","output_index":0,"item":item}),
                json!({"type":"response.completed","response":{"id":"resp_fixture","object":"response","status":"completed","model":"gpt-6-sol","output":[item],"usage":{"input_tokens":1,"output_tokens":3,"total_tokens":4}}}),
            ]
        }
        LocalAgentKind::ClaudeCode => vec![
            json!({"type":"message_start","message":{"id":"msg_fixture","type":"message","role":"assistant","model":"claude-sonnet-4-6","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":0}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":ANSWER}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":3}}),
            json!({"type":"message_stop"}),
        ],
    };
    events
        .into_iter()
        .map(|value| {
            format!(
                "event: {}\ndata: {value}\n\n",
                value["type"].as_str().unwrap()
            )
        })
        .collect()
}

async fn native_cli(kind: LocalAgentKind, variable: &str) {
    let executable = PathBuf::from(
        std::env::var_os(variable)
            .expect("opt-in requires an explicitly supplied absolute native executable"),
    );
    let scratch = tempfile::tempdir().unwrap();
    for name in ["AGENTS.md", "CLAUDE.md"] {
        std::fs::write(scratch.path().join(name), UNREVIEWED_CANARY).unwrap();
    }
    let mut server = Server::start(kind);
    let model = if kind == LocalAgentKind::Codex {
        "gpt-6-sol"
    } else {
        "claude-sonnet-4-6"
    };
    let config = LocalAgentConfig::new(kind, executable, scratch.path(), model)
        .unwrap()
        .with_inference_endpoint(&server.endpoint)
        .unwrap()
        .with_limits(
            LocalAgentLimits::new(
                Duration::from_secs(30),
                2 * 1024 * 1024,
                256 * 1024,
                1024 * 1024,
                512,
            )
            .unwrap(),
        );
    let result = LocalAgentClient
        .ask(
            config
                .prepare(ContextDraft::new(QUESTION), &[], 8192)
                .unwrap()
                .approve(),
            LocalAgentCredential::new("fixture-only-not-a-credential").unwrap(),
            &RequestCancellation::new(),
        )
        .await;
    server.close();
    let reply = result.unwrap();
    assert_eq!(reply.text(), ANSWER);
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 2);
    let requests = server.requests.lock().unwrap().clone();
    let inference: Vec<_> = requests
        .iter()
        .filter(|entry| entry["inference"] == true)
        .collect();
    assert_eq!(inference.len(), 1, "exactly one local inference request");
    for entry in requests.iter().filter(|entry| entry["inference"] == false) {
        assert_eq!(entry["path"], "/api/hello");
    }
    assert_eq!(inference[0]["explicit_context_present"], true);
    assert_eq!(inference[0]["unreviewed_file_canary_present"], false);
    assert!(
        inference[0]["tools"].is_null()
            || inference[0]["tools"].as_array().is_some_and(Vec::is_empty),
        "CLI advertised no tools on actual wire"
    );
    let expected_path = if kind == LocalAgentKind::Codex {
        "/v1/responses"
    } else {
        "/v1/messages?beta=true"
    };
    assert_eq!(inference[0]["path"], expected_path);
    let receipt = json!({"kind":format!("{kind:?}"),"version":reply.version().to_string(),"frames":reply.frame_count(),"requests":requests,"complete_answer":true,"scratch_removed":true,"ancestor_rules_excluded":true,"inference_transport":"owned loopback SSE fixture","supplier_model_or_account_used":false});
    if let Some(directory) = std::env::var_os("KEELSHELL_CLI_FIXTURE_RECEIPT_DIR") {
        let path = PathBuf::from(directory);
        assert!(path.is_absolute());
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join(format!("{kind:?}.json")),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }
    println!("{receipt}");
}

#[tokio::test]
#[ignore = "opt-in: set KEELSHELL_CODEX_EXECUTABLE to installed native Codex; loopback service only"]
async fn installed_codex_ask_uses_no_tools_and_cleans_isolation() {
    native_cli(LocalAgentKind::Codex, "KEELSHELL_CODEX_EXECUTABLE").await;
}

#[tokio::test]
#[ignore = "opt-in: set KEELSHELL_CLAUDE_EXECUTABLE to installed native Claude; loopback service only"]
async fn installed_claude_ask_uses_no_tools_and_cleans_isolation() {
    native_cli(LocalAgentKind::ClaudeCode, "KEELSHELL_CLAUDE_EXECUTABLE").await;
}
