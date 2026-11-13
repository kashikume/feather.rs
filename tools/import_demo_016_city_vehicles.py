#!/usr/bin/env python3
"""Import demo 016's vehicles from a low-poly OBJ and generate their texture.

    python tools/import_demo_016_city_vehicles.py path/to/cars-obj.obj [--preview out.png]

The source is a Cinema 4D export of eight toy-like vehicles standing in a row (Y up, lined up
along X, each one's front towards -Z): a truck, a school bus, a van, a sedan, a pickup, an SUV,
a delivery truck and a hatchback. It has texture coordinates but no normals, and its material
library (`cars.mtl`) isn't needed: the materials are named after colours, which are chosen here
to match the original renders.

For each vehicle this writes `resources/demo_016_city_<name>_mesh.obj`: Z up, the front towards
+X, centred on the origin at ground level, triangulated, with flat normals. (The test app's build
converts it into `demo_016_city_<name>_mesh.fmesh`, which the scene loads.) All of them share one
texture, `atlases/demo_016_city/vehicles.png`, generated here: a swatch per colour (paint shaded
darker towards the ground, glass reflecting the sky, rubber, ...). Each face is mapped onto its
swatch by where it lies on its vehicle, so the shading runs across the whole body. The body paint
of every vehicle uses a white swatch with alpha 1, so the demo can paint each car with `tint`.
"""

from __future__ import annotations

import argparse
import math
import sys

sys.dont_write_bytecode = True
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate_demo_016_city_assets import (  # noqa: E402
    RES_DIR,
    TEX_DIR,
    VEHICLES,
    V,
    Mesh,
    Noise,
    clamp8,
    glass,
)

# Metres per source unit: makes the hatchback about 3.9 m long and the truck about 9.5 m.
SCALE = 0.9
# And across: the toy-like models are wide for their length (the bus would be 3.2 m wide).
WIDTH = 0.85

# The material each vehicle's body is painted with. The vehicles stand from +X to -X in the
# source in the order of `VEHICLES`.
PAINTS = {
    "truck": "dark blue_1",
    "bus": "orange_1",
    "van": "milk-white_1",
    "sedan": "blue_1",
    "pickup": "milk-white_1",
    "suv": "red orange_1",
    "delivery": "Pink_1",
    "hatchback": "sky blue_1",
}

# sRGB colours of the source materials, picked from the original renders.
COLOURS = {
    "milk-white_1": (0.94, 0.92, 0.88),
    "black_1": (0.12, 0.12, 0.14),
    "dark blue_1": (0.20, 0.20, 0.36),
    "blue_1": (0.08, 0.13, 0.36),
    "sky blue_1": (0.60, 0.80, 0.93),
    "red orange_1": (0.93, 0.32, 0.17),
    "orange_1": (0.97, 0.63, 0.12),
    "Pink_1": (0.95, 0.68, 0.72),
    "red _1": (0.80, 0.12, 0.16),
    "default": (0.78, 0.78, 0.80),
}
GLASS = "sky blue_1"
RUBBER = "black_1"
PAINT = "paint"

# The texture: a 4 x 3 grid of square swatches. Faces use the middle of their swatch only, so filtering
# and mip maps don't bleed in the neighbours' colours.
COLUMNS, ROWS = 4, 3
SWATCH = 128
SWATCHES = [PAINT] + list(COLOURS)
MARGIN = 0.18


