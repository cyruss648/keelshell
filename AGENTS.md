# KeelShell engineering contract

Read `docs/HANDOFF.md` before continuing this in-progress implementation; the latest remote-only user requirements supersede historical scope.

- Maintain a real native GPUI Kit application for macOS, Windows and Linux. Never represent mock data, planned functionality, compilation or emulation as native acceptance.
- Keep UI (`keelshell-app`), domain/storage (`keelshell-core`), transports (`keelshell-session`) and AI (`keelshell-ai`) separate. Blocking I/O belongs off the UI thread.
- All direct registry dependency requirements must be `x.y`. Keep the application Cargo.lock with resolved patch versions. Pin toolchains exactly to avoid known compiler defects; toolchain pins are not library dependency requirements.
- Do not introduce private Git dependencies, developer-specific absolute paths, embedded credentials or production logs. Credentials stay ephemeral by default. Explicit persistence uses an OS credential store or the authenticated, master-password-encrypted vault; never write a master password or a plaintext credential to profile metadata.
- AI suggestions must be reviewable. Sending context is explicit; model output cannot bypass command review or issue actions autonomously. Host key changes must fail closed.
- Public library APIs need rustdoc. Comments explain invariants and non-obvious decisions. Use typed errors and avoid panics outside tests.
- Add meaningful unit and integration tests for behavior. Use bounded waits and isolated temporary data, no real customer machines.
- Run formatting, Clippy, tests, dependency policy and relevant native checks before local feature commits. Preserve failing evidence and document unverified boundaries.
- Track requirements and acceptance in docs/product, design decisions in docs/adr, test runs in docs/testing, and remaining work in docs/ROADMAP.md. Update status honestly in the same feature commit.
- Describe KeelShell through its own capabilities. Keep named proprietary SSH product comparisons out of repository text, filenames, source comments and release notes; maintain the Chinese and English README together.
- The user authorized a public GitHub repository, pushing commits, and tag-triggered multi-platform Releases on 2026-10-03. Keep release validation and incomplete product scope explicit. This does not authorize telemetry uploads or installation over an existing application.
