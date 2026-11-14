# Feather UI: plan (done)

> Archived: this note was deleted from `doc/todo` in d414c4f (2026-11-11) when the `feather-ui`
> crate was added, and restored here from git history. Item numbers of the missing features note
> are the ones it had then; its *Later* ideas are in the *UI ideas* of
> [../todo/missing-features-vs-cpp.md](../todo/missing-features-vs-cpp.md).

Covers items 33 and 37–42 of [missing-features-vs-cpp.md](../todo/missing-features-vs-cpp.md): mouse
input and the `featherui` library of the C++ engine
(`guitarprompter/contrib/feather/src/featherui`, about 1,200 lines), plus buttons and labels,
which C++ featherui doesn't have.

Its first user is the test app: a featherui HUD replaces the info overlay scene
(`info_overlay.rs`, `testapp_overlay_scene.json`). It shows the demo's name and frame rate as
today, and adds a previous and a next button on the left and right sides of the window to switch
demos. The HUD belongs to the test app (`TestApp`), not to the demos: it stays when demos are
switched, and no demo knows about it.

## What C++ does

- **`rectangle`**: `left, top, right, bottom` in pixels, Y down, with `check_hit(x, y)`.
- **`object`** (base of everything, `shared_ptr` + virtual methods): a rectangle, a preferred
  `width` / `height` (default 16), `visibility` (`automatic`, `gone`, `invisible`, `visible`),
  `should_layout()` (not `gone`), `is_visible()`, `update(anchor, layout_rc)` (takes
  `layout_rc` as its rectangle unless `gone`, shows/hides its node), and mouse hooks
  `mouse_move`, `mouse_drag`, `mouse_button_down` (returns the object that captures the drag)
  and `mouse_button_up`. It owns a weak pointer to the scene node it built under `anchor`.
- **`layout`**: an object with a vector of children; forwards `mouse_move` to all and
  `mouse_button_down` to the first that takes it.
- **`layout_nswe`**: five slots. West and east take their child's `width`, north and south their
  `height`, between west and east; center gets the rest. `gone` children take no space.
- **`control`**: starts a drag when the button goes down inside it.
- **`distance`**: a control that ignores clicks: an empty spacer.
- **`scrollbar`** (`vertical_scrollbar`, `horisontal_scrollbar`): `min_value`, `max_value`,
  `value` (clamped), `page`, `min_knob_size`; a track and a knob, each a unit plane with a
  material and a highlighted material (hover); knob size `track * page / range`, at most half
  the track, at least `min_knob_size`. A click before / after the knob pages, on the knob starts
  dragging it. Observers (weak `std::function`s) are told when the value changes. With
  `automatic` visibility it hides itself (and takes no space) when `max <= min`.
- **`ui_hud`**: owns a node root, batcher and rendering state; `update(rc)` sets a top-left,
  Y-down orthographic camera of `rc`'s size and lays out its root layout; `render()` draws it
  clearing only depth; `mouse_*` route events, keep the drag object until the button goes up,
  and return whether the UI took the event.

GuitarPrompter's `LibraryView` uses exactly this: an `ui_hud` whose `layout_nswe` has a
`vertical_scrollbar` in `east`, four materials from its material file, an observer that moves the
list, `min/max/page` set from the content height, and the mouse wheel changing `value`. Its own
mouse handling runs only when the hud returns `false`.

### C++ bugs not to port

- `scrollbar::is_visible` returns `should_layout()` for non-automatic visibility, so an
  `invisible` scrollbar is drawn.
- `layout::mouse_button_down` returns the child layout, not the control hit inside it, so a
  control in a nested layout never gets `mouse_drag`.
- Knob position divides by `max - min`, and dragging by `track - knob`: both can be zero when the
  scrollbar is forced visible.
- `control::mouse_button_up` calls `mouse_move` after a drag; the scrollbars override it anyway.

## Design for Rust

### A crate of its own: `crates/feather-ui`

Like C++, the UI is a library on top of the engine (`feather_ui`, depending on `feather`), not
part of it. It only uses public engine API: scenes, nodes, meshes, materials, cameras. The one
engine change it needs is mouse input (step 1).

