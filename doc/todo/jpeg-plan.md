# JPEG images: plan

Goal: **JPEG images wherever feather.rs takes PNG today**: atlases loaded by the engine (scene
files' `bitmap`, `Scene::create_atlas_from_png`, from bytes in memory), and images packed by the
atlas creator (`.atlist` lists). Also what GuitarPrompter needs to port its cover images: decode
a JPEG (or PNG) from memory into RGBA pixels.

Covers item 20 of [missing-features-vs-cpp.md](missing-features-vs-cpp.md) (JPEG loading).

## What the C++ engine does

`feather::jpeg::read_from_memory(bytes, &width, &height, &pixels)` (`feather-jpeg.h`): decodes a
JPEG in memory into 8-bit RGBA pixels, alpha 255. Returns `false` on failure. Two
implementations, chosen at compile time:

- **Windows** (`feather-jpeg.cpp`, when `FEATHER_HAS_LJPEG`): **libjpeg**, found by CMake
  (`find_package(JPEG REQUIRED)`, from vcpkg). Errors go through `setjmp` / `longjmp`, because
  throwing from libjpeg's error callback crashed release builds on broken files. Rows are read
  one scanline at a time and expanded from RGB to RGBA.
- **Android** (`feather-jpeg-android.cpp` → `feather-image-android.cpp`): the NDK's
  **`AImageDecoder`** (API 30+), asked for `RGBA_8888`. The same function decodes PNGs on
  Android (`feather-png-android.cpp`), so Android doesn't link libpng nor libjpeg.

Limits and bugs:
- The libjpeg path assumes 3 components per pixel: a **grayscale JPEG** (1 component) is read
  past its row buffer, giving garbage or a crash. CMYK JPEGs fail the same way.
- Windows and Android decode with different libraries, so pixels can differ slightly between
  platforms (IDCT and upsampling differ).
- The engine itself never uses JPEG: atlases are PNG only (`Atlas::CreateFromPngFile`, and
  materials append `.png` to atlas names). JPEG decoding is only a utility for the app.
- EXIF orientation and ICC colour profiles are ignored (by both paths).

## What GuitarPrompter does with it

Song **cover images**, the only use:

- `CoverImageLoader` loads a cover from a file (`.png`, `.jpg`, `.jpeg`), from a song document
  (stored with its type, `"png"` or `"jpeg"`), or from an MP3's ID3v2 picture frame (by MIME
  type, `image/jpeg` or `image/png`). It checks each file by decoding it, trying the format the
  extension suggests first, then the other one.