def swatch_rect(material: str) -> tuple[float, float, float, float]:
    k = SWATCHES.index(material)
    x, y = (k % COLUMNS) / COLUMNS, (k // COLUMNS) / ROWS
    return (x, y, x + 1 / COLUMNS, y + 1 / ROWS)


def swatch_uv(material: str, u: float, v: float) -> tuple[float, float]:
    """`u`, `v` (0..1, `v` down from the top of the vehicle) inside the swatch's middle."""
    x0, y0, x1, y1 = swatch_rect(material)
    mx, my = (x1 - x0) * MARGIN, (y1 - y0) * MARGIN
    u, v = max(0.0, min(1.0, u)), max(0.0, min(1.0, v))
    return (x0 + mx + (x1 - x0 - 2 * mx) * u, y0 + my + (y1 - y0 - 2 * my) * v)


def make_texture(path: Path):
    """A swatch per material. `u` runs along the vehicle, `v` down from its top: lighter at the
    top, darker towards the ground, with fine grain."""
    n = Noise(31)
    img = Image.new("RGBA", (COLUMNS * SWATCH, ROWS * SWATCH))
    px = img.load()
    m = SWATCH * MARGIN
    for k, material in enumerate(SWATCHES):
        x0, y0 = (k % COLUMNS) * SWATCH, (k // COLUMNS) * SWATCH
        for y in range(y0, y0 + SWATCH):
            for x in range(x0, x0 + SWATCH):
                u = max(0.0, min(1.0, (x - x0 + 0.5 - m) / (SWATCH - 2 * m)))
                v = max(0.0, min(1.0, (y - y0 + 0.5 - m) / (SWATCH - 2 * m)))
                grain = n.fbm(x / 6.0, y / 6.0, 512, 3) - 0.5
                # Ambient occlusion: the lower part of the body sees less sky, the sills least.
                sill = max(0.0, min(1.0, (v - 0.78) / 0.22))
                shade = 1.06 - 0.2 * v**1.5 - 0.12 * sill * sill * (3 - 2 * sill)
                if material == PAINT:
                    k = 0.9 * shade + 0.03 * grain
                    rgba = (k, k, k, 1.0)
                elif material == GLASS:
                    r, g, b, _ = glass(u * 3.0, 0.15 + 0.7 * v, n)
                    tint = COLOURS[GLASS]
                    # Soft diagonal highlights, as from a bright sky.
                    sheen = 0.16 * max(0.0, 1.0 - abs((u * 3.0 + v * 1.5) % 1.0 - 0.3) / 0.09) ** 2
                    rgba = (r * 0.5 + tint[0] * 0.5 + sheen, g * 0.5 + tint[1] * 0.5 + sheen, b * 0.5 + tint[2] * 0.5 + sheen, 0.0)
                elif material == RUBBER:
                    k = COLOURS[RUBBER][0] * (1.0 + 0.6 * grain) * shade
                    rgba = (k, k, k * 1.08, 0.0)
                else:
                    c = COLOURS[material]
                    rgba = tuple(c[i] * shade * (1.0 + 0.06 * grain) for i in range(3)) + (0.0,)
                px[x, y] = tuple(clamp8(c) for c in rgba)
    img.save(path)


class Source:
    """The objects of the OBJ: name -> faces as (material, [positions])."""

    def __init__(self, path: Path):
        verts: list[tuple[float, float, float]] = []
        self.objects: dict[str, list[tuple[str, list[tuple[float, float, float]]]]] = {}
        faces = None
        material = "default"
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                t = line.split()
                if not t:
                    continue
                if t[0] == "v":
                    verts.append((float(t[1]), float(t[2]), float(t[3])))
                elif t[0] == "o":
                    faces = self.objects.setdefault(t[1], [])
                elif t[0] == "usemtl":
                    material = line.strip()[len("usemtl"):].strip()
                elif t[0] == "f":
                    if faces is None:
                        raise ValueError("a face before any object")
                    idx = [int(r.split("/")[0]) for r in t[1:]]
                    faces.append((material, [verts[i - 1 if i > 0 else i] for i in idx]))

    def centre_x(self, name: str) -> float:
        xs = [p[0] for _, ps in self.objects[name] for p in ps]
        return (min(xs) + max(xs)) / 2


def group_vehicles(source: Source) -> list[list[str]]:
    """The objects of each vehicle, from +X to -X: they stand in a row along X, apart."""
    names = sorted(source.objects, key=source.centre_x, reverse=True)
    groups: list[list[str]] = []
    for name in names:
        if groups and source.centre_x(groups[-1][0]) - source.centre_x(name) < 2.5:
            groups[-1].append(name)
        else:
            groups.append([name])
    if len(groups) != len(VEHICLES):
        raise ValueError(f"expected {len(VEHICLES)} vehicles, found {len(groups)}: {groups}")
    return groups


def length(v: V) -> float:
    return math.sqrt(v.dot(v))


def newell(points: list[V]) -> V:
    n = V(0, 0, 0)
    for a, b in zip(points, points[1:] + points[:1]):
        n = n + V((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y))
    return n


def triangulate(points: list[V], normal: V) -> list[tuple[int, int, int]]:
    """Ear clipping in the polygon's plane: some faces are concave n-gons."""
    if len(points) == 3:
        return [(0, 1, 2)]
    axis = max(range(3), key=lambda k: abs((normal.x, normal.y, normal.z)[k]))
    flat = [((p.y, p.z), (p.z, p.x), (p.x, p.y))[axis] for p in points]
    sign = 1.0 if (normal.x, normal.y, normal.z)[axis] > 0 else -1.0

    def cross(o, a, b):
        return ((a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])) * sign

    def inside(p, a, b, c):
        return cross(a, b, p) >= 0 and cross(b, c, p) >= 0 and cross(c, a, p) >= 0

    left = list(range(len(points)))
    out = []
    while len(left) > 3:
        for k in range(len(left)):
            i, j, l = left[k - 1], left[k], left[(k + 1) % len(left)]
            a, b, c = flat[i], flat[j], flat[l]
            if cross(a, b, c) <= 1e-12:
                continue
            if any(inside(flat[o], a, b, c) for o in left if o not in (i, j, l) and flat[o] not in (a, b, c)):
                continue
            out.append((i, j, l))
            left.pop(k)
            break
        else:  # Degenerate: fall back to a fan.
            out += [(left[0], left[k], left[k + 1]) for k in range(1, len(left) - 1)]
            return out
    out.append(tuple(left))
    return out


def build_vehicle(source: Source, objects: list[str], paint: str):
    """The vehicle's mesh, its length and its faces (for the preview)."""
    faces = []
    for name in objects:
        tire = name.startswith("tire")
        for material, ps in source.objects[name]:
            # Source: Y up, front towards -Z, right towards +X. Ours: Z up, front +X, left +Y.
            pts = [V(-p[2], -p[0] * WIDTH, p[1]) * SCALE for p in ps]
            faces.append(("paint" if material == paint and not tire else material, pts))
    all_pts = [p for _, ps in faces for p in ps]
    lo = V(min(p.x for p in all_pts), min(p.y for p in all_pts), min(p.z for p in all_pts))
    hi = V(max(p.x for p in all_pts), max(p.y for p in all_pts), max(p.z for p in all_pts))
    shift = V(-(lo.x + hi.x) / 2, -(lo.y + hi.y) / 2, -lo.z)
    size = hi - lo
    faces = [(m, [p + shift for p in ps]) for m, ps in faces]

    # Are the faces wound counter-clockwise seen from outside? Ask the big, flat ones.
    out = sum(
        (1 if newell(ps).dot(sum(ps[1:], ps[0]) * (1 / len(ps)) - V(0, 0, size.z / 2)) > 0 else -1) * length(newell(ps))
        for _, ps in faces
    )
    flip = out < 0

    mesh = Mesh()
    for material, ps in faces:
        if flip:
            ps = ps[::-1]
        normal = newell(ps)
        if length(normal) < 1e-9:
            continue
        normal = normal.norm()
        a = max(range(3), key=lambda k: abs((normal.x, normal.y, normal.z)[k]))

        def uv(p: V) -> tuple[float, float]:
            along = (p.x + size.x / 2) / size.x
            across = (p.y + size.y / 2) / size.y
            down = 1.0 - p.z / size.z
            # Sides and ends show the height; tops and bottoms are seen from above.
            return swatch_uv(material, (across, along, along)[a], (down, down, across)[a])

        ids = [mesh.vertex(p, uv(p), normal) for p in ps]
        for i, j, k in triangulate(ps, normal):
            mesh.tri(ids[i], ids[j], ids[k])
    return mesh, size, faces, flip


def preview(vehicles, path: Path):
    """A flat-shaded picture of the vehicles in a row, colours as in the texture."""
    w, h = 1600, 700
    img = Image.new("RGB", (w, h), (205, 203, 210))
    from PIL import ImageDraw

    draw = ImageDraw.Draw(img)
    yaw, pitch = math.radians(215), math.radians(28)
    light = V(0.4, 0.6, 0.7).norm()
    tris = []
    offset = 0.0
    for name, (_, size, faces, flip) in vehicles:
        paint = COLOURS[PAINTS[name]]
        y0 = offset + size.y / 2
        offset += size.y + 2.0
        for material, ps in faces:
            if flip:
                ps = ps[::-1]
            n = newell(ps)
            if length(n) < 1e-9:
                continue
            n = n.norm()
            colour = paint if material == PAINT else COLOURS[material]
            k = 0.45 + 0.55 * max(0.0, n.dot(light))
            world = [p + V(0, -y0, 0) for p in ps]
            cam = []
            for p in world:
                x = p.x * math.cos(yaw) - p.y * math.sin(yaw)
                y = p.x * math.sin(yaw) + p.y * math.cos(yaw)
                z = p.z
                # Looking down at `pitch`, the camera on -y.
                sy = z * math.cos(pitch) + y * math.sin(pitch)
                depth = y * math.cos(pitch) - z * math.sin(pitch)
                cam.append((x, sy, depth))
            # Back faces: facing along the view (deeper into the picture).
            if n.x * math.sin(yaw) + n.y * math.cos(yaw) - n.z * math.tan(pitch) > 0:
                continue
            depth = sum(c[2] for c in cam) / len(cam)
            tris.append((depth, [(c[0], c[1]) for c in cam], tuple(int(255 * min(1, c * k)) for c in colour)))
    xs = [x for _, pts, _ in tris for x, _ in pts]
    ys = [y for _, pts, _ in tris for _, y in pts]
    scale = min((w - 60) / (max(xs) - min(xs)), (h - 60) / (max(ys) - min(ys)))
    for _, pts, colour in sorted(tris, key=lambda t: -t[0]):
        draw.polygon([(30 + (x - min(xs)) * scale, h - 30 - (y - min(ys)) * scale) for x, y in pts], fill=colour)
    img.save(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("source", type=Path, help="the vehicles' OBJ (cars-obj.obj)")
    parser.add_argument("--preview", type=Path, help="also draw the vehicles into this PNG")
    args = parser.parse_args()

    source = Source(args.source)
    groups = group_vehicles(source)
    TEX_DIR.mkdir(parents=True, exist_ok=True)
    make_texture(TEX_DIR / "vehicles.png")
    built = []
    for name, objects in zip(VEHICLES, groups):
        mesh, size, faces, flip = build_vehicle(source, objects, PAINTS[name])
        path = RES_DIR / f"demo_016_city_{name}_mesh.obj"
        mesh.write(path)
        built.append((name, (mesh, size, faces, flip)))
        print(f"{name:10s} {size.x:5.2f} x {size.y:4.2f} x {size.z:4.2f} m  triangles={len(mesh.faces):5d}  flipped={flip}  {objects}")
    if args.preview:
        preview(built, args.preview)


if __name__ == "__main__":
    main()
