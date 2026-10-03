#!/usr/bin/env python3
"""Resize an approved PNG source and validate portable application icon formats.

This script changes sizes/containers only: it never draws, crops, retouches,
composites a background or modifies the approved icon design.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import sys
import tempfile

try:
    import PIL
    from PIL import Image, features
except ImportError:
    raise SystemExit("Pillow is required: install packaging/icon-requirements.txt in a virtual environment")

ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "assets" / "icons"
PNG_SIZES = (16, 20, 24, 32, 40, 48, 64, 96, 128, 256, 512, 1024)
ICO_SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256)
LINUX_SIZES = (16, 24, 32, 48, 64, 96, 128, 256, 512, 1024)
ICONSET = {
    "icon_16x16.png": 16, "icon_16x16@2x.png": 32,
    "icon_32x32.png": 32, "icon_32x32@2x.png": 64,
    "icon_128x128.png": 128, "icon_128x128@2x.png": 256,
    "icon_256x256.png": 256, "icon_256x256@2x.png": 512,
    "icon_512x512.png": 512, "icon_512x512@2x.png": 1024,
}
ICNS = {
    b"icp4": 16, b"icp5": 32, b"icp6": 64,
    b"ic07": 128, b"ic08": 256, b"ic09": 512, b"ic10": 1024,
    b"ic11": 32, b"ic12": 64, b"ic13": 256, b"ic14": 512,
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def source_image(path: Path) -> Image.Image:
    with Image.open(path) as image:
        require(image.format == "PNG", "Source must be a PNG")
        require(image.width == image.height and 1024 <= image.width <= 4096,
                "Source must be square and between 1024 and 4096 pixels")
        require(image.mode == "RGBA", "Source must have a native RGBA alpha channel")
        image.load()
        result = image.copy()
    low, high = result.getchannel("A").getextrema()
    require(low == 0 and high == 255, "Source must include both fully transparent and opaque pixels")
    return result


def encode_png(image: Image.Image) -> bytes:
    buffer = io.BytesIO()
    image.save(buffer, format="PNG", optimize=False, compress_level=9)
    return buffer.getvalue()


def encode_ico(pngs: dict[int, bytes]) -> bytes:
    offset = 6 + 16 * len(ICO_SIZES)
    directory, images = [], []
    for size in ICO_SIZES:
        data = pngs[size]
        directory.append(struct.pack("<BBBBHHII", size % 256, size % 256,
                                     0, 0, 1, 32, len(data), offset))
        images.append(data)
        offset += len(data)
    return struct.pack("<HHH", 0, 1, len(directory)) + b"".join(directory + images)


def encode_icns(pngs: dict[int, bytes]) -> bytes:
    entries = [(kind, pngs[size]) for kind, size in ICNS.items()]
    toc = b"".join(kind + struct.pack(">I", len(data) + 8) for kind, data in entries)
    chunks = [b"TOC " + struct.pack(">I", len(toc) + 8) + toc]
    chunks.extend(kind + struct.pack(">I", len(data) + 8) + data for kind, data in entries)
    data = b"".join(chunks)
    return b"icns" + struct.pack(">I", len(data) + 8) + data


def expected_files(source: Image.Image) -> dict[str, bytes]:
    pngs = {size: encode_png(source.resize((size, size), Image.Resampling.LANCZOS))
            for size in PNG_SIZES}
    files = {f"png/{size}.png": data for size, data in pngs.items()}
    files.update({f"macos/KeelShell.iconset/{name}": pngs[size] for name, size in ICONSET.items()})
    files.update({f"linux/hicolor/{size}x{size}/apps/keelshell.png": pngs[size] for size in LINUX_SIZES})
    files["macos/KeelShell.icns"] = encode_icns(pngs)
    files["windows/keelshell.ico"] = encode_ico(pngs)
    files["preview.png"] = pngs[256]
    return files


def check_png(data: bytes, size: int, expected: Image.Image) -> dict:
    with Image.open(io.BytesIO(data)) as image:
        image.load()
        require(image.format == "PNG" and image.mode == "RGBA", "Icon payload must be RGBA PNG")
        require(image.size == (size, size), f"Wrong image size for {size}px icon")
        require(image.tobytes() == expected.tobytes(), f"Changed pixels or alpha at {size}px")
        low, high = image.getchannel("A").getextrema()
        require(low == 0 and high > 0, f"Transparency/content missing from {size}px icon")
        return {"width": size, "height": size, "mode": image.mode, "alpha_min": low, "alpha_max": high}


def check_ico(data: bytes, images: dict[int, Image.Image]) -> list[int]:
    require(len(data) >= 6, "ICO header is truncated")
    reserved, kind, count = struct.unpack_from("<HHH", data)
    require((reserved, kind, count) == (0, 1, len(ICO_SIZES)), "ICO directory count/type mismatch")
    offset = 6 + count * 16
    sizes = []
    for index in range(count):
        width, height, colors, reserved, planes, bpp, length, entry_offset = struct.unpack_from("<BBBBHHII", data, 6 + index * 16)
        width, height = width or 256, height or 256
        require(width == height and width == ICO_SIZES[index], "ICO resolution list mismatch")
        require((colors, reserved, planes, bpp) == (0, 0, 1, 32), "ICO alpha/plane metadata mismatch")
        require(entry_offset == offset and offset + length <= len(data), "ICO payload offset/length mismatch")
        check_png(data[offset:offset + length], width, images[width])
        offset += length
        sizes.append(width)
    require(offset == len(data), "ICO has unaccounted bytes")
    return sizes


def check_icns(data: bytes, images: dict[int, Image.Image]) -> dict[str, int]:
    require(data[:4] == b"icns" and len(data) >= 8, "ICNS header mismatch")
    require(struct.unpack_from(">I", data, 4)[0] == len(data), "ICNS file length mismatch")
    offset, found, toc, table = 8, {}, None, []
    while offset < len(data):
        require(offset + 8 <= len(data), "ICNS chunk header is truncated")
        kind, length = struct.unpack_from(">4sI", data, offset)
        require(length >= 8 and offset + length <= len(data), "ICNS chunk length mismatch")
        payload = data[offset + 8:offset + length]
        if kind == b"TOC ":
            require(toc is None, "ICNS has duplicate table of contents")
            toc = payload
        else:
            require(kind in ICNS and kind not in found, "ICNS has unknown or duplicate icon type")
            size = ICNS[kind]
            check_png(payload, size, images[size])
            found[kind] = size
            table.append(kind + struct.pack(">I", length))
        offset += length
    require(found == ICNS, "ICNS resolution directory mismatch")
    require(toc == b"".join(table), "ICNS table of contents does not match its payloads")
    return {kind.decode("ascii"): size for kind, size in found.items()}


def validate(source: Image.Image, files: dict[str, bytes]) -> dict:
    images = {size: source.resize((size, size), Image.Resampling.LANCZOS) for size in PNG_SIZES}
    records = {}
    for name, data in sorted(files.items()):
        record = {"bytes": len(data), "sha256": digest(data)}
        if name.endswith(".png"):
            with Image.open(io.BytesIO(data)) as image:
                size = image.width
            require(size in images, f"Unexpected PNG size in {name}")
            record.update(check_png(data, size, images[size]))
        elif name.endswith(".ico"):
            record["sizes"] = check_ico(data, images)
        elif name.endswith(".icns"):
            record["directory"] = check_icns(data, images)
        records[name] = record
    return records


def write_file(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as temporary:
        temporary.write(data)
        temporary_path = Path(temporary.name)
    try:
        temporary_path.chmod(0o644)
        temporary_path.replace(path)
    finally:
        temporary_path.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=ICONS / "source.png")
    parser.add_argument("--output", type=Path, default=ICONS)
    parser.add_argument("--verify", action="store_true", help="Check stored products without changing files")
    args = parser.parse_args()
    source = source_image(args.source)
    expected = expected_files(source)
    manifest_path = args.output / "manifest.json"
    if args.verify:
        files = {name: (args.output / name).read_bytes() for name in expected}
        records = validate(source, files)
        manifest = json.loads(manifest_path.read_text())
        require(manifest["source"]["sha256"] == digest(args.source.read_bytes()), "Source hash changed; regenerate icons")
        require(manifest["files"] == records, "Artifact hashes/metadata changed; regenerate icons")
        require(manifest["source"]["size"] == [source.width, source.height], "Source dimensions changed")
    else:
        records = validate(source, expected)
        for name, data in expected.items():
            write_file(args.output / name, data)
        manifest = {
            "schema_version": 1,
            "source": {"name": args.source.name, "sha256": digest(args.source.read_bytes()),
                       "size": [source.width, source.height], "mode": "RGBA"},
            "conversion": {"pillow": PIL.__version__, "zlib": features.version_codec("zlib"),
                           "filter": "LANCZOS", "png_compress_level": 9,
                           "content_edits": False},
            "files": records,
        }
        write_file(manifest_path, (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode())
    print(f"{'Verified' if args.verify else 'Generated and verified'} {len(expected)} icon files; source {source.width}x{source.height} RGBA")
    print(f"ICO: {', '.join(map(str, ICO_SIZES))}; ICNS: {len(ICNS)} image entries; alpha preserved")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, struct.error) as error:
        print(f"Icon build failed: {error}", file=sys.stderr)
        sys.exit(1)
