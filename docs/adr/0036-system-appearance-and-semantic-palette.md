# ADR 0036: Persisted system appearance and a semantic palette

Status: accepted, 2026-10-04.

## Decision

Reuse the existing `System / Light / Dark` preference without a schema bump.
New state and a missing `theme` field default to System; explicit preferences in
older documents keep their meaning, and unknown values are rejected. Resolve
System against platform appearance at startup and retain a window appearance
subscription. Explicit Light/Dark ignore subsequent system notifications.

On macOS, clear the application's native appearance override before reading its
effective appearance when returning to System. A window can still cache the
previous explicit value until the platform notification arrives. Other platforms
prefer the current window appearance. Set the GPUI Kit mode first, then install
the matching application colors, because changing its mode reloads toolkit colors.

Views copy a semantic palette at render time. Canvas, surface, border, text,
secondary text, accent, selected, primary button states, success, warning and
danger colors cover both custom views and toolkit controls. Keep terminal ANSI
content and dark diff previews independent; application appearance does not
rewrite terminal buffers or interpret remote escape sequences differently.

The compact toolbar exposes bilingual System/Light/Dark choices. Persist through
the existing background state worker and apply only after success. A conflicting
save retains the prior appearance and late edits. Appearance-only saves do not
advance command suggestion source revisions, rebuild input/terminal/AI entities,
or change a reviewed SSH destination. System notifications change visuals only;
they do not write preferences or trigger network work.

## Verification and limits

Meaningful migration, contrast and real GPUI behavior tests cover the preference,
save conflicts, source revisions, retained unsent drafts, existing SSH pane
entities, and the bilingual minimum toolbar. A new independent agent reviewed
the frozen implementation and independently repeated its targeted tests. Earlier
review found and corrected a light odd-row background and a low-contrast fixed
monitor success color.

The final macOS build was operated with isolated state and two controlled TCP SSH
sessions. Explicit appearances, native titlebar, return to System, retained drafts
and Light/Dark restart persistence were observed. The echo fixture executes no
program; monitor refusal is correctly shown, not counted as monitor acceptance.
Actual OS theme notification, all modal/terminal pixels, a reliable minimum
native window, and native Windows/Linux interaction remain separate acceptance.
See [the verification record](../testing/records/2026-10-04-system-themes.md).