### Coordinates

UI rectangles are in window pixels, origin top left, Y down: the same as mouse positions from
winit, so hit tests need no conversion. The hud's camera is
`Camera::orthographic(ScreenOrigin::TopLeft, ScreenUnits::Pixels)`, which shows y `-height..0`
(Y up), so a rectangle is drawn at node position `(left, -bottom)` scaled by
`(width, height)`, on a unit XY plane (`MeshBuilderPlane::new((0, 1), (0, 1), (1, 1))`). Z
orders layers like C++ (track 0.2, knob 0.3); the orthographic camera draws greater z on top.

**Scale.** `Hud::set_scale(pixels_per_unit)` lays the UI out in units of that many pixels:
mouse positions are divided by it, and the camera shows `window height / scale` units
(`ScreenUnits::Height`). The default is 1 (window pixels). The test app sets
`window height / 768`, so the HUD scales with the window as the overlay does today (it is laid
out as if the window were 768 pixels high). A display-scaling factor is another choice an app
can make.

### Alignment in a slot

C++ widgets always fill the rectangle their layout gives them, so a scrollbar in `east` is as
tall as the window. A button on the side of the window should keep its own size, centered
vertically. `WidgetBase` gets an alignment per axis, `Align::{Fill, Start, Center, End}`
(default `Fill`, as in C++), plus a margin: the widget takes its preferred size (`width`,
`height`) within the slot at that alignment.

### Widgets live in an arena, addressed by handles

C++ shares `shared_ptr`s between the layout and the app (`m_layout->east = m_scrollbar`, and the
app keeps `m_scrollbar` to set its range). In Rust the hud owns every widget and the app keeps
handles, as it does for nodes, meshes and materials:

```rust
pub struct WidgetId<T: ?Sized = dyn Widget> { index: usize, _type: PhantomData<fn() -> T> }

let bar: WidgetId<Scrollbar> = hud.add(Scrollbar::vertical());
let root: WidgetId<LayoutNswe> = hud.add(LayoutNswe::new());
hud.get_mut(root).set_east(bar);
hud.set_root(root);
hud.get_mut(bar).set_range(0.0, content_height).set_page(view_height);
```

Typed handles let `get` / `get_mut` return `&Scrollbar` (via `Any` downcast) without the app
casting; `WidgetId<T>` converts into the untyped `WidgetId` for layout slots. Storage is the
engine's `ObjDB` or a plain `Vec<Option<Box<dyn Widget>>>`.

Widgets must be `Send + Sync`, so `Hud` stays `Send + Sync` like `Scene`.

### The `Widget` trait

```rust
pub trait Widget: Any + Send + Sync {
    fn base(&self) -> &WidgetBase;          // rect, preferred size, visibility, node
    fn base_mut(&mut self) -> &mut WidgetBase;
    /// Children, for layouts: the hud recurses into them for layout and hit tests.
    fn children(&self) -> Vec<WidgetId> { Vec::new() }
    /// Places the children in `rect` (layouts) and builds or moves this widget's nodes.
    fn layout(&mut self, rect: Rect, cx: &mut LayoutContext) {}
    /// Whether the widget takes space, and whether it is drawn. Defaults follow `Visibility`;
    /// the scrollbar overrides them for `Automatic`.
    fn takes_space(&self) -> bool;
    fn is_visible(&self) -> bool;
    fn mouse_move(&mut self, pos: Point, cx: &mut EventContext) {}
    /// `true` if the widget takes the press and wants the drag that follows.
    fn mouse_down(&mut self, pos: Point, cx: &mut EventContext) -> bool { false }
    fn mouse_drag(&mut self, pos: Point, cx: &mut EventContext) {}
    fn mouse_up(&mut self, pos: Point, cx: &mut EventContext) {}
}
```

