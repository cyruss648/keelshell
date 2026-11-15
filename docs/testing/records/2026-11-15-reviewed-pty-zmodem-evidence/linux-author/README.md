# Linux file interoperability fixture — 2026-11-15

Baseline: `721bad2157da8d3a0bc11a1eab5ac59b6616aa9c`.
This isolated checkout changes only `scripts/run_linux_file_interop.py` and
`scripts/test_linux_file_interop.py`; this directory contains task-owned
receipts. No Cargo, target access, core/session/app edits, commit or push.

## Final delivery state

The parent has inspected the native peer build and the successful container
cleanup receipt. All further probes stopped at the parent's request. Copy the
two new scripts and this **entire ignored work directory** explicitly; Git will
not include the work evidence automatically. The exact current inventory and
script hashes are in `FINAL_HANDOFF.json`; `sha256.json` indexes the files.

The native macOS peer, license, source tarball, extracted external source and
build directory remain available until the parent copies/verifies the needed
artifacts. They are task-only test resources. The parent may then remove the
redundant external source/build directories while retaining source provenance,
build receipts and any peers still in use. This worktree must remain open until
that transfer is confirmed; the parent owns its later closure/archive.

## Linux fixture state

**The earlier ready endpoint is historical and must not be used.**

`linux-r1` successfully authenticated SSH, completed the real Linux archive/ACL
probes and reference lrzsz transfer, then failed during its live container lease.
Podman began returning an overlay temporary-directory permission error and
socket connection failure. The runner's `finally` removed its private host
keys and stopped its observed host processes, but could not verify/delete the
remaining containers through the unavailable API at that time.
`linux-r1/result.json` remains deliberately `failed`, despite both functional
probe groups passing.

The authorized bounded final followup is now complete:
`final-followup-r1/result.json` records one API recheck, healthy in **0.177 s**,
then exact full-ID, both-label and name verification for the two pending owned
containers. Both were removed and independently read back absent
(`podman container exists` exit **1**). All observed owned host processes were
stopped. **No owned process or container remains pending.** No new fixture was
created; there is no live SSH endpoint. The earlier failure receipts are intact.

No machine restart, service restart, shared storage repair, image deletion,
volume operation, or action on Slipway containers was attempted.

## Native macOS external lrzsz peer

`native-macos-r1/source-verification.json` records fresh, independent HTTPS
requests on **2026-11-15**, with actual HTTP **200** bodies and headers for:

- `https://ohse.de/uwe/software/lrzsz.html`
- `https://raw.githubusercontent.com/Homebrew/homebrew-core/HEAD/Formula/l/lrzsz.rb`
- The formula's actual release URL:
  `https://www.ohse.de/uwe/releases/lrzsz-0.13.1.tar.gz`

The actual upstream page advertises **0.13.1 (October 2026)**. The actual formula
selects that release and advertises SHA256
`112f91059fbd118e016d800c1a5c5a1702e965d8b9bfbda6015e44e20bfb93c9`.
The fetched **483,782-byte** tarball independently matched that hash **before
extraction**. Archive inspection retained `configure.ac`'s actual
`AC_INIT([lrzsz],[0.13.1])`, source hashes and member validation. The parent-supplied
candidate version/hash was not accepted as evidence. The pre-extraction receipt
remains an immutable checkpoint; the subsequent build is recorded separately.

`native-macos-r1/build-result.json` records a successful unpatched out-of-tree
build with Apple clang **21.0.0**, `--disable-nls`, a private prefix and
`CFLAGS="-O2 -Wno-implicit-int"`. The last flag follows the fetched Homebrew
formula. There was **no Homebrew invocation, host installation or make install**.
Both binaries are real **Mach-O arm64** and depend only on
`/usr/lib/libSystem.B.dylib`:

| Peer path relative to this directory | Actual version | SHA256 |
| --- | --- | --- |
| `native-macos-r1/peer/bin/lrz` | `lrz (GNU lrzsz) 0.13.1` | `8ae7357c579d181fafa209132d4b6f40fff4eaaa00c8178b828f8d8b490d1008` |
| `native-macos-r1/peer/bin/lsz` | `lsz (lrzsz) 0.13.1` | `5e7c1c83b7a211f2c1bea5a30037fcfb20ca37c25d71d4d290ce14a954f1ad90` |

Private `rz`/`sz` symlinks are also provided. Copy the complete `peer/` directory
to retain `COPYING` (GPL-2.0-or-later), AUTHORS, README and NEWS beside the
executables. All GPL material remains under this ignored external test directory;
it has not entered application source, Cargo dependencies or packaging.

The parent can place the copied binary in a reviewed raw-PTY command:

```text
cd POSIX_QUOTED_OWNED_RECEIVE_DIR && stty raw -echo -onlcr && exec POSIX_QUOTED_LRZ_PATH -b -Z
cd POSIX_QUOTED_OWNED_SEND_DIR && stty raw -echo -onlcr && exec POSIX_QUOTED_LSZ_PATH -b -Z -- POSIX_QUOTED_FILE
```

