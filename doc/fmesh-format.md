# The `.fmesh` file format

An `.fmesh` file holds one triangle mesh, ready for feather.rs: per-vertex positions, normals
and texture coordinates, and triangle indices. It is made from a Wavefront OBJ file by the
`mesh-converter` tool (`tools/mesh-converter`), and loaded by the engine's
`MeshBuilderFmeshFile` (or by a scene file's mesh `file` ending in `.fmesh`).

It replaces the C++ feather's `.mesh` files (made by its `ObjMeshConverter`); see
[archive/done-fmesh-plan.md](archive/done-fmesh-plan.md) for what changed and why.

This document is enough to write a reader or a writer without the Rust code. The Rust
implementation is the `feather-fmesh` crate (`crates/feather-fmesh`).

Contents: [Layout](#layout) · [Header](#header) · [Flags](#flags) ·
[Stream descriptors](#stream-descriptors) · [Data](#data) · [Compression and checksum](#compression-and-checksum) ·
[Conventions](#conventions) · [Validation](#validation) · [Example](#example) ·
[Versions](#versions) · [How the converter chooses the precision](#how-the-converter-chooses-the-precision) ·
[Measurements and decisions](#measurements-and-decisions)

## Layout

```
+------------------------------+  offset 0
| header (76 bytes in v1)      |
+------------------------------+  offset = header size
| data, possibly deflated:     |
|   positions  (3 per vertex)  |
|   normals    (3 per vertex)  |
|   UVs        (2 per vertex)  |
|   indices    (3 per triangle)|
+------------------------------+  end of file
```

Every number is **little-endian**. Integers are two's complement; floats are IEEE 754
binary32. Nothing is aligned or padded.

## Header

| Offset | Size | Type | Field | Meaning |
|---:|---:|---|---|---|
| 0 | 8 | `u8[8]` | magic | `46 4D 45 53 48 0D 0A 1A`: `FMESH`, CR, LF, Ctrl-Z. As in PNG, the CR LF pair reveals a file damaged by a text-mode transfer. |
| 8 | 2 | `u16` | version | 1. See [Versions](#versions). |
| 10 | 2 | `u16` | header size | 76 in version 1. The data starts at this offset. A reader skips any header bytes it doesn't know; a header smaller than 76 is invalid. |
| 12 | 4 | `u32` | flags | See [Flags](#flags). |
| 16 | 4 | `u32` | vertex count | V |
| 20 | 4 | `u32` | index count | I, a multiple of 3 (three per triangle) |
| 24 | 4 | descriptor | positions | How the positions are stored, see [Stream descriptors](#stream-descriptors) |
| 28 | 4 | descriptor | normals | |
| 32 | 4 | descriptor | UVs | |
| 36 | 4 | descriptor | indices | |
| 40 | 12 | `f32[3]` | bounds min | Smallest x, y, z of the positions (all 0 for a mesh without vertices). For culling and framing; readers need not use it. |
| 52 | 12 | `f32[3]` | bounds max | Largest x, y, z of the positions |
| 64 | 4 | `u32` | data size | Size of the data once decompressed, in bytes; at most 2^30 |
| 68 | 4 | `u32` | stored size | Size of the data as stored in the file; equal to the data size when not compressed |
| 72 | 4 | `u32` | CRC-32 | Checksum of the stored data, see [Compression and checksum](#compression-and-checksum) |

The file is exactly `header size + stored size` bytes long.

## Flags

| Bit | Mask | Name | Meaning |
|---:|---:|---|---|
| 0 | `0x1` | compressed | The data is deflated |

Every other bit must be 0. A reader rejects a file with a bit it doesn't know, as such a file
may need that bit understood to be read correctly.

## Stream descriptors

Each of the four streams (positions, normals, UVs, indices) has a 4-byte descriptor:

| Byte | Field | Meaning |
|---:|---|---|
| 0 | encoding | How each value is stored (table below) |
| 1 | decimals `d` | A stored integer `n` stands for `n / 10^d`. 0 to 18; must be 0 for `f32` and for indices. |
| 2–3 | reserved | Written as 0, ignored by readers |

| Code | Encoding | Allowed for | Bytes per value | Value |
|---:|---|---|---|---|
| 0 | `f32` | positions, normals, UVs | 4 | the float itself |
| 1 | `i8` | positions, normals, UVs | 1 | `n / 10^d` |
| 2 | `i16` | positions, normals, UVs | 2 | `n / 10^d` |
| 3 | `i32` | positions, normals, UVs | 4 | `n / 10^d` |
| 4 | delta | all four streams | 1 to 10 (varint) | see below |
| 5 | `u16` | indices | 2 | the index |
| 6 | `u32` | indices | 4 | the index |

Any other code, or a code not allowed for its stream, makes the file invalid.

**Delta.** Each value is stored as the difference from the value one vertex earlier (for
indices, from the previous index); the first vertex's values (and the first index) are
differences from 0. In a vertex stream with C components per vertex (3 for positions and
normals, 2 for UVs, 1 for indices), value `k` is stored as `v[k] - v[k - C]`. Each difference
is zigzag-encoded (`(x << 1) ^ (x >> 63)` on 64-bit integers: 0, -1, 1, -2, 2... become 0, 1, 2,
3, 4...) and written as an unsigned LEB128 varint: 7 bits per byte, least significant group
first, the top bit set on every byte but the last; at most 10 bytes. The decoded vertex
values must fit an `i32`, the indices a `u32`. For vertex streams, the decoded integers are
`n` with decimals `d` as above.

**From integers to floats.** The value of `n` with `d` decimals is the `f32` nearest to the
decimal number `n / 10^d`: what parsing its text (`"0.573651"` for `n` = 573651, `d` = 6)
gives. A reader gets exactly that `f32` this way:

- when `|n| < 2^24` and `d ≤ 10`: `(f32)n / (f32)10^d`, in `f32`. Both operands are exact in
  `f32` and IEEE division rounds correctly, so the quotient is the nearest `f32`;
- otherwise: `(f32)((f64)n / (f64)10^d)`. This can be one ulp away in rare cases, but the
  converter never writes such a value: it checks every value it stores and keeps the stream
  as `f32` instead (see [below](#how-the-converter-chooses-the-precision)).

## Data

After the header, once decompressed (if the `compressed` flag is set), the data is the four
streams one after the other, with nothing between them:

1. positions: 3 × V values (x, y, z of vertex 0, then of vertex 1, ...);
2. normals: 3 × V values;
3. UVs: 2 × V values (u, v of each vertex);
4. indices: I values.

With a fixed-width encoding a stream takes exactly `count × width` bytes; with delta, as many
bytes as its `count` varints take, so a reader decodes the streams in order. After the
indices, no byte may be left.

## Compression and checksum

When the `compressed` flag is set, the stored data is **raw deflate** (RFC 1951), without the
zlib (RFC 1950) or gzip (RFC 1952) wrapper. *Stored size* is the deflated size; inflating it
must give exactly *data size* bytes. When the flag is clear, the stored data is the data, and
both sizes are equal.

The **CRC-32** is the one of zip, gzip, zlib and PNG (ISO-HDLC: reflected polynomial
`0xEDB88320`, initial value `0xFFFFFFFF`, final XOR `0xFFFFFFFF`; the CRC of `"123456789"` is
`0xCBF43926`), computed over the **stored** bytes, so a damaged file is found before it is
inflated. It doesn't cover the header; the header's own checks (magic, sizes, counts) catch
most damage there.

## Conventions

- **Axes**: feather.rs's, Z up. The converter keeps the OBJ's coordinates as they are; the
  asset scripts in `tools/` write OBJs in these axes.
- **Units**: those of the OBJ (metres in the test app).
- **Triangles**: three indices each, counter-clockwise seen from the front (the OBJ's order).
  Polygons of the OBJ are split into fans around their first corner.
- **Normals**: as in the OBJ (not normalized again), or computed by the converter (unit
  length, 4 decimals).
- **UVs**: V is already flipped for Vulkan: `v` here is `1 - v` of the OBJ, so 0 is the top of
  the image (unless converted with `--no-flip-v`). Values outside 0–1 (tiling) are allowed.
- **Vertices**: identical corners (same position, normal and UV) are one vertex, in the order
  of their first use by the triangles, as `MeshBuilderObjFile` makes them. `-0.0` is stored
  as `0.0`.

## Validation

A reader rejects a file, with an error saying why, when:

| Problem | Error (`FmeshError`) |
|---|---|
| The first 8 bytes aren't the magic | not an .fmesh file (`BadMagic`) |
| The version isn't one the reader knows | "the file is version N, this build reads version 1: convert the mesh again" (`UnsupportedVersion`) |
| The header size is below 76 | `BadHeaderSize` |
| The file is shorter than the header, or than `header size + stored size` | truncated (`Truncated`) |
| Bytes follow the stored data | `Invalid` |
| An unknown flag is set | `UnknownFlags` |
| An encoding code is unknown or not allowed for its stream | `BadEncoding` |
| Decimals above 18, or not 0 for `f32` / indices | `BadDecimals` |
| The index count isn't a multiple of 3 | `Invalid` |
| Data size above 2^30, or below `8 × V + I` (every value takes at least a byte) | `Invalid` |
| Not compressed, but stored size ≠ data size | `Invalid` |
| The CRC-32 doesn't match | checksum mismatch (`Checksum`) |
| Inflating fails, or doesn't give exactly data size bytes | `Decompress` |
| A stream ends before its last value, a varint is longer than 10 bytes, a decoded value doesn't fit, or bytes are left after the indices | `Invalid` |
| An index is V or more | `Invalid` |

The bounds are not checked.

## Example

One triangle, `crates/feather-fmesh/testdata/triangle.obj`:

```
v 0 0 0
v 1.5 0 0
v 0 1.25 0
vt 0 0
vt 1 0
vt 0 1
vn 0 0 1
f 1/1/1 2/2/1 3/3/1
```

converted with `mesh-converter triangle.obj --encoding int --compress off` into
`triangle.fmesh`, 115 bytes. The positions have at most 2 decimals, so they're stored as
hundredths (`1.5` is 150, too large for `i8`, so `i16`); the normals and UVs have none. A test
(`example_in_the_documentation`) reads the bytes of this table and compares them with
`testdata/triangle.fmesh`, which another test makes again from the OBJ.

<!-- example:start -->
| Offset | Bytes | Meaning |
|---:|---|---|
| 0 | `46 4d 45 53 48 0d 0a 1a` | Magic: `FMESH`, CR, LF, Ctrl-Z |
| 8 | `01 00` | Version 1 |
| 10 | `4c 00` | Header size 76 |
| 12 | `00 00 00 00` | Flags: none (not compressed) |
| 16 | `03 00 00 00` | Vertex count 3 |
| 20 | `03 00 00 00` | Index count 3 |
| 24 | `02 02 00 00` | Positions: encoding 2 (`i16`), 2 decimals |
| 28 | `01 00 00 00` | Normals: encoding 1 (`i8`), 0 decimals |
| 32 | `01 00 00 00` | UVs: encoding 1 (`i8`), 0 decimals |
| 36 | `05 00 00 00` | Indices: encoding 5 (`u16`) |
| 40 | `00 00 00 00 00 00 00 00 00 00 00 00` | Bounds min: 0.0, 0.0, 0.0 |
| 52 | `00 00 c0 3f 00 00 a0 3f 00 00 00 00` | Bounds max: 1.5, 1.25, 0.0 |
| 64 | `27 00 00 00` | Data size 39 |
| 68 | `27 00 00 00` | Stored size 39 |
| 72 | `72 41 73 a1` | CRC-32 of the 39 bytes of data: `0xa1734172` |
| 76 | `00 00 00 00 00 00` | Position 0: 0, 0, 0 → (0, 0, 0) |
| 82 | `96 00 00 00 00 00` | Position 1: 150, 0, 0 → (1.5, 0, 0) |
| 88 | `00 00 7d 00 00 00` | Position 2: 0, 125, 0 → (0, 1.25, 0) |
| 94 | `00 00 01` | Normal 0: (0, 0, 1) |
| 97 | `00 00 01` | Normal 1: (0, 0, 1) |
| 100 | `00 00 01` | Normal 2: (0, 0, 1) |
| 103 | `00 01` | UV 0: (0, 1), the OBJ's (0, 0) with V flipped |
| 105 | `01 01` | UV 1: (1, 1), from (1, 0) |
| 107 | `00 00` | UV 2: (0, 0), from (0, 1) |
| 109 | `00 00 01 00 02 00` | Indices: 0, 1, 2 |
<!-- example:end -->

## Versions

| Version | Changes |
|---:|---|
| 1 | First version (2026-11-12). |

Rules:

- Any change to the layout or the meaning of a field makes a new version. Readers read only
  the versions they know, and reject others with a message asking to convert the mesh again.
  Meshes are converted at build time, so a new version only needs a rebuild.
- A later version may make the header larger (new fields after the version 1 ones); the
  header size field says where the data starts.
- A new encoding gets a new code, and a new flag a new bit; in a new version.
- Every change updates this document (header, tables, example, this history) and
  `feather_fmesh::VERSION` in the same commit.

## How the converter chooses the precision

The converter has no precision setting. It reads every number of the OBJ **as text** and
keeps it exactly, as a decimal `mantissa / 10^decimals`:

- Trailing zeros don't count: `0.50000` has 1 decimal, `1.0000` and `-0.0` have none.
- Exponents are applied: `1.5e-07` is 15 × 10⁻⁸ (8 decimals), `2E3` is 2000 (0).
- Numbers that need more than 18 significant digits, or more than 40 decimals, and texts that
  aren't plain numbers (`nan`, `inf`), aren't decimals.

For each stream (positions, normals, UVs) of a mesh, the decimals `d` are the most of any of
its values, and every value is stored as the integer `n = value × 10^d`, exactly. The V flip
is exact too: `1 - v` is `10^d - n`. The stream is stored as `f32` instead (with a warning)
when:

- some value isn't a decimal;
- `d` is more than the maximum, 9 by default (`--max-decimals`): exporters sometimes write
  float noise such as `0.1000000015`, whose many decimals would turn every value into a large
  integer;
- some `n` doesn't fit an `i32` (e.g. 150.123456789: 9 decimals make it 150 123 456 789);
- some `n` wouldn't read back as exactly the `f32` its text parses to (only possible when
  `|n| ≥ 2^24` or `d > 10`, see [above](#stream-descriptors)).

So a reader always gets the floats the OBJ's text stands for. Then the narrowest encoding is
picked per stream: by default (`--encoding auto`), whichever of fixed-width (`i8`, `i16` or
`i32`, the narrowest holding every value) and delta takes fewer bytes; indices likewise,
`u16` (up to 65 536 vertices) or `u32` against delta.

**Computed normals** (`--flat-normals`, `--soft-normals`) don't come from text: they're rounded
to 4 decimals (an error under 0.00005 per component, invisible in lighting), and stored with
as many of those decimals as they need (axis-aligned normals need none).

In the test app's OBJs, positions have 3 to 6 decimals and normals and UVs 3 to 6; no stream
falls back to `f32`.

## Measurements and decisions

Measured on 2026-11-12, on all 43 OBJ files of the test app, with
`cargo run --release -p mesh-converter --example measure -- apps/testapp/resources/*.obj`
(Windows on ARM, Snapdragon laptop). Sizes are of the data, without the 76-byte header.

**Sizes**, in KiB, all 43 files together. "+deflate" is the same data deflated (level 9).
zstd (level 19) and lz4 compress the automatic encoding, for reference only.

| | Size | +deflate |
|---|---:|---:|
| OBJ text (what was shipped before) | 30 847 | 6 003 |
| `f32` | 29 733 | 4 451 |
| fixed-width integers (`int`) | 20 146 | 4 216 |
| delta | 9 572 | 899 |
| auto (the narrower of the two, per stream) | 9 572 | 902 |
| auto, zstd level 19 | | 708 |
| auto, lz4 | | 1 309 |

(`f32` is about as large as the OBJ text: merging vertices is per (position, normal, UV)
triple, so the city's facades have more vertices than the OBJ has positions.)

A few files:

| File | OBJ | OBJ+deflate | int | delta | delta+deflate |
|---|---:|---:|---:|---:|---:|
| demo_000_viking_room | 484.0 | 123.5 | 142.4 | 89.7 | 62.7 |
| demo_007_vases_classic | 754.8 | 105.7 | 205.9 | 94.1 | 33.4 |
| demo_013_simple_shadow_human | 745.5 | 99.0 | 209.9 | 102.1 | 29.1 |
| demo_016_city_glass_facade | 5 590.1 | 1 146.7 | 4 840.5 | 2 162.6 | 28.6 |
| demo_016_city_tree_tops | 8 238.3 | 1 387.4 | 3 481.7 | 1 525.5 | 433.8 |
| demo_016_city_sedan | 86.1 | 23.3 | 47.1 | 28.2 | 13.0 |

**Load times** on the desktop, from memory, fastest of 15 runs, all 43 files together: the OBJ
the way `MeshBuilderObjFile` reads it (tobj, then merging vertices), against reading the
`.fmesh` and making the same vertices.

| | Time |
|---|---:|
| OBJ (tobj + merging) | 427.8 ms |
| `.fmesh`, `int` | 14.2 ms |
| `.fmesh`, auto (delta) | 38.4 ms |
| `.fmesh`, auto (delta), deflated | 45.8 ms |

The largest OBJ, the city's tree tops (8.0 MiB, 128 400 triangles): 83.7 ms from the OBJ, 5.6 ms
from the `.fmesh`, 8.2 ms deflated. The slowest, the glass facades (185 002 vertices): 94.1 ms,
9.6 ms, 10.6 ms.

**Galaxy S25** (2026-11-12): the same program built for Android (`cargo ndk -t arm64-v8a build
--release -p mesh-converter --example measure`) and run on the phone with `adb shell`, on the
same files:

| | All 43 files | Demo 16's 27 |
|---|---:|---:|
| OBJ (tobj + merging) | 505.4 ms | 481.5 ms |
| `.fmesh`, `int` | 24.2 ms | 23.6 ms |
| `.fmesh`, auto (delta), as Android builds make them | 53.4 ms | 51.7 ms |
| `.fmesh`, auto (delta), deflated | 63.0 ms | 60.5 ms |

In the app, reading the assets from the APK (which inflates them) and adding the meshes to the
scene included, demo 16 logs "`27 meshes built in 64.7 ms`" (logcat, tag `feather`, level
info): about a seventh of parsing its OBJs alone. Every demo draws as before.

**Decisions**:

- **Delta encoding** (what `auto` picks for every stream of every test app mesh): less than
  half the size of fixed-width integers, and it compresses far better (899 KiB against
  4 216 KiB deflated). It costs about 24 ms more than `int` to decode the 43 meshes, still a
  tenth of parsing the OBJs.
- **Deflate on the desktop** (`--compress auto`: kept when it saves 10% or more, which it does
  for every test app mesh): 9.6 MB down to 0.9 MB, for 7 ms more load time.
- **No compression for Android builds** (`feather_build::build_meshes`): the APK deflates its
  assets already, so the meshes take about the same room in the APK either way (1.0 MB of the
  APK instead of about 6 MB of OBJs before), and the device inflates them only once.
- **Deflate, not zstd**: zstd would save another 20%, but needs C code built for every
  target; deflate is pure Rust (`miniz_oxide`, already used by `png`).
- **No common divisor** per stream (the plan's optional second factor): delta + deflate
  already take most of what it would save.
