# Connection library slice — 2026-10-03

This record covers the first complete connection-library interaction slice: a
user can copy a validated connection JSON document to the system clipboard,
paste/import it into the saved SSH library, mark a profile as a favorite, and
delete a profile. All operations use the existing versioned `AppState` snapshot
and durable `StateStore`; no password, private-key contents, host-key pins or AI
credentials are placed in the export document.

## Implementation

- `AppState::remove_connection` removes exactly one profile by stable ID and
  returns the removed metadata for UI status handling.
- `AppState::toggle_connection_favorite` changes only the profile marker and
  reports its resulting value.
- The connection manager exposes bilingual **Import JSON** and **Export JSON**
  actions through the system clipboard. Import remains all-or-nothing and
  keeps the existing endpoint/ID de-duplication rules.
- The table exposes a favorite marker and a delete action. A delete is saved
  through the same optimistic-concurrency path as profile edits; failures leave
  the in-memory form and persisted snapshot unchanged.

## Verification

```sh
cargo test -p keelshell-core
```

Result: **51 passed, 0 failed, 0 ignored** across unit and integration tests.
The new profile test verifies favorite toggling, removal by ID, returned
metadata and the not-found error. Existing export/import tests continue to
verify schema validation, duplicate handling, atomic invalid-batch behavior,
and exclusion of non-connection state.

The GPUI workspace test suite remains the next integration step for these new
buttons. Clipboard behavior is implemented through GPUI's platform clipboard;
the current deterministic UI fixture does not claim to prove native clipboard
permissions or a desktop file-picker flow.

## Boundaries

- JSON is intentionally transferred through the clipboard in this slice; a
  native file chooser and encrypted export container remain future work.
- Deletion is immediate and durable after the save completes. A recycle bin or
  undo history is not yet implemented.
- Favorites are persisted metadata but there is not yet a dedicated favorites
  tree node or recent-connection ranking.
