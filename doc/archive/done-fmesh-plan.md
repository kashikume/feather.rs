# Mesh converter and `.fmesh` files: plan

Goal: **a Rust mesh converter that turns Wavefront OBJ files into compact binary `.fmesh` files,
and the engine loading them**. It's a port of the C++ feather's `ObjMeshConverter`
(`guitarprompter/contrib/feather/tools/ObjMeshConverter`) and its `.mesh` format, keeping the idea
(read the OBJ once, offline; ship a binary file that loads with no text parsing) but with a file
format designed for feather.rs.

Covers items 29 and 33 of [missing-features-vs-cpp.md](../todo/missing-features-vs-cpp.md) (binary mesh
loader and a loader picked by the file's extension; the `ObjMeshConverter` tool), since removed
from it.

## Status (2026-11-12)

Steps 1 to 7 are done. On the Galaxy S25, the 43 meshes decode in 53 ms instead of 505 ms for
their OBJs, and demo 16 builds its 27 meshes in 65 ms; every demo runs (see
[../fmesh-format.md](../fmesh-format.md#measurements-and-decisions)).

- Format spec: [../fmesh-format.md](../fmesh-format.md), with the measurements and decisions.
- `crates/feather-fmesh` (reader and writer), `tools/mesh-converter` (library, command line,
  `examples/measure.rs`), `MeshBuilderFmeshFile`, scene files choosing the loader by extension,
  `feather_build::build_meshes`; the test app's scene files load `.fmesh` files.
- Checked: every test app OBJ converts and reads back exactly (`--check`, and a test comparing
  each with `MeshBuilderObjFile`'s mesh); every demo runs on Windows and on the S25; the APK holds only
  uncompressed `.fmesh` files (1.1 MB in the APK, against about 6 MB of OBJs before).

Where the result differs from this plan:

- The header has a fourth stream descriptor, for the indices (`u16`, `u32` or delta), instead
  of a flag for 32-bit indices; and only one flag, `compressed`: every mesh has normals and UVs.
- Degenerate triangles get the normal (0, 0, 1), not a neighbour's: they draw nothing.
- No common divisor per stream: not needed (see the format document's decisions).
- The converter's crates are built optimized in dev builds too (`[profile.dev.package...]` in
  the root `Cargo.toml`), so the build converts the 32 MB of OBJs in about a second.

Why: the test app ships 43 OBJ files, about 32 MB of text (the city's tree tops alone are 8.4 MB,
its glass facades 5.7 MB), parsed by `tobj` every time a demo starts, on Android too.

## What the C++ converter does

- **Command line**: `ObjMeshConverter inFile outPrefix --order xyz --verbose --precision 10000`.
- **Reading**: its own line parser. `o` starts a new object; `v`, `vn`, `vt` are collected;
  `f` must have exactly three `v/t/n` corners. Each corner becomes a vertex (position, normal,
  UV), merged with an identical one already in the object (a linear search over all of them).
- **Writing**: one file per `o` object, named `<outPrefix><object>.mesh`:

  | Offset | Type | Content |
  |---|---|---|
  | 0 | `int` | `'mesh'` (a multi-character constant) |
  | 4 | `int` | version, 2 (version 1 had no factor) |
  | 8 | `int` | vertex count |
  | 12 | `int` | triangle count |
  | 16 | `int` | factor (the `--precision`, default 10000) |
  | 20 | `int[3n]` | positions, `(int)(f * factor)` |
  | | `int[3n]` | normals, same factor |
  | | `int[2n]` | UVs, same factor |
  | | `uint[3t]` | triangle indices |

- **The engine** (`feather-MeshLoaderMeshFile.cpp`) reads that back, dividing every number by
  the factor.

### Workarounds for the C++ engine (not to port)

These exist because of how the C++ engine draws, and feather.rs doesn't need them. Its
`MeshBuilderObjFile` is the reference for what a mesh must look like.

- **Normals are negated** (`vn` read as `-x, -y, -z`). The C++ OBJ loader does the same
  (`invert(readPoint3d(line))`). feather.rs uses OBJ normals as they are.
- **Triangle winding is swapped**: each face is stored as corners 2, 1, 3. feather.rs keeps the
  OBJ order.
- **No V flip**: feather.rs uses `1 - v` for texture coordinates (`MeshBuilderObjFile`).
- **`--order` axis swap**: no longer needed; the asset scripts in `tools/` already write OBJs in
  feather.rs's axes (Z up).

### Bugs and limits (fixed by the new one)

- **Precision is lost**: numbers are parsed into `float`, multiplied and *truncated*
  (`(int)(0.29f * 10000)` is 2899). A single factor serves positions, normals and UVs, and is
  set by hand: too small loses detail, too large overflows `int` without a warning.
- Only triangles, and only `v/t/n` corners: no quads or polygons, no `v//n`, no negative
  (relative) indices, no `vt` with a third component.
- A face before any `o` line crashes (no current object). Lines it doesn't know (`s`, `g`,
  `usemtl`, `mtllib`) are reported as errors. Lines over 1024 characters are cut.
- Merging vertices is quadratic: slow for the city's meshes (hundreds of thousands of corners).
- Byte order and the `'mesh'` constant depend on the compiler and CPU; `fopen_s` is Windows only.
- The header carries no flags, no bounds, no checksum, and the loader doesn't notice a
  truncated file.

## Precision: automatic

The converter doesn't take a precision. It reads each number **as text** and counts its decimal
places, dropping trailing zeros (`0.50000` has 1, `1.0000` has 0). For each stream (positions,
normals, UVs) the largest count `d` gives the factor `10^d`, and every number of the stream is
stored as the exact integer `n` with `value = n / 10^d`, computed from the digits, never via a
float. Nothing is lost.

What the test app's OBJs have:

| Files | Decimals (v / vn / vt) | Largest coordinate |
|---|---|---|
| `demo_000_viking_room` | 6 / 4 / 6 | ~1 |
| `demo_007`, `008`, `009`, `013` (Blender) | 5 / 5 / 5 | ~2 |
| `demo_016_city_*` (generated) | 3 / 3 / 3 | ~150 |

So `n` fits in 16 bits for most normals and UVs (|n| ≤ 10^d), and in 32 bits for every position.
For each stream the converter picks the narrowest integer width that holds all its values.

Loading gives back **exactly the float the OBJ text would parse to**: when `|n| < 2^24` and
`d ≤ 10`, both `n` and `10^d` are exact in `f32` and IEEE division rounds correctly, so `n as f32 /
10^d as f32` is the nearest `f32` to the decimal, as `str::parse::<f32>` gives. Beyond that
range, the loader divides in `f64` (rounding twice, at most one ulp away). Tests check it
bit-for-bit against `tobj` for every test app mesh.

Edge cases:
- **Exponents** (`1.5e-07`): the decimal count is `digits after the point - exponent`; capped
  at, say, 9. A stream whose numbers don't fit (huge `d`, or `|n|` over 2^31) is stored as plain
  `f32` instead, with a warning.
- **`-0.0`** becomes `0.0`. Harmless (and `Vertex` already treats them as equal).
- The **V flip** happens in integers (`10^d - n`), so it's exact too.
- Optional, decided by the investigation: dividing a stream by the greatest common divisor of
  its integers (e.g. every coordinate a multiple of 0.005), stored as a second factor.

## The `.fmesh` format (version 1)

Little-endian, fixed-size header, then the data. One mesh per file (see [Decisions](#decisions)).

| Offset | Type | Content |
|---|---|---|
| 0 | `[u8; 8]` | magic `"FMESH\r\n\x1a"` (also catches text-mode line-ending damage, like PNG's) |
| 8 | `u16` | format version (1) |
| 10 | `u16` | header size in bytes (so a later version can grow it) |
| 12 | `u32` | flags: data compressed, has normals, has UVs, 32-bit indices |
| 16 | `u32` | vertex count |
| 20 | `u32` | index count (a multiple of 3) |
| 24 | 3 × stream descriptor | positions, normals, UVs: encoding (`f32`, `i8`, `i16`, `i32`, or delta+varint), decimals `d`, reserved |
| | `f32[6]` | bounds: min and max position (for culling and cameras later) |
| | `u32` | data size, uncompressed |
| | `u32` | data size, as stored |
| | `u32` | CRC-32 of the stored data |
| | data | positions, normals, UVs, indices, each as one stream (not interleaved) |

- **Streams, not interleaved vertices**: similar numbers next to each other pack and compress
  better. The loader builds `Vec<Vertex>` from them.
- **Indices** are `u16` when there are at most 65 536 vertices, `u32` otherwise; the loader
  widens them to the `u32` that `Mesh` keeps.
- **Vertex order** is the order of first use, as `MeshBuilderObjFile` makes it, so a `.fmesh`
  and its OBJ give the same `Mesh`.
- **Versions**: the loader reads the versions it knows and otherwise fails with "`file.fmesh`
  is version 2, this build reads 1: convert it again". Any change to the layout bumps the
  version; the spec keeps a history.

### Format documentation

The detailed description of the file format is part of the documentation, in
`doc/fmesh-format.md`, written so that someone could write a reader or a writer from it alone,
without the Rust code. It covers:

- **Header**, field by field: offset, size, type, byte order, meaning, allowed values; the magic
  bytes in hex; what a reader does with a larger header size than it knows.
- **Flags**: each bit, its meaning, and that unknown bits must be zero.
- **Stream descriptors**: every encoding (`f32`, `i8`, `i16`, `i32`, delta + zigzag + varint)
  with its exact byte layout, how the decimals `d` turn integers back into floats, and how a
  reader must divide (`f32` when `|n| < 2^24`, `f64` otherwise) to get the same floats.
- **Data section**: order and size of each stream, index width, alignment (if any).
- **Compression**: which deflate variant (raw deflate or zlib wrapper), what the two sizes
  mean, what the CRC-32 covers (stored or uncompressed bytes) and which polynomial.
- **Conventions**: axes (Z up), V already flipped, triangle winding, units, vertex order.
- **Validation**: what a reader must reject (bad magic, unknown version, sizes that don't add
  up, an index pointing past the vertices, bad checksum) and the error to report.
- **Example**: a hex dump of a small file (one triangle), annotated byte by byte.
- **Version history**: what each version changed; a reader's rules for versions it doesn't
  know.
- **How the precision is found**: the decimal counting, exponents, `--max-decimals`, the
  fallback to `f32`, and the fixed 4 decimals of computed normals.
- **Measurements**: the compression investigation's table and the decisions taken from it
  (including compression off for Android builds).

The document is written in step 1, before the code, and kept in step with it: any change to the
format changes the document and the version in the same commit. A test reads the example file
from the document (stored in `testdata/`) to make sure the two agree.

## Compression: measured

(Results and decisions: [../fmesh-format.md](../fmesh-format.md#measurements-and-decisions).
Delta encoding, deflated on the desktop: 30.8 MB of OBJ become 0.9 MB, and load in 46 ms
instead of 428 ms.)

`.fmesh` data can be stored deflated (the zip algorithm), with `miniz_oxide`: pure Rust, so
nothing to set up for Android. Before deciding, measure on all the test app's meshes:

- **Size** of: OBJ text; raw `f32` vertices; fixed-width integers; delta + zigzag + varint
  integers; each of these deflated; for reference, `zstd` and `lz4` too (in the measurement
  only).
- **Load time** against `tobj` parsing the OBJ:
  - desktop: read + decompress + decode into `Vec<Vertex>`, for compressed and uncompressed
    files;
  - Galaxy S25: read + decode only, uncompressed files (Android builds don't compress, see
    below). Reading through `AAssetManager` already includes inflating the APK's own
    compression, so this measures what the device really does.

Things to keep in mind:
- **The APK is a zip already**: Gradle deflates assets by default, so compressing `.fmesh`
  files barely shrinks the APK; the gain would be on disk on Windows and in the installed app,
  while Android would inflate twice (APK, then file). An uncompressed `.fmesh` could even be
  stored uncompressed in the APK (`noCompress`) and read with no copy.
- Integers of narrow width probably take most of the gain on their own.

The outcome sets the defaults: compression always, never, or per file when it saves more than
some threshold (the `compressed` flag lets each file decide). Results go in `doc/fmesh-format.md`.

**Android builds: compression off.** When `build.rs` builds for Android
(`CARGO_CFG_TARGET_OS == "android"`), `build_meshes` writes every `.fmesh` uncompressed, whatever
the investigation decides for the desktop. The APK already deflates its assets, so compressing
the files too would gain next to nothing and make the device inflate each mesh twice. The loader
still reads compressed files on Android (an app may ship its own), it's only the build that
doesn't make them.

## Command line

```
mesh-converter [options] <input.obj> [output.fmesh]
mesh-converter [options] --out-dir <dir> <input.obj>...
```

Arguments are parsed by hand, like `font-creator` (no `clap`): long options with a short form
for the common ones, `--help` printing the usage with an example.

| Option | Default | What it does |
|---|---|---|
| `<input.obj>` | required | The OBJ file(s) to convert. Several are allowed with `--out-dir`. |
| `[output.fmesh]` | the input's name with `.fmesh` | The file to write, for a single input. |
| `--out-dir <dir>`, `-o` | next to the input | Writes each output to `<dir>/<input name>.fmesh`. Needed with several inputs. |
| `--split`, `-s` | off | One file per `o` / `g` object, named `<output name>_<object>.fmesh` (as the C++ did). Without it, all objects are merged into one mesh, as `MeshBuilderObjFile` does. |
| `--compress <auto\|on\|off>`, `-c` | `auto` | Deflates the data: always, never, or only when it saves enough (threshold set by the investigation). |
| `--encoding <auto\|f32\|int\|delta>`, `-e` | `auto` | How the vertex streams are stored: `auto` picks the smallest per stream; `f32` plain floats; `int` fixed-width integers; `delta` delta + zigzag + varint integers. For the investigation and for checking; `auto` is the normal choice. |
| `--max-decimals <n>` | 9 | The most decimals the automatic precision accepts. A stream with more (e.g. an exporter writing `0.1000000015`) is stored as `f32` with a warning, unless this is raised. Nothing is ever rounded silently. |
| `--flat-normals` | off | Computes the normals instead of taking them from the OBJ: every triangle's corners get the triangle's own normal, so each face is lit evenly and edges look sharp. See [Computed normals](#computed-normals). |
| `--soft-normals` | off | Computes the normals instead of taking them from the OBJ: each corner gets the average of the normals of the faces around its position, so the surface looks smooth. See [Computed normals](#computed-normals). |
| `--no-flip-v` | off | Keeps the OBJ's V coordinate instead of storing `1 - v` (for textures already flipped). |
| `--check` | off | After writing, reads the file back and compares every value with the OBJ's numbers parsed as `f32`; exits with an error on any difference. |
| `--stats` | off | Prints, per output: objects, vertices before and after merging, triangles, decimals and encoding of each stream, index width, and sizes (OBJ, raw, encoded, stored). The investigation uses it. |
| `--verbose`, `-v` | off | Also prints every line skipped and every polygon triangulated. |
| `--quiet`, `-q` | off | Prints only errors. |
| `--help`, `-h` | | Prints the usage. |

Exit code 0 on success, 1 on any error (unreadable or invalid OBJ, a face pointing at a missing
vertex, missing normals without `--flat-normals` or `--soft-normals`, both of them given, a
failed `--check`). With several inputs, it carries on with the others and fails at the end.

Dropped from the C++ tool: `--precision` (automatic now) and `--order` (axis swap; the asset
scripts already write feather.rs's axes).

Later, if needed: `--scale <f>`, and a crease angle for `--soft-normals`.

### Computed normals

`--flat-normals` and `--soft-normals` can't be used together. Either one replaces any `vn`
normals in the OBJ (with a warning if there were some, as they are thrown away); without
either, an OBJ with faces lacking normals is an error.

- **Face normal**: `normalize((b - a) × (c - a))` for a triangle `a, b, c` in the OBJ's order,
  counter-clockwise seen from the front (the OBJ convention, kept by feather.rs). Computed in
  `f64` from the exact decimal positions.
- **`--flat-normals`**: each corner takes its triangle's normal. Corners of neighbouring faces
  with different normals no longer merge, so the mesh has more vertices (up to three per
  triangle); faces in the same plane still share theirs.
- **`--soft-normals`**: each corner takes the sum of the normals of all the triangles using
  the same position, weighted by the triangle's angle at that corner (so splitting a face into
  more triangles doesn't change the result), then normalized. Corners are matched by position,
  so seams where the OBJ repeats a position for a different UV still get the same normal.
  OBJ smoothing groups are honoured: only faces in the same `s` group are averaged, and faces
  under `s off` (or `s 0`) stay flat. An OBJ without `s` lines is one smoothing group.
- **Degenerate triangles** (zero area) have no normal: they're left out of the averages, and in
  flat mode get `(0, 0, 1)`, with a warning (they draw nothing, so it's never seen).
- **Precision**: computed normals don't come from text, so they're stored with 4 decimals (in
  `i16`), an error under 0.0001, invisible in lighting. `--check` compares them with that
  tolerance.

This would let the asset scripts in `tools/` (e.g. the vehicles importer, which computes flat
normals itself) leave normals to the converter.

The library behind it takes the same settings as a struct, so `feather_build::build_meshes` uses
the defaults without going through the command line.

## Steps

1. **Spec**: write `doc/fmesh-format.md` with everything listed in
   [Format documentation](#format-documentation). Agree on it before coding.
2. **Format crate** `crates/feather-fmesh`: the header, writing and reading `.fmesh` into
   positions, normals, UVs and indices; no dependency on Vulkan, so both the engine and the
   build scripts can use it. Tests: round trip, every encoding, wrong magic, unknown version,
   truncated file, bad checksum.
3. **Converter** `tools/mesh-converter` (library + binary, like `atlas-creator`):
   - its own OBJ parser, to see the numbers as text: `v`, `vn`, `vt` (2 or 3 values), `f` with
     any number of corners (fan triangulation, like `tobj`), `v/t/n`, `v//n`, `v/t`, `v` and
     negative indices; `o` / `g` names; `s` smoothing groups (for `--soft-normals`);
     `usemtl`, `mtllib` and comments skipped quietly;
   - vertices merged with a hash map on their integers;
   - missing UVs as zeros; missing normals an error unless computed;
   - `--flat-normals` and `--soft-normals` as described in [Computed normals](#computed-normals);
   - the command line described in [Command line](#command-line).
   Tests: precision detection (trailing zeros, exponents, overflow to `f32`, `--max-decimals`),
   polygons, index forms, merged vertices, `--split` naming, option parsing; flat normals of a
   cube (24 vertices, axis normals); soft normals of a sphere (pointing away from the centre),
   smoothing groups, degenerate triangles, both options rejected together.
4. **Investigation**: run `--stats` and a load-time benchmark on every test app OBJ, desktop
   and S25; record the table, choose the encodings and the compression default.
5. **Engine**: `MeshBuilderFmeshFile` in `crates/feather`, and the scene file's mesh `file`
   choosing the builder by extension (`.obj` or `.fmesh`). Test: for every test app OBJ, the
   converted `.fmesh` gives the same vertices (bit-for-bit, apart from `-0.0`) and indices as
   `MeshBuilderObjFile`.
6. **Build**: `feather_build::build_meshes`, used by the test app's `build.rs`: converts
   `resources/*.obj` into `.fmesh` in the assets directory (and stops copying the OBJs), so, as
   with atlases, only OBJs are in the repository. The Android build picks them up with the other
   assets; for an Android target, `build_meshes` turns compression off (see
   [Compression](#compression-measured)). Scene files switch to `.fmesh`.
7. **Check and document**: every demo on Windows and on the S25, start-up times before and
   after; read `doc/fmesh-format.md` through against the final code and add the measurements;
   update `doc/scene-format.md`, `missing-features-vs-cpp.md` (item 29), the README and
   the Python asset scripts' notes.

## Decisions

Agreed on 2026-11-12:

- **One mesh per file.** All the OBJ's objects are merged into one mesh, as
  `MeshBuilderObjFile` does; `--split` writes one file per `o` / `g` object, as the C++
  converter did. A file holding several named meshes (e.g. all eight vehicles) would need the
  scene file to name the object too; it may come with a later version of the format.
- **Generated at build time.** `build.rs` converts the OBJs into `.fmesh` files in the assets
  directory, like atlases; only the OBJs are in the repository.
- **The OBJ loader stays.** `MeshBuilderObjFile` remains for quick tests and for apps not using
  the build step; the scene file picks the loader by the file's extension.
