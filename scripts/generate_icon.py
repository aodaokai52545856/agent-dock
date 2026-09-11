"""Build Agent Dock PNG/ICO/ICNS from scripts/icon-master.png."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
MASTER = Path(__file__).resolve().parent / "icon-master.png"
OUT = ROOT / "src-tauri" / "icons"

# Face of the 3D tile is ~lum 32–56. The “transparent” canvas is baked RGB ~220+.
TILE_LUM = 90
RIM_LUM = 110
GLYPH_LUM = 120


def png_chunk(tag: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def write_png(path: Path, image: Image.Image) -> None:
    image = image.convert("RGBA")
    size = image.size[0]
    raw = b"".join(b"\x00" + image.tobytes()[y * size * 4 : (y + 1) * size * 4] for y in range(size))
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n" + png_chunk(b"IHDR", ihdr) + png_chunk(b"IDAT", zlib.compress(raw, 9)) + png_chunk(b"IEND", b"")
    )


def write_icns(path: Path, png_by_type: dict[str, bytes]) -> None:
    chunks = b"".join(
        tag.encode("ascii") + struct.pack(">I", 8 + len(data)) + data for tag, data in png_by_type.items()
    )
    path.write_bytes(b"icns" + struct.pack(">I", 8 + len(chunks)) + chunks)


def write_ico(path: Path, images: list[tuple[int, bytes]]) -> None:
    offset = 6 + 16 * len(images)
    entries = []
    for size, png in images:
        entries.append((size, offset, png))
        offset += len(png)
    data = struct.pack("<HHH", 0, 1, len(images))
    for size, offset, png in entries:
        data += struct.pack("<BBBBHHII", size if size < 256 else 0, size if size < 256 else 0, 0, 0, 1, 32, len(png), offset)
    for _, _, png in entries:
        data += png
    path.write_bytes(data)


def to_square(image: Image.Image) -> Image.Image:
    side = max(image.size)
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(image, ((side - image.size[0]) // 2, (side - image.size[1]) // 2), image)
    return canvas


def _glyph_box(width: int, height: int) -> tuple[int, int, int, int]:
    return int(width * 0.28), int(width * 0.72), int(height * 0.28), int(height * 0.72)


def isolate_tile(image: Image.Image) -> Image.Image:
    """Drop the baked-in light canvas. Keep the dark tile and the silver caret."""
    image = image.convert("RGBA").copy()
    width, height = image.size
    pixels = image.load()
    dark = Image.new("L", (width, height), 0)
    dark_px = dark.load()
    for y in range(height):
        for x in range(width):
            red, green, blue, _alpha = pixels[x, y]
            if (red + green + blue) / 3 < TILE_LUM:
                dark_px[x, y] = 255
    keep = dark.filter(ImageFilter.MaxFilter(9))
    keep_px = keep.load()
    gx0, gx1, gy0, gy1 = _glyph_box(width, height)
    for y in range(height):
        for x in range(width):
            red, green, blue, _alpha = pixels[x, y]
            lum = (red + green + blue) / 3
            in_glyph = gx0 <= x < gx1 and gy0 <= y < gy1 and lum > GLYPH_LUM
            if in_glyph:
                continue
            if keep_px[x, y] == 0 or lum > RIM_LUM:
                pixels[x, y] = (0, 0, 0, 0)
    box = image.getbbox()
    if not box:
        raise SystemExit("icon-master.png has no dark tile")
    return to_square(image.crop(box))


def punch_light_rim(image: Image.Image, light: float) -> Image.Image:
    """Turn leftover light silhouette pixels into transparency. Do not paint them black."""
    image = image.copy()
    width, height = image.size
    pixels = image.load()
    alpha = image.getchannel("A")
    holes = alpha.point(lambda value: 255 if value < 28 else 0)
    radius = max(3, int(min(width, height) * (0.12 if min(width, height) <= 48 else 0.03)))
    if radius % 2 == 0:
        radius += 1
    rim = holes.filter(ImageFilter.MaxFilter(radius))
    rim_px = rim.load()
    gx0, gx1, gy0, gy1 = _glyph_box(width, height)
    for y in range(height):
        for x in range(width):
            red, green, blue, alpha_v = pixels[x, y]
            if alpha_v == 0:
                continue
            lum = (red + green + blue) / 3
            in_glyph = gx0 <= x < gx1 and gy0 <= y < gy1
            if alpha_v < 40 or (rim_px[x, y] and lum > light and not in_glyph):
                pixels[x, y] = (0, 0, 0, 0)
    box = image.getbbox()
    return to_square(image.crop(box)) if box else image


def resize_premultiplied(image: Image.Image, size: int) -> Image.Image:
    src = image.convert("RGBA").copy()
    pixels = src.load()
    width, height = src.size
    for y in range(height):
        for x in range(width):
            red, green, blue, alpha = pixels[x, y]
            if alpha == 0:
                pixels[x, y] = (0, 0, 0, 0)
            elif alpha < 255:
                pixels[x, y] = (red * alpha // 255, green * alpha // 255, blue * alpha // 255, alpha)
    out = src.resize((size, size), Image.Resampling.LANCZOS)
    pixels = out.load()
    for y in range(size):
        for x in range(size):
            red, green, blue, alpha = pixels[x, y]
            if alpha == 0:
                pixels[x, y] = (0, 0, 0, 0)
            elif alpha < 255:
                pixels[x, y] = (
                    min(255, red * 255 // alpha),
                    min(255, green * 255 // alpha),
                    min(255, blue * 255 // alpha),
                    alpha,
                )
    return out


def load_master() -> Image.Image:
    if not MASTER.is_file():
        raise SystemExit(f"missing {MASTER}")
    return isolate_tile(Image.open(MASTER))


def scale(master: Image.Image, size: int) -> Image.Image:
    resized = resize_premultiplied(master, size)
    light = 48 if size <= 48 else 88
    return punch_light_rim(resized, light)


def light_ring_pixels(image: Image.Image, ring: int = 2) -> int:
    """Count non-glyph light pixels in the outer ring. Used to prove the halo is gone."""
    width, height = image.size
    pixels = image.load()
    gx0, gx1, gy0, gy1 = _glyph_box(width, height)
    count = 0
    for y in range(height):
        for x in range(width):
            if ring <= x < width - ring and ring <= y < height - ring:
                continue
            red, green, blue, alpha = pixels[x, y]
            if alpha < 20:
                continue
            if gx0 <= x < gx1 and gy0 <= y < gy1:
                continue
            if (red + green + blue) / 3 > 80:
                count += 1
    return count


def main() -> None:
    master = load_master()
    OUT.mkdir(parents=True, exist_ok=True)
    named = {
        "16x16.png": 16,
        "24x24.png": 24,
        "32x32.png": 32,
        "48x48.png": 48,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
    }
    pngs: dict[int, bytes] = {}
    images: dict[int, Image.Image] = {}
    for name, size in named.items():
        dest = OUT / name
        icon = scale(master, size)
        write_png(dest, icon)
        pngs[size] = dest.read_bytes()
        images[size] = icon
    for size in (64, 1024):
        dest = OUT / f"_icns_{size}.png"
        icon = scale(master, size)
        write_png(dest, icon)
        pngs[size] = dest.read_bytes()
        dest.unlink(missing_ok=True)
    write_ico(
        OUT / "icon.ico",
        [
            (16, pngs[16]),
            (24, pngs[24]),
            (32, pngs[32]),
            (48, pngs[48]),
            (256, pngs[256]),
        ],
    )
    write_icns(
        OUT / "icon.icns",
        {
            "ic07": pngs[128],
            "ic08": pngs[256],
            "ic09": pngs[512],
            "ic10": pngs[1024],
            "ic11": pngs[32],
            "ic12": pngs[64],
            "ic13": pngs[256],
            "ic14": pngs[512],
        },
    )
    for size in (16, 32):
        halo = light_ring_pixels(images[size])
        if halo:
            raise SystemExit(f"{size}x{size} still has {halo} light halo pixels")
    print(f"wrote icons to {OUT}")


if __name__ == "__main__":
    main()
