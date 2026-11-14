"""Writes the tutorial's feather model (doc/tutorial): feather_vane.obj and feather_shaft.obj.

Usage: python tools/scripts/make_feather_obj.py <output dir>

The repository's copies are made with: python tools/scripts/make_feather_obj.py doc/tutorial

The feather stands along Z (the engine's up), about 2.4 long, centered on the origin, curved
towards +Y, with its vane in the XZ plane. Units and axes are the engine's: the OBJ files are
used as they are.

- The vane is a thin surface on both sides of the shaft, drawn twice (one face per side, with
  opposite normals), as the engine culls back faces. The right side is wider than the left, as on
  a bird's flight feather; the edges droop away from the shaft, and the barbs slant towards the
  tip.
- The shaft is a tapered tube with a cap at each end.

Texture coordinates: u goes across the vane (0 at the left edge, 1 at the right edge, 0.5 on
the shaft); v goes along the feather, written as 1 - t so that the engine, which flips v when it
loads an OBJ, sees 0 at the base and 1 at the tip.
"""

import math
import os
import sys

LENGTH = 2.4          # along Z
CURVE = 0.25          # how far the tip bends towards +Y
VANE_START = 0.15     # where the vane begins, as a fraction of the length (below: bare quill)
WIDTH_RIGHT = 0.42    # half widths of the vane at its widest
WIDTH_LEFT = 0.30
DROOP = 0.12          # how far the vane's edges bend down (towards -Y)
SLANT = 0.35          # how far the barbs slant towards the tip, per unit of width
THICKNESS = 0.004     # between the vane's two faces
STEPS_ALONG = 100
STEPS_ACROSS = 6      # per side
SHAFT_SIDES = 10
SHAFT_STEPS = 32
SHAFT_END = 0.97


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def add(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def scale(a, k):
    return (a[0] * k, a[1] * k, a[2] * k)


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def normalize(a):
    n = math.sqrt(dot(a, a))
    return scale(a, 1.0 / n) if n > 0 else (0.0, 0.0, 1.0)


def spine(t):
    """The shaft's centre line at t (0 = base, 1 = tip)."""
    return (0.0, CURVE * t * t - CURVE / 3, LENGTH * (t - 0.5))


def spine_tangent(t):
    return normalize((0.0, 2 * CURVE * t, LENGTH))


# Splits in the vane, where barbs came apart: (side, s, depth as a fraction of the width).
NOTCHES = [(True, 0.52, 0.55), (False, 0.33, 0.45), (False, 0.62, 0.3)]
NOTCH_WIDTH = 0.03    # in s, each side of the notch's bottom


def half_width(s, right):
    """The vane's half width at s (0 = where the vane begins, 1 = tip): it widens quickly from
    the quill, stays almost as wide, and ends in a rounded tip."""
    w = WIDTH_RIGHT if right else WIDTH_LEFT
    base = min(1.0, s / 0.12) ** 0.5
    tip = math.sqrt(max(0.0, 1 - ((s - 0.65) / 0.35) ** 2)) if s > 0.65 else 1.0
    w *= base * tip * (1 - 0.2 * s)
    for side, at, depth in NOTCHES:
        if side == right:
            w *= 1 - depth * max(0.0, 1 - abs(s - at) / NOTCH_WIDTH)
    return w


def vane_point(s, u):
    """A point of the vane: s along it, u across (-1 = left edge, 0 = shaft, 1 = right edge)."""
    t = VANE_START + (1 - VANE_START) * s
    c = spine(t)
    w = half_width(s, u > 0)
    x = u * w
    y = c[1] - DROOP * u * u * (w / WIDTH_RIGHT)
    z = c[2] + SLANT * abs(x)
    return (x, y, z)


class Obj:
    def __init__(self, name):
        self.name = name
        self.lines = []
        self.count = 0

    def vertex(self, p, n, uv):
        self.lines.append("v %.4f %.4f %.4f" % p)
        self.lines.append("vn %.4f %.4f %.4f" % n)
        self.lines.append("vt %.4f %.4f" % uv)
        self.count += 1
        return (self.count, p, n)

    def triangle(self, a, b, c):
        """Writes a triangle wound counter-clockwise as seen from the side its normals face."""
        facing = cross(sub(b[1], a[1]), sub(c[1], a[1]))
        if dot(facing, add(add(a[2], b[2]), c[2])) < 0:
            b, c = c, b
        self.lines.append("f %d/%d/%d %d/%d/%d %d/%d/%d" % (
            a[0], a[0], a[0], b[0], b[0], b[0], c[0], c[0], c[0]))

    def quad(self, a, b, c, d):
        """a, b, c, d around the quad."""
        self.triangle(a, b, c)
        self.triangle(a, c, d)

    def write(self, path):
        with open(path, "w", newline="\n") as f:
            f.write("# %s, made by tools/scripts/make_feather_obj.py\n" % self.name)
            f.write("o %s\n" % self.name)
            f.write("\n".join(self.lines) + "\n")


def make_vane():
    obj = Obj("feather_vane")
    across = 2 * STEPS_ACROSS
    for side in (1, -1):  # 1: the face towards +Y, -1: the face towards -Y
        grid = []
        for i in range(STEPS_ALONG + 1):
            s = i / STEPS_ALONG
            t = VANE_START + (1 - VANE_START) * s
            row = []
            for j in range(across + 1):
                u = -1 + 2 * j / across
                p = vane_point(s, u)
                # The normal from the surface's slopes, turned to the face's side.
                ds, du = 1e-3, 1e-3
                along = sub(vane_point(min(s + ds, 1), u), vane_point(max(s - ds, 0), u))
                side_dir = sub(vane_point(s, min(u + du, 1)), vane_point(s, max(u - du, -1)))
                n = cross(along, side_dir)
                # At the base and the tip the vane has no width: face straight to the side.
                n = normalize(n) if dot(n, n) > 1e-12 else (0.0, 1.0, 0.0)
                if dot(n, (0, 1, 0)) * side < 0:
                    n = scale(n, -1)
                p = add(p, scale(n, THICKNESS / 2))
                row.append(obj.vertex(p, n, ((u + 1) / 2, 1 - t)))
            grid.append(row)
        for i in range(STEPS_ALONG):
            for j in range(across):
                obj.quad(grid[i][j], grid[i][j + 1], grid[i + 1][j + 1], grid[i + 1][j])
    return obj


def make_shaft():
    obj = Obj("feather_shaft")
    rings = []
    for i in range(SHAFT_STEPS + 1):
        t = SHAFT_END * i / SHAFT_STEPS
        c = spine(t)
        tangent = spine_tangent(t)
        n1 = (1.0, 0.0, 0.0)
        n2 = cross(tangent, n1)
        r = 0.028 * (1 - t) ** 0.7 + 0.004
        ring = []
        for k in range(SHAFT_SIDES):
            a = 2 * math.pi * k / SHAFT_SIDES
            n = add(scale(n1, math.cos(a)), scale(n2, math.sin(a)))
            ring.append(obj.vertex(add(c, scale(n, r)), n, (0.5, 1 - t)))
        rings.append(ring)
    for i in range(SHAFT_STEPS):
        for k in range(SHAFT_SIDES):
            k2 = (k + 1) % SHAFT_SIDES
            obj.quad(rings[i][k], rings[i][k2], rings[i + 1][k2], rings[i + 1][k])
    # Caps: a fan at each end, facing out along the shaft.
    for t, ring, sign in ((0.0, rings[0], -1), (SHAFT_END, rings[-1], 1)):
        n = scale(spine_tangent(t), sign)
        centre = obj.vertex(spine(t), n, (0.5, 1 - t))
        edge = [obj.vertex(v[1], n, (0.5, 1 - t)) for v in ring]
        for k in range(SHAFT_SIDES):
            obj.triangle(centre, edge[k], edge[(k + 1) % SHAFT_SIDES])
    return obj


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "."
    make_vane().write(os.path.join(out, "feather_vane.obj"))
    make_shaft().write(os.path.join(out, "feather_shaft.obj"))


if __name__ == "__main__":
    main()
