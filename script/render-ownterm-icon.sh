#!/usr/bin/env bash
# Rasterize the OwnTerm icon from its SVG. The SVG is the source.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
svg="$root/app/channels/oss/icon/ownterm.svg"
out="$root/app/channels/oss/icon/no-padding"
docs="$root/docs"

if ! command -v rsvg-convert >/dev/null 2>&1; then
  echo "rsvg-convert is required" >&2
  exit 1
fi

mkdir -p "$out" "$docs"
cp "$svg" "$docs/icon.svg"
cp "$svg" "$docs/favicon.svg"

for size in 16 32 48 64 128 256 512; do
  rsvg-convert -w "$size" -h "$size" "$svg" -o "$out/${size}x${size}.png"
done
rsvg-convert -w 32 -h 32 "$svg" -o "$docs/favicon-32.png"
rsvg-convert -w 180 -h 180 "$svg" -o "$docs/apple-touch-icon.png"

python3 - "$out" <<'PY'
import io
import struct
import sys
from pathlib import Path

from PIL import Image

out = Path(sys.argv[1])
master = Image.open(out / "512x512.png").convert("RGBA")

def png_bytes(size):
    image = master.resize((size, size), Image.Resampling.LANCZOS)
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    return buffer.getvalue()

def write_ico(path, images):
    count = len(images)
    header = struct.pack("<HHH", 0, 1, count)
    entries = b""
    payload = b""
    offset = 6 + 16 * count
    for size, png in images:
        dimension = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dimension, dimension, 0, 0, 1, 32, len(png), offset)
        payload += png
        offset += len(png)
    path.write_bytes(header + entries + payload)

ico_sizes = [16, 32, 48, 64, 128, 256]
write_ico(out / "icon.ico", [(size, png_bytes(size)) for size in ico_sizes])

# PNG-backed icns entries. Sizes follow the Apple icon type table.
entries = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"ic11", 32),
    (b"icp6", 64),
    (b"ic12", 64),
    (b"ic07", 128),
    (b"ic08", 256),
    (b"ic13", 256),
    (b"ic09", 512),
    (b"ic14", 512),
]
body = b""
for ostype, size in entries:
    data = png_bytes(size)
    body += ostype + struct.pack(">I", 8 + len(data)) + data
blob = b"icns" + struct.pack(">I", 8 + len(body)) + body
(out / "ownterm.icns").write_bytes(blob)
PY

echo "rendered OwnTerm icons from $svg"