`LayoutContext` gives the scene and the widget's anchor node; `EventContext` gives the scene
(for material swaps) and the event queue. Because a layout can't borrow its children while the
hud borrows it, layouts don't call their children directly: `LayoutNswe::layout` returns the
rectangle of each slot (`Vec<(WidgetId, Rect)>`) and the hud lays the children out after it.
Hit testing is done by the hud the same way: it walks down to the deepest visible widget under
the pointer that takes the press (this fixes the nested-layout bug above).

### Events instead of observers

C++ observers are callbacks; in Rust a callback can't reach the app's state while the hud is
borrowed (the same problem scene animations have). So widgets push events and the app reads
them after routing input:

```rust
pub enum UiEvent {
    ValueChanged { widget: WidgetId, value: f64 },
    Clicked { widget: WidgetId },
}

if hud.mouse_down(x, y) { /* the UI took it */ }
for event in hud.take_events() { ... }
```

The app can also just read `hud.get(bar).value()` every frame.

### The hud draws its own scene

`Hud` owns a `Scene` with the top-left pixel camera, and the app draws it like any other scene
(after the scenes it covers: `get_scene_to_render`). Since resources are per scene (item 17),
the widgets' materials live in that scene: the app builds the hud from a scene file holding its
shaders, atlases and materials (`Hud::from_scene(Scene::load(assets, "ui_scene.json")?)`, like
the test app's overlay), then names them: `Scrollbar::vertical().track_material(m)` etc. The
hud adds the camera if the scene has none, and a root node every widget's nodes go under.

`hud.layout(width, height)` lays the widgets out in the window; the app calls it on resize and
after changing widgets. Setters that change layout mark the hud dirty, and `layout` is cheap when
nothing changed, so calling it every frame is fine too.

The `Widget` trait is public, so apps can write their own widgets (the test app's info panel
below is one).

### Labels and buttons (not in C++ featherui)

- **`Label`**: a `Text` (from `text.rs`) in the hud's scene, with a `TextBuilder` for font,
  height and method. Its preferred size is the measured text; `set_text` changes it and marks the
  layout dirty. `TextMethod::NodePerGlyph` for text that changes often, like the frame rate.
- **`Button`**: a background (unit plane) with a material per state, `normal`, `hover` and
  `pressed`, and an optional `Label` on top, centered. Pressing takes the drag; releasing over
  the button pushes `Clicked`, releasing elsewhere doesn't (as desktop buttons do). Hover follows
  `mouse_move`, like the scrollbar's highlight.

## The test app's HUD

`TestHud` (`testapp_hud.rs`) replaces `InfoOverlay`; `testapp_hud_scene.json` replaces
`testapp_overlay_scene.json`, with the same font atlas, text and panel shaders, the `Panel`
material, and button materials (`Button`, `Button hover`, `Button pressed`: the panel shader
with lighter colors). It has no camera and no nodes of its own any more: the hud adds them.

```
outer LayoutNswe
├─ south:  InfoPanel   (align Start, End; margin 12)    demo name + frame rate, bottom left
└─ center: inner LayoutNswe
   ├─ west:   Button "<"  (align Start, Center; margin 12)   previous demo
   └─ east:   Button ">"  (align End, Center; margin 12)     next demo
```

The panel is in the outer layout's `south` so it keeps the bottom left corner (an `NSWE` layout's
`south` lies between `west` and `east`). The inner layout's center is empty: clicks there are not
the hud's, and go to the demo.

- **`InfoPanel`** is a test app widget: the see-through background, the title `Label` and the
  frame rate `Label`, with the sizes and padding of today's overlay. Its preferred size fits the
  title and the widest frame rate, as `fit_panel` does now.
- **Frame rate**: counted in `TestApp::on_update` (called once per frame) instead of a scene
  animation, since the label now belongs to the panel; shown every 0.5 s as today.
- **Buttons**: about 48 × 96 units, `<` and `>` from the overlay font. `Clicked` on them calls
  `show_demo` with the previous / next index, wrapping around, exactly like the arrow keys, which
  stay.
- **Input**: `TestApp::on_mouse_*` gives each event to the hud first, then reads its events; if
  the hud didn't take it, the event goes to the demo through the `Demo` trait's mouse methods
  (no-ops by default; no demo uses the mouse yet).