Use the actual copied absolute path and POSIX-quote every path/name. The peers
are available for the parent's existing OpenSSH fixture; this subtask did not
create a new SSH endpoint or run the parent's Rust executable.

Native raw local PTY smoke evidence is separate from the earlier Linux
distribution package **0.12.21rc**:

- `native-macos-r1/binary-crc32.json`: **262,156 bytes**, exit **0/0**, full SHA256
  match using the default CRC32 transfer.
- `native-macos-r1/binary-crc16.json`: the first CRC16 attempt timed out after
  **20 seconds**; sender exited **0**, receiver needed guarded cleanup. The
  original failed aggregate `native-pty-result.json` and script snapshot remain.
- `native-macos-r1/pty-r2/binary-crc16.json`: a bounded diagnostic with protocol
  logging subsequently passed, **262,156 bytes**, exit **0/0**, full SHA256 match.
- `native-macos-r1/pty-r2/multi-file.json`: empty, space-containing and Unicode
  filenames all had the expected complete readback hashes, but the final
  handshake did not complete within **20 seconds**. Sender exited **0**;
  receiver required guarded cleanup. The captured sender wire tail ends at
  ZFIN, without the final `OO` bytes. Root cause remains **UNCLASSIFIED**; no
  upstream or product defect is asserted from this local bridge probe.

Both native probe aggregates deliberately remain **failed**. No further probe
was started after the parent requested finalization, and no aggregate success
is inferred from successful individual cases or matching file contents.
All observed native peer processes were stopped/reaped and scratch payload
directories removed. `native-macos-r1/final-state.json` records the final local
state. The parent owns its separate seven-test Rust/OpenSSH validation.

## Actual evidence

- `linux-r1/archive-python3.json`: **17 passed / 0 failed**, actual Linux
  `aarch64`, kernel `7.1.10-200.fc44.aarch64`, Debian 12, Python **3.11.2**,
  unprivileged **euid 1000**, actual **overlayfs**.
- Exact copied helper SHA256:
  `c38955fe645d7ab26af8a5538d63911af87dda24db0af125e361eb49976f3df2`.
- Both ZIP and tar.gz: ordinary and inherited-default-ACL CLI inspect/pack,
  full member/content/digest readback, private final mode, single link,
  zero remaining ACL xattrs, absent completed stage and unchanged parent ACL.
- Both formats reject changed source and existing destination before creating
  a stage. Actual late kernel ACL injection into the stage or payload rejects
  publication and preserves the quarantine stage. These four race probes use
  observational wrappers around the unchanged helper, not an emulated platform.
- Real fd inherited access/default ACL inspection, clear and reinspection;
  unknown ACL-family xattr fail-closed; symlink, hardlink and FIFO rejection.
- `linux-r1/zmodem-reference.json`: **262,156 bytes** transferred between two
  actual OpenSSH raw PTYs using packaged GNU lrzsz **0.12.21rc**, exits **0/0**,
  complete readback SHA256
  `67a361ed3d5941e4a0bddecd317aaf99728f0989e6d1a1612c2ffa3b570c0b2a`.
  This proves the reference fixture works, not the parent's new Zmodem engine.
- `final-checks-r1`: final fixture-runner unit tests **10/10**, including
  literal quoting, fixed public package-origin/path enforcement, exact
  ID/label ownership, stale lease-token rejection, private stdin exclusion,
  preserved command failure and real birth-checked child/grandchild timeout
  cleanup. Python 3.8 **grammar only** passed for both new scripts.
- `linux-r1/tools.json`: actual OpenSSH **9.2**, setfacl/getfacl **2.3.1**.
  `python3.8` was not installed. **Actual old Python 3.8 runtime remains unverified.**

The final runner adds an explicit host-package-bridge option, an inheritable
remote-root ACL option and more complete reference failure receipts. The r1
source snapshot is preserved separately. The helper bytes and the embedded
Linux archive probes are unchanged. The final runner's complete provisioning
cycle remains **NOT_RERUN**: the bounded recovery followup only verified and
cleaned existing resources, and did not create another fixture.

## Reproduce when the Podman API is healthy

The default base is the already cached public
`docker.io/library/rust:1.98.1-slim-bookworm` image; no Rust executable is
invoked. An explicit cached immutable image ID can be provided with `--image`.
`--pull=never` and unique project/owner labels always apply.

```sh
python3 -B scripts/run_linux_file_interop.py \
  --host-package-bridge \
  --default-acl-root \
  --output work/zmodem-linux-interop-20261115/linux-r2 \
  --hold-seconds 1200
```

The output directory must be new. Without `--hold-seconds`, the one-shot runner
performs its checks and cleans the fixture immediately. Every actual package
operation has a 40-second in-container watchdog and 45-second host deadline.
The optional host bridge relays only public Debian HTTPS files, leaves normal
APT signature/hash verification enabled and closes its local listener.

