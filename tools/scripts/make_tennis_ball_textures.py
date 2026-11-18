"""Writes the tennis ball demo's textures (demo 22): tennis_ball.png and tennis_ball_normal.png.

Usage: python tools/scripts/make_tennis_ball_textures.py <output dir> [width]

The repository's copies are made with:
    python tools/scripts/make_tennis_ball_textures.py apps/testapp/resources

Both images wrap a ball like a world map (equirectangular, as the engine's generated sphere,
`MeshBuilderSphere`, maps them): x goes once around the ball (u, eastwards), y from the north
pole (top row) to the south pole. Width defaults to 2048, height is half the width.

- tennis_ball.png (sRGB colours): optic yellow felt with soft blotches and fine fibre noise, and
  the white seam, two dog-bone halves meeting along one closed curve.
- tennis_ball_normal.png (linear data, the atlas needs "color_space": "linear"): tangent-space
  normals, OpenGL convention: red = +x (towards +u, east), green = up in the image (towards -v,
  north), blue = out of the surface, each mapped from -1..1 to 0..255. They come from a height
  field: the felt is a raised layer whose edges roll down into the seam, a groove with a slight
  rounded rubber ridge in its middle, and the felt is covered with fine fuzz. Slopes are taken
  over the real distances on the ball, so the relief is the same at every latitude.

The seam is the classic tennis ball curve, p(t) = (a cos t + b cos 3t, a sin t - b sin 3t,
c sin 2t) with a + b = 1 and c = 2 sqrt(ab), which lies exactly on the unit sphere.

Standard library only (the PNGs are written with zlib); takes about a minute at 2048.
"""

import math
import os
import random
import struct
import sys
import zlib

# Seam curve: a + b = 1, c = 2 sqrt(ab) keeps it on the unit sphere.
SEAM_A = 0.7
SEAM_B = 1.0 - SEAM_A
SEAM_C = 2.0 * math.sqrt(SEAM_A * SEAM_B)

# Distances are angles on the ball, in radians (a real ball's radius is about 33 mm).
SEAM_HALF_WIDTH = 0.055   # the white line, edge to edge about 3.6 mm
FELT_EDGE = 0.03          # over which the felt rolls down into the seam
REACH = SEAM_HALF_WIDTH + FELT_EDGE + 0.02   # distances further than this don't matter

# Heights in ball radii.
FELT_HEIGHT = 0.045       # the felt layer above the seam's floor
RIDGE_HEIGHT = 0.012      # the rubber ridge in the middle of the seam
FUZZ_HEIGHT = 0.0012      # fine fuzz on the felt

# Colours (sRGB, 0..1).
FELT = (0.80, 0.88, 0.17)
SEAM = (0.93, 0.93, 0.90)


def smoothstep(e0, e1, x):
    t = min(max((x - e0) / (e1 - e0), 0.0), 1.0)
    return t * t * (3.0 - 2.0 * t)


def seam_point(t):
    x = SEAM_A * math.cos(t) + SEAM_B * math.cos(3 * t)
    y = SEAM_A * math.sin(t) - SEAM_B * math.sin(3 * t)
    z = SEAM_C * math.sin(2 * t)
    n = math.sqrt(x * x + y * y + z * z)
    return x / n, y / n, z / n


def ball_point(u, v):
    """Unit vector of texture coordinates (u, v), as MeshBuilderSphere places them."""
    lon = 2.0 * math.pi * u - math.pi
    lat = math.pi / 2 - math.pi * v
    return math.cos(lat) * math.cos(lon), math.cos(lat) * math.sin(lon), math.sin(lat)


