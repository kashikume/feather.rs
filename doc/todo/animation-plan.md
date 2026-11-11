# Animation: plan

Covers items 1–4 of [missing-features-vs-cpp.md](missing-features-vs-cpp.md): `Animator`,
`SlideAnimation`, `PerpetualAnimation` and the easing functions of the C++ engine
(`guitarprompter/contrib/feather/src/feather/feather-Animation*`, `feather-Utils.h`).

**Done**: steps 1–6 below (`easing.rs`, `animation.rs`, `animator.rs`, scene animations in
`scene.rs`, the call in `App::prepare_scenes`, demo 21). Where the code differs from this plan:

- The trait's target getter is `Animation::get_target`, not `target`: `target(node)` is the
  builder method of `SlideAnimation` and `PerpetualAnimation`, and the same name hid the trait
  method. The trait also has `set_elapsed` (default: ignored), used by
  `Animator::set_elapsed` / `Scene::set_animation_elapsed`.
- `Animator::add` takes an optional name, like `Scene::add_animation`; `Animator::find` looks it
  up. The scene also has `animation_count` and `cancel_all_animations`.
- `Easing::apply` gives exactly 0 and 1 at the ends (and clamps outside 0..1), so slides end
  exactly in place whatever the function's rounding. C++'s `easeInOut3` is the same function as
  `cubic_ease_in_out`, so it has no variant of its own.
- Bounce easings stay within 0..1 but aren't monotonic; the tests check only the range for them.
- Back easings overshoot by about 35% of the slide (C++ formula); demo 21 leaves room for it.

## What C++ does

- `Animator::add(animation)` stores the animation and calls `start()`, which zeroes its time.
  `Animator::update(dt)` drops the animations finished in an earlier update and calls
  `update(dt)` on the rest.
- `SlideAnimation(duration, step, finish)` adds `dt` to its time, calls `step(self, progress)`
  with `progress = time / duration` clamped to 1, and once it reaches 1 calls `finish()` and
  marks itself finished. `finish()` can also be called early, through the pointer `step` receives.
- `PerpetualAnimation(step)` calls `step(self, dt, time)` every update and never finishes on its
  own; `resetCurrentTime` changes its time.
- Easing functions are free functions `f(p) -> p'` on 0..1, applied by the caller inside `step`.

GuitarPrompter uses it like this: slide a node in or out over 200 ms while fading an `alpha`
uniform, then remove the node in the finish callback. The closures capture `shared_ptr`s to the
nodes they change. It also has a separate animator for the camera (`m_cameraAnimator`).

## Design for Rust

Two places hold animations:

- **The scene** (the usual case): `Scene` keeps its animations in an `ObjDB`, addressed by
  handles like nodes, meshes and materials. Callbacks get `&mut Scene`. The engine drives them.
- **A standalone `Animator<C>`**: for what lives outside a scene, e.g. cameras (cameras are not
  part of `Scene`; `FeatherApp::get_camera_to_render_scene` returns them separately). The app owns
  and drives it.

Both run the same animation types, which are generic over the context they change.

### The `Animation` trait

```rust
pub trait Animation<C>: Send + Sync {
    /// Advances by `dt` seconds. Returns `true` when finished.
    fn update(&mut self, ctx: &mut C, dt: f32) -> bool;
    /// Called when the animation is finished early: jump to the end and run the
    /// end-of-animation callback.
    fn finish(&mut self, ctx: &mut C);
    /// Sets the time elapsed since the animation started. The default ignores it.
    fn set_elapsed(&mut self, _time: f32) {}
    /// Node this animation changes, if any. Scene animations are cancelled when it is removed.
    fn get_target(&self) -> Option<usize> { None }
}
```

Apps can implement it for their own animations; the two below cover the C++ ones.

**Thread safety.** `Scene` is `Send + Sync` today (no `Rc`, `RefCell` or raw pointers), so it
can be built or updated on another thread and shared read-only. Animations must keep that, hence
the `Send + Sync` bound: `SlideAnimation` and `PerpetualAnimation` require their closures to be
`Send + Sync + 'static`, and boxes are stored as `Box<dyn Animation<C>>` (the bound comes from the
trait). Closures that capture node handles, numbers or `Arc`s satisfy it; ones capturing `Rc` or
`RefCell` don't compile, and should use `Arc` / `Mutex` (or atomics) instead. The same bound
makes `Animator<C>` `Send + Sync` whenever it holds only animations. This gives the usual Rust
thread safety: one thread changes the scene at a time (`&mut Scene`); there is no internal
locking like C++'s `FEATHER_THREADSAFE`.

