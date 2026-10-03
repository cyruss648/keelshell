# Keyboard-interactive authentication — 2026-10-04

Implemented bounded keyboard-interactive authentication in `keelshell-session` using russh's challenge/response API and connected the prompted mode to the real GPUI workspace. A password or private-key login can explicitly switch to keyboard-interactive/MFA. The server challenge name/instructions and each prompt are rendered in a bilingual modal; answers stay in `Zeroizing<String>` values and a one-shot channel, and the UI never offers vault persistence for them. Route UUID/hop checks reject stale challenges and cancellation sends an explicit empty result to the transport.

Checks run:

- `cargo fmt --all`
- `cargo fmt --all -- --check` — passed
- `cargo check -p keelshell-session --locked` — passed
- `cargo test -p keelshell-session --lib --locked` — 63 passed
- GPUI regression `keyboard_interactive_prompt_is_ephemeral_and_cancelable` — passed inside the 253-test `keelshell-app` run
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — passed
- `python3 scripts/check.py` — passed; the six OpenSSH interoperability cases remained intentionally ignored without the disposable-server environment

Unit coverage verifies ordered response mapping and that missing-response diagnostics do not echo prompt text or secret values. The GPUI regression verifies bilingual modal controls, answer handoff and cancellation clearing the route. The test suite does not claim acceptance against an external MFA service or native Windows/Linux UI. Responses remain ephemeral and host key approval remains fail closed.
