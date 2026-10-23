# ADR 0089 — Bounded language highlighting for reviewed remote drafts

Status: implemented in the current working tree; local engineering gate passed
on 2026-10-23. Commit CI, syntax-color rendering, IME, VoiceOver, minimum-window
behavior and Windows/Linux native acceptance remain open.

## Context

The remote file editor is a reviewed draft surface. It must make source text
easier to inspect without changing draft bytes, adding an execution path or
creating a second search surface. The existing KeelShell literal find/replace
controls and full-text save review remain part of that contract.

`gpui-kit` 0.7 provides an `EditorState` backed by optional Tree-sitter
grammars. Enabling grammars adds native C compilation dependencies and therefore
needs a bounded first set with explicit platform follow-up. The patch input and
manual conflict replacements have different review semantics and do not need
source highlighting.

## Decision

Use `EditorState` for the main remote draft and retain `TextareaState` for patch
input and manual conflict replacements. Select a language only from the final
filename component of the remote path. The first enabled set is:

| Filename | GPUI Kit language |
|---|---|
| `.rs` | Rust |
| `.py`, `.pyi` | Python |
| `.json`, `.jsonc` | JSON |
| `.toml` | TOML |
| `.yaml`, `.yml` | YAML |
| `.sh`, `.bash`, `.zsh`, `.bashrc`, `.zshrc`, `.profile` | Bash |

Unknown, ambiguous, extensionless, unsupported and SQL filenames use plain
text. Fish is intentionally plain text because the first dependency set does
not include a Fish grammar. Filename matching is case-insensitive. Content
sniffing, path execution and language-driven remote work are forbidden.

Disable the editor's built-in search, folding, auto-closing and smart
indentation. This preserves the existing explicit literal search/replace
controls and prevents typing from silently changing reviewed draft input.

## Consequences

The editor can provide bounded syntax cues for common configuration and script
files while preserving the existing UTF-8, NUL, 1 MiB, full-review, backup and
byte-readback boundaries. Language selection changes presentation only; it does
not affect `editing`, the checked baseline, the pending review identity, or the
save worker.

The initial language set is intentionally incomplete. SQL, Fish, Dockerfile,
Makefile and other languages require a separate dependency, rendering and
platform review before activation. The enabled grammar set currently resolves
`cc` to the compatible locked line recorded in `Cargo.lock`.

## Verification and limits

The language selector has unit coverage for supported extensions, case
insensitivity, ambiguous names and unsupported fallbacks. The GPUI integration
test opens every enabled language plus Fish and plain text, checks the resolved
`language_name()` and verifies the draft text remains unchanged. The full local
`python3 scripts/check.py` gate passes.

The test suite does not establish native syntax-color pixels, IME behavior,
VoiceOver output, minimum-window usability or Windows/Linux desktop acceptance.
Those remain separate platform evidence requirements. See the
[language highlighting test record](../testing/records/2026-10-23-editor-language-highlighting.md).
