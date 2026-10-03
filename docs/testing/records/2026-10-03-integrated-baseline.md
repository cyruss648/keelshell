# Integrated baseline gate — 2026-10-03

This is a verified engineering checkpoint preceding the user's remote-SSH-only scope correction and workspace redesign. It is not a finished product or a parity claim.

- `python3 scripts/check.py`: PASS; dependency x.y policy, workspace format, all-target Clippy with warnings denied, and locked workspace tests.
- 128 tests passed, no failures or ignored cases in this gate: AI 28 plus one doctest; application 28; core 34; session 37. The GUI fixture has three additional separately run filesystem/security tests documented in the session record.
- Initial integrated gate failed on formatting, then 23 Clippy findings in application/tests. These were fixed without blanket lint suppression, and the complete gate was rerun successfully.
- Native macOS development build ran. Actual shell output, Unicode clipboard input, two-column split and exact AI request preview were observed. A connection-modal event leak was found, fixed with occlusion and focus ownership, and covered by a GPUI regression. A subsequent native form correctly received all fields and saved its isolated test connection.
- Actual OS IME candidate windows, external OpenSSH servers, Windows/Linux desktop operation, monitor commands on a native Linux host and paid AI-provider operation were not verified.
- No real customer SSH endpoint was contacted. No Git remote is configured. Test application data, temporary service keys and screenshots containing user connection names stay outside the repository.

The next change removes product-local terminal management and redesigns the UI around a compact remote SSH workspace with Chinese as the default language. Existing transport/emulator evidence remains useful where reused, but does not establish acceptance of the redesigned interface.
