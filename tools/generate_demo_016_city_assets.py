#!/usr/bin/env python3
"""Generate demo 016 city textures, OBJ meshes, scenes and atlas lists."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
ATLAS_DIR = ROOT / "apps" / "testapp" / "atlases"
TEX_DIR = ATLAS_DIR / "demo_016_city"
RES_DIR = ROOT / "apps" / "testapp" / "resources"

BLOCKS = 5
PITCH = 50.0
BLOCK = 36.0
ROAD_HALF = 3.5
SIDEWALK = 3.5
CURB = 0.15
BAY = 3.0
FLOOR = 3.5
PARK = (3, 1)
LANE_LENGTH = 360.0

CAR = {
    "SIDE": (0.0, 0.0, 1.0, 0.25),
    "SIDE_WINDOWS": (0.0, 0.25, 1.0, 0.5),
    "PAINT": (0.0, 0.5, 0.25, 0.75),
    "WINDSHIELD": (0.25, 0.5, 0.5, 0.75),
    "FRONT": (0.5, 0.5, 0.75, 0.75),
    "REAR": (0.75, 0.5, 1.0, 0.75),
    "TIRE": (0.0, 0.75, 0.25, 1.0),
    "RIM": (0.25, 0.75, 0.5, 1.0),
    "PLASTIC": (0.5, 0.75, 0.75, 1.0),
    "TAXI_SIGN": (0.75, 0.75, 1.0, 1.0),
}
PERSON = {
    "SKIN": (0.02, 0.02, 0.48, 0.48),
    "HAIR": (0.52, 0.02, 0.98, 0.48),
    "SHIRT": (0.02, 0.52, 0.48, 0.98),
    "PANTS": (0.52, 0.52, 0.98, 0.73),
    "SHOES": (0.52, 0.77, 0.98, 0.98),
}
SIGNAL = {
    "HOUSING": (0.02, 0.02, 0.48, 0.48),
    "RED": (0.52, 0.02, 0.98, 0.48),
    "YELLOW": (0.02, 0.52, 0.48, 0.98),
    "GREEN": (0.52, 0.52, 0.98, 0.98),
}


class Random:
    def __init__(self, seed: int):
        self.x = seed | 1

    def next(self) -> float:
        x = self.x & 0xFFFFFFFF
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        self.x = x & 0xFFFFFFFF
        return (self.x >> 8) / float(1 << 24)

    def range(self, lo: float, hi: float) -> float:
        return lo + (hi - lo) * self.next()


class Noise:
    def __init__(self, seed: int):
        self.seed = seed & 0xFFFFFFFF

    def hash(self, x: int, y: int) -> float:
        h = ((x & 0xFFFFFFFF) * 0x8DA6_B343) & 0xFFFFFFFF
        h ^= ((y & 0xFFFFFFFF) * 0xD816_3841) & 0xFFFFFFFF
        h ^= (self.seed * 0xCB1A_B31F) & 0xFFFFFFFF
        h ^= h >> 13
        h = (h * 0x5BD1_E995) & 0xFFFFFFFF
        h ^= h >> 15
        return (h & 0xFF_FFFF) / float(0x100_0000)

    def value(self, x: float, y: float, period: int) -> float:
        p = max(1, int(period))
        xi, yi = math.floor(x), math.floor(y)
        fx, fy = x - xi, y - yi
        sx, sy = fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy)

        def at(i: int, j: int) -> float:
            return self.hash(i % p, j % p)

        top = at(xi, yi) * (1.0 - sx) + at(xi + 1, yi) * sx
        bottom = at(xi, yi + 1) * (1.0 - sx) + at(xi + 1, yi + 1) * sx
        return top * (1.0 - sy) + bottom * sy

    def fbm(self, x: float, y: float, period: int, octaves: int) -> float:
        total = weight = scale = 1.0
        acc = self.value(x, y, period)
        for _ in range(1, octaves):
            weight *= 0.5
            scale *= 2.0
            acc += self.value(x * scale, y * scale, int(period * scale)) * weight
            total += weight
        return acc / total


def mix(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def clamp8(x):
    return int(max(0, min(1, x)) * 255 + 0.5)


def glass(u, v, noise):
    sky = mix((0.48, 0.62, 0.78), (0.08, 0.13, 0.22), v**0.7)
    streak = 0.07 * noise.value(u * 4.0 + v * 7.0, v * 2.0, 64)
    return (sky[0] + streak, sky[1] + streak, sky[2] + streak, 0.0)


def image(name: str, size: int, fn: Callable[[int, int, float, float], tuple[float, float, float, float]]):
    img = Image.new("RGBA", (size, size))
    px = img.load()
    for y in range(size):
        for x in range(size):
            u, v = (x + 0.5) / size, (y + 0.5) / size
            px[x, y] = tuple(clamp8(c) for c in fn(x, y, u, v))
    img.save(TEX_DIR / f"{name}.png")


def make_textures():
    TEX_DIR.mkdir(parents=True, exist_ok=True)

    def grain(c, n, amount):
        k = 1.0 + amount * (n - 0.5)
        return (c[0] * k, c[1] * k, c[2] * k, c[3])

    n = Noise(11)
    image("glass_facade", 512, lambda _x, _y, u, v: (
        (0.80, 0.82, 0.86, 1.0) if (u < 0.025 or u > 0.975 or v < 0.025 or abs(u - 0.5) < 0.009 or abs(u - 0.25) < 0.006 or abs(u - 0.75) < 0.006)
        else (0.20, 0.22, 0.27, 1.0) if v > 0.81
        else glass(u, v / 0.81, n)
    ))

    n = Noise(12)
    def brick(_x, _y, u, v):
        row = math.floor(v * 30.0)
        bu = (u * 8.0 + (0.5 if row % 2 else 0.0)) % 1.0
        bv = (v * 30.0) % 1.0
        if 0.23 < u < 0.77 and 0.15 < v < 0.76:
            if u < 0.27 or u > 0.73 or v < 0.19 or abs(u - 0.5) < 0.01 or abs(v - 0.42) < 0.01:
                return (0.90, 0.89, 0.86, 0.0)
            return glass((u - 0.27) / 0.46, (v - 0.19) / 0.57, n)
        if 0.20 < u < 0.80 and 0.76 < v < 0.81:
            return (0.80, 0.78, 0.72, 0.0)
        if bu < 0.045 or bv < 0.11:
            return (0.72, 0.68, 0.62, 1.0)
        shade = 0.75 + 0.45 * n.value(math.floor(u * 8.0) * 7.3, row * 3.1, 64)
        return grain((0.62 * shade, 0.31 * shade, 0.23 * shade, 1.0), n.value(u * 20, v * 20, 64), 0.08)
    image("brick_facade", 512, brick)

    n = Noise(13)
    def concrete_facade(_x, _y, u, v):
        if 0.18 < v < 0.67:
            mullion = u < 0.018 or u > 0.982 or abs(u - 0.33) < 0.01 or abs(u - 0.66) < 0.01
            return (0.33, 0.33, 0.35, 0.0) if mullion else glass(u, (v - 0.18) / 0.49, n)
        line = abs(v - 0.84) < 0.007 or u < 0.006 or u > 0.994
        base = (0.60, 0.60, 0.58, 1.0) if line else (0.78, 0.77, 0.74, 1.0)
        return grain(base, n.fbm(u * 16, v * 16, 16, 4), 0.16)
    image("concrete_facade", 512, concrete_facade)

    n = Noise(14)
    def stone(_x, _y, u, v):
        if 0.30 < u < 0.70 and 0.10 < v < 0.82:
            if u < 0.33 or u > 0.67 or v < 0.13 or abs(u - 0.5) < 0.012:
                return (0.25, 0.22, 0.20, 0.0)
            return glass((u - 0.33) / 0.34, (v - 0.13) / 0.69, n)
        groove = (v * 8.0) % 1.0 < 0.045 or (u * 2.0) % 1.0 < 0.018
        base = (0.70, 0.64, 0.54, 1.0) if groove else (0.86, 0.80, 0.68, 1.0)
        return grain(base, n.fbm(u * 12, v * 12, 16, 4), 0.13)
    image("stone_facade", 512, stone)

    n = Noise(15)
    image("roof", 256, lambda x, y, u, v: (lambda k: (0.50 * k, 0.49 * k, 0.47 * k, 1.0))(0.75 + 0.5 * n.fbm(u * 8, v * 8, 8, 5) + 0.15 * (n.value(x * 0.9, y * 0.9, 230) - 0.5)))

    def asphalt_at(noise, x, y, u, v):
        k = 0.8 + 0.35 * noise.fbm(u * 4, v * 4, 4, 5) + 0.2 * (noise.value(x * 0.7, y * 0.7, 179) - 0.5)
        return (0.20 * k, 0.20 * k, 0.21 * k)

    n = Noise(16)
    def road(x, y, u, v):
        a = asphalt_at(n, x, y, u, v)
        edge = abs(u - 0.05) < 0.008 or abs(u - 0.95) < 0.008
        centre = abs(u - 0.5) < 0.010 and (v * 1.0) % 1.0 < 0.43
        if edge or centre:
            worn = 0.85 + 0.15 * n.value(u * 40, v * 40, 40)
            return (0.86 * worn, 0.86 * worn, 0.82 * worn, 1.0)
        return (*a, 1.0)
    image("road", 512, road)
    image("intersection", 256, lambda x, y, u, v: (*asphalt_at(n, x, y, u, v), 1.0))
    n = Noise(17)
    image("crosswalk", 256, lambda x, y, u, v: ((0.88, 0.88, 0.85, 1.0) if ((u * 7) % 1.0 < 0.5 and 0.08 < v < 0.92) else (*asphalt_at(n, x, y, u, v), 1.0)))

    n = Noise(18)
    def sidewalk(_x, _y, u, v):
        su, sv = (u * 4.0) % 1.0, (v * 4.0) % 1.0
        if su < 0.03 or sv < 0.03:
            return (0.45, 0.44, 0.42, 1.0)
        k = 0.9 + 0.2 * n.value(math.floor(u * 4) * 5.7, math.floor(v * 4) * 3.3, 64)
        return grain((0.72 * k, 0.70 * k, 0.66 * k, 1.0), n.fbm(u * 16, v * 16, 16, 4), 0.10)
    image("sidewalk", 256, sidewalk)

    n = Noise(19)
    image("concrete", 256, lambda _x, _y, u, v: grain((0.66, 0.65, 0.62, 1.0), n.fbm(u * 8, v * 8, 8, 4), 0.25))
    n = Noise(20)
    image("grass", 256, lambda x, y, u, v: (lambda k, dry, blade: tuple(list(c * k for c in mix((0.26, 0.46, 0.17), (0.45, 0.47, 0.22), max(0.0, dry - 0.45) * 2.5)) + [1.0]))(0.7 + 0.5 * n.fbm(u * 6, v * 6, 6, 5) + 0.25 * (n.value(x * 1.3, y * 0.6, 333) - 0.5), n.fbm(u * 2 + 3, v * 2, 2, 3), 0))
    n = Noise(21)
    image("bark", 256, lambda _x, _y, u, v: (lambda k: (0.36 * k, 0.27 * k, 0.19 * k, 1.0))(0.6 + 0.8 * n.fbm(u * 16, v * 2, 16, 4)))
    n = Noise(22)
    image("leaves", 256, lambda x, y, u, v: (lambda k, leaf: tuple(list(c * k for c in mix((0.16, 0.36, 0.10), (0.36, 0.50, 0.16), leaf)) + [1.0]))(0.45 + 0.8 * n.fbm(u * 8, v * 8, 8, 4) * (0.7 + 0.6 * n.value(x * 0.5, y * 0.5, 128)), n.value(x * 0.5, y * 0.5, 128)))
    n = Noise(23)
    image("metal", 256, lambda _x, _y, u, v: grain((0.25, 0.27, 0.28, 1.0), n.fbm(u * 4, v * 4, 4, 4), 0.2))

    make_car_texture()
    make_person_texture()
    make_signal_texture()


def fill_region(px, size, r, fn):
    x0, y0, x1, y1 = [int(c * size) for c in r]
    pad = max(1, int(size * 0.004))
    for y in range(max(0, y0 - pad), min(size, y1 + pad)):
        for x in range(max(0, x0 - pad), min(size, x1 + pad)):
            u = (x + 0.5) / size
            v = (y + 0.5) / size
            xx = min(1, max(0, (u - r[0]) / (r[2] - r[0])))
            yy = min(1, max(0, (v - r[1]) / (r[3] - r[1])))
            px[x, y] = tuple(clamp8(c) for c in fn(xx, yy))


def make_car_texture():
    size = 512
    img = Image.new("RGBA", (size, size), (25, 25, 25, 255))
    px = img.load()
    n = Noise(24)
    paint = lambda s: (s, s, s, 1.0)
    dark = lambda k: (k, k, k, 0.0)
    wheels = (0.19, 0.8)

    def side(x, y):
        for w in wheels:
            dx, dy = (x - w) / 0.1, (y - 1.05) / 0.72
            if dx * dx + dy * dy < 1.0:
                return dark(0.03)
        if y > 0.88:
            return dark(0.07)
        if any(abs(x - s) < 0.002 for s in (0.3, 0.52, 0.72)) and y < 0.86:
            return paint(0.45)
        if any(abs(x - h) < 0.018 for h in (0.36, 0.58)) and 0.16 < y < 0.24:
            return (0.75, 0.76, 0.78, 0.0)
        return paint(1.0 if abs(y - 0.3) < 0.012 else 0.93 - 0.18 * y)

    fill_region(px, size, CAR["SIDE"], side)
    fill_region(px, size, CAR["SIDE_WINDOWS"], lambda x, y: dark(0.04) if y < 0.06 or y > 0.94 or x < 0.012 or x > 0.988 or abs(x - 0.49) < 0.02 else tuple([glass(x * 3, y, n)[i] * (0.55, 0.6, 0.68, 1)[i] for i in range(4)]))
    fill_region(px, size, CAR["PAINT"], lambda _x, _y: paint(0.92))
    fill_region(px, size, CAR["WINDSHIELD"], lambda x, y: dark(0.04) if x < 0.06 or x > 0.94 or y < 0.07 or y > 0.93 else tuple([glass(x, y, n)[i] * (0.6, 0.65, 0.72, 1)[i] for i in range(4)]))
    fill_region(px, size, CAR["FRONT"], lambda x, y: (0.95, 0.96, 1.0, 0.0) if min(((x - 0.15) / 0.15) ** 2 + ((y - 0.3) / 0.16) ** 2, ((x - 0.85) / 0.15) ** 2 + ((y - 0.3) / 0.16) ** 2) < 0.55 else dark(0.05) if (0.3 < x < 0.7 and 0.18 < y < 0.5) or (y > 0.78 and 0.15 < x < 0.85) else (0.92, 0.92, 0.88, 0.0) if 0.4 < x < 0.6 and 0.58 < y < 0.72 else paint(0.88))
    fill_region(px, size, CAR["REAR"], lambda x, y: (0.75, 0.04, 0.04, 0.0) if (x < 0.3 or x > 0.7) and 0.12 < y < 0.42 else (0.93, 0.92, 0.85, 0.0) if 0.37 < x < 0.63 and 0.45 < y < 0.66 else dark(0.06) if y > 0.78 else paint(0.86))
    fill_region(px, size, CAR["TIRE"], lambda _x, y: dark(0.05 + 0.025 * (((y * 40) % 1.0) < 0.5)))
    def rim(x, y):
        dx, dy = x - 0.5, y - 0.5
        r = (dx * dx + dy * dy) ** 0.5 * 2
        if r > 0.92:
            return dark(0.06)
        if r > 0.8 or r < 0.22:
            return (0.72, 0.73, 0.76, 0.0)
        spoke = abs(((math.atan2(dy, dx) / (2 * math.pi) * 8) % 1.0) - 0.5) < 0.12
        return (0.78, 0.79, 0.82, 0.0) if spoke else dark(0.1)
    fill_region(px, size, CAR["RIM"], rim)
    fill_region(px, size, CAR["PLASTIC"], lambda _x, _y: dark(0.07))
    fill_region(px, size, CAR["TAXI_SIGN"], lambda x, y: dark(0.05) if 0.12 < x < 0.88 and (abs(y - 0.5) < 0.07 or abs(x - 0.2) < 0.03 or abs(x - 0.8) < 0.03) else (0.98, 0.80, 0.10, 0.0))
    img.save(TEX_DIR / "car.png")


def make_person_texture():
    size = 512
    img = Image.new("RGBA", (size, size), (128, 128, 128, 255))
    px = img.load()
    n = Noise(25)
    for name, color, mask in [
        ("SKIN", (0.86, 0.66, 0.52), 0.0),
        ("HAIR", (0.20, 0.13, 0.08), 0.0),
        ("SHIRT", (0.92, 0.92, 0.92), 1.0),
        ("PANTS", (0.18, 0.25, 0.42), 0.0),
        ("SHOES", (0.12, 0.10, 0.09), 0.0),
    ]:
        fill_region(px, size, PERSON[name], lambda x, y, c=color, m=mask: (*[cc * (0.93 + 0.14 * n.fbm(x * 16, y * 16, 16, 3)) for cc in c], m))
    img.save(TEX_DIR / "person.png")


def make_signal_texture():
    size = 256
    img = Image.new("RGBA", (size, size), (20, 20, 20, 255))
    px = img.load()
    for name, color in [("HOUSING", (0.10, 0.10, 0.10)), ("RED", (0.32, 0.04, 0.03)), ("YELLOW", (0.34, 0.25, 0.03)), ("GREEN", (0.03, 0.24, 0.10))]:
        fill_region(px, size, SIGNAL[name], lambda x, y, c=color: (*c, 0.0))
    for y in range(size):
        for x in range(size):
            u, v = (x + 0.5) / size, (y + 0.5) / size
            if u >= 0.5 or v >= 0.5:
                lx, ly = (u * 2) % 1.0 - 0.5, (v * 2) % 1.0 - 0.5
                r = (lx * lx + ly * ly) ** 0.5
                old = tuple(c / 255.0 for c in px[x, y])
                if r > 0.42:
                    px[x, y] = tuple(clamp8(c) for c in (old[0] * 0.5, old[1] * 0.5, old[2] * 0.5, 0.0))
                elif ((lx + 0.15) ** 2 + (ly + 0.15) ** 2) ** 0.5 < 0.08:
                    px[x, y] = tuple(clamp8(c) for c in (old[0] + 0.25, old[1] + 0.25, old[2] + 0.25, 0.0))
    img.save(TEX_DIR / "signal.png")


@dataclass
class V:
    x: float
    y: float
    z: float

    def __add__(self, o): return V(self.x + o.x, self.y + o.y, self.z + o.z)
    def __sub__(self, o): return V(self.x - o.x, self.y - o.y, self.z - o.z)
    def __mul__(self, s: float): return V(self.x * s, self.y * s, self.z * s)
    def dot(self, o): return self.x * o.x + self.y * o.y + self.z * o.z
    def cross(self, o): return V(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    def norm(self):
        l = max(1e-9, math.sqrt(self.dot(self)))
        return self * (1.0 / l)


def mat_identity():
    return [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]]


def mat_mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def tr(v):
    m = mat_identity()
    m[0][3], m[1][3], m[2][3] = v.x, v.y, v.z
    return m


def sc(s):
    return [[s, 0, 0, 0], [0, s, 0, 0], [0, 0, s, 0], [0, 0, 0, 1]]


def rz(a):
    c, s = math.cos(a), math.sin(a)
    return [[c, -s, 0, 0], [s, c, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]]


def rx(a):
    c, s = math.cos(a), math.sin(a)
    return [[1, 0, 0, 0], [0, c, -s, 0], [0, s, c, 0], [0, 0, 0, 1]]


def ry(a):
    c, s = math.cos(a), math.sin(a)
    return [[c, 0, s, 0], [0, 1, 0, 0], [-s, 0, c, 0], [0, 0, 0, 1]]


def tp(m, p):
    return V(m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + m[0][3], m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + m[1][3], m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + m[2][3])


def td(m, p):
    return V(m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z, m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z, m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z).norm()


class Mesh:
    def __init__(self):
        self.verts = []
        self.uvs = []
        self.norms = []
        self.faces = []

    def vertex(self, p, uv, n):
        self.verts.append(p)
        self.uvs.append(uv)
        self.norms.append(n.norm())
        return len(self.verts)

    def tri(self, a, b, c):
        self.faces.append((a, b, c))

    def quad(self, pts, uvs):
        n = (pts[1] - pts[0]).cross(pts[3] - pts[0]).norm()
        ids = [self.vertex(p, uv, n) for p, uv in zip(pts, uvs)]
        self.tri(ids[0], ids[1], ids[2])
        self.tri(ids[0], ids[2], ids[3])

    def facing_quad(self, pts, uvs, outward):
        """A flat quad facing `outward`, whatever the order of its corners: two triangles,
        no centre vertex (unlike `polygon`)."""
        if (pts[1] - pts[0]).cross(pts[3] - pts[0]).dot(outward) < 0:
            pts, uvs = pts[::-1], uvs[::-1]
        self.quad(pts, uvs)

    def polygon(self, points, uvs, outward):
        cen = V(sum(p.x for p in points) / len(points), sum(p.y for p in points) / len(points), sum(p.z for p in points) / len(points))
        cuv = (sum(u for u, _ in uvs) / len(uvs), sum(v for _, v in uvs) / len(uvs))
        normal = V(0, 0, 0)
        for a, b in zip(points, points[1:] + points[:1]):
            normal = normal + a.cross(b)
        flip = normal.dot(outward) < 0
        normal = (normal * (-1 if flip else 1)).norm()
        c = self.vertex(cen, cuv, normal)
        ids = [self.vertex(p, uv, normal) for p, uv in zip(points, uvs)]
        for i in range(len(ids)):
            a, b = ids[i], ids[(i + 1) % len(ids)]
            self.tri(c, b, a) if flip else self.tri(c, a, b)

    def top(self, lo, hi, z, tile):
        uv = lambda x, y: (x / tile, -y / tile)
        self.quad([V(lo[0], lo[1], z), V(hi[0], lo[1], z), V(hi[0], hi[1], z), V(lo[0], hi[1], z)], [uv(lo[0], lo[1]), uv(hi[0], lo[1]), uv(hi[0], hi[1]), uv(lo[0], hi[1])])

    def strip(self, lo, hi, z, along_x, tile):
        c = [V(lo[0], lo[1], z), V(hi[0], lo[1], z), V(hi[0], hi[1], z), V(lo[0], hi[1], z)]
        if along_x:
            l = (hi[0] - lo[0]) / tile
            uvs = [(0, 0), (0, l), (1, l), (1, 0)]
        else:
            l = (hi[1] - lo[1]) / tile
            uvs = [(1, 0), (0, 0), (0, l), (1, l)]
        self.quad(c, uvs)

    def box(self, mat, mn, mx, region=(0, 0, 1, 1), top=True, bottom=True):
        """A box from `mn` to `mx`. `top=False` / `bottom=False` leave those faces out, e.g.
        where another surface lies in the same plane: coplanar faces z-fight (flicker)."""
        r = region
        uv = [(r[0], r[3]), (r[2], r[3]), (r[2], r[1]), (r[0], r[1])]
        def p(x, y, z): return V(mx.x if x else mn.x, mx.y if y else mn.y, mx.z if z else mn.z)
        faces = [[p(0,0,0),p(1,0,0),p(1,0,1),p(0,0,1)], [p(1,0,0),p(1,1,0),p(1,1,1),p(1,0,1)], [p(1,1,0),p(0,1,0),p(0,1,1),p(1,1,1)], [p(0,1,0),p(0,0,0),p(0,0,1),p(0,1,1)]]
        if top:
            faces.append([p(0,0,1),p(1,0,1),p(1,1,1),p(0,1,1)])
        if bottom:
            faces.append([p(0,1,0),p(1,1,0),p(1,0,0),p(0,0,0)])
        for face in faces:
            self.quad([tp(mat, q) for q in face], uv)

    def cylinder(self, mat, radius, height, seg, side=(0,0,1,1), cap=(0,0,1,1), r_top=None):
        r_top = radius if r_top is None else r_top
        rows = []
        for i in range(seg + 1):
            a = i / seg * math.tau
            n = V(math.cos(a), math.sin(a), 0)
            u = side[0] + (side[2] - side[0]) * i / seg
            rows.append((self.vertex(tp(mat, n * radius), (u, side[3]), td(mat, n)), self.vertex(tp(mat, n * r_top + V(0, 0, height)), (u, side[1]), td(mat, n))))
        for i in range(seg):
            b0, t0 = rows[i]
            b1, t1 = rows[i + 1]
            self.tri(b0, b1, t1); self.tri(b0, t1, t0)
        for z, rr, up in [(height, r_top, 1), (0, radius, -1)]:
            n = td(mat, V(0, 0, up))
            cen = self.vertex(tp(mat, V(0, 0, z)), ((cap[0] + cap[2]) / 2, (cap[1] + cap[3]) / 2), n)
            ids = []
            for i in range(seg + 1):
                a = i / seg * math.tau
                ids.append(self.vertex(tp(mat, V(math.cos(a) * rr, math.sin(a) * rr, z)), ((cap[0]+cap[2])/2 + (cap[2]-cap[0])*0.5*math.cos(a), (cap[1]+cap[3])/2 + (cap[3]-cap[1])*0.5*math.sin(a)), n))
            for i in range(seg):
                self.tri(cen, ids[i], ids[i+1]) if up > 0 else self.tri(cen, ids[i+1], ids[i])

    def ellipsoid(self, mat, radii, seg, rings, region=(0,0,1,1), inward=False):
        grid = []
        for r in range(rings + 1):
            th = math.pi * r / rings
            row = []
            for s in range(seg + 1):
                ph = math.tau * s / seg
                unit = V(math.sin(th) * math.cos(ph), math.sin(th) * math.sin(ph), math.cos(th))
                p = V(unit.x * radii.x, unit.y * radii.y, unit.z * radii.z)
                n = V(unit.x / radii.x, unit.y / radii.y, unit.z / radii.z).norm()
                if inward:
                    n = n * -1
                uv = (region[0] + (region[2] - region[0]) * s / seg, region[1] + (region[3] - region[1]) * r / rings)
                row.append(self.vertex(tp(mat, p), uv, td(mat, n)))
            grid.append(row)
        for r in range(rings):
            for s in range(seg):
                a, b, c, d = grid[r][s], grid[r][s+1], grid[r+1][s], grid[r+1][s+1]
                if r > 0:
                    self.tri(a, b, c) if inward else self.tri(a, c, b)
                if r < rings - 1:
                    self.tri(b, d, c) if inward else self.tri(b, c, d)

    def write(self, path):
        def key3(p):
            return (round(p.x, 3), round(p.y, 3), round(p.z, 3))

        def key2(t):
            return (round(t[0], 3), round(t[1], 3))

        pos, uv, norm = [], [], []
        pos_i, uv_i, norm_i = {}, {}, {}
        refs = []
        for face in self.faces:
            out = []
            for raw in face:
                i = raw - 1
                pk, uk, nk = key3(self.verts[i]), key2(self.uvs[i]), key3(self.norms[i])
                if pk not in pos_i:
                    pos_i[pk] = len(pos) + 1
                    pos.append(pk)
                if uk not in uv_i:
                    uv_i[uk] = len(uv) + 1
                    uv.append(uk)
                if nk not in norm_i:
                    norm_i[nk] = len(norm) + 1
                    norm.append(nk)
                out.append((pos_i[pk], uv_i[uk], norm_i[nk]))
            refs.append(out)
        with open(path, "w", encoding="ascii", newline="\n") as f:
            f.write("# generated by tools/generate_demo_016_city_assets.py\n")
            for x, y, z in pos:
                f.write(f"v {x:g} {y:g} {z:g}\n")
            # Wavefront v = 0 at the bottom; the engine flips to top-left (v = 1 - obj_v).
            # Our region UVs are top-left (matching the PNGs), so write the flipped v here.
            for u, v in uv:
                f.write(f"vt {u:g} {1.0 - v:g}\n")
            for x, y, z in norm:
                f.write(f"vn {x:g} {y:g} {z:g}\n")
            for face in refs:
                f.write("f " + " ".join(f"{p}/{t}/{n}" for p, t, n in face) + "\n")


def inset(r, pad=0.004):
    return (r[0] + pad, r[1] + pad, r[2] - pad, r[3] - pad)


def within(r, x, y):
    rr = inset(r)
    return (rr[0] + (rr[2] - rr[0]) * max(0, min(1, x)), rr[1] + (rr[3] - rr[1]) * max(0, min(1, y)))


# Each facade style's window opening within one bay and floor, in its texture's coordinates
# (u across, v down from the top of the floor): the painted frame and glass. Also how deep the
# window is set into the wall, and a patch of plain wall in the texture for the reveals.
WINDOWS = {
    "brick": {"u": (0.23, 0.77), "v": (0.15, 0.76), "depth": 0.15, "reveal": (0.05, 0.45)},
    "stone": {"u": (0.30, 0.70), "v": (0.10, 0.82), "depth": 0.20, "reveal": (0.15, 0.40)},
    "concrete": {"u": (0.0, 1.0), "v": (0.18, 0.67), "depth": 0.15, "reveal": (0.5, 0.92)},
    "glass": {"u": (0.025, 0.975), "v": (0.025, 0.81), "depth": 0.04, "reveal": (0.5, 0.012)},
}


def facade_wall(mesh, a, b, z0, z1, style):
    """A wall from `a` to `b` (looking at it from outside, `a` on the left), `z0` to `z1` high:
    a bay and floor at a time, the wall around each window opening, and the window set back
    into it: the pane (frame and glass from the texture) and the four reveals around it."""
    length = math.sqrt((b.x - a.x) ** 2 + (b.y - a.y) ** 2)
    bays = max(1, round(length / BAY))
    floors = max(1, round((z1 - z0) / FLOOR))
    outward = V(b.y - a.y, a.x - b.x, 0).norm()
    along = (b - a) * (1.0 / bays)
    side = along.norm()
    up = V(0, 0, 1)
    window = WINDOWS[style]
    (u0, u1), (tv0, tv1) = window["u"], window["v"]
    # Heights within a floor, from its bottom (texture v runs from the top).
    h0, h1 = 1.0 - tv1, 1.0 - tv0
    back = outward * -window["depth"]
    ru, rv = window["reveal"]
    reveal_uv = [(ru - 0.004, rv + 0.004), (ru + 0.004, rv + 0.004), (ru + 0.004, rv - 0.004), (ru - 0.004, rv - 0.004)]

    for i in range(bays):
        for j in range(floors):
            origin = a + along * i + V(0, 0, z0 + j * FLOOR)

            def at(u, h, offset=V(0, 0, 0)):
                return origin + along * u + V(0, 0, FLOOR * h) + offset

            def uv(u, h):
                return (i + u, floors - j - h)

            def rect(ua, ub, ha, hb, offset, facing):
                if ub - ua < 1e-4 or hb - ha < 1e-4:
                    return
                corners = [(ua, ha), (ub, ha), (ub, hb), (ua, hb)]
                mesh.facing_quad([at(u, h, offset) for u, h in corners], [uv(u, h) for u, h in corners], facing)

            # The wall around the opening.
            rect(0.0, 1.0, 0.0, h0, V(0, 0, 0), outward)
            rect(0.0, 1.0, h1, 1.0, V(0, 0, 0), outward)
            rect(0.0, u0, h0, h1, V(0, 0, 0), outward)
            rect(u1, 1.0, h0, h1, V(0, 0, 0), outward)
            # The window, set back.
            rect(u0, u1, h0, h1, back, outward)
            # The reveals: sill, head and jambs, facing into the opening.
            front = [at(u0, h0), at(u1, h0), at(u1, h1), at(u0, h1)]
            rear = [p + back for p in front]
            for k, facing in enumerate([up, side * -1, up * -1, side]):
                q = [front[k], front[(k + 1) % 4], rear[(k + 1) % 4], rear[k]]
                mesh.facing_quad(q, reveal_uv, facing)


def building(mesh, mn, mx, style):
    pts = [V(mn.x, mn.y, 0), V(mx.x, mn.y, 0), V(mx.x, mx.y, 0), V(mn.x, mx.y, 0)]
    for a, b in zip(pts, pts[1:] + pts[:1]):
        facade_wall(mesh, a, b, mn.z, mx.z, style)


def block_centre(i):
    return (i + 0.5 - BLOCKS / 2.0) * PITCH


def street(k):
    return (k - BLOCKS / 2.0) * PITCH


def split_lots(random):
    options = [[12], [6, 6], [4, 8], [7, 5], [4, 4, 4], [3, 5, 4], [5, 3, 4]]
    return [b * BAY for b in options[int(random.next() * len(options)) % len(options)]]


def tree(bark, leaves, base, size, random):
    full = (0, 0, 1, 1)
    t = mat_mul(tr(base), sc(size))
    bark.cylinder(t, 0.17, 3.3, 8, (0, 0, 1, 2), full, r_top=0.10)
    clumps = []
    for _ in range(3):
        clumps.append((V(random.range(-0.7, 0.7), random.range(-0.7, 0.7), random.range(3.4, 4.4)), V(random.range(1.3, 1.8), random.range(1.3, 1.8), random.range(1.1, 1.5))))
    h = int((base.x * 17 + base.y * 31) * 1000) & 0xFFFFFFFF
    local = Random(h or 17)
    for _ in range(2):
        clumps.append((V(local.range(-0.55, 0.55), local.range(-0.55, 0.55), local.range(3.55, 4.25)), V(local.range(1.1, 1.55), local.range(1.1, 1.55), local.range(0.9, 1.25))))
    for off, radii in clumps:
        leaves.ellipsoid(mat_mul(t, tr(off)), radii, 10, 7, (0, 0, 3, 2))


def lamp_post(metal, base, side):
    full = (0, 0, 1, 1)
    pole = tr(base)
    metal.cylinder(pole, 0.08, 6.5, 8, full, full)
    turn = rz(math.radians(180 + 90 * side))
    m = mat_mul(pole, turn)
    metal.box(m, V(-0.06, -0.06, 6.3), V(0.06, 2.6, 6.42), full)
    metal.box(m, V(-0.17, 2.0, 6.14), V(0.17, 2.75, 6.32), full)
    metal.cylinder(mat_mul(m, mat_mul(tr(V(0, 2.48, 6.12)), rx(math.pi / 2))), 0.18, 0.45, 10, full, full)


def traffic_light(signals, metal, base, facing):
    full = (0, 0, 1, 1)
    head = mat_mul(tr(base), rz(facing))
    metal.cylinder(tr(base), 0.08, 2.6, 8, full, full)
    signals.box(head, V(-0.22, -0.26, 2.46), V(-0.17, 0.26, 3.74), SIGNAL["HOUSING"])
    signals.box(head, V(-0.17, -0.17, 2.56), V(0.16, 0.17, 3.64), SIGNAL["HOUSING"])
    for z, reg in zip((3.44, 3.1, 2.76), (SIGNAL["RED"], SIGNAL["YELLOW"], SIGNAL["GREEN"])):
        lens = mat_mul(head, mat_mul(tr(V(0.15, 0, z)), ry(math.pi / 2)))
        signals.cylinder(lens, 0.11, 0.025, 12, reg, reg)
        signals.box(head, V(0.15, -0.14, z + 0.11), V(0.36, 0.14, z + 0.14), SIGNAL["HOUSING"])


def make_city_meshes():
    styles = ["glass", "brick", "concrete", "stone"]
    facades = {s: Mesh() for s in styles}
    roofs = Mesh(); roads = Mesh(); intersections = Mesh(); crosswalks = Mesh()
    sidewalks = Mesh(); curbs = Mesh(); grass = Mesh()
    trunks = Mesh(); tops = Mesh(); poles = Mesh(); signals = Mesh()
    random = Random(2024)

    grass.top((-600, -600), (600, 600), -0.02, 4)
    for bi in range(BLOCKS):
        for bj in range(BLOCKS):
            cx, cy = block_centre(bi), block_centre(bj)
            outer = BLOCK / 2 + SIDEWALK
            lo, hi = (cx - outer, cy - outer), (cx + outer, cy + outer)
            # Two-step beveled curb: a short lower skirt and an inset top skirt. The top skirt
            # has no top face: it would lie at CURB, in the plane of the sidewalk and the park's
            # lawn, and z-fight with them (the lawn and pavers flickered).
            curbs.box(mat_identity(), V(lo[0], lo[1], 0), V(hi[0], hi[1], CURB * 0.55), (0, 0, 1, 1), bottom=False)
            inset_lo, inset_hi = (lo[0] + 0.10, lo[1] + 0.10), (hi[0] - 0.10, hi[1] - 0.10)
            curbs.box(mat_identity(), V(inset_lo[0], inset_lo[1], CURB * 0.55), V(inset_hi[0], inset_hi[1], CURB), (0, 0, 1, 1), top=False, bottom=False)
            if (bi, bj) == PARK:
                inner = BLOCK / 2
                lawn_lo, lawn_hi = (cx - inner, cy - inner), (cx + inner, cy + inner)
                for a, b in [(lo, (hi[0], lawn_lo[1])), ((lo[0], lawn_hi[1]), hi), ((lo[0], lawn_lo[1]), (lawn_lo[0], lawn_hi[1])), ((lawn_hi[0], lawn_lo[1]), (hi[0], lawn_hi[1]))]:
                    sidewalks.top(a, b, CURB, 2)
                grass.top(lawn_lo, lawn_hi, CURB, 4)
                for _ in range(14):
                    tree(trunks, tops, V(cx + random.range(-inner + 3, inner - 3), cy + random.range(-inner + 3, inner - 3), CURB), random.range(0.9, 1.4), random)
            else:
                sidewalks.top(lo, hi, CURB, 2)
                centre_distance = math.sqrt(cx * cx + cy * cy)
                tallest = 5 + 15 * max(0, 1 - centre_distance / 150)
                xs, ys = split_lots(random), split_lots(random)
                x0 = cx - BLOCK / 2
                for w in xs:
                    y0 = cy - BLOCK / 2
                    for d in ys:
                        style = styles[int(random.next() * len(styles)) % len(styles)]
                        floors = max(3, round(3 + random.next() * (tallest - 3)))
                        mn, mx = V(x0, y0, CURB), V(x0 + w, y0 + d, CURB + floors * FLOOR)
                        building(facades[style], mn, mx, style)
                        roofs.top((mn.x, mn.y), (mx.x, mx.y), mx.z, 8)
                        if floors >= 8 and w >= 12 and d >= 12 and random.next() < 0.5:
                            more = round(2 + random.next() * 6)
                            tmn, tmx = V(mn.x + BAY, mn.y + BAY, mx.z), V(mx.x - BAY, mx.y - BAY, mx.z + more * FLOOR)
                            building(facades[style], tmn, tmx, style)
                            roofs.top((tmn.x, tmn.y), (tmx.x, tmx.y), tmx.z, 8)
                        y0 += d
                    x0 += w
            curb_line = BLOCK / 2 + SIDEWALK - 1.1
            for side in range(4):
                for k in range(3):
                    along = -12 + 12 * k
                    dx, dy = [(along, -curb_line), (curb_line, along), (-along, curb_line), (-curb_line, -along)][side]
                    p = V(cx + dx, cy + dy, CURB)
                    lamp_post(poles, p, side) if k == 1 else tree(trunks, tops, p, random.range(0.7, 0.85), random)

    far = LANE_LENGTH / 2
    edges = [-far]
    for k in range(BLOCKS + 1):
        edges += [street(k) - ROAD_HALF, street(k) + ROAD_HALF]
    edges.append(far)
    for k in range(BLOCKS + 1):
        s = street(k)
        for a, b in zip(edges[0::2], edges[1::2]):
            roads.strip((a, s - ROAD_HALF), (b, s + ROAD_HALF), 0, True, 7)
            roads.strip((s - ROAD_HALF, a), (s + ROAD_HALF, b), 0, False, 7)
        for l in range(BLOCKS + 1):
            t = street(l)
            intersections.top((s - ROAD_HALF, t - ROAD_HALF), (s + ROAD_HALF, t + ROAD_HALF), 0, 7)
            w, z = 3, 0.005
            crosswalks.strip((s + ROAD_HALF, t - ROAD_HALF), (s + ROAD_HALF + w, t + ROAD_HALF), z, True, w)
            crosswalks.strip((s - ROAD_HALF - w, t - ROAD_HALF), (s - ROAD_HALF, t + ROAD_HALF), z, True, w)
            crosswalks.strip((s - ROAD_HALF, t + ROAD_HALF), (s + ROAD_HALF, t + ROAD_HALF + w), z, False, w)
            crosswalks.strip((s - ROAD_HALF, t - ROAD_HALF - w), (s + ROAD_HALF, t - ROAD_HALF), z, False, w)
            for corner, facing in [((-1, -1), math.pi), ((1, 1), 0), ((1, -1), -math.pi / 2), ((-1, 1), math.pi / 2)]:
                traffic_light(signals, poles, V(s + corner[0] * (ROAD_HALF + 1), t + corner[1] * (ROAD_HALF + 1), CURB), facing)

    return {
        "glass_facade_mesh": facades["glass"], "brick_facade_mesh": facades["brick"],
        "concrete_facade_mesh": facades["concrete"], "stone_facade_mesh": facades["stone"],
        "roofs_mesh": roofs, "roads_mesh": roads, "intersections_mesh": intersections,
        "crosswalks_mesh": crosswalks, "sidewalks_mesh": sidewalks, "curbs_mesh": curbs,
        "grass_mesh": grass, "tree_trunks_mesh": trunks, "tree_tops_mesh": tops,
        "poles_mesh": poles, "traffic_lights_mesh": signals,
    }


SHAPES = {
    "sedan": ([(2.35,0.34),(2.42,0.48),(2.40,0.63),(2.32,0.78),(1.68,0.89),(0.7,0.94),(-0.7,0.96),(-1.6,0.95),(-2.18,0.86),(-2.34,0.66),(-2.32,0.46),(-2.26,0.34)], 0.89, [(1.45,0.9),(0.82,1.28),(0.35,1.42),(-0.72,1.44),(-1.34,1.18),(-1.58,0.95)], (0.8,0.66), 0.33),
    "hatchback": ([(2.05,0.33),(2.13,0.50),(2.10,0.66),(1.92,0.78),(1.15,0.89),(-0.25,0.94),(-1.65,0.95),(-1.95,0.86),(-2.05,0.62),(-2.02,0.44),(-1.96,0.33)], 0.85, [(1.18,0.9),(0.48,1.3),(0.10,1.42),(-1.42,1.45),(-1.88,0.96)], (0.78,0.65), 0.31),
    "suv": ([(2.38,0.42),(2.48,0.62),(2.45,0.82),(2.34,1.00),(1.48,1.11),(0.25,1.15),(-1.4,1.15),(-2.18,1.10),(-2.42,0.92),(-2.40,0.62),(-2.32,0.42)], 0.95, [(1.48,1.1),(0.75,1.58),(0.48,1.73),(-1.85,1.75),(-2.28,1.14)], (0.88,0.78), 0.39),
}


def xz_range(profile):
    xs, zs = [p[0] for p in profile], [p[1] for p in profile]
    return max(xs), min(xs), min(zs), max(zs)


def extrude_car(m, profile, half_width_fn, part):
    front, rear, low, high = xz_range(profile)
    centre = (sum(x for x, _ in profile) / len(profile), sum(z for _, z in profile) / len(profile))
    side_region = CAR["SIDE"] if part == "body" else CAR["SIDE_WINDOWS"]
    for side in (-1, 1):
        pts = [V(x, side * half_width_fn(z), z) for x, z in profile]
        uvs = [within(side_region, (front - x) / (front - rear), (high - z) / (high - low)) for x, z in profile]
        m.polygon(pts, uvs, V(0, side, 0))
    edges = []
    for i, a in enumerate(profile):
        b = profile[(i + 1) % len(profile)]
        nx, nz = b[1] - a[1], -(b[0] - a[0])
        l = max(1e-6, (nx * nx + nz * nz) ** 0.5)
        nx, nz = nx / l, nz / l
        mid = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
        if nx * (mid[0] - centre[0]) + nz * (mid[1] - centre[1]) < 0:
            nx, nz = -nx, -nz
        edges.append((a, b, nx, nz))
    for a, b, nx, nz in edges:
        corners = [V(a[0], -half_width_fn(a[1]), a[1]), V(b[0], -half_width_fn(b[1]), b[1]), V(b[0], half_width_fn(b[1]), b[1]), V(a[0], half_width_fn(a[1]), a[1])]
        outward = V(nx, 0, nz)
        if nz < -0.6:
            if part == "body":
                m.polygon(corners, [within(CAR["PLASTIC"], 0.5, 0.5)] * 4, outward)
            continue
        if part == "body" and abs(nx) > 0.6:
            region = CAR["FRONT"] if nx > 0 else CAR["REAR"]
            flip = 1 if nx > 0 else -1
            uvs = [within(region, 0.5 - flip * p.y / (2 * half_width_fn(p.z)), (high - p.z) / max(1e-3, high - low)) for p in corners]
        elif part == "cabin" and abs(nx) > 0.35:
            uvs = [within(CAR["WINDSHIELD"], 0.5 + p.y / (2 * half_width_fn(p.z)), k) for p, k in zip(corners, (0, 0, 1, 1))]
        else:
            uvs = [within(CAR["PAINT"], 0.5, 0.5)] * 4
        m.polygon(corners, uvs, outward)


def car_mesh(kind):
    base_kind = "sedan" if kind == "taxi" else kind
    body, w, cabin, cw, r = SHAPES[base_kind]
    m = Mesh()
    extrude_car(m, body, lambda _z: w, "body")
    z0, z1 = min(z for _, z in cabin), max(z for _, z in cabin)
    extrude_car(m, cabin, lambda z: cw[0] + (cw[1] - cw[0]) * (z - z0) / (z1 - z0), "cabin")
    front, rear, low, _high = xz_range(body)
    m.box(mat_identity(), V(front - 0.14, -w - 0.03, low + 0.04), V(front + 0.07, w + 0.03, low + 0.21), CAR["PLASTIC"])
    m.box(mat_identity(), V(rear - 0.07, -w - 0.03, low + 0.04), V(rear + 0.14, w + 0.03, low + 0.21), CAR["PLASTIC"])
    mx, mz = cabin[0]
    for side in (-1, 1):
        y0, y1 = side * (cw[0] + 0.02), side * (cw[0] + 0.22)
        m.box(mat_identity(), V(mx - 0.34, min(y0, y1), mz + 0.08), V(mx - 0.17, max(y0, y1), mz + 0.21), CAR["PAINT"])
    length = front - rear
    for frac in (0.19, 0.8):
        x = front - frac * length
        for side in (-1, 1):
            out = rx(math.radians(-90 * side))
            m.cylinder(mat_mul(tr(V(x, side * (w - 0.2), r)), out), r, 0.23, 32, CAR["TIRE"], CAR["PLASTIC"])
            m.cylinder(mat_mul(tr(V(x, side * (w + 0.035), r)), out), r * 0.7, 0.018, 32, CAR["PLASTIC"], CAR["RIM"])
    if kind == "taxi":
        roof = max(z for _, z in cabin)
        x = (cabin[1][0] + cabin[2][0]) / 2
        m.box(mat_identity(), V(x - 0.14, -0.34, roof), V(x + 0.14, 0.34, roof + 0.22), CAR["TAXI_SIGN"])
    return m


def person_body():
    m = Mesh()
    def at(x, y, z): return tr(V(x, y, z))
    for t, r, reg in [
        (at(0,0,1.645), V(0.105,0.095,0.12), PERSON["SKIN"]),
        (at(-0.012,0,1.69), V(0.11,0.102,0.095), PERSON["HAIR"]),
        (at(0,0,1.49), V(0.055,0.055,0.075), PERSON["SKIN"]),
        (at(0,0,1.31), V(0.12,0.19,0.19), PERSON["SHIRT"]),
        (at(0,0,1.08), V(0.105,0.16,0.145), PERSON["SHIRT"]),
        (at(0,0,0.93), V(0.11,0.17,0.105), PERSON["PANTS"]),
    ]:
        m.ellipsoid(t, r, 18, 12, reg)
    for side in (-1, 1):
        m.ellipsoid(mat_mul(at(0, side * 0.235, 1.23), rx(math.radians(9 * side))), V(0.05,0.048,0.16), 14, 10, PERSON["SHIRT"])
        m.ellipsoid(mat_mul(at(0.01, side * 0.28, 0.95), rx(math.radians(7 * side))), V(0.042,0.04,0.14), 14, 10, PERSON["SKIN"])
        m.ellipsoid(mat_mul(at(0.015, side * 0.305, 0.765), rx(math.radians(5 * side))), V(0.045,0.03,0.07), 12, 8, PERSON["SKIN"])
    return m


def leg_mesh():
    m = Mesh()
    m.ellipsoid(tr(V(0, 0, -0.22)), V(0.075, 0.07, 0.24), 14, 10, PERSON["PANTS"])
    m.ellipsoid(tr(V(-0.01, 0, -0.62)), V(0.055, 0.052, 0.22), 14, 10, PERSON["PANTS"])
    m.ellipsoid(tr(V(0.06, 0, -0.865)), V(0.12, 0.045, 0.036), 12, 8, PERSON["SHOES"])
    return m


def lamp_mesh():
    m = Mesh()
    m.cylinder(ry(math.pi / 2), 0.11, 0.025, 24)
    return m


def sky_dome():
    m = Mesh()
    m.ellipsoid(mat_identity(), V(1, 1, 1), 48, 24, inward=True)
    return m


def make_scene_files(mesh_names):
    stems = ["glass_facade", "brick_facade", "concrete_facade", "stone_facade", "roof", "road", "intersection", "crosswalk", "sidewalk", "concrete", "grass", "bark", "leaves", "metal", "signal", "car", "person"]
    with open(ATLAS_DIR / "demo_016_city_atlas.atlist", "w", encoding="ascii", newline="\n") as f:
        f.write("\n".join(f"demo_016_city/{s}.png" for s in stems) + "\n")
    mats = []
    for s in stems:
        if s == "glass_facade":
            vals = (90.0, 0.9, 0.55)
        elif s == "car":
            vals = (60.0, 0.7, 0.25)
        elif s in ("concrete_facade", "stone_facade", "brick_facade"):
            vals = (40.0, 0.25, 0.12)
        elif s == "metal":
            vals = (30.0, 0.4, 0.1)
        elif s in ("leaves", "grass"):
            vals = (6.0, 0.05, 0.0)
        else:
            vals = (12.0, 0.1, 0.0)
        mats.append({"name": s, "shader": "Lit", "texture": {"atlas": "City", "name": s}, "uniforms": {"shininess": {"float": vals[0]}, "specularStrength": {"float": vals[1]}, "reflectivity": {"float": vals[2]}}})
    scene = {
        "atlases": [{"name": "City", "bitmap": "demo_016_city_atlas.png", "metadata": "demo_016_city_atlas.atlas"}],
        "meshes": [{"name": name, "file": f"demo_016_city_{name}.obj"} for name in mesh_names],
        "shaders": [
            {"name": "Lit", "vertex": "demo_016_city_shader.vert.spv", "fragment": "demo_016_city_shader.frag.spv", "model_uniform": "model", "view_uniform": "view", "projection_uniform": "proj", "umin_uniform": "umin", "umax_uniform": "umax", "vmin_uniform": "vmin", "vmax_uniform": "vmax", "shadow_matrix_uniform": "lightViewProj", "shadow_map_sampler": "shadowMap"},
            {"name": "Sky", "vertex": "demo_016_city_sky_shader.vert.spv", "fragment": "demo_016_city_sky_shader.frag.spv", "model_uniform": "model", "view_uniform": "view", "projection_uniform": "proj"},
        ],
        "materials": [{"name": "Sky", "shader": "Sky"}] + mats,
        "root": {"name": "Scene root", "children": [
            {"name": "Sky", "mesh": "sky_dome_mesh", "material": "Sky", "uniforms": {"sunColor": {"vec3": [1.6, 1.45, 1.2]}, "skyColor": {"vec3": [0.14, 0.32, 0.72]}, "horizonColor": {"vec3": [0.62, 0.7, 0.8]}, "groundColor": {"vec3": [0.28, 0.27, 0.26]}}},
            {"name": "City", "uniforms": {"sunColor": {"vec3": [1.6, 1.45, 1.2]}, "skyColor": {"vec3": [0.14, 0.32, 0.72]}, "horizonColor": {"vec3": [0.62, 0.7, 0.8]}, "groundColor": {"vec3": [0.28, 0.27, 0.26]}, "fogDensity": {"float": 0.0055}, "tint": {"vec3": [1.0, 1.0, 1.0]}, "emissive": {"vec3": [0.0, 0.0, 0.0]}}, "children": [{"name": "Static"}]},
        ]},
    }
    (RES_DIR / "demo_016_city_scene.json").write_text(json.dumps(scene, indent=2) + "\n", encoding="ascii")


def main():
    RES_DIR.mkdir(parents=True, exist_ok=True)
    make_textures()
    meshes = make_city_meshes()
    meshes.update({
        "sedan_mesh": car_mesh("sedan"),
        "hatchback_mesh": car_mesh("hatchback"),
        "suv_mesh": car_mesh("suv"),
        "taxi_mesh": car_mesh("taxi"),
        "person_mesh": person_body(),
        "leg_mesh": leg_mesh(),
        "lamp_mesh": lamp_mesh(),
        "sky_dome_mesh": sky_dome(),
    })
    for name, mesh in meshes.items():
        mesh.write(RES_DIR / f"demo_016_city_{name}.obj")
    make_scene_files(list(meshes.keys()))
    total = 0
    for name, mesh in meshes.items():
        path = RES_DIR / f"demo_016_city_{name}.obj"
        size = path.stat().st_size
        total += size
        print(f"{name:24s} vertices={len(mesh.verts):7d} triangles={len(mesh.faces):7d} size={size/1048576:6.2f} MiB")
    print(f"OBJ total: {total/1048576:.2f} MiB")


if __name__ == "__main__":
    main()
