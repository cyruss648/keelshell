# 2026-10-06 AI request review scrolling and confirmation

## Scope and preserved findings

This follow-up combines the reviewed API inference v2 feature with reachable settings and request review controls. It keeps the original send callback, prepared-request identity, revision, credentials, captured SSH target and manual command-review guards. It adds no MCP client and makes no autonomous model action possible.

The first native parameter attempt could not reach controls below the settings viewport and issued zero HTTP requests. An explicit settings scrollbar then exposed them. Independent review found a separate English 900×580 invalid-draft status pushing Apply outside the footer; the status now wraps within its available width while the actions retain their width. Six production GPUI language/theme cases verify invalid feedback, correction and exactly one Apply.

The subsequent macOS run sent one Chat request, but a long Messages preview clipped Send. That run remains a limited result, not acceptance of Messages or Responses. A new independent six-case reproduction confirmed the clipped control. Old screenshots, logs, failures and frozen packages remain preserved in ignored evidence directories.

## Final implementation

Both settings and assistant review use persistent `ScrollHandle` instances with the installed vertical `Scrollbar` component, an always-visible 12-pixel track and a 7-pixel thumb. The assistant body scrolls independently of its fixed confirmation footer. A prepared request is still required for Send; editing or changing its bound context revokes the old confirmation.

Protocol-specific temperature placeholders now distinguish Messages 0–1 from Chat/Responses 0–2. Request-options help describes model-specific inference and sampling. Placeholder changes happen only after an editor-cache miss, preventing repeated render notifications on a stable cache hit.

Eighteen formal production GPUI cases cover three protocols, Chinese/English and System/Light/Dark at 380×580. They actually drag the thumb to the JSON end, check the fixed Send geometry and revoke the prepared request on context change. The existing long local-agent review regression now verifies its fixed footer too.

## Independent review

The final five-file repair was frozen before the non-author review. The conclusion is `PASS_LIMITED_CODE_AND_GPUI`, with no new P1/P2.

- The original six clipped-Send cases pass without weakening their assertions.
- Thirty-six real Workspace cases cover 900×580 and 1280×840, both languages, three themes and three protocols. Actual pointer dragging reaches the full request end; Send does not move. Actual question editing removes the old Send after input processing.
- Ten real protocol-switch/language cases verify the 0–2/0–1/0–2 placeholders and no extra input notifications over five cache-hit renders per state.
- The scoped dependency policy, formatting, strict locked app all-targets Clippy, 47 assistant tests, 57 settings tests and 4 numeric consumers pass: exit 0, 54.806802 seconds. All 558 review inputs match before/after. The 36 Workspace pointer cases separately pass in 45.176980 seconds.

The independent packet contains 602 payloads. Its manifest is 105752 bytes, SHA-256 `7e0c7c3330fa48f664f996dff7896e4bcd3706b89e704fa32c0a2aa5ad6e1d63`; root copied and rehashed every payload. One reviewer getter-API compile failure remains preserved; only the test was corrected. Controlled GPUI geometry is not native desktop acceptance.

## Root gate, build and final native run

The final root `python3 scripts/check.py` passed: exit 0, 180.646134 seconds; 1211 ordinary Rust tests, 11 ignored, 8 doc tests and 6 Python tests, formatting, x.y policy and strict workspace all-targets Clippy. Default and explicit 2 MiB controllers each completed 626 records. All 554 visible repository inputs match before/after. Log: 357371 bytes, SHA-256 `095403ad20c2c7df241e1abf6279c935038517f9106c4d0b1e03400e4465ab59`.

The fresh standard macOS app/MCP build passed in 5.375726 seconds; standard packaging passed in 0.293950 seconds. The final arm64 app is 183081712 bytes, SHA-256 `08170948e044d729862fced59b4c6547f027c344ab145cbecfc2f3aa3972c467`. The MCP companion is 16709088 bytes, SHA-256 `2963d9bb7023aa476ecfab3f88638043afc4c4e02c6824e1baa13606c5f32503`. Mach-O load commands and the bundle were inspected; the application requires macOS 15. No existing installation was overwritten.

The newly started native app used Chinese/System, an empty SSH catalog and three anonymous API profiles pointed only at an owned loopback fixture. Actual captures show:

| Protocol | Actual UI and recorded wire fields | POST body |
| --- | --- | --- |
| Chat | Drag settings, declare sampling support, apply explicit temperature 0, reopen with value 0, review `temperature: 0.0`, click the fully visible confirmation, observe fixed reply | 739 bytes; SHA-256 `70b7b1fcf023c3b2e9728938c8e4464f179d19bc933585a4b58f11a2c84a044c` |
| Messages | Apply medium effort/adaptive thinking; see 0–1 placeholder and updated help; actually drag review thumb to final JSON brace while Send stays fixed; manually confirm and observe reply. Wire has `output_config.effort: medium`, `thinking.type: adaptive`, `max_tokens: 4096` | 771 bytes; SHA-256 `6246a56b8a160a1a656afd1ec1b94684346e9a3740d206f3fc07370237774ee5` |
| Responses | Apply medium effort; review nested `reasoning.effort: medium` and `max_output_tokens: 4096`; manually confirm and observe reply | 673 bytes; SHA-256 `4842b2075ed1525e48caf258c924bf3831ea6e3a3a437f42429bdf72eeccaf0b` |

Exactly three POSTs were recorded, one per protocol, with no Authorization or API-key header. Final settings readback preserves explicit Chat zero and the two reasoning declarations. The fixture records only whitelisted inference fields and body digests, not raw questions or credentials.

The owned controller was actually waited/reaped with exit 0; the app was explicitly stopped and waited with -15. HTTP activity is zero, the HTTP thread ended, the own port closed and private fixtures were removed. Root separately checked the two saved PID/birth identities were gone. The same 554 engineering inputs match through the end of this native run. This is bounded ownership verification, not a full descendant census.

The final native packet contains 41 payloads, manifest 5854 bytes, SHA-256 `7fea0cfef15d28da284e7c06175ac95e991c3e63b603ae4163ebcaf455a9a21d`. Its native finding is `PASS_LIMITED_NATIVE_API_AND_SCROLLING`. A new non-author evidence review independently verified all 41 payloads, viewed all 14 actual screenshots, checked the three wire observations and settings export, recalculated the gate counts, and read back the actual stage binaries. Its verdict is `PASS_LIMITED_NATIVE_API_AND_SCROLLING_EVIDENCE`, with no new P1/P2; the original clipped-Send P2 closes only in the observed final macOS Chinese/System large-window scope. The portable independent packet has 611 payloads, manifest 107112 bytes, SHA-256 `a68f8f0c8644ce66f3942a492a7c17448490f34f2c6144703b2006468c9b4e10`, and root copied/rehashed every payload. It includes the six actual stage files and 551 exact bound-source bodies; three old documentation bodies were unavailable, so only their historical hashes remain. Current updated documentation was not substituted. There is no separately saved Chat reopen screenshot, and the wire whitelist does not include Chat `max_completion_tokens`; the reviewer does not claim either from those captures alone. Subsequent public documentation updates deliberately occur after the frozen engineering checkpoint.

## Remaining acceptance

This closes the reachable-configuration and long-request confirmation path only for the observed final macOS Chinese/System large window. Native minimum-window measurements, final English/light/theme matrix, screen-reader/IME and full keyboard navigation, Windows/Linux desktop, cloud-model compatibility, SSH execution/file mutation and external MCP authorization business are not established by this run. The older failed attempts are not relabeled as passes. Fresh commit CI and release/install/update acceptance remain separate.
