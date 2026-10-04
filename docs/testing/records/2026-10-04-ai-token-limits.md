# AI output limits and context admission — 2026-10-04

## Scope

AI settings expose optional output Token limits and a declared context window.
Request preparation maps the limit to the explicit protocol field and binds it
to the immutable JSON review. The local context policy reserves output and
framing, truncates optional UTF-8 selections, and refuses to truncate a question.
See [ADR 0033](../../adr/0033-ai-token-limits-and-local-context-admission.md).

## Verification

- `cargo test -p keelshell-ai -p keelshell-core --locked`: AI 33 unit tests, 20
  async HTTP fixture tests, 12 blocking HTTP fixture tests and 1 doctest passed.
  Core 77 unit tests, the complete integration suite and 3 doctests passed.
- `cargo clippy -p keelshell-ai -p keelshell-core --all-targets --all-features
  --locked -- -D warnings`: passed.
- New HTTP tests cover all three protocols and require byte-for-byte equality
  of the reviewed and sent body with an explicit 512-token output limit and
  declared context window. Connectivity probes verify the same exact field.
- New GPUI regression tests cover limit edits cancelling requests, invalid
  drafts surviving selection/locale changes and blocking Apply/request launch,
  native Input events cancelling requests, edits surviving pending save
  acknowledgement, and changed profile limits revoking approval/late replies.
  `cargo test -p keelshell-app --locked token_`: all 3 new GPUI tests passed.
  The full app/workspace gate is owned by the integration run after concurrent
  feature work is complete.
- After the first integration gate exposed two regressions, the targeted
  `manual_discovery_uses_real_loopback_http_and_selection_updates_only_model`,
  `unsupported_configuration_and_missing_bearer_key_never_prepare`, and
  `token_` runs passed (1 + 1 + 3 tests). The discovery test performs a real
  native pointer dispatch on the model button, verifies a minimum 28 px hit
  area, and verifies the selected model changes without changing the endpoint.

## Preserved intermediate failures

The first AI/core test pass failed the historical core assertion that an
Anthropic profile with `max_output_tokens` must be rejected. That assertion was
updated to the newly supported contract and supplemented with transport-limit
and legacy-projection tests. The rerun passed all 77 core unit tests.

An intermediate `cargo fmt --all` and app compile could not resolve
`files/sync.rs` while that separately owned module was being created. No AI
file caused that error. The AI-owned files were formatted directly using
`rustfmt --edition 2024 --config skip_children=true`; full integration formatting
and app tests remain required once the module exists.

The first app test compile used the nonexistent `Language::English` test enum
variant. It was corrected to `Language::En`; the subsequent Token GPUI run
passed. A concurrent Files test module briefly had a test-macro recursion
failure; that separately owned change was corrected before the successful run.

The initial integration log `work/gate-20261004-sync-ai.log` found two additional
failures. The assistant's old unsupported-configuration test used an output
limit, which is now supported; it now uses the still unsupported explicit proxy
route and retains the actual reject-before-prepare assertion. More materially,
adding the advanced fields to a height-constrained flex column compressed the
discovered-model container so a native click did not select the model. The form
now keeps its natural content height inside a separate scroll viewport, keeping
model result hit areas intact. The click test gained a hit-area regression
assertion; all targeted reruns passed. The failing integration log remains in
ignored work, and the full integration gate must include the corrected layout.

## Source review

The settings editor stores invalid numeric strings only in its in-memory draft
map, preserves them across selection and locale changes, and rejects all Apply
or request launch paths until corrected. Request changes share the established
operation cancellation/revision guard. Applying an old save acknowledgement
cannot replace newer Token text. The assistant compares complete profile
metadata, so changing either limit revokes the preview and ignores old results.
The request transport sends the prepared JSON unchanged and does not read live
profile metadata during dispatch. Limits are validated before the request path.

## Evidence boundaries

All HTTP fixtures use bounded loopback sockets and synthetic responses. They do
not prove a commercial provider account, real tokenizer accuracy, model-specific
maxima, billing behavior, or native Windows/Linux interaction. No terminal text
is sent during discovery or fixed connection tests. Custom headers, proxies,
reasoning selections and Agent workflows remain outside this slice.


## Final integrated local gate

`python3 scripts/check.py` passed on the integrated frozen source: dependency version policy, formatting, strict workspace/all-targets Clippy, and 853 Rust unit/integration/documentation tests (848 ordinary tests plus 5 doctests), with zero failures. The seven opt-in OpenSSH tests were ignored in the ordinary gate and executed separately; all seven passed. Packaging regression tests passed 47/47. The gate log is preserved at ignored `work/gate-20261004-integrated-final.log`; earlier failed logs remain preserved. These results do not certify Windows/Linux native GUI interaction or paid AI providers. Remote CI for the new commits is recorded separately.
