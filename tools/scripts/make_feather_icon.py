"""Writes the test app's feather icon for Windows and Android, from one description.

Usage: python tools/scripts/make_feather_icon.py <icon dir> <android res dir>

The repository's copies are made with:
    python tools/scripts/make_feather_icon.py apps/testapp/icon apps/testapp/android/app/src/main/res

The icon: a white feather (a curved quill, a vane wider on the right, its tip leaning to the
upper right, three splits in the vane), the right half of the vane pale blue-grey, the quill
light blue-grey, on a sky gradient (bright blue at the top, deeper blue at the bottom).

Writes:
- <icon dir>/feather.ico: Windows icon, 16, 24, 32, 48, 64 and 256 pixels (PNG entries), on a
  rounded square; compiled into testapp.exe by build.rs.
- <icon dir>/feather_256.png: the same at 256 pixels, the window's icon at run time.
- <android res dir>/drawable/ic_launcher_foreground.xml: the feather as a vector drawable.
- <android res dir>/drawable/ic_launcher_background.xml: the gradient, filling the square (the
  launcher cuts adaptive icons to its own shape).
- <android res dir>/mipmap-anydpi/ic_launcher.xml: the adaptive icon made of the two.

Standard library only: shapes are polygons, filled with 4 x 4 supersampling; PNGs are written
with zlib.
"""

import math
import os
import struct
import sys
import zlib

SUPERSAMPLING = 4

# Colours.
BACKGROUND_TOP = '#3A8DDE'
BACKGROUND_BOTTOM = '#1B4F9C'
VANE_LEFT = '#FFFFFF'
VANE_RIGHT = '#DCE8EE'
QUILL = '#8FB3C6'

# The feather's spine in a unit square, y down: a quadratic curve from the end of the quill
# (bottom left) to the tip (top right).
P0, P1, P2 = (0.20, 0.88), (0.40, 0.40), (0.82, 0.12)
# Where the vane starts along the spine (0..1).
VANE_START = 0.22
# Splits in the vane: (where along the spine, how deep, 0..1), right and left.
SPLITS_RIGHT = [(0.45, 0.8), (0.62, 0.6)]
SPLITS_LEFT = [(0.5, 0.7)]
# The feather is drawn this big in the icon's square, from this corner.
FEATHER_SCALE = 0.86
FEATHER_OFFSET = (0.07, 0.07)

ICO_SIZES = [16, 24, 32, 48, 64, 256]


def spine(t):
    a, b, c = (1 - t) ** 2, 2 * (1 - t) * t, t * t
    return (a * P0[0] + b * P1[0] + c * P2[0], a * P0[1] + b * P1[1] + c * P2[1])


def spine_direction(t):
    dx = 2 * (1 - t) * (P1[0] - P0[0]) + 2 * t * (P2[0] - P1[0])
    dy = 2 * (1 - t) * (P1[1] - P0[1]) + 2 * t * (P2[1] - P1[1])
    n = math.hypot(dx, dy)
    return dx / n, dy / n


def vane(side):
    """One half of the vane (`side` -1 left, +1 right) as a polygon: along the spine, then back
    along its edge. Barbs slant towards the tip; splits pull the edge in."""
    splits = SPLITS_RIGHT if side > 0 else SPLITS_LEFT
    steps = 80
    along = [VANE_START + (1 - VANE_START) * i / steps for i in range(steps + 1)]
    edge = []
    for t in along:
        x, y = spine(t)
        tx, ty = spine_direction(t)
        nx, ny = (-ty, tx) if side > 0 else (ty, -tx)
        u = (t - VANE_START) / (1 - VANE_START)
        width = (0.17 if side > 0 else 0.11) * math.sin(math.pi * min(u, 1.0) ** 0.85) ** 0.7
        for where, depth in splits:
            d = abs(t - where)
            if d < 0.035:
                width *= 1 - depth * (1 - d / 0.035)
        slant = 0.35 * width
        edge.append((x + nx * width + tx * slant, y + ny * width + ty * slant))
    return [spine(t) for t in along] + edge[::-1]


