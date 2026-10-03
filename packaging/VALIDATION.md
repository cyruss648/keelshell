# Icon and packaging validation — 2026-10-03

The approved white-tile source is 1254 × 1254 RGBA. Its SHA-256 is
`4ec61292dba2a8c9a151b47beb1bcfea8114be33f86d52c6c931f9aedb6fcf60`.
Pillow 12.3.0 was installed in an ignored repository-local virtual environment;
the icon manifest records the encoder versions and all generated file hashes.

Completed on macOS:

- `scripts/build-icons.py` generated and validated 35 image/container files.
- `scripts/build-icons.py --verify` checked their dimensions, pixels, alpha,
  hashes, ICO directory and ICNS table of contents without writing files.
- Repeating generation with the same source and toolchain preserved all 37
  source/manifest/artifact file hashes byte for byte.
- A separate Pillow ICO decoder read all nine resolutions and recovered exactly
  the expected RGBA pixels.
- Apple's `iconutil` decoded the ICNS successfully into eleven PNG entries.
- Deliberately corrupted ICO directory counts and ICNS file lengths were rejected
  by the validators.
- Native-size 16, 32 and 64 px previews were inspected. The K and white tile remain
  recognizable. The 16 px border/shadow is relatively prominent, but there were
  no obvious broken contours or new jagged artifacts. No artwork was retouched.
  Alpha spans 0–255 at all three sizes, with 31, 49 and 83 distinct alpha levels.
- The actual existing macOS debug Mach-O was staged into a `.app` in the ignored
  `work/packages/macos-icon-validation` directory. Its `Info.plist` passed
  `plutil -lint`; the manifest records the exact binary hash.
- Linux staging correctly rejected that macOS binary instead of generating a
  misleading Linux application tree.
- Python syntax compilation, dependency version policy and whitespace checks
  passed.

The debug executable used for packaging validation predates the ongoing UI
refresh. This test proves staging and resource layout, not current-feature
acceptance. No app was installed, launched, signed, notarized or published by the
packaging scripts. Windows resource compilation and Explorer/taskbar acceptance,
native Linux compilation/desktop icon lookup, and current macOS Dock/launch
acceptance remain separate gates. A format conversion is not native execution
evidence.

Subsequent integration adds the process identity, matching Linux window/desktop
IDs, and a Linux-only embedded PNG decoder for X11. The APIs were checked in the
resolved GPUI source; `image` reuses the already-locked 0.25.10 version with a
0.25 requirement. Formatting and manifest policy were checked, but Cargo was
left to the root agent's unified gate. These source checks are not a native X11,
Wayland, Windows or macOS acceptance result.

## Root 原生补充 — 2026-10-03

最新 macOS debug 构建打包后实际启动，SSH/SFTP/AI受控流程通过。访达应用简介与预览正确显示白底蓝青K图标。最新远程文件编辑器高度修复也经过实际截图核对。记录见 docs/testing/records/2026-10-03-remote-ai-icons.md。此补充不改变 Windows/Linux 原生、签名和安装仍未验证的边界。
