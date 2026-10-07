# Selected-directory desktop test fixtures — 2026-10-07

Status: local full gate and non-author source/behavior/evidence review passed. New commit CI, corrected Windows execution and native acceptance remain pending. Production directory admission is unchanged.

## Exact CI observation

[Quality 37600399634](https://github.com/cyruss648/keelshell/actions/runs/37600399634) for `319cb2a7dd26b683262eb1c14c6a1d9144a909f4` completed with macOS and Linux success and Windows failure. All three complete logs were fetched and read. Initial fetches refused terminal escape sequences; the documented raw-output option succeeded. Fetch errors remain separate from test failures.

| Job | Conclusion | Raw bytes | SHA-256 |
| --- | --- | ---: | --- |
| macOS `112723082812` | success | 557057 | `211f55dd4251dd3b307dc2912e40cc82c56968b4a98c92cfe0542d55dcf9aa00` |
| Linux `112723083217` | success | 587687 | `8db0b563330adc1b221b66a26efca9c9980a552e786079671a18d3244a9f31cd` |
| Windows `112723083304` | failure | 287504 | `bb1135b186c6bd2577315e4fb310439dcb350cedf830408a0cae2d32a919f6ae` |

Windows completed the AI unit and directory controller stages, then the application reported 576 passed, 2 failed and 2 ignored. Failures were `selected_directory_review_runs_in_background_and_preserves_scrollable_payload` at `panel.busy`, and `changing_selected_directory_cancels_pending_prepare_without_replacing_new_review` at `background owner`. Both fixtures call `canonicalize` before submitting a selected path. Windows creates a verbatim namespace that production intentionally rejects before starting a background job. The failure is consistent with that mismatch; Windows execution of the corrected commit remains required.

## Narrow correction

The test helper resolves Unix aliases and preserves ordinary Windows drive paths. The preview test parses JSON and compares selected and independently canonicalized paths as values, rather than searching serialized text for raw backslashes. Background execution, cancellation, immutable review, session changes, two-axis scrolling and existing waits remain asserted. Production code and namespace restrictions are unchanged.

The successful Linux job belongs to this exact commit and does not establish the cause of older synchronization failures. Source CI and controlled tests do not constitute desktop, cloud-model, customer-server or installation acceptance. Raw logs and fetch receipts remain in ignored `work/ci-319cb2a-20261007/`.

## Completed local gate and independent review

The canonical `scripts/check.py` actually exited 0 in 757.508 seconds: dependency policy, formatting, workspace/all-targets strict Clippy, 6 Python tests, 1612 ordinary Rust tests, 8 doc-tests, the 20-case Unix directory controller, and default/explicit 2 MiB process controllers with 626 continuous stages each. The 22 ignored tests were not executed. All 736 inputs, 15079619 bytes, were equal before/after; the held leader was reaped, PGID 86657 absent, and its empty private temporary directory removed. Raw log: 410499 bytes, SHA-256 `b437d7b28df8a6503ac2c9da9c8da206696ef6598b534d68bc24b433e057de6f`.

A non-author read the six-path frozen candidate, all 736 inputs and full gate log, all three exact CI logs, and separate native-startup/probe evidence. Two selected-directory GPUI cases independently exited 0 in 1.082 and 0.168 seconds against the exact full-gate test binary, with unchanged inputs/binary, owned groups absent and private temporary data removed. The scoped frozen review returned `NO_BLOCKER_ROOT6PATH_FROZEN`, SHA-256 `740309bfa73cb5a093b29cd2d93bed7290c6c23fe379c3401ea7673a6e8e9de4`. Root read and hashed all 57 peer payloads, 2492492 bytes, and directly rechecked the peer groups absent. Final result-only updates affect this record, HANDOFF and ROADMAP; code and tests retain the validated bytes.

Full receipts and failed evidence remain in ignored `work/selected-directory-app-platform-followup-20261007-v1/` and `work/selected-directory-non-author-review-20261007/`. Corrected Windows CI is still required; neither the review nor macOS checks establish Windows execution.
