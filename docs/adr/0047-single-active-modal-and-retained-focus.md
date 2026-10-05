# ADR 0047: Single active modal subtree and retained focus

- Date: 2026-10-05
- Status: Implemented; independent review, integration and native platform acceptance pending
- Scope: UI-05, workspace root modals

## Context

The workspace previously mounted the background and every open overlay at once.
A pointer-blocking backdrop did not exclude background controls from GPUI's
accessibility tree, and the keyboard close order differed from the paint order.

The locked GPUI Kit 0.7 / base 0.7 / GPUI pre 0.3 APIs provide
`FocusTrapElement`, `Window::focused`, `Window::on_next_frame` and public keyed
state. They do not expose a subtree `inert` or accessibility exclusion builder.
`Role::Dialog` and `occlude()` alone do not isolate background controls.

## Decision

Define one explicit `ModalKind` order and use it for rendering, keyboard close
and surface focus. Only the active modal is mounted. Its background is a themed
canvas rather than another interactive workspace or a screenshot. The retained
business entities, profile inputs, terminal transports and tasks stay owned by
`Workspace`; unmounting changes presentation and input dispatch, not their data.
Do not call a hidden modal's renderer merely to reproduce its visual backdrop.

Server identity, credential and keyboard-interactive decisions take precedence
when they arrive asynchronously. Nonmodal handshake progress remains a status
card, so it does not freeze an unrelated terminal or draft during background
connection work. A child destination dialog takes precedence over its connection
editor, which in turn takes precedence over the connection manager.

The mounted scope is a named `Dialog` and uses the Kit's real `FocusTrapElement`.
The root's existing shortcuts resolve against this same scope. Escape closes only
the visible layer; save guards, vault cleanup and connection cancellation retain
their existing semantics. Background connection-manager and assistant shortcuts
cannot replace a visible draft or reveal another layer.

A compact, scope-owned bar supplies the current task title, theme selection and
language selection. It is inside the same focus trap; the background toolbar is
not mounted. The bar reserves 34 pixels, while the actual panel keeps its existing
scrollable content and fixed action area. Preference actions use the existing
persistence path and are disabled during saves and credential-vault operations.

Keep a stack of return focus handles. Restore after the remounted parent's frame
has finished. A generation and active-kind check rejects callbacks belonging to
an earlier modal transition; asynchronous authentication cannot lose its focus to
an older callback. If the original target no longer belongs to the visible
surface, choose a current, mounted focus target instead.

Give each newly selected layer its real input or panel focus immediately, then
validate it against the completed scope. A previous layer's containment result
cannot prove that a replacement is already focused. The ordinary workspace root
does not take default focus on a mouse-down, so command selection retains its
input's focus-dependent insertion behavior.

Styled Kit buttons create keyed focus handles that normally expire when their
parent is unmounted. A thin `RenderOnce` wrapper seeds that **public** keyed
state with a retained handle before directly rendering the actual Kit
button. The button's real roles, labels, disabled rules and activation handlers
remain in use. Main toolbar, connection library/form and command-library buttons
therefore retain keyboard trigger identity while a nested layer is visible. The
workspace caches weak handles and prunes expired entries every render; only
mounted controls and genuine return targets retain strong handles.

An asynchronous challenge can arrive after the previous frame has painted and
before its pointer release is dispatched. The locked Div capture callback also
requires a hovered ancestor hitbox; `occlude()` can make that test false even
during capture. A thin outer `Element` therefore registers the window's raw
pointer-down and pointer-up capture listeners **before** painting its child.
It does not depend on a hitbox and rejects a painted surface whose active kind
or generation has changed. Current-frame layout, accessibility nodes and Kit
controls remain the child's own implementation.

Each keyboard-interactive prompt also owns an ephemeral UUID. Route, hop and
modal kind can recur, including challenges with no answer fields. The painted
boundary captures this UUID, so an old button cannot consume a replacement
challenge before it is drawn. This identity is never persisted or exposed to
the model. Challenge replacement also advances the actual focus generation and
uses a fresh content element namespace, while keeping the outer focus trap
stable. This discards Kit's held mouse/key activation state; checking a new
frame's UUID alone cannot distinguish an old Enter/Space release. It avoids
creating an unbounded set of trap registrations for a retained trap handle.
Raw-event regressions cover both pointer-down orders, a repeated
release after remounting, normal current clicks and same-kind zero-field
challenge replacement, including held Enter/Space and repeated mouse releases.
Native accessibility activation remains a separate
acceptance requirement.

## Consequences and validation

Removing background elements also removes their current-frame accessibility
nodes, action registrations, hitboxes and tab stops. This is stronger than a
visual shield. The locked GPUI clears AX action listeners and node bounds at each
frame; stale node identifiers do not preserve handlers after that frame.

The visual tradeoff is intentional: only the current task and its actual target
metadata are shown against a quiet canvas. No copied or fake background controls
are introduced. Entity-owned drafts and ongoing tasks are not recreated.

GPUI tests inspect real completed frames, focus, actual key dispatch and the
registered element accessibility semantics. The headless platform does not
activate the native AccessKit adapter, so no live AX adapter tree is claimed.
These tests do not establish VoiceOver, Narrator, AT-SPI,
platform IME or GPU-pixel acceptance. Those native checks and fresh independent
review remain necessary before declaring UI-05 complete.

See [modal design and native checklist](../design/MODALS.md) and the
[implementation test record](../testing/records/2026-10-05-modal-isolation.md).
