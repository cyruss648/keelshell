# 2026-10-06 AI settings persistent visible scrollbar — author follow-up

This is a narrow UI follow-up over the frozen API inference v2 candidate, whose 23-file source and prior evidence remain separately preserved. It does not change inference mapping, known-secret admission, authentication, model declarations, dependencies or command/review behavior. Root integration, a new independent review and native testing are separate.

## Design and actual component API

AI settings retains the existing `ai-profile-form-scroll` ID, test-support boundary, flex/minimum dimensions and natural-height form. A panel-owned `ScrollHandle` persists across rendering and is attached to this same scroll container. A fixed overlay beside translated content uses the installed GPUI Kit/component 0.7 API `Scrollbar::vertical(&handle).mode(ScrollbarMode::Always)`, with a 12-pixel track and 7-pixel resting thumb following the current application palette. The explicit handle drives both wheel and thumb-pointer interactions; Apply/Cancel remain outside the scroll viewport. The configuration list is unchanged.

The installed source confirms `Scrollbar::vertical`, `mode`, `styles`, viewport bounds and `ScrollHandle` support; the track style needs `Hsla`, while the thumb style accepts a background. Component `ScrollableElement::vertical_scrollbar` exists but inherits theme auto-hide, so the direct component is used for an always-visible control. No invented API or new dependency is introduced.

An earlier native attempt found offscreen settings controls and could not click them. That observation does not establish a product wheel defect. This follow-up provides a clear thumb interaction; actual native reachability remains for root to re-test.

## Controlled production control regressions

Two GPUI tests mount the real production settings panel at 900×580. They read live geometry to choose a thumb grab point and dispatch real mouse down/move/up using `TestWindowExt::drag`; no offset setter or direct scroll-to-item call proves reachability.

- API cases cover Chinese/System preference, English/Dark and Chinese/Light. A thumb drag reaches the maximum scroll offset and the last proxy input, actual keyboard events edit that input, then another thumb drag reaches temperature and edits it. The observed input value and emitted Apply catalog retain the selected profile/model, exact 0.25 draft and proxy URL; theme/locale remain; rerender retains the shared handle and offset; the fixed Apply keeps its bounds and is clicked through the production control.
- The CLI case reaches and edits the final output-limit input via thumb drag. Wheel events then move the same container, followed by a second thumb drag. Model/profile, language/theme and fixed Apply remain intact; no CLI operation starts.
- The broader AI settings regression set covers existing API/CLI scrolling, profile/model/locale drafts, connectivity fixture behavior and credentials. Production Workspace numeric admission/StateStore tests verify that the overlay does not obstruct Apply or weaken either save boundary.

## Preserved failures

Pointer v1 stopped at compilation because track `bg` needs an explicit `Rgba`→`Hsla` conversion. Pointer v2 stopped on an accidentally broadened conversion in an ordinary `Div` background, with a duplicate test theme binding; the conversion was restricted to the track and the test binding corrected. Pointer v3 compiled, then both tests stopped before dragging because the fixture treated the positive `max_offset` extent as negative. The fixture now reads that positive extent and expects the actual offset to be its negative at the bottom. Production scrolling logic was not changed to accommodate this fixture correction. All three exits 101 retain original logs/receipts, stable 268-input snapshots and the three source files for each failed attempt.

## Actual author results

- Pointer v4: exit 0, 10.297399 seconds including compilation; both pointer tests passed (three API language/theme cases plus CLI). Log 862 bytes, SHA-256 `f933e54595d060c9de3ca39da0b5b94e270ec575f964d2b8776fc8eb27319cdb`. All 268 engineering inputs matched before/after.
- Relevant scoped gate: exit 0, 37.951694 seconds; dependency policy-only, formatting, strict locked `keelshell-app --all-targets` Clippy passed; 52 AI settings tests and 4 numeric admission/persistence tests passed. Log 8061 bytes, SHA-256 `f6bdca07311dc88da4a291a9c85b95c020d3fa8118e72fb44f473c370cfa08c4`. The same 268 inputs matched before/after and matched the final pointer test.

Commands used an isolated writable target, private TMP directories and bounded owned process groups. Direct leaders were waited/reaped; no timeout or original numeric-group survivors were reported. Five TMP directories were removed only after confirming empty. This is not an escaped-descendant census.

No follow-up full-workspace gate, standard native dual-program build, GUI launch, pixel capture, OS-theme notification, cloud-model/vendor CLI call, target-native desktop, CI/package/install/update acceptance is supplied by this author. The root will run the combined integration gate and repeat native scrolling after independent review.

## Root integration and subsequent repairs

Independent review found an English 900×580 invalid-draft footer clipping Apply. Root corrected status wrapping and retained action widths; six production language/theme cases and a new non-author review pass. A subsequent native run reached settings, persisted explicit Chat zero and issued one reviewed Chat request, but exposed a separate clipped Send in a long Messages preview. That failed boundary remains preserved.

The assistant now has its own persistent scrollbar and a fixed confirmation footer. A new non-author review covers the original failing cases, 36 actual Workspace pointer cases, 10 protocol hint/cache cases and strict scoped checks. The final combined root gate passes 1211 ordinary/8doc/6Python. The fresh final macOS Chinese/System run actually reaches settings, sends all three API protocols manually and shows their fixed replies; Messages dragging reaches the full JSON end while Send stays visible. Complete hashes, lifecycle evidence and native limits are recorded in [request review follow-up](2026-10-06-ai-request-review-scrollbar.md). The author-only limitations above remain the scope of the original evidence, not a description of the later root run.