def quill(width_base, width_tip):
    """The quill as a polygon, tapering from `width_base` to `width_tip` (half widths)."""
    steps = 60
    left, right = [], []
    for i in range(steps + 1):
        t = i / steps
        x, y = spine(t)
        tx, ty = spine_direction(t)
        w = width_base + (width_tip - width_base) * t
        left.append((x - ty * w, y + tx * w))
        right.append((x + ty * w, y - tx * w))
    return left + right[::-1]


def quill_width(size):
    """Thicker at small sizes, so it stays visible."""
    return 0.012 if size >= 48 else 0.022


def feather_polygons(size):
    """The feather's shapes with their colours, bottom to top, in the unit square."""
    return [
        (vane(-1), VANE_LEFT),
        (vane(1), VANE_RIGHT),
        (quill(quill_width(size), quill_width(size) * 0.35), QUILL),
    ]


def rounded_square(margin, radius):
    points = []
    lo, hi = margin, 1 - margin
    corners = ((hi - radius, lo + radius, -90), (hi - radius, hi - radius, 0),
               (lo + radius, hi - radius, 90), (lo + radius, lo + radius, 180))
    for cx, cy, start in corners:
        for k in range(13):
            a = math.radians(start + 90 * k / 12)
            points.append((cx + radius * math.cos(a), cy + radius * math.sin(a)))
    return points


def placed(polygon):
    """`polygon` (feather coordinates) placed in the icon's square."""
    return [(x * FEATHER_SCALE + FEATHER_OFFSET[0], y * FEATHER_SCALE + FEATHER_OFFSET[1])
            for x, y in polygon]


# --- raster ---

def coverage(polygon, size):
    """How much of each pixel `polygon` (unit coordinates) covers, size x size, row by row."""
    n = size * SUPERSAMPLING
    cover = [0.0] * (size * size)
    points = [(x * n, y * n) for x, y in polygon]
    edges = list(zip(points, points[1:] + points[:1]))
    step = 1.0 / (SUPERSAMPLING * SUPERSAMPLING)
    for row in range(n):
        y = row + 0.5
        xs = sorted(x0 + (y - y0) * (x1 - x0) / (y1 - y0)
                    for (x0, y0), (x1, y1) in edges if (y0 <= y < y1) or (y1 <= y < y0))
        base = (row // SUPERSAMPLING) * size
        for a, b in zip(xs[0::2], xs[1::2]):
            for col in range(max(0, math.ceil(a - 0.5)), min(n - 1, math.floor(b - 0.5)) + 1):
                cover[base + col // SUPERSAMPLING] += step
    return cover


def rgb(hex_colour):
    return tuple(int(hex_colour[i:i + 2], 16) / 255 for i in (1, 3, 5))


def to_linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def to_srgb(c):
    return c * 12.92 if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055


def render(size):
    """The Windows icon at `size` pixels: RGBA bytes. Colours are blended in linear light."""
    pixels = [(0.0, 0.0, 0.0, 0.0)] * (size * size)
    top = [to_linear(c) for c in rgb(BACKGROUND_TOP)]
    bottom = [to_linear(c) for c in rgb(BACKGROUND_BOTTOM)]

    def background(i):
        f = (i // size + 0.5) / size
        return tuple(top[k] + (bottom[k] - top[k]) * f for k in range(3))

    layers = [(coverage(rounded_square(0.04, 0.2), size), background)]
    for polygon, colour in feather_polygons(size):
        linear = tuple(to_linear(c) for c in rgb(colour))
        layers.append((coverage(placed(polygon), size), lambda i, c=linear: c))
    for cover, paint in layers:
        for i, c in enumerate(cover):
            c = min(c, 1.0)
            if c <= 0:
                continue
            r, g, b, a = pixels[i]
            colour = paint(i)
            alpha = c + a * (1 - c)
            pixels[i] = tuple((colour[k] * c + (r, g, b)[k] * a * (1 - c)) / alpha
                              for k in range(3)) + (alpha,)
    out = bytearray()
    for r, g, b, a in pixels:
        out.extend(round(to_srgb(v) * 255) for v in (r, g, b))
        out.append(round(a * 255))
    return bytes(out)


def png(size, rgba):
    raw = bytearray()
    for row in range(size):
        raw.append(0)
        raw.extend(rgba[row * size * 4:(row + 1) * size * 4])

    def chunk(kind, data):
        return (struct.pack('>I', len(data)) + kind + data
                + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff))

    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(bytes(raw), 9))
            + chunk(b'IEND', b''))


