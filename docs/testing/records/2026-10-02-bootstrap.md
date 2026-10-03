# M0 bootstrap verification

Platform: macOS 27, Apple Silicon arm64. Rust 1.98.1; GPUI Kit 0.7.0 / gpui-pre 0.3.7.

- `cargo check`: passed, native macOS target.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed for the bootstrap app.
- `cargo fmt --all`: applied; direct dependency x.y policy passed.
- No behavior tests existed at this bootstrap commit. This is an engineering baseline, not a feature acceptance.
- Cargo reports a future-compatibility warning in transitive `block 0.1.6`; it does not fail the current compiler and must be tracked during GPUI upgrades.
- No Windows/Linux native test, GUI interaction or remote service acceptance yet.
- Source Reef/template checkouts were read only. No Git remote configured.
