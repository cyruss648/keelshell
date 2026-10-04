# ADR 0033: AI Token limits and local context admission

- Status: accepted
- Date: 2026-10-04

## Decision

Named AI configurations expose optional output limits and a declared context
window. Values remain non-sensitive profile metadata. The supported output
range is 1–1,000,000; the context range is 1–16,777,216. The selected model must
actually accept the configured value; KeelShell does not infer capabilities from
the model name or make an automatic fallback request.

The wire field is explicit by protocol: Chat Completions uses
`max_completion_tokens`, Responses uses `max_output_tokens`, and Anthropic
Messages uses `max_tokens`. With no configured limits, existing request bodies
remain compatible; Messages keeps its required default 4096. Supplying a context
window also serializes an output reserve of 4096 when no output limit was set, so
the displayed review and provider request have the same reservation.

`ContextDraft::prepare_with_limits` reserves output plus system text and 1024
framing units. The remaining local budget is reduced to a conservative UTF-8
byte capacity with sixfold worst-case JSON escaping. It checks the final
serialized user context again. This is an admission heuristic, not a tokenizer
measurement or a guarantee about arbitrary compatible providers. The complete
question is never truncated; optional context is truncated only at character
boundaries and every omitted byte is counted in the existing redaction report.
Impossible limits fail before producing an approval or opening a socket.

The immutable reviewed body contains the limit. Settings changes cancel current
discovery/test futures and advance their revision. Profile changes in the
assistant revoke the prepared body and reject late results. Invalid numeric
drafts survive profile selection and block Apply/test/discovery rather than
silently replacing the saved numeric metadata. Edits during a pending save
remain governed by the existing exact revision acknowledgement.

The editor form has a natural content height inside a separate scroll viewport.
Advanced fields increase scrollable content instead of shrinking the model
discovery result container or changing the hit areas of its selection buttons.

Fixed connectivity tests honor the same protocol output fields and local
context admission policy. Model discovery is metadata-only and sends no output
limit. Custom headers, explicit proxy routes and non-default reasoning selections
remain rejected by the current transport; this decision does not enable them.
Header values still have no plaintext profile representation.

## Protocol evidence

Checked official public references on 2026-10-04:

- [OpenAI Python Chat Completions parameter contract](https://github.com/openai/openai-python/blob/main/src/openai/resources/chat/completions/completions.py): `max_completion_tokens` is the modern completion limit.
- [OpenAI token counting](https://developers.openai.com/api/docs/guides/token-counting): output limits include visible and non-visible generated tokens; actual counting requires the provider's tokenizer/counting path.
- [OpenAI Responses create](https://developers.openai.com/api/reference/cli/resources/responses/methods/create): `max_output_tokens` is the response output upper bound.
- [Anthropic Messages create](https://platform.claude.com/docs/en/api/messages/create): `max_tokens` is an explicit generation limit with model-specific maxima.

## Limits

No provider account or model quota was exercised. The local budget can reject
requests that a specific tokenizer would accept. It never fetches a remote
token count, sends additional context, changes a limit after approval, or treats
a configuration as a verified model capability. Native Windows/Linux UI and
commercial provider acceptance remain separate verification work.
