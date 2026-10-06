# ADR 0054: Explicit API inference options per model

- Status: independently reviewed repaired candidate integrated; combined mainline checks and native acceptance are tracked separately.
- Date: 2026-10-06

## Problem

Named API profiles already remember model-specific reasoning capabilities, but the transport rejects every non-default selection. Users also need optional sampling controls without losing the distinction between omission and an explicitly chosen value. Model IDs returned by discovery do not establish these capabilities.

## Decision

Reuse `reasoning_by_model` for OpenAI effort and a typed Messages combination with independent optional effort and thinking (omit/adaptive/disabled/legacy manual budget). Existing stored single-field effort/thinking/budget choices retain their wire meaning and upgrade to the combined form only on a UI edit. A matching typed capability declaration admits each selected field for the exact model. Add a model-keyed `sampling_by_model` with a user support declaration, optional temperature, and optional Top P. Defaults remain absent from the wire. Choosing a reasoning setting declares support for that exact model; a separate sampling button declares sampling support. These declarations describe the user's configuration and do not certify the remote service.

Core stores decimals exactly in thousandths; plain editor decimals accept at most three fractional digits. Integer storage preserves equality, prevents NaN/infinity, and gives deterministic profile revision comparison. Explicit zero is distinct from an empty field. Temperature is 0–2, with Messages limited to 0–1; Top P is 0–1. The editor supports one sampling parameter at a time. Mixing sampling with active reasoning is rejected; omission, OpenAI `none`, and Messages omitted/disabled thinking with effort omitted are admitted. Explicit Messages effort remains incompatible with sampling under this adapter's conservative policy.

The AI crate uses closed typed enums and inserts only known protocol fields into the final immutable payload. Chat uses `reasoning_effort`, Responses uses `reasoning.effort`. Messages independently emits `output_config.effort` and `thinking.type`, with `budget_tokens` only for legacy manual thinking. Adaptive + medium and legacy budget + effort are supported together when the user has checked that exact model/combination. Effort omission preserves the thinking choice, and thinking omission preserves effort. That budget is at least 1024 and strictly below the effective output ceiling (4096 when omitted). The same mapper is used by Ask and the explicitly clicked fixed connectivity test. Discovery remains a GET without inference fields.

Profile/model draft identity keeps invalid raw values for correction. Apply and network operations refuse invalid drafts instead of using their previous typed values. Changing the destination, protocol or backend clears capability declarations and inference drafts; changing the model remembers each model independently. Inference edits participate in existing cancellation and revision invalidation. Known secrets from inactive configurations guard all persisted model selections before Apply and again before disk dispatch. For selected sampling values, admission checks both the exact integer thousandths stored in metadata and the exact JSON number emitted by the API adapter (including explicit `0.0`); selected legacy or composed Messages budgets check their integer token value. This is scoped to editable selections, not a whole-catalog JSON scan: generated UUIDs, fixed schema tags and existing numeric limits retain their previous admission behavior. Transport preparation separately rejects secret-bearing inference values instead of redacting them into a different parameter.

Local CLI profiles continue to reject explicit API inference controls. The feature does not add arbitrary JSON, provider tools, third-party MCP clients, automatic retries, or credential persistence.

## Protocol evidence

The following primary documentation was opened on 2026-10-06:

- [OpenAI Chat create](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create): effort values vary by model; sampling fields have bounded numeric ranges and should generally be varied separately.
- [OpenAI Responses create](https://developers.openai.com/api/reference/python/resources/responses/methods/create) and [reasoning guide](https://developers.openai.com/api/docs/guides/reasoning): Responses nests effort under `reasoning` and supports optional numeric sampling fields.
- [OpenAI deployment checklist](https://developers.openai.com/api/docs/guides/deployment-checklist): current reasoning models have sampling restrictions when effort is active. The adapter's incompatibility rule is deliberately explicit and is not a claim that every vendor/model has the same support matrix.
- [Claude Messages create](https://platform.claude.com/docs/en/api/messages/create): effort belongs to `output_config`; temperature/Top P are deprecated, with tighter support on newer models. The UI requires an explicit legacy-model support declaration rather than assuming availability from the protocol.
- [Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort) and [adaptive thinking](https://platform.claude.com/docs/en/build-with-claude/adaptive-thinking): effort and thinking are independent; documented combinations include adaptive thinking with effort and manual budgets with effort on supporting legacy models. Some newer models reject disabled thinking.
- [Claude extended thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking): manual budgets require a supporting model, at least 1024 tokens, and room below `max_tokens`. Newer models reject manual mode.
- [Claude steering thinking](https://platform.claude.com/docs/en/build-with-claude/thinking-steering-and-cost): adaptive thinking and effort are distinct from a fixed manual budget.

## Consequences and boundaries

An incorrect model support declaration can still produce a server rejection. Rejection is reported through the existing bounded error category; the approved payload is never silently changed or retried. Reasoning effort is guidance, not a measured thinking-token cap. Three-digit decimal precision is a documented editor/storage limit. OpenAI keeps one effort choice per model. Messages thinking and effort compose independently; budget plus effort is limited to a model declaring both capabilities and remains subject to the strict output bound. The editor does not infer model-specific disabled-thinking/effort restrictions from the model spelling; an incorrect declaration can still be rejected remotely.

Unit, real loopback HTTP, real GPUI and source gates do not prove a cloud model accepts the settings or a target-native desktop flow works. The current acceptance record states those boundaries.
