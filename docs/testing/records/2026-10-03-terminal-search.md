# Terminal scrollback search — 2026-10-03

## Scope

The focused SSH terminal exposes a compact search overlay. `Cmd+F` on macOS,
or `Ctrl+Shift+F` on Windows/Linux, opens it and focuses the query field.
Search uses case-sensitive literal text across the emulator's retained
scrollback, including wrapped lines and CJK characters. Regex punctuation is
escaped; the query must be a single line of at most 4 KiB.

Previous/next buttons and `Shift+Enter`/`Enter` cycle through matches. A match is
selected and highlighted in the terminal, and an offscreen result is scrolled
into view. A result already inside the viewport does not move the viewport.
`Esc` and the close button remove search highlighting and restore terminal
keyboard focus. Reopening searches the retained query again.

The search bar is an occluding sibling of the terminal transport surface.
Search input, clipboard paste, pointer motion, scrolling and button clicks
cannot bubble through the remote mouse encoder. Search never sends a command,
reads a remote file, or writes output/query text to persistent configuration.
Placeholder, button tooltips and status follow the current Chinese/English
locale, while language refresh retains the query and current match.

Queries update from `InputEvent::Change` subscribers. Render only reads search
state; it does not start searches or issue notifications. New remote output,
terminal resize, alternate-screen changes or a new manual selection invalidate
search coordinates. New output retains the query and prompts the user to press
Enter to search again, rather than using stale coordinates or continuously
jumping through a changing buffer.

## Review findings and fixes

The original GPUI check called `open_search` directly after a shortcut test
failed before the overlay had test observation enabled. That check did not
prove the actual shortcut or focus-return contract. This review restored the
platform shortcut and reproduced a real failure: the search closed on `Esc`,
but terminal focus was not restored.

The corrected implementation focuses the terminal explicitly on close, handles
Enter/Escape through the search input action scope, and restricts SSH key
encoding to the focused terminal surface. The previous overlay was a child of
the terminal's mouse-event element; it is now a separate, occluding sibling.
The tests enable remote SGR mouse reporting and assert that no transport write
is produced by search interaction.

The earlier renderer also ran searches and notified the application during
render. Query updates now occur in the input event subscription, with tests
allowing native event transactions to flush before checking subscription-driven
state. Search coordinates and highlighting are invalidated before changed
output is parsed or the grid is resized. Query changes restart at the retained
buffer boundary, and clearing search does not erase a later manual selection.

## Verification

Executed on the local macOS host:

```sh
cargo test -p keelshell-app --locked terminal_search -- --nocapture
cargo test -p keelshell-app --locked --all-targets
cargo clippy -p keelshell-app --all-targets --locked -- -D warnings
rustfmt --edition 2024 --check crates/keelshell-app/src/emulator.rs crates/keelshell-app/src/terminal.rs crates/keelshell-app/src/ui_tests.rs
git diff --check
```

Results at the reviewed workspace state:

- Three focused GPUI search tests passed.
- Full application suite: **72 passed, 0 failed, 0 ignored**. This count also
  includes concurrently integrated credential UI tests; it is not a search-only
  count.
- Clippy and formatting checks passed. Cargo reports the existing upstream
  `block 0.1.6` future-incompatibility notice.
- Five emulator search tests passed within the full application suite.

The emulator tests verify exact match movement and wraparound in both
directions, scrollback visibility, stable visible viewport, literal punctuation,
case sensitivity, CJK/wrapped text, old history eviction, resize/alternate-screen
invalidation, query reset, manual-selection preservation, and rejection of
multiline/oversized queries.

The GPUI tests use the real platform shortcut, native text/key dispatch and
rendered controls. They assert that typed and pasted search text, Enter,
Shift+Enter, Esc, input clicks, arrows, close, hover and wheel events produce no
SSH write bytes. After closing, `id` plus Enter produces exactly `id\r`, proving
that terminal keyboard input resumes. They also verify output-driven
invalidation, explicit repeat search, offscreen selection, locale refresh,
missing queries and empty queries.

## Boundaries

These deterministic GPUI tests use controlled transport queues and do not open
a local shell or SSH network connection. They verify event routing, focus,
render/layout execution and emulator selection state, not GPU pixel appearance
or production-server interoperability. Native Windows/Linux desktop shortcuts,
clipboard integration and OS IME candidate windows still need acceptance on
those platforms.

Search is bounded by retained terminal scrollback. It does not search remote
files, command history, other tabs, or discarded history, and it offers no
regex, fuzzy, case-insensitive or Unicode normalization controls. Search state
is memory-only and is lost when the tab or application closes.