def seam_distance(w, h):
    """Angle from each pixel centre (w x h, row by row) to the seam, at most REACH."""
    dist = [REACH] * (w * h)
    # Samples closer than a pixel along the curve.
    pixel = math.pi / h
    samples = int(2 * math.pi * 1.6 / (pixel * 0.5))
    rows = int(REACH / pixel) + 2
    for i in range(samples):
        qx, qy, qz = seam_point(2 * math.pi * i / samples)
        lat = math.asin(qz)
        lon = math.atan2(qy, qx)
        row_c = (math.pi / 2 - lat) / pixel - 0.5
        col_c = (lon + math.pi) / (2 * math.pi) * w - 0.5
        for row in range(max(0, int(row_c) - rows), min(h, int(row_c) + rows + 2)):
            v = (row + 0.5) / h
            plat = math.pi / 2 - math.pi * v
            cl = math.cos(plat)
            sl = math.sin(plat)
            cols = rows if cl < 1e-6 else min(w // 2, int(rows / max(cl, 1e-3)) + 2)
            base = row * w
            for col in range(int(col_c) - cols, int(col_c) + cols + 2):
                c = col % w
                plon = 2 * math.pi * (c + 0.5) / w - math.pi
                px = cl * math.cos(plon)
                py = cl * math.sin(plon)
                dx, dy, dz = px - qx, py - qy, sl - qz
                chord = math.sqrt(dx * dx + dy * dy + dz * dz)
                if chord < dist[base + c]:
                    dist[base + c] = chord
    # Chords to angles.
    return [2.0 * math.asin(min(d / 2.0, 1.0)) if d < REACH else REACH for d in dist]


def upsample(src, sw, sh, w, h):
    """Bilinear resize (wrapping in x) of a sw x sh grid to w x h."""
    out = [0.0] * (w * h)
    for row in range(h):
        y = (row + 0.5) * sh / h - 0.5
        y0 = min(max(int(math.floor(y)), 0), sh - 1)
        y1 = min(y0 + 1, sh - 1)
        fy = min(max(y - y0, 0.0), 1.0)
        for col in range(w):
            x = (col + 0.5) * sw / w - 0.5
            x0 = int(math.floor(x))
            fx = x - x0
            a = src[y0 * sw + x0 % sw]
            b = src[y0 * sw + (x0 + 1) % sw]
            c = src[y1 * sw + x0 % sw]
            d = src[y1 * sw + (x0 + 1) % sw]
            out[row * w + col] = (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy
    return out


class ValueNoise3:
    """Smooth 3D value noise, so the blotches have no seam at u = 0/1 and no pinch at the poles."""

    def __init__(self, seed):
        rnd = random.Random(seed)
        self.values = [rnd.random() * 2 - 1 for _ in range(256)]
        self.perm = list(range(256))
        rnd.shuffle(self.perm)
        self.perm += self.perm

    def lattice(self, x, y, z):
        p = self.perm
        return self.values[p[p[p[x & 255] + (y & 255)] + (z & 255)]]

    def __call__(self, x, y, z):
        xi, yi, zi = math.floor(x), math.floor(y), math.floor(z)
        fx, fy, fz = x - xi, y - yi, z - zi
        sx = fx * fx * (3 - 2 * fx)
        sy = fy * fy * (3 - 2 * fy)
        sz = fz * fz * (3 - 2 * fz)
        l = self.lattice

        def lerp(a, b, t):
            return a + (b - a) * t

        x00 = lerp(l(xi, yi, zi), l(xi + 1, yi, zi), sx)
        x10 = lerp(l(xi, yi + 1, zi), l(xi + 1, yi + 1, zi), sx)
        x01 = lerp(l(xi, yi, zi + 1), l(xi + 1, yi, zi + 1), sx)
        x11 = lerp(l(xi, yi + 1, zi + 1), l(xi + 1, yi + 1, zi + 1), sx)
        return lerp(lerp(x00, x10, sy), lerp(x01, x11, sy), sz)


def blotches(w, h):
    """Soft large-scale colour variation of the felt, -1..1."""
    noise = ValueNoise3(7)
    out = [0.0] * (w * h)
    for row in range(h):
        for col in range(w):
            x, y, z = ball_point((col + 0.5) / w, (row + 0.5) / h)
            value = 0.0
            for octave, (scale, weight) in enumerate([(3.0, 0.6), (7.0, 0.3), (15.0, 0.15)]):
                value += weight * noise(x * scale + 10 * octave, y * scale, z * scale)
            out[row * w + col] = value
    return out


def fuzz(w, h):
    """Fine fibre noise, -1..1: white noise blurred over 3 x 3 pixels (wrapping in x)."""
    rnd = random.Random(11)
    white = [rnd.random() * 2 - 1 for _ in range(w * h)]
    across = [0.0] * (w * h)
    for row in range(h):
        base = row * w
        for col in range(w):
            across[base + col] = (white[base + (col - 1) % w] + 2 * white[base + col]
                                  + white[base + (col + 1) % w])
    out = [0.0] * (w * h)
    for row in range(h):
        up = max(row - 1, 0) * w
        mid = row * w
        down = min(row + 1, h - 1) * w
        for col in range(w):
            out[mid + col] = (across[up + col] + 2 * across[mid + col] + across[down + col]) / 8.0
    return out


def height_at(d, fuzz_value):
    """Height of the surface at seam distance d (radians)."""
    felt = smoothstep(SEAM_HALF_WIDTH, SEAM_HALF_WIDTH + FELT_EDGE, d)
    ridge = 1.0 - smoothstep(0.0, SEAM_HALF_WIDTH * 0.6, d)
    return FELT_HEIGHT * felt + RIDGE_HEIGHT * ridge * ridge + FUZZ_HEIGHT * fuzz_value * felt


def write_png(path, w, h, rgb_rows):
    """Writes 8-bit RGB rows (bytes of 3 * w each), with the Up filter on every row."""
    raw = bytearray()
    previous = bytes(3 * w)
    for row in rgb_rows:
        raw.append(2)  # Up
        raw.extend((a - b) & 255 for a, b in zip(row, previous))
        previous = row

    def chunk(kind, data):
        return (struct.pack('>I', len(data)) + kind + data
                + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff))

    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n')
        f.write(chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)))
        f.write(chunk(b'IDAT', zlib.compress(bytes(raw), 9)))
        f.write(chunk(b'IEND', b''))


