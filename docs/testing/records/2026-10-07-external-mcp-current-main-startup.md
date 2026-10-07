# External MCP current-main startup attempt — 2026-10-07

Status: first native UI confirmation failed; no external CLI, model or business requests started. KeelShell exposes an MCP server to external agents and does not add a third-party MCP client.

## Preparation and exact input

The attempt used clean main `319cb2a7dd26b683262eb1c14c6a1d9144a909f4`, 734 source inputs, a new macOS arm64 development bundle containing GUI and MCP companion, and a private byte-identical copy of the previously observed Codex `0.160.0` executable. That executable was not launched. The 758-file native binding SHA-256 was `b103e835ab42a52a274f6b2ef59a1e822c1df3a263260dda58cb6fff05f43cdb`; post-attempt full readback verified all 485895147 bytes unchanged before subsequent edits.

A new non-author preparation review returned `NONAUTHOR_PREPARATION_PASS`, SHA-256 `809be160e7bde907b98ad58fb50436a20a3d6ae8b1f75a48e1bd11de76a3f801`. Preparation, pure controller cases, build and packaging inspection are not native business acceptance.

## Failed native stage

The private GUI started and its accessible interface was observed. The operator capture script requested an unavailable runtime timing API; importing that module was also disallowed. No valid screenshot/observation marker arrived before the original 120-second `gui_ready` deadline. The owner actually exited 1 with `TimeoutError: actual UI marker deadline: gui_ready`. No deadline or assertion was weakened. This establishes an operator recording failure, not a proven application defect.

Failure SHA-256: `33d0d9e1a31741b0ede3ef19eaf2d99c2172e4e971469b2bc92f262a6236c7eb`. Cleanup receipt SHA-256: `4575fca4786d71d6ecb040b1b5bf718480bcafec04cc24ef18bc4bbe6ea7ebdd`. GUI actual wait was `-15`, fixture actual wait was `0`, both reaped; four known owned groups were absent, the owned port refused connections, and private fixture/temporary data were removed. Root independently rechecked groups and port. No full process census is claimed.

## Separate operator probe

A private GUI probe exercised explicit accessibility capture and screenshot APIs without SSH, MCP, CLI or model use. It preserved an original 315329-byte JPEG, 2880×1866 pixels, SHA-256 `9e397fdc36d80e9d05d2906dd5ab4a8c017a2e4e8990c344e940b465dc5ba959`, and original accessibility text, SHA-256 `8ec8ec5a3dce12ecdfac3aa0ba1f3dc682424d331e3c5534998e29eaca019a06`.

An actual runtime wall-clock capture-completion sample was mapped between host wall/monotonic samples; observed offset change was 873458 ns. This derived time has millisecond resolution and is not a direct capture-runtime monotonic reading. The probe exited 0, reaped its GUI, observed its owned group absent and removed private data. It validates only this capture route; its image cannot be reused as an attempt marker. A fresh bounded marker pipeline and review are required before another attempt.

The failed attempt and operator probe remain in separate ignored `work/codex-external-business-fresh-v12-20261007-current-main-v1/` and `work/mcp-ui-capture-operator-probe-20261007-v1/`. Earlier 20 paired Codex calls belong to their historical source and partial attempt; this run contributed zero business calls. Full current Codex approval/rejection/revocation, product UI and other-platform acceptance remain open.
