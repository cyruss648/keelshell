# Application icon assets

`source.png` is the original approved imagegen PNG. Keep it unchanged when
regenerating sizes; do not reconstruct the mark in drawing code. The final design
uses a white rounded tile, blue/cyan keel-inspired K, and transparent exterior.

`scripts/build-icons.py` performs only square LANCZOS resampling and container
encoding. It does not crop, paint, remove backgrounds or change the design.

- `png/`: 16–1024 px RGBA icons for application UI and previews.
- `macos/KeelShell.iconset/`: Apple's ten 1×/2× source filenames.
- `macos/KeelShell.icns`: eleven PNG image entries plus a checked directory.
- `windows/keelshell.ico`: nine 32-bit RGBA sizes, including 256 px.
- `linux/hicolor/`: ten standard per-size application-icon directories.
- `preview.png`: unchanged-design 256 px preview.
- `manifest.json`: source hash, encoder versions, every artifact's SHA-256,
  dimensions/alpha range, and ICO/ICNS directory metadata.

See `packaging/README.md` for setup, validation and bundle staging. A generated
icon container does not by itself prove that Explorer, Dock or a Linux desktop
displays the packaged app correctly; that requires native acceptance.
