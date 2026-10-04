# AI transport contract

The crate implements three explicit wire formats: OpenAI-compatible Chat
Completions, the OpenAI Responses API, and Anthropic Messages. OpenAI-compatible
requests use explicit Bearer or no authentication. Anthropic requests use
`x-api-key` and always send `anthropic-version: 2023-06-01`; they never send the
key as a Bearer token. Named configuration metadata belongs in `keelshell-core`;
the transport does not load profiles, resolve credential references, read
environment variables for credentials, or execute local provider processes.

`ProviderEndpoint::new` validates a complete provider URL independently of model
selection. `models_endpoint()` replaces only a literal `/chat/completions`,
`/responses`, or Anthropic `/messages` suffix with `/models`, keeping the scheme,
host, port and prefix. Anthropic model listing follows the provider's bounded
cursor pages (`limit=1000`, `after_id`) and rejects missing cursors, repeated
cursors, more than 4096 IDs, or more than 64 pages instead of silently
returning an incomplete catalog.
A custom endpoint which does not support this convention fails without guessing
another route; a manually entered model remains possible.

`ProviderConfig::new` retains Chat Completions as the compatibility default.
`ProviderConfig::new_with_protocol` binds one protocol to the immutable preview.
Responses previews use `instructions`, `input`, `model` and `stream: false`; the
parser accepts only closed `message` items containing `output_text`. Anthropic
previews use `model`, `system`, `messages`, `max_tokens` (4096 by default) and
`stream: false`. `ContextDraft::prepare_with_limits` accepts optional output and
declared context limits. Output limits are serialized as `max_completion_tokens`
for Chat Completions, `max_output_tokens` for Responses, and `max_tokens` for
Messages, bounded to 1–1,000,000. Compatible endpoints must support the selected
field; the transport never retries with another field after rejection. An
omitted limit keeps the old request shape, except Messages still requires 4096.
A declared context window reserves output (4096 if omitted), system text and
1024 framing units, then applies a conservative UTF-8 byte budget with worst-case
JSON escaping. This local heuristic is not a provider tokenizer or a measured
token count. The complete question must fit; selected context may be truncated
with omitted bytes shown in the review report. Its parser requires an assistant message and accepts
only `text` blocks. Thinking and tool blocks are never converted to displayed
text, and a response containing only those blocks is rejected. No tools, function
calls, remote files or autonomous actions are enabled by these adapters.

`ProviderClient` provides three asynchronous operations:

- `discover_models` sends a same-origin GET and returns sorted, deduplicated
  provider model identifiers. The bounded response must contain at most 4096
  identifiers, each satisfying the same validation as `ProviderConfig::model`.
- `test_connection` sends only `CONNECTIVITY_PROMPT`, the selected model, and
  `stream: false` in the selected protocol (`messages` for Chat Completions,
  `input` for Responses). It is a billable provider request and must only be
  called after an explicit user action. Its report contains elapsed time and the
  server's optional actual model field. Missing actual-model metadata must stay
  visibly missing rather than being replaced with the user's requested model.
  `test_connection_with_limits` uses the same explicit output fields and local
  context admission policy, with no terminal context.
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
fixed probe bodies, Anthropic headers and paginated model pages, immutable approvals,
malformed models, response limits,
status categories, redirects, header/body timeouts, and cancellation. Proxy
environment isolation uses a bounded subprocess, avoiding unsafe process-wide
environment changes in a parallel Rust test process. These tests are transport
evidence only; they do not prove availability of a public provider account or
native desktop interaction.
