# Engine performance improvements (done)

Done items of [../todo/engine-performance-improvements.md](../todo/engine-performance-improvements.md),
from a review of the engine (`crates/feather`) on 2026-11-12. File references are relative to
`crates/feather/src/` and were taken before the changes.

## GPU

### 1. Don't store the multisampled color image

**Problem**: the render pass stores the MSAA color attachment (`store_op(STORE)`,
`pipeline.rs:28`), although only its resolve (the swapchain image) is read afterwards. On
tile-based GPUs (every Android device) this writes the whole multisampled image to memory every
frame.

**Proposal**: `AttachmentStoreOp::DONT_CARE` for attachment 0. The resolve attachment keeps
`STORE`.

**Check**: same image; GPU frame time and memory bandwidth (Android GPU Inspector or Snapdragon
Profiler) on the Galaxy S25 should drop, most visibly in demos that fill the screen.

**Done** (2026-11-14): the MSAA color attachment uses `DONT_CARE` (`create_render_pass`). The
image is unchanged. GPU time per frame (see *Measurements* below) drops by about 2 ms (10%) in
demo 15 on the S25 and by about 0.35 ms (30%) in demo 0 on Windows. Demo 16 on the S25 and the
City demos on Windows vary too much between runs to show a difference. Memory bandwidth wasn't
measured: the S25 offers no GPU counters to Perfetto without Android GPU Inspector.

### 2. Cheaper MSAA

**Problem**: `get_max_msaa_samples` (`physicaldevice.rs:104`) takes the highest sample count the
device offers (up to 64×), and every pipeline turns on sample-rate shading whenever the device
supports it (`pipeline.rs:519-521`, `min_sample_shading(0.2)`), so fragment shaders run on several
samples per pixel.

**Proposal**: cap the sample count at 4× (nearly free on tiled GPUs, the usual quality/cost
balance on desktop) and leave sample shading off by default, with a setting to turn it on.
Possibly let the app choose the sample count.

**Check**: edges look the same to the eye; GPU frame time in demos 15 and 16 on Windows and
Android.

**Done** (2026-11-14): `FeatherApp::render_settings` returns a `RenderSettings`
(`featherapp.rs`): `max_msaa_samples` (default 4; the engine takes the largest count up to it
that the device supports, `choose_msaa_samples`) and `sample_shading` (default off; enabled on
the device only when asked for and supported). The chosen count is logged at start-up. Both
the S25 and the Adreno X1-85 offer at most 4 samples, so the cap changes nothing on them: the
gain is sample shading being off. On the S25 GPU time drops from about 24 to 14 ms in demo 15
(26 to 40 fps) and from about 26 to 22 ms in demo 16 (about 26 to 30 fps); on Windows, demo 0
from 0.83 to 0.69 ms. Edges look the same (screenshots of demo 15 before and after, zoomed
3×); the inside of textured surfaces shows no visible difference at the phone's pixel density.

#### Measurements (2026-11-14)

GPU time per frame from Vulkan timestamp queries at the start and end of each frame's command
buffer (a temporary module, not committed), averaged over 20–26 s after the demo loaded,
release builds. Variants: V0 the engine before items 1–3; V1 V0 with item 1; V2 items 1–3
(sample shading off); V3 V2 with sample shading turned on. Two runs each (V3 one), the second
in reverse order to show drift; the phone tapped to the demo (camera and traffic move, so runs
aren't identical frame by frame).

| GPU ms/frame | V0 | V1 | V3 | V2 |
|---|---|---|---|---|
| S25, demo 15 (2340×1080) | 25.7 / 26.9 | 23.9 / 22.8 | 24.2 | 13.8 / 14.4 |
| S25, demo 16 | 35.4 / 25.0 | 26.9 / 25.0 | 26.1 | 21.1 / 22.2 |
| Windows, demo 0 (1280×960) | 1.23 / 1.33 | 0.96 / 0.84 | 0.83 | 0.68 / 0.69 |
| Windows, demo 15 | 3.64 / 4.34 | 3.15 / 4.35 | 3.34 | 3.89 / 2.54 |
| Windows, demo 16 | 5.73 / 4.91 | 5.44 / 5.65 | 5.06 | 4.15 / 4.85 |

Frame rates on the S25 (`FIFO`, 120 Hz display): demo 15 26 → 40 fps, demo 16 21–28 → 30
fps. On Windows the frame rate follows the display (120 fps, sometimes more) in every variant,
so only GPU time tells them apart. The first S25 demo 16 run of V0 (35.4 ms) is an outlier.

### 3. Lazily allocated memory for the MSAA color and depth images

**Problem**: the multisampled color image is `TRANSIENT_ATTACHMENT` but allocated `DEVICE_LOCAL`
(`colorobjects.rs:28-29`); the depth image is neither transient nor lazily allocated
(`dephobjects.rs:30`). At phone resolutions with MSAA they take tens of MB that tiled GPUs never
need.

**Proposal**: give the depth image `TRANSIENT_ATTACHMENT` too, and allocate both from a
`LAZILY_ALLOCATED` memory type when the device has one, falling back to `DEVICE_LOCAL`. Needs
`get_memory_type_index` to try the preferred flags first.

**Check**: the app's memory use on the S25 (`dumpsys meminfo`, graphics) drops by about the size
of the two images; nothing changes on Windows (desktop GPUs have no lazily allocated memory).

**Done** (2026-11-14): the depth image is `TRANSIENT_ATTACHMENT` too, and `create_image` asks
for `LAZILY_ALLOCATED` memory for transient images, falling back to the requested properties
(`get_memory_type_index_preferring`, `choose_memory_type` in `other.rs`). Each transient image
logs its memory and size when made. Measured on the Galaxy S25 (demo 0, 2340×1080, 4× MSAA:
two images of 41 MB each): `GL mtrack` dropped from 93 MB to 12.7 MB, `Graphics` from 164 MB
to 84 MB and the total PSS from 233 MB to 155 MB. Background and back still draw. Contrary to
the guess above, Windows changes too on this machine: the Adreno X1-85 is a tiled GPU and has
lazily allocated memory, so both images are lazily allocated there as well (image unchanged,
no validation errors).
