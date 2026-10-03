# AI transport contract

The crate implements the Chat Completions wire format with explicit Bearer or no
authentication. Named configuration metadata belongs in `keelshell-core`; the
transport does not load profiles, resolve credential references, read environment
variables for credentials, or execute local provider processes.

`ProviderEndpoint::new` validates a complete chat URL independently of model
selection. `models_endpoint()` replaces only the literal `/chat/completions`
suffix with `/models`, keeping the scheme, host, port and prefix. A custom
endpoint which does not support this convention fails without guessing another
route; a manually entered model remains possible.

`ProviderClient` provides three asynchronous operations:

- `discover_models` sends a same-origin GET and returns sorted, deduplicated
  provider model identifiers. The bounded response must contain at most 4096
  identifiers, each satisfying the same validation as `ProviderConfig::model`.
- `test_connection` sends only `CONNECTIVITY_PROMPT`, the selected model, and
  `stream: false`. It is a billable provider request and must only be called after
  an explicit user action. Its report contains elapsed time and the server's
  optional actual model field. Missing actual-model metadata must stay visibly
  missing rather than being replaced with the user's requested model.
- `send_approved` consumes one `ApprovedRequest`, sending its exact prepared JSON
  and provider snapshot. Changing a saved profile cannot alter that snapshot.
  The existing blocking `AiClient::send` remains supported off the UI thread.

All three async operations share one client policy: verified TLS outside
loopback HTTP, no redirects, no retries, no environment proxies, bounded response
bodies, and a total deadline including response headers and body. Keys are
borrowed per call and marked sensitive in HTTP headers. Raw error bodies and
transport errors are never included in public diagnostics. `AiError::category`
provides stable classifications for localization; it does not authorize retry.

Every async operation observes a `RequestCancellation`. Cancellation drops the
local HTTP future and never waits for a detached blocking request. A request
already accepted by the provider may still be processed or charged. The caller
owns request revisions and must discard results from configurations superseded
while a request was running. A cancellation token is one-way; each new operation
needs a fresh token.

`tests/discovery_http.rs` uses bounded loopback HTTP fixtures for exact paths,
fixed probe bodies, immutable approvals, malformed models, response limits,
status categories, redirects, header/body timeouts, and cancellation. Proxy
environment isolation uses a bounded subprocess, avoiding unsafe process-wide
environment changes in a parallel Rust test process. These tests are transport
evidence only; they do not prove availability of a public provider account or
native desktop interaction.
