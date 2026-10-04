# Application packaging

`package.py` creates reviewable local staging folders without installation,
signing or publication. `release.py` verifies and archives them; `publish.py`
publishes an explicitly named GitHub Release after all assets are verified.
See [release operations](../docs/RELEASING.md). Always supply a new output
directory to preserve previous artifacts and evidence.

## Regenerate and validate icons

Use Python 3.11+ and an isolated environment; no global Python installation is
changed:

```sh
python3 -m venv work/icon-tools
work/icon-tools/bin/python -m pip install -r packaging/icon-requirements.txt
work/icon-tools/bin/python scripts/build-icons.py
work/icon-tools/bin/python scripts/build-icons.py --verify
```

On Windows use `work\icon-tools\Scripts\python.exe` for the last three commands.
The dependency requirement is `Pillow~=12.3`; the conversion manifest records
the actual resolved version. Pillow 12.3.0 is the latest stable version observed
on 2026-10-03 in the [official release notes](https://pillow.readthedocs.io/en/stable/releasenotes/index.html).

Input is the approved square RGBA PNG at `assets/icons/source.png`, from 1024
through 4096 pixels, with transparent exterior. The file is retained verbatim.
The script validates source alpha, pixel/alpha equivalence after resampling,
all PNG dimensions, every ICO offset/resolution, the ICNS table of contents and
embedded PNG entries, and all artifact hashes. It adds no timestamp or absolute
developer path to the manifest. Byte-for-byte reproduction requires the same
Pillow/zlib versions; those are recorded alongside source and artifact hashes.

macOS may also validate that its native decoder accepts the generated container:

```sh
iconutil --convert iconset --output work/KeelShell-native.iconset assets/icons/macos/KeelShell.icns
```

The iconset has the ten filenames from Apple's
[high resolution icon guide](https://developer.apple.com/library/archive/documentation/GraphicsAnimation/Conceptual/HighResolutionOSX/Optimizing/Optimizing.html).
The containers use PNG alpha, supported by
[Pillow's ICNS and ICO documentation](https://pillow.readthedocs.io/en/stable/handbook/image-file-formats.html#icns).

## macOS `.app`

```sh
cargo build -p keelshell-app -p keelshell-mcp --release --locked
python3 packaging/package.py macos --binary target/release/keelshell-app --mcp-binary target/release/keelshell-mcp --output work/packages/macos-0.1.0
plutil -lint work/packages/macos-0.1.0/KeelShell.app/Contents/Info.plist
```

The staging script checks the Mach-O header and icon hashes, writes
`Contents/MacOS/keelshell-app`, its adjacent `Contents/MacOS/keelshell-mcp`,
and `Contents/Resources/KeelShell.icns`, and sets
`CFBundleIconFile=KeelShell.icns`. It reads the version from the workspace
manifest. The local bundle identifier is `app.keelshell.desktop`. Chinese is the
development language with Chinese/English localizations declared. The minimum
system version is macOS 15.0, matching the release build deployment target. See Apple's
[bundle key documentation](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html).

This is a local development bundle. Signing/notarization and launch acceptance
must be performed separately before external distribution. The script makes no
claim that a copied binary has passed a launch, deployment-target or dependency
audit. It preserves binary bytes and records their hash.

## Windows executable resources

`crates/keelshell-app/build.rs` embeds the ICO as resource ID 1 and version
information. It uses `CARGO_CFG_TARGET_OS`, because the build script host OS can
differ from the target. GPUI supplies the sole application manifest with
`asInvoker` privileges and PerMonitorV2 DPI support. Declaring another manifest
in the app would duplicate resource 1/RT_MANIFEST; long-path support is not claimed.

The direct build dependency is `winresource = "0.1"`; 0.1.31 is the latest version
observed on 2026-10-03. Its [official documentation](https://docs.rs/winresource/latest/winresource/)
requires Windows SDK `rc.exe` for MSVC, or a suitable MinGW `windres` toolchain
for GNU targets. The build fails if Windows resources cannot compile, instead of
silently emitting an unbranded executable. No SDK is installed by our scripts.

On a prepared Windows host:

```powershell
cargo build -p keelshell-app -p keelshell-mcp --release --locked
python packaging/package.py windows --binary target/release/keelshell-app.exe --mcp-binary target/release/keelshell-mcp.exe --output work/packages/windows-0.1.0
```

The script checks both PE signatures and stages `keelshell-app.exe`, the adjacent
`keelshell-mcp.exe`, and the ICO. This is not
an installer or a resource-loader test. Verify Explorer/taskbar icons, resource
ID 1, manifest, DPI behavior and launch on native Windows before acceptance.

## Linux application tree

On a prepared Linux host:

```sh
cargo build -p keelshell-app -p keelshell-mcp --release --locked
python3 packaging/package.py linux --binary target/release/keelshell-app --mcp-binary target/release/keelshell-mcp --output work/packages/linux-0.1.0
desktop-file-validate work/packages/linux-0.1.0/usr/share/applications/keelshell.desktop
```

The output contains adjacent `usr/bin/keelshell-app` and `usr/bin/keelshell-mcp`, the desktop entry and per-size icons under
`usr/share/icons/hicolor`. `Icon=keelshell` uses theme lookup according to the
[freedesktop icon specification](https://specifications.freedesktop.org/icon-theme/latest/).
The desktop entry has Chinese/English labels, `Terminal=false` and an executable
name without shell interpolation, following the
[desktop entry keys specification](https://specifications.freedesktop.org/desktop-entry/latest/recognized-keys.html).
The script checks ELF format, but does not prove ABI compatibility or install
desktop dependencies. Test launch, Wayland/X11 desktop association, icon lookup
and the target distribution separately.

At startup, the app sets process identity `app.keelshell.desktop` (matching the
macOS bundle identifier and Windows AppUserModelID) and window `app_id=keelshell`
(matching `keelshell.desktop`). GPUI's current Linux backend maps the latter to
Wayland's app ID and X11's instance/class `WM_CLASS`; the desktop entry therefore
also declares `StartupWMClass=keelshell`. On X11 the app additionally supplies
the embedded 256 px RGBA PNG as `WindowOptions.icon`, so running the binary does
not depend on a working-directory-relative image path.

PNG decoding is enabled only for the app's Linux target with workspace
`image = { version = "0.25", default-features = false, features = ["png"] }`.
The [current image crate docs](https://docs.rs/image/latest/image/) identify
0.25.10; that exact version was already in the application's lockfile through
GPUI, so this adds no second image-crate version or additional default codecs.
macOS and Windows continue to obtain their native icon from `.icns` bundle
resources and the PE resource respectively, without compiling this Linux-only
decoder helper. This integration is statically verified against GPUI Kit 0.7 /
GPUI pre 0.3.7; native desktop behavior still requires the gates below.

## Installer candidate and proof boundaries

`cargo-packager` 0.11.8 was the latest observed official release; if adopted,
use a `0.11` requirement. Its [official docs](https://docs.rs/cargo-packager/latest/cargo_packager/)
support `.app`/DMG, Linux packages and Windows installers. It is not added to the
build or invoked here: installer policy, platform signing and native validation
remain explicit release work.

Every staged folder requires the explicitly supplied application and MCP binaries.
It includes `package-manifest.json` with file and separate application/MCP binary hashes,
source icon identity, version and explicit `installed=false` / native-validation
boundaries. A successful format conversion or macOS staging run is not evidence
of Windows/Linux compilation, native execution, installation or distribution.

The manifest keeps schema 1 and adds required `mcp_binary_sha256`. Release validation
checks both fixed executable paths, their target architectures, file hashes and Unix
execute bits; native inspection checks both runtime dependencies and macOS minimum
versions. Windows icon/DPI resources remain an application-only check. Historical
application helpers can read the extended schema and install the companion through
the existing `files` list. New application validators reject packages missing the
companion or its receipt before installation. See [ADR 0041](../docs/adr/0041-mcp-companion-packaging-and-update-recovery.md).

Updates replace both installed images through the reviewed manifest. Running stdio
MCP processes must be restarted by the external agent to use the new image; a locked
Windows image may cause bounded retries and rollback. A failed rollback preserves
its exact staging directory and old-file backups and stops restart/retry. Updates
across filesystem volumes may fail when moving old files into the staging backup;
this is a refused installation boundary, not a successful installed update. Real
installed-directory and Windows/Linux native process acceptance remain separate.