`readiness.json` is emitted as soon as pinned SSH authentication and tool
availability are confirmed, before archive/transfer probes. It provides:

- `environment_file`: private temporary JSON containing the `KEELSHELL_OPENSSH_*`
  variables, plus `KEELSHELL_LINUX_CONTAINER` and
  `KEELSHELL_LINUX_SSH_CONFIG`; no key bytes are included.
- `ssh_config`: exact fixture-only client configuration, pinned known_hosts,
  no agent, forwarding, password or user SSH configuration.
- `remote_root`, `archive_source`, exact helper hash, receive/send PTY command
  templates, and whether the remote root has the inheritable default ACL.
- `release_file` and `owner`: write the exact owner plus newline after every
  consumer has stopped. Wrong/stale tokens do not release another fixture.

`result.json` means that the runner has finished and temporary keys are gone;
its `readiness.json` is then historical. Confirm that `result.json` does not
exist before loading the temporary environment.

A caller-provided existing test command can instead be passed after `--`.
Select transport-only tests; several other OpenSSH tests manipulate remote
pathnames using the host filesystem and are unsuitable for a container.

```sh
python3 -B scripts/run_linux_file_interop.py \
  --host-package-bridge --default-acl-root \
  --output work/zmodem-linux-interop-20261115/linux-rust-r1 \
  -- /absolute/path/to/parent-validated-copied-openssh-test \
  --ignored --exact --test-threads=1 \
  openssh_server_archive_packs_remote_sources_with_exact_manifest_and_readback
```

The runner neither discovers nor builds that executable and does not claim its
source provenance. The parent must supply its validated copied binary. Other
applicable exact tests include archive-tool inspection, reviewed client archive
pack/extract, and concurrent destination preservation.

## Raw PTY and shared-container interface

For application transport tests, load the private environment into the supplied
test process. Connect to its loopback port using the exact generated client key
and expected host key. Allocate a PTY with echo, canonical input and output
translation disabled. The fixed remote command also applies
`stty raw -echo -onlcr` before exec:

```text
cd /home/fixture/interop/NEW-RECEIVE-DIR && stty raw -echo -onlcr && exec rz -b
cd /home/fixture/interop/SEND-DIR && stty raw -echo -onlcr && exec sz -b -- FILE
```

Create fresh, owned directories first and quote every actual path/basename with
POSIX shell quoting. These are binary protocol streams; do not route them
through text decoding, terminal rendering or newline normalization. Use the
same container to invoke `/fixture/server_archive.py` with `python3` and to
inspect kernel ACLs with `setfacl/getfacl`. `--default-acl-root` sets a real
named-user inheritable ACL before the parent command starts; subsequent Rust
fixture directories therefore inherit it.

Use `ssh -F <ssh_config> -tt fixture <reviewed-raw-command>` for a CLI reference,
or the generated key/host-key fields directly in the parent's SSH transport.
The helper's ordinary CLI `inspect` and `pack` protocol remains unchanged.

## Completed resource cleanup

`cleanup-attempt-r1/result.json` preserves the earlier unsuccessful guarded
attempts. `final-followup-r1/result.json` and its command logs record the final
successful cleanup. Each **full ID**, **both** labels and exact name was
verified before removing only that ID; each subsequent
`podman container exists` returned exit **1**:

| Container | Full ID | Owner |
| --- | --- | --- |
| `keelshell-file-interop-504212acd132` | `8fe260493f5bff36406c62dd116ddb23801f9c169867065a02bb98a61ec6e33b` | `504212acd1324ac694dd5ba1e2cf1950` |
| `keelshell-file-interop-618e6bfd6e38` | `013138d70686a25b2a4b90897e7d6a8206d9825cdb9a56dbd29c4e097579200e` | `618e6bfd6e384bc882a696297c201ecc` |

Both listed containers are now absent. The original
`feasibility-r2/result.json` and `feasibility-r3/result.json`
preserve their ownership provenance. Verified labels were
`io.keelshell.fixture=linux-file-interop` and
`io.keelshell.owner=<exact owner>`. The first failed Ubuntu fixture
`keelshell-file-interop-97f5cd562d63` was already verified removed in
`feasibility-r1/result.json`. No images or volumes were created, and no
shared API, storage or VM repair was attempted.

`feasibility-r1/r2/r3` and their public-package bridge logs preserve the direct
download timeouts, unavailable Alpine lrzsz package and the corrected encoded
package-version URL failure. Host public-download scratch bytes and Python
bytecode created during initial discovery were removed. All remaining local
files under this directory are durable task receipts or inert source snapshots.

None of this signs Linux desktop UI, the application's in-progress Zmodem
implementation, a Rust OpenSSH executable not supplied by the parent, other
Linux filesystems/distributions, actual Python 3.8, Windows, packaging or release.