def to_byte(x):
    return min(max(int(round(x * 255.0)), 0), 255)


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    out_dir = sys.argv[1]
    w = int(sys.argv[2]) if len(sys.argv) == 3 else 2048
    h = w // 2

    # The seam distance and the blotches are smooth: computed at a quarter of the size.
    dist = upsample(seam_distance(w // 2, h // 2), w // 2, h // 2, w, h)
    blotch = upsample(blotches(w // 8, h // 8), w // 8, h // 8, w, h)
    fibres = fuzz(w, h)
    # The fuzz is per pixel, and pixels get narrower towards the poles: lower it there so its
    # slopes stay the same.
    heights = [0.0] * (w * h)
    for row in range(h):
        narrow = max(math.cos(math.pi / 2 - math.pi * (row + 0.5) / h), 0.05)
        for i in range(row * w, (row + 1) * w):
            heights[i] = height_at(dist[i], fibres[i] * narrow)

    colour_rows = []
    normal_rows = []
    pixel_v = math.pi / h  # radians per pixel down the image
    for row in range(h):
        lat = math.pi / 2 - math.pi * (row + 0.5) / h
        # Radians per pixel across the image at this latitude (never 0, near the poles).
        pixel_u = 2 * math.pi / w * max(math.cos(lat), 0.02)
        up = max(row - 1, 0) * w
        mid = row * w
        down = min(row + 1, h - 1) * w
        colours = bytearray()
        normals = bytearray()
        for col in range(w):
            i = mid + col
            d = dist[i]
            # Colour: felt with blotches, fibres and a little shade where it rolls into the seam,
            # the seam off-white with a slightly soft edge.
            seam = 1.0 - smoothstep(SEAM_HALF_WIDTH - 0.004, SEAM_HALF_WIDTH + 0.004, d)
            shade = (1.0 + 0.07 * blotch[i] + 0.10 * fibres[i]) \
                * (0.85 + 0.15 * smoothstep(SEAM_HALF_WIDTH, SEAM_HALF_WIDTH + FELT_EDGE, d))
            grey = 1.0 + 0.03 * fibres[i]
            for k in range(3):
                colours.append(to_byte(FELT[k] * shade * (1 - seam) + SEAM[k] * grey * seam))
            # Normal: slopes east and north over the real distances.
            east = (heights[mid + (col + 1) % w] - heights[mid + (col - 1) % w]) / (2 * pixel_u)
            north = (heights[up + col] - heights[down + col]) / (2 * pixel_v)
            nx, ny, nz = -east, -north, 1.0
            n = math.sqrt(nx * nx + ny * ny + nz * nz)
            normals.append(to_byte(nx / n * 0.5 + 0.5))
            normals.append(to_byte(ny / n * 0.5 + 0.5))
            normals.append(to_byte(nz / n * 0.5 + 0.5))
        colour_rows.append(bytes(colours))
        normal_rows.append(bytes(normals))

    os.makedirs(out_dir, exist_ok=True)
    write_png(os.path.join(out_dir, 'tennis_ball.png'), w, h, colour_rows)
    write_png(os.path.join(out_dir, 'tennis_ball_normal.png'), w, h, normal_rows)


if __name__ == '__main__':
    main()