### `SlideAnimation`

```rust
SlideAnimation::new(duration, |scene: &mut Scene, progress: f32| { ... })
    .easing(Easing::CubicInOut)                 // optional, default Linear
    .on_finish(|scene: &mut Scene| { ... })     // optional
    .target(node)                               // optional
```

- `progress` is `easing(min(time / duration, 1))`; the last step always gets exactly 1 (before
  easing), then `on_finish` runs and the animation is removed in the same update.
- `duration <= 0` finishes on the first update with progress 1 (C++ divides by zero).
- Finishing early calls the step with 1 and then `on_finish`, so the target always ends in its
  final state. C++ only calls the finish callback; the Rust behaviour avoids nodes stuck halfway.

### `PerpetualAnimation`

```rust
PerpetualAnimation::new(|scene: &mut Scene, dt: f32, elapsed: f32| -> ControlFlow<()> { ... })
    .target(node)                               // optional
```

- Returns `ControlFlow::Break(())` to stop itself, replacing C++'s `finish()` through the pointer
  passed to the step.
- `set_elapsed(time)` (through the scene or animator, by handle) replaces `resetCurrentTime`.

### Easing

`easing.rs` holds the C++ functions as `pub fn` (`quadratic_ease_in`, ..., `bounce_ease_in_out`,
`ease_in_out3`) and an `Easing` enum with one variant per function plus `Linear`, and
`Easing::apply(p)`. The enum lets `SlideAnimation` and scene files name an easing; the functions
stay usable directly.

### Animations in the scene

```rust
impl Scene {
    pub fn add_animation(&mut self, name: Option<String>, animation: impl Animation<Scene> + 'static) -> usize;
    pub fn find_animation(&self, name: &str) -> Option<usize>;
    pub fn is_animation_running(&self, handle: usize) -> bool;
    pub fn finish_animation(&mut self, handle: usize);     // run to the end now
    pub fn cancel_animation(&mut self, handle: usize);     // drop, no callbacks
    pub fn set_animation_elapsed(&mut self, handle: usize, time: f32);
    pub fn update_animations(&mut self, dt: f32);
    pub fn advance_animations_to(&mut self, time: f32);    // keeps the last time, calls update
}
```

Storage: `animations: ObjDB<SceneAnimation>`, where `SceneAnimation` has an optional name (for
`ObjDB` name lookup), a serial number unique within the scene, and
`animation: Option<Box<dyn Animation<Scene>>>`.

**Updating while callbacks hold `&mut Scene`.** The scene can't lend an animation and itself at
the same time, so `update_animations`:

1. collects the handles of the running animations;
2. for each handle, takes the `Box` out of its slot (`Option::take`), leaving the slot occupied so
   `ObjDB` can't hand the handle to another animation, calls `update(self, dt)`, then:
   - finished: removes the slot;
   - still running: puts the `Box` back, but only if the slot still has the same serial number
     (a callback may have cancelled this animation and a new one may have reused the handle; then
     the taken animation is dropped).

Consequences:

- **Callbacks can add animations** with `scene.add_animation(...)`, so sequences need no special
  support (start the next slide in `on_finish`). Animations added during an update start in the
  next one.
- Callbacks can finish or cancel other animations. Finishing an animation whose `Box` is taken
  out (the one running) is ignored; it can stop itself instead (`ControlFlow::Break`, or the
  slide simply reaching 1).

