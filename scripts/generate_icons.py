#!/usr/bin/env python3
"""Rebuild platform icon containers from the checked-in native SVG."""
from pathlib import Path
import shutil
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "assets/brand/zcall-icon.svg"
DEST = ROOT / "packaging"

def main():
    if not shutil.which("rsvg-convert"):
        raise SystemExit("Install librsvg's rsvg-convert to regenerate developer icon assets")
    images = {}
    for size in (16, 32, 48, 64, 128, 256, 512, 1024):
        data = subprocess.check_output(["rsvg-convert", "-w", str(size), "-h", str(size), str(SOURCE)])
        if data[:8] != b"\x89PNG\r\n\x1a\n" or struct.unpack(">II", data[16:24]) != (size, size):
            raise SystemExit("The SVG renderer returned an invalid icon size")
        images[size] = data
    DEST.mkdir(exist_ok=True)
    (DEST / "zcall-gui.svg").write_bytes(SOURCE.read_bytes())
    (DEST / "zcall-gui.png").write_bytes(images[256])
    sizes = (16, 32, 48, 64, 128, 256)
    offset = 6 + 16 * len(sizes)
    entries = []
    for size in sizes:
        data = images[size]
        entries.append(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    (DEST / "zcall-gui.ico").write_bytes(struct.pack("<HHH", 0, 1, len(sizes)) + b"".join(entries) + b"".join(images[size] for size in sizes))
    chunks = []
    for kind, size in ((b"ic07", 128), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024)):
        data = images[size]
        chunks.append(kind + struct.pack(">I", len(data) + 8) + data)
    body = b"".join(chunks)
    (DEST / "zcall-gui.icns").write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)

if __name__ == "__main__":
    main()
