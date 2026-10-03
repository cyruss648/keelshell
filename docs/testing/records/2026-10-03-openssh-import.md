# OpenSSH configuration import — 2026-10-03

The connection manager now exposes **Import SSH config** beside JSON import. It reads clipboard text only, parses the bounded exact-host subset, resolves exact `ProxyJump` aliases, and persists a candidate state only after validation. Unsupported or semantic-changing source items are returned as warnings; no proxy command or shell expression is executed.

Checks run:

- `cargo fmt --all -- --check`
- `cargo test -p keelshell-core --lib --locked` — 47 tests covering parser, route import, Include and fail-closed behavior
- `cargo test -p keelshell-app --all-targets --locked` — production import action and GPUI regression suite (243 passed)
- `cargo build -p keelshell-app --locked`
- `python packaging/package.py macos --binary target/debug/keelshell-app --output /tmp/keelshell-native-ssh-import-20261003 --target aarch64-apple-darwin` — staged native macOS app and inspected the rendered toolbar; no installation or replacement was performed

The tests cover exact aliases, source-order first-value semantics, explicit Include content, warning preservation, unresolved jump aliases, shell expansion rejection and all-or-nothing route import. They do not prove complete OpenSSH evaluator compatibility, native file-picker behavior, or imports from a production user's filesystem. Keyboard-interactive authentication remains a separate transport capability and still needs application-level prompt collection.