- `GenericBitmap::CreateFromJpeg` / `CreateFromPng` decode it to RGBA; the library scales it to
  256 × 256 (`BitmapTools::scaleTo`, GuitarPrompter's own) and makes an atlas from the pixels
  (`Atlas::Create(w, h, RGBA, data)`) to show it.
- The Windows file dialog offers `*.png;*.jpg`.

So a port needs: decoding PNG or JPEG **from memory** to RGBA, telling the formats apart by
content, and an atlas from RGBA pixels (feather.rs already has `Atlas::from_rgba`). Scaling
stays the app's business.

## What feather.rs does today

- The engine decodes PNG only (`atlas.rs`, `read_png_rgba`, with the `png` crate): any colour
  type and bit depth, converted to 8-bit RGBA.
- The atlas creator decodes PNG only, separately (`images.rs`), and only 8-bit RGBA, RGB, grey
  and grey + alpha; it panics on 16-bit and indexed PNGs. It reads each image twice (size,
  then pixels).

## Proposal

### Decoder: `zune-jpeg`, on every platform

A pure-Rust JPEG decoder (MIT / Apache-2.0 / Zlib), maintained, used by the `image` crate. It
handles baseline and progressive JPEGs, grey, YCbCr, CMYK and YCCK, restart markers, and
outputs RGBA directly; SIMD on x86 and ARM (NEON). Nothing to set up for Android, as with
`png` and `miniz_oxide`.

Rather than the C++ split (libjpeg / `AImageDecoder`), the same decoder everywhere gives the
same pixels on Windows and Android, and keeps the Rust code free of platform `cfg`s.
Alternatives considered: `jpeg-decoder` (in maintenance mode, its authors point to
`zune-jpeg`); the whole `image` crate (many formats and dependencies not needed);
`AImageDecoder` on Android (platform code, and pixels differing from Windows).

Not supported (rare; reported as an error): arithmetic-coded and 12-bit JPEGs, JPEG 2000 / XL,
lossless JPEG.

### A shared crate `crates/feather-image`

Decoding moves into a small crate with no Vulkan, like `feather-fmesh`, used by the engine and
by the atlas creator:

```rust
pub enum ImageFormat { Png, Jpeg }
pub fn detect(bytes: &[u8]) -> Option<ImageFormat>;          // by the magic bytes
pub fn decode_rgba(bytes: &[u8]) -> Result<Rgba, ImageError>; // PNG or JPEG
pub struct Rgba { pub width: u32, pub height: u32, pub pixels: Vec<u8> }
```

- **Format by content, not by extension**: PNG starts with `89 50 4E 47 0D 0A 1A 0A`, JPEG with
  `FF D8 FF`. A `.jpg` that's really a PNG still loads (GuitarPrompter meets such covers). An
  unknown format is an error naming both.
- PNG decoding is the engine's current code, moved: so the atlas creator also gains 16-bit and
  indexed PNGs instead of panicking.
- JPEG: grey becomes R = G = B, every pixel gets alpha 255.
- **Limits**: images over 16384 × 16384, or whose RGBA pixels would take over 1 GiB, are
  rejected before decoding (a damaged header can't make it allocate without limit).
- **EXIF orientation is ignored**, as in C++: pixels come as stored. Applying it is out of
  scope (see [Decisions](#decisions)).

### Engine

- `Atlas::new_from_image(assets, bitmap, metadata, name)` and
  `Atlas::new_from_image_bytes(bytes, name)`, `Scene::create_atlas_from_image(...)`: PNG or
  JPEG. The `..._png` functions stay as they are named today but call the same code (so they
  accept JPEG too), marked deprecated in their documentation; callers in the workspace switch
  to the new names. (See open questions.)
- Scene files: an atlas's `bitmap` may be a PNG or a JPEG. Nothing else changes; `metadata`
  works the same.
- Errors name the asset and the format found: "Cannot read image asset x.jpg (JPEG): ...".

### Atlas creator

- `.atlist` lines may name `.jpg` / `.jpeg` (or any PNG / JPEG, detected by content). Texture
  names are the file stem, as now (`wall.jpg` → `wall`), so `wall.png` and `wall.jpg` in one list
  is an error (same name twice).
- Each image is decoded once (size and pixels together); the output atlas stays a PNG
  (lossless, with alpha). JPEG images are opaque in it.
- No more panics: an unreadable image is an error naming the file.
- Font atlases don't change (the font creator makes PNGs).

### Test app

- Demo 0's room texture (`demo_000_viking_room_atlas.png`, 1.0 MB, a photo-like texture)
  becomes a JPEG (quality 90), so a demo shows a JPEG atlas from a scene file; and a JPEG
  image in one `.atlist` (demo 2's plates) shows the atlas creator path. Before and after sizes
  and load times recorded (Windows and S25).

## Steps

1. **`crates/feather-image`**: `detect`, `decode_rgba` (PNG moved from `atlas.rs`, JPEG with
   `zune-jpeg`), limits, errors. Tests on files in `testdata/` made with Pillow (baseline,
   progressive, grey, CMYK, 4:2:0 and 4:4:4 subsampling, restart markers, a truncated file, a
   PNG named `.jpg`), comparing pixels with Pillow's decoding (a difference of a few levels is
   allowed: IDCTs differ).
2. **Engine**: `Atlas::new_from_image*`, `Scene::create_atlas_from_image`, the `..._png` names
   calling them, scene files' `bitmap` accepting JPEG. Tests: a JPEG atlas from an asset, from
   bytes, from a scene file, with metadata.
3. **Atlas creator**: decode through `feather-image`, once per image; JPEG in `.atlist`; errors
   instead of panics. Tests: a list mixing PNG and JPEG, 16-bit and indexed PNGs, a name clash,
   an unreadable image.
4. **Test app**: demo 0's texture as JPEG, a JPEG in demo 2's plates; every demo checked on
   Windows and on the S25; sizes and decode times.
5. **Docs**: README (features, assets), `doc/scene-format.md` (`bitmap`), the atlas creator's
   help, `missing-features-vs-cpp.md` (item 20 removed, items renumbered), `doc/android.md`
   (APK contents) if the sizes change.

## Decisions

Agreed on 2026-11-12:

- **Names**: `Atlas::new_from_image`, `Atlas::new_from_image_bytes` and
  `Scene::create_atlas_from_image` are added; the `_png` names stay as deprecated aliases
  (accepting JPEG too), and the workspace's callers switch to the new names.
- **Demo assets**: demo 0's room texture becomes a JPEG, and demo 2's atlas gets a JPEG image.
- **EXIF orientation**: out of scope. Pixels come as stored; applying the orientation may be a
  later feature.
