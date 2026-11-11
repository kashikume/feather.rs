# Demos that could use animations

Review of demos 0–20 after animations were added (see [animation-plan.md](animation-plan.md)).
Item 1 is done; the rest is not.

## Worth converting

### 1. Frame-rate counter (demos 12, 15, 16)

**Done, differently**: the test app shows the frame rate (and the demo's number and name) over
every demo in its own overlay scene (`info_overlay.rs`), counted by a `PerpetualAnimation` that
owns a per-glyph `Text`. Demos 12, 15 and 16 lost their counters; 15 and 16 lost their overlay
scenes and assets.

The same code is copied into three demos: `fps_start` and `frames` fields and a half-second check
in `update` (`demo_012_overlay.rs`, `demo_015_city.rs`, `demo_016_city.rs`).

**Proposal**: one shared helper, `fps_counter(texts: Vec<Text>) -> PerpetualAnimation<Scene>`,
added to the overlay scene. Its step runs once per frame, so it counts frames and sums `dt`
itself, and calls `set_text` every half second. It owns the `Text`s: they hold only handles,
numbers and strings, so they are `Send + Sync` and can be moved into the closure.

**Gain**: two fields and about 10 lines fewer in each demo, one helper instead of three copies,
and animations shown running on a second (overlay) scene.

### 2. Crawl episodes (demo 11)

Each episode moves at a constant speed until it has faded out, then is removed with its texts.
The demo does this by hand: a `VecDeque<Episode>` with `start_z` / `start_time` per episode, a
removal loop and a loop moving every episode (`demo_011_crawl.rs`, `update`).

**Proposal**: each new episode gets a linear `SlideAnimation` targeting its node, lasting
(distance to `REMOVE_Z`) / `SPEED`, whose `on_finish` removes the texts (moved into the closure)
and the node. `update` keeps only the spawning, which needs the demo's `Random`, episode counter
and font; it reads the last episode's position from its node to know when to spawn.

**Gain**: moderate. The move and remove bookkeeping goes; the episode lifecycle reads clearly.

### 3. Overlay intro (demo 12, optional)

New behaviour rather than a simplification: the overlay's panels slide and fade in at the start,
with easing. This is how GuitarPrompter's HUD uses the C++ animations, and no demo shows
animations on a 2D overlay yet.

## Not worth converting

- **Positions computed from `time`**: demos 0, 1, 4, 5, 7, 14, 18, the turntable, colour and wave
  of 10, and the walking paths and cameras of 8, 9, 15, 16, 19 and 20. Each sets a transform
  straight from `time`; a `PerpetualAnimation` would do the same less directly. Their tests call
  `demo.update(t)` and check positions, which would break if the engine drove the movement.
- **Simulations**: the bouncing balls (6, 17) and the traffic (15, 16) keep state and use fixed
  sub-steps; animations aren't meant for that.
- **Demo 10's clock**: changes from `time` ten times a second; rebuilding the string in
  `update` is already as simple as it gets.

## Gap found

Scene animation callbacks only get `&mut Scene`. Anything else must be moved into the closure
(like the `Text`s above) or handled by a standalone `Animator` with its own context. Demo 11's
spawning shows the limit: it needs app state (random generator, counter) that isn't in the
scene. If this comes up often, consider storing `Text`s in the scene, or letting scenes hold user
data.
