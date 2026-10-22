"""Builds the platform icon files from raw RGBA dumps of the app icon.

    for n in 16 32 48 64 128 256 512 1024; do
        ICON_OUT=/tmp/icon_$n.rgba ICON_SIZE=$n cargo test --release dump_icon
    done
    python3 packaging/tools/make_icons.py /tmp            # reads /tmp/icon_<n>.rgba

Writes packaging/windows/tubeglass.ico, packaging/macos/tubeglass.icns and
packaging/tubeglass.png. Only the standard library is needed.
"""
import os
import struct
import sys
import zlib


def png(rgba: bytes, n: int) -> bytes:
    """Encodes an n x n RGBA image as a PNG."""
    raw = b"".join(b"\x00" + rgba[y * n * 4:(y + 1) * n * 4] for y in range(n))

    def chunk(kind: bytes, data: bytes) -> bytes:
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", n, n, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def main(src: str) -> None:
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.normpath(os.path.join(here, ".."))
    imgs = {}
    for n in (16, 32, 48, 64, 128, 256, 512, 1024):
        with open(os.path.join(src, f"icon_{n}.rgba"), "rb") as f:
            imgs[n] = png(f.read(), n)

    # .ico: a directory of PNG images (supported since Windows Vista)
    sizes = [16, 32, 48, 64, 128, 256]
    out = struct.pack("<HHH", 0, 1, len(sizes))
    offset = 6 + 16 * len(sizes)
    body = b""
    for n in sizes:
        data = imgs[n]
        out += struct.pack("<BBBBHHII", 0 if n >= 256 else n, 0 if n >= 256 else n, 0, 0, 1, 32, len(data), offset + len(body))
        body += data
    with open(os.path.join(root, "windows", "tubeglass.ico"), "wb") as f:
        f.write(out + body)

    # .icns: PNG payloads (macOS 10.7+)
    kinds = [(b"icp4", 16), (b"icp5", 32), (b"icp6", 64), (b"ic07", 128), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024)]
    parts = b"".join(k + struct.pack(">I", 8 + len(imgs[n])) + imgs[n] for k, n in kinds)
    with open(os.path.join(root, "macos", "tubeglass.icns"), "wb") as f:
        f.write(b"icns" + struct.pack(">I", 8 + len(parts)) + parts)

    with open(os.path.join(root, "tubeglass.png"), "wb") as f:
        f.write(imgs[256])


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