- **Resize**: `TestApp::on_resized` sets the hud scale (`height / 768`) and lays it out.
- **Drawing**: the hud's scene is drawn after the demo's scenes, where the overlay is today.

## Steps

1. **Mouse input and window size in `FeatherApp`** (item 33). New default methods:
   `on_mouse_moved(x, y)`, `on_mouse_button(button, pressed, x, y)` and
   `on_mouse_wheel(delta_x, delta_y, x, y)` (winit `CursorMoved`, `MouseInput` and
   `MouseWheel`; the app keeps the last cursor position since `MouseInput` has none; line deltas
   and pixel deltas both become lines), and `on_resized(width, height)` (also called once at
   startup), with positions in physical pixels, top left, Y down. Forward them through the test
   app's `Demo` trait with default no-ops.
2. **Crate skeleton**: `crates/feather-ui` with `Rect` (`width`, `height`, `contains`),
   `Visibility`, `WidgetBase`, the `Widget` trait, `WidgetId`, and `Hud` with the arena, its
   scene and camera, `layout` and the mouse routing (drag capture). Tests: layout of a single
   widget, visibility, capture (a drag keeps going to the pressed widget outside its rectangle,
   until release).
3. **`LayoutNswe`, `Spacer` and alignment** (`layout_nswe`, `distance`, `Align`, margins,
   `Hud::set_scale`). Tests: the five rectangles with every slot, with empty and `Gone` slots,
   nested layouts (hit test reaches the inner control), each alignment with a margin, and mouse
   positions divided by the scale.
4. **`Label` and `Button`**. Tests: a label's preferred size follows its text; a button's
   material follows hover and press; `Clicked` on release over the button only.
5. **The test app's HUD** (see above): `TestHud`, `InfoPanel`, the scene file, mouse routing
   in `TestApp`, and removing `InfoOverlay` and its scene. Tests (CPU only, like the overlay's
   today): the hud loads and has a camera; after `on_resized`, the buttons sit at the middle of
   the left and right edges and the panel at the bottom left; a simulated click on `>` shows the
   next demo and on `<` from demo 0 shows the last; a click in the middle doesn't switch and goes
   to the demo; a press on `>` released outside it doesn't switch; the panel shows the demo's
   label after switching.
6. **`Scrollbar`**: one type with `Orientation::{Vertical, Horizontal}` instead of two classes
   (they differ only in the axis). Range, value, page, minimum knob size, track and knob nodes,
   hover materials, paging clicks, knob drag, `ValueChanged` events, automatic hiding, wheel
   (`hud.mouse_wheel` scrolls the scrollbar under the pointer, or a scrollbar the app names).
   Fix the C++ bugs listed above. Tests: knob size and position (including `max == min` and
   `page > range`), clamping, paging, dragging to both ends, events only on change, automatic
   visibility.
7. **Demo 22: a scrolling list**. A long list of text lines in a clipped-looking panel (no
   scissor yet: the list sits under opaque north / south bars), a vertical scrollbar in `east`,
   a horizontal one in `south` for a wide line, a `Spacer` in the south-east corner. The wheel
   scrolls, dragging and paging work, the knobs light up under the pointer, resizing the window
   lays it out again. Test: the demo loads, and simulated mouse input scrolls the list.
8. **Notes**: move items 33 and 37–42 to the covered features in the missing features note,
   keep the "Later" list there, drop this plan.

## Later (not in C++ either)

- **9-patch track and knob** (item 30): rounded knobs that don't stretch their corners.
- **Clipping**: a scissor rectangle per scene or per node, so a scrolled list can be cut at the
  view's edges instead of hidden under other panels.
- **Buttons that fade in**: the test app's side buttons could show only while the pointer is
  near their edge, with a scene animation on their `alpha`.
- **A vertical stack layout**, so panels like `InfoPanel` can be built from labels instead of
  being a widget of their own.
- **Widgets in scene files**: a `"ui"` section describing the layout, like cameras and nodes.
- **Keyboard focus** and **touch** (drag to scroll, as `LibraryView` does by hand).