**Node removal.** `remove_node` cancels, without callbacks, every animation whose `get_target()` is
one of the removed nodes. Without a target, callbacks must cope with missing nodes themselves
(the scene's methods return `Option`/`Result`).

**Who drives them.** `App::prepare_scenes` calls `scene.advance_animations_to(time)` before
`update_nodes`, with the same time it passed to `on_update`, so animated changes are rendered in
the same frame. Apps that want animations applied before their own `on_update` logic can call
`advance_animations_to` themselves; the engine's call then advances by 0. A scene that was not
rendered for a while gets one large step when it comes back (its slides finish).

**Handles are reused** by `ObjDB` after removal, as for nodes: a stale handle can refer to a newer
animation. Same rule as for other handles: forget a handle once its animation is finished or
cancelled.

### Standalone `Animator<C>`

```rust
impl<C> Animator<C> {
    pub fn new() -> Self;
    pub fn add(&mut self, name: Option<String>, animation: impl Animation<C> + 'static) -> usize;
    pub fn find(&self, name: &str) -> Option<usize>;
    pub fn update(&mut self, ctx: &mut C, dt: f32);
    pub fn advance_to(&mut self, ctx: &mut C, time: f32);
    pub fn finish(&mut self, ctx: &mut C, handle: usize);
    pub fn cancel(&mut self, handle: usize);
    pub fn set_elapsed(&mut self, handle: usize, time: f32);
    pub fn is_running(&self, handle: usize) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn clear(&mut self);
}
```

The app keeps it next to what it animates, so the borrows don't overlap:
`self.camera_animator.update(&mut self.camera, dt)`. Its callbacks get the context, not the
animator, so they can't add animations to it; this is the price of a generic context and fine for
the few non-scene uses.

### Changes from C++ behaviour

- Finished animations are removed in the update that finishes them, not one update later.
- Finishing early also applies the final step (see `SlideAnimation`).
- Animations whose target node is removed are cancelled automatically.
- Animations are `Send + Sync`, so `Scene` and `Animator` stay `Send + Sync`; no internal
  locking (see "Thread safety").

## Steps

1. **Easing** (item 4): `crates/feather/src/easing.rs`, the functions and the `Easing` enum.
   Tests: every function gives 0 at 0 and 1 at 1; in-out variants give 0.5 at 0.5; out variants
   mirror in variants (`out(p) == 1 - in(1 - p)`) where C++ defines them that way;
   non-overshooting ones (all but elastic and back) stay in 0..1 and never decrease.
2. **Animation trait, `SlideAnimation`, `PerpetualAnimation`** (items 2, 3):
   `crates/feather/src/animation.rs`. Tests with a plain struct as the context: progress values
   for given `dt`s, clamping and the exact final 1, easing applied, zero duration, `on_finish`
   runs once, finishing early, perpetual `elapsed` sums `dt`s and stops on `Break`.
3. **Standalone `Animator<C>`** (item 1): `crates/feather/src/animator.rs`. Tests: `update`
   drives every animation and removes finished ones in the same update, `finish` jumps to the end
   and runs `on_finish`, `cancel` runs nothing, `is_running` after each, `advance_to` gives the
   right deltas, `set_elapsed`.
4. **Scene animations**: storage and methods in `scene.rs`, cancellation in `remove_node`, the
   call in `App::prepare_scenes`. Tests: a slide moves a node and finishes; a callback adds an
   animation (runs from the next update); a callback cancels itself or another animation;
   cancel-then-reuse of a handle during an update keeps the new animation; removing a target node
   (or its ancestor) cancels its animations without callbacks; `find_animation` by name;
   `advance_animations_to` twice with the same time advances by 0; compile-time checks that
   `Scene` and `Animator<Scene>` are `Send + Sync` (`fn assert_send_sync<T: Send + Sync>() {}`);
   a scene with running animations moved to another thread, updated there and sent back.
5. **Demo** `demo_021_animation` in the test app: a column of cubes, each sliding across the
   screen with a different easing and labelled with its name; a perpetual animation spinning a
   cube; a slide that fades a node's `alpha` and removes it in `on_finish`, then starts the next
   one from there; a standalone animator moving the camera; a key to restart all slides.
6. **Docs**: module docs with a short example; mark items 1–4 done in
   [missing-features-vs-cpp.md](missing-features-vs-cpp.md).

## Later (not needed for parity)

- Replace-by-name: `add_animation` with a name already running cancels the old one, so two slides
  don't fight over one node (GuitarPrompter avoids this by hand with separate animators).
- Generation counters in handles, so stale handles are detected (would apply to every `ObjDB`).
- Animations declared in scene files (target node, uniform or transform, duration, easing).
- Reaching app state from scene animations: their callbacks only get `&mut Scene`, so anything
  else must be moved into the closure (like the `Text`s of demos 11 and 12) or handled by a
  standalone `Animator` with its own context. Demo 11's episode spawning stays in `update`
  because it needs the demo's random generator and counter. If this comes up often, consider
  storing `Text`s in the scene, or letting scenes hold user data.
