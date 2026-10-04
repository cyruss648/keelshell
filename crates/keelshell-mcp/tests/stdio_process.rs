#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{process::Stdio, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
};

struct Client {
    child: Child,
    input: Option<ChildStdin>,
    output: Lines<BufReader<ChildStdout>>,
}
impl Client {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_keelshell-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap()).lines();
        Self {
            child,
            input,
            output,
        }
    }
    async fn send(&mut self, value: Value) {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        self.input
            .as_mut()
            .unwrap()
            .write_all(&bytes)
            .await
            .unwrap();
    }
    async fn read(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(3), self.output.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(line.len() < 256 * 1024);
        serde_json::from_str(&line).unwrap()
    }
    async fn initialize(&mut self) {
        self.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"independent-json-test","version":"1.0"}}})).await;
        let response = self.read().await;
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(response["result"]["serverInfo"]["name"], "keelshell-mcp");
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
    }
    async fn finish(mut self) {
        drop(self.input.take());
        let status = tokio::time::timeout(Duration::from_secs(3), self.child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(status.success());
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .await
            .unwrap();
        assert!(stderr.is_empty(), "unexpected diagnostic: {stderr:?}");
        assert!(self.output.next_line().await.unwrap().is_none());
    }
}

fn meta(version: &str) -> Value {
    json!({"io.modelcontextprotocol/protocolVersion":version,"io.modelcontextprotocol/clientInfo":{"name":"independent-json-test","version":"1.0"},"io.modelcontextprotocol/clientCapabilities":{}})
}

#[tokio::test]
async fn legacy_stdio_discovers_fixed_schemas_and_disabled_access_without_pollution() {
    let mut client = Client::spawn();
    client.initialize().await;
    client
        .send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let tools = client.read().await;
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 7);
    assert!(
        tools
            .iter()
            .all(|tool| !tool["name"].as_str().unwrap().contains("approve")
                && !tool["name"].as_str().unwrap().contains("exec"))
    );
    for tool in tools {
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    }
    let proposal = tools
        .iter()
        .find(|tool| tool["name"] == "keelshell_propose_command")
        .unwrap();
    assert_eq!(proposal["annotations"]["readOnlyHint"], false);
    assert_eq!(proposal["annotations"]["destructiveHint"], false);
    client.send(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"keelshell_list_sessions","arguments":{}}})).await;
    let response = client.read().await;
    assert_eq!(response["result"]["isError"], true);
    assert_eq!(
        response["result"]["structuredContent"]["error"]["code"],
        "DISABLED"
    );
    assert!(!response.to_string().contains("connection_id"));
    client.finish().await;
}

#[tokio::test]
async fn discover_lifecycle_requires_metadata_and_rejects_unsupported_versions() {
    let mut client = Client::spawn();
    client.send(json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":meta("2026-07-28")}})).await;
    let discovered = client.read().await;
    assert_eq!(discovered["id"], 1);
    assert!(discovered.get("error").is_none(), "{discovered}");
    client.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":meta("2026-07-28")}})).await;
    assert_eq!(
        client.read().await["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    client
        .send(json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}))
        .await;
    assert!(client.read().await["error"]["code"].is_number());
    client.send(json!({"jsonrpc":"2.0","id":4,"method":"tools/list","params":{"_meta":meta("2099-01-01")}})).await;
    assert!(client.read().await["error"]["code"].is_number());
    client.send(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"_meta":meta("2026-07-28"),"name":"keelshell_list_sessions","arguments":{}}})).await;
    assert_eq!(
        client.read().await["result"]["structuredContent"]["error"]["code"],
        "DISABLED"
    );
    client.finish().await;
}

#[tokio::test]
async fn malformed_shapes_and_unsupported_actions_have_bounded_protocol_errors() {
    let mut client = Client::spawn();
    client.initialize().await;
    client
        .input
        .as_mut()
        .unwrap()
        .write_all(b"not-json\n")
        .await
        .unwrap();
    client.send(json!({"jsonrpc":"2.0","id":2,"method":"keelshell/approve/do-not-echo-method","params":{"secret":"do-not-echo"}})).await;
    let response = client.read().await;
    assert_eq!(response["error"]["code"], -32601);
    assert!(!response.to_string().contains("do-not-echo"));
    client.send(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"keelshell_list_sessions","arguments":{"credential":"do-not-echo"}}})).await;
    let response = client.read().await;
    assert_eq!(response["error"]["code"], -32602);
    assert!(!response.to_string().contains("do-not-echo"));
    client
        .send(json!({"jsonrpc":"2.0","id":4,"method":9}))
        .await;
    assert_eq!(client.read().await["error"]["code"], -32600);
    client
        .send(
            json!({"jsonrpc":"2.0","id":5,"method":"tools/list","params":{"cursor":"unsupported"}}),
        )
        .await;
    assert_eq!(client.read().await["error"]["code"], -32602);
    client.finish().await;
}

#[tokio::test]
async fn oversized_unterminated_input_closes_process_with_static_diagnostic() {
    let mut client = Client::spawn();
    let bytes = vec![b'x'; keelshell_mcp::MAX_REQUEST_BYTES + 8192];
    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        client.input.as_mut().unwrap().write_all(&bytes),
    )
    .await
    .unwrap();
    drop(client.input.take());
    let status = tokio::time::timeout(Duration::from_secs(3), client.child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(!status.success());
    let mut stderr = String::new();
    client
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .await
        .unwrap();
    assert_eq!(stderr, "MCP startup failed or exceeded its deadline\n");
    assert!(client.output.next_line().await.unwrap().is_none());
}

#[tokio::test]
async fn arguments_are_rejected_without_echoing_private_values_or_starting_protocol() {
    let output = Command::new(env!("CARGO_BIN_EXE_keelshell-mcp"))
        .arg("private-argument")
        .kill_on_drop(true)
        .output()
        .await
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("private-argument"));
    assert!(stderr.len() < 128);
}

#[tokio::test]
async fn idle_client_cannot_keep_process_alive_after_startup_deadline() {
    let mut client = Client::spawn();
    // Keep stdin open without sending bytes; this exercises Tokio's blocking
    // stdin worker as well as the protocol startup deadline.
    let status = tokio::time::timeout(Duration::from_secs(12), client.child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(!status.success());
    let mut stderr = String::new();
    client
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .await
        .unwrap();
    assert_eq!(stderr, "MCP startup failed or exceeded its deadline\n");
    assert!(client.output.next_line().await.unwrap().is_none());
}