def ico(images):
    """An .ico file of PNG `images` ((size, png bytes), ...)."""
    header = struct.pack('<HHH', 0, 1, len(images))
    offset = len(header) + 16 * len(images)
    entries, data = b'', b''
    for size, image in images:
        side = 0 if size >= 256 else size
        entries += struct.pack('<BBBBHHII', side, side, 0, 0, 1, 32, len(image), offset)
        offset += len(image)
        data += image
    return header + entries + data


# --- Android ---

ANDROID_VIEWPORT = 108
# Adaptive icons show at least the middle 66 of 108; the feather's square fills it.
ANDROID_SAFE = 66


def android_path(polygon):
    def point(x, y):
        k = ANDROID_SAFE
        o = (ANDROID_VIEWPORT - ANDROID_SAFE) / 2
        return f'{o + x * k:.2f},{o + y * k:.2f}'

    points = placed(polygon)
    return 'M' + ' L'.join(point(x, y) for x, y in points) + ' Z'


def android_foreground():
    paths = ''.join(
        f'''
    <path
        android:fillColor="{colour}"
        android:pathData="{android_path(polygon)}" />'''
        for polygon, colour in feather_polygons(256))
    return f'''<?xml version="1.0" encoding="utf-8"?>
<!-- Made by tools/scripts/make_feather_icon.py: the launcher icon's feather. -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="{ANDROID_VIEWPORT}"
    android:viewportHeight="{ANDROID_VIEWPORT}">{paths}
</vector>
'''


def android_background():
    v = ANDROID_VIEWPORT
    return f'''<?xml version="1.0" encoding="utf-8"?>
<!-- Made by tools/scripts/make_feather_icon.py: the launcher icon's sky gradient. -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:aapt="http://schemas.android.com/aapt"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="{v}"
    android:viewportHeight="{v}">
    <path android:pathData="M0,0 L{v},0 L{v},{v} L0,{v} Z">
        <aapt:attr name="android:fillColor">
            <gradient
                android:type="linear"
                android:startX="{v / 2:g}"
                android:startY="0"
                android:endX="{v / 2:g}"
                android:endY="{v}"
                android:startColor="#FF{BACKGROUND_TOP[1:]}"
                android:endColor="#FF{BACKGROUND_BOTTOM[1:]}" />
        </aapt:attr>
    </path>
</vector>
'''


ANDROID_ADAPTIVE = '''<?xml version="1.0" encoding="utf-8"?>
<!-- Made by tools/scripts/make_feather_icon.py: the launcher icon. -->
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@drawable/ic_launcher_background" />
    <foreground android:drawable="@drawable/ic_launcher_foreground" />
</adaptive-icon>
'''


def write(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    mode = 'wb' if isinstance(data, bytes) else 'w'
    with open(path, mode, **({} if mode == 'wb' else {'encoding': 'utf-8', 'newline': '\n'})) as f:
        f.write(data)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    icon_dir, res_dir = sys.argv[1], sys.argv[2]
    images = [(size, png(size, render(size))) for size in ICO_SIZES]
    write(os.path.join(icon_dir, 'feather.ico'), ico(images))
    write(os.path.join(icon_dir, 'feather_256.png'), dict(images)[256])
    write(os.path.join(res_dir, 'drawable', 'ic_launcher_foreground.xml'), android_foreground())
    write(os.path.join(res_dir, 'drawable', 'ic_launcher_background.xml'), android_background())
    write(os.path.join(res_dir, 'mipmap-anydpi', 'ic_launcher.xml'), ANDROID_ADAPTIVE)


if __name__ == '__main__':
    main()
