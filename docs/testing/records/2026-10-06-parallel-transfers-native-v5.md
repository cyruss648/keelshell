# Native parallel uploads — 2026-10-06

Status: the controlled macOS arm64 two-upload scenario passed. This covers a
fresh standard development package, explicit SSH trust and upload reviews,
acknowledged pause/continue, two simultaneously Running tasks, two fully visible
Completed rows, separate task path inspection, exact remote contents and owned
resource cleanup. It does not close the complete remote SSH product, all
transfer modes, minimum-window matrix or Windows/Linux native acceptance.

## Source and package

The fresh admission bound 615 complete engineering inputs, 13,726,803 bytes.
The input map SHA-256 was
`569579c00563b50c0d311d5c8e48399007cb1027b90897bb4e628b4ea830d609`.
Compared with the [combined-main gate](2026-10-06-remote-workspace-main-integration.md),
only eleven existing Markdown files differed. Rust, tests, Cargo manifests,
lockfile, toolchain and scripts stayed equal. That earlier gate passed 1,394
ordinary tests, eight doctests, six Python tests, strict checks, eleven OpenSSH
tests and 57 packaging tests; it is not a test run of this later documentation.

New locked application/companion and fixture build commands, standard staging
and package inspection all returned 0. Cargo could reuse unchanged artifacts;
these are not clean-rebuild claims. The six staged files matched the manifest,
with native arm64 architecture, expected system dependencies, bilingual bundle
metadata, white icon and macOS 15 deployment minimum. No existing application
was replaced and no release was published.

A new nonauthor prestart review checked complete source bodies, bound binaries,
full logs and receipts. Its first auxiliary locale assertion incorrectly
expected a different locale spelling; the corrected validator accepted the
actual Apple bundle spelling without changing production files. Both outputs
remain retained. Prestart review alone does not establish runtime success.

## Actual native observations

The Chinese/System window connected to its owned loopback SSH/SFTP service.
The displayed fingerprint matched that service's startup identity. The public
fixture password was used only for the current session, without saving it.
Each source was separately reviewed and confirmed for its destination:
`/bin/parallel-one.bin` and `/bin/parallel-two.bin`. Both files were 4 MiB and the
fixture retained its original 250 ms I/O delay.

The first task visibly progressed, then its Pause action acknowledged
1,540,096 / 4,194,304 bytes. It remained Paused while the second independent
upload was reviewed and started. The second showed Running at 163,840 bytes.
Continuing the first task produced one actual screenshot with both Running:

| Task | Completed bytes | Total bytes | State |
| --- | ---: | ---: | --- |
| `/bin/parallel-one.bin` | 1,572,864 | 4,194,304 | Running |
| `/bin/parallel-two.bin` | 2,031,616 | 4,194,304 | Running |

A later screenshot showed both Completed at 4,194,304 / 4,194,304 bytes. The
second row was initially clipped by the footer; scrolling made both complete
rows and their result messages visible together. Each row was selected
separately and its Path popup showed its own full local source and remote
destination. Task identity was checked through these distinct paths and
selection; numeric job IDs are not a product requirement.

In this native automation session, `scroll(..., "up", ...)` revealed later
content and `"down"` returned to earlier content. Actual list and tool scrolling
therefore established queue reachability at this window size. Earlier attempts
in only one direction do not establish a wheel or layout defect.

These visual facts are the root agent's actual CUA observations in task tool
history. There is no claimed standalone PNG archive. The independent reviewer
checked the recorded facts' consistency, binding and cleanup; it did not
directly review the screenshot pixels.

## Content and lifecycle readback

Before stopping the service, root read both real fixture files and matched
their byte counts and SHA-256 against their respective sources:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `parallel-one.bin` | 4,194,304 | `a58789e910e5f939afc433a00fef5930702927dc192cb237fd9e7449bd6ffe1d` |
| `parallel-two.bin` | 4,194,304 | `61d678b48de600e6922df82ac9fb5d208d19e98064d0d1d5c14a2ee50481c593` |

The controller independently obtained identical readback before stopping.
Application and fixture were explicitly waited; their admitted process births
and original groups were gone. The reader stopped, the loopback port returned
ECONNREFUSED and the owned private directory was removed. The same controller
execution returned 0 with no failures. Root and the nonauthor reviewer then
independently rechecked the controller, application and fixture as absent,
their original groups absent, the port refusing connections and private data
absent. This concerns the explicitly owned resources, without claiming a census
of every unobserved descendant.

## Remaining boundaries

The fixture rejects arbitrary exec, so this scene does not validate Linux
monitoring or arbitrary shell commands. Its Up-directory action returned
Permission denied; explicitly setting `/` and `/bin` and refreshing succeeded.
That negative observation is retained and does not establish complete
navigation acceptance. A second independent Running-growth image was not
obtained. Cancellation, isolation-risk acknowledgement, all transfer modes,
minimum-window, language/theme and other-platform matrices remain separate.

The original zero-completion upload failure and previous partial scene remain
retained with their original scope. This success does not retrospectively
diagnose them. Target-parameter and disk-monitor native checks, new-head CI,
full releases and installed updates remain open. Documentation added after the
frozen admission is not represented as part of those 615 input bodies.
