# OpenSSH configuration import — 2026-10-03

The connection manager now exposes **Import SSH config** beside JSON import. It reads clipboard text only, parses the bounded exact-host subset (including the usual whitespace and `key=value` spellings), resolves exact `ProxyJump` aliases, and presents a source-located candidate review before saving. The user must confirm the modal; cancelling leaves the connection library unchanged. Unsupported, duplicate, conditional, or semantic-changing source items are returned as warnings; no proxy command or shell expression is executed. Only global Include directives are evaluated; Include inside a Host or ignored pattern block is skipped safely.

Checks run:

- `cargo fmt --all -- --check`
- `cargo test -p keelshell-core --lib --locked` — 52 tests covering parser, route import, Include scope safety, equals separators, duplicate/unsupported warnings and fail-closed behavior
- `cargo test -p keelshell-core --test openssh --locked` — 6 integration tests covering source import and route resolution
- `cargo test -p keelshell-app --bin keelshell-app connection_library_imports_reviewable_openssh_config_from_clipboard --locked` — candidate is not saved until confirmation
- `cargo test -p keelshell-app --bin keelshell-app cancelling_reviewable_openssh_import_keeps_library_unchanged --locked` — cancellation preserves the previous library
- `cargo build -p keelshell-app --locked`
- `python3 scripts/check.py` — dependency policy, format, strict workspace Clippy, workspace tests and doctests passed
- `python packaging/package.py macos --binary target/debug/keelshell-app --output /tmp/keelshell-native-ssh-import-20261003 --target aarch64-apple-darwin` — staged native macOS app and inspected the rendered toolbar; no installation or replacement was performed

Remote verification for commit `51c0eac2970b3fa9c663ec8ff28e2d08353ebd90`:

- GitHub Actions Quality `37155464824` — macOS 26, Ubuntu 24.04 and Windows 2025 all passed. The macOS runner reported only a capacity annotation; no job failed or was cancelled.

The tests cover exact aliases, source-order first-value semantics, explicit global Include content, source provenance, warning preservation, unresolved jump aliases, shell expansion rejection, equals-separated directives, duplicate/unsupported directives, review confirmation/cancellation, and all-or-nothing route import. They do not prove complete OpenSSH evaluator compatibility, conditional Host-scoped Include behavior, native file-picker behavior, or imports from a production user's filesystem. Keyboard-interactive authentication remains a separate transport capability and still needs application-level prompt collection.
