//! The hud: a scene of its own holding a tree of widgets, laid out in the window and fed the
//! mouse.

use std::any::Any;
use std::collections::HashSet;

use anyhow::Result;
use feather::camera::{Camera, ScreenOrigin, ScreenUnits};
use feather::meshbuilderplane::MeshBuilderPlane;
use feather::scene::Scene;

use crate::rect::Rect;
use crate::widget::{position_transform, EventContext, LayoutContext, UiEvent, Widget, WidgetId};

/// A widget in the store, with its anchor node. `widget` is `None` only while the hud has
/// taken it out to call it.
struct Slot {
    widget: Option<Box<dyn Widget>>,
    node: usize,
}

/// The hud's widgets, by handle.
#[derive(Default)]
pub(crate) struct Store {
    slots: Vec<Option<Slot>>,
    free: Vec<usize>,
}

impl Store {
    fn add(&mut self, widget: Box<dyn Widget>, node: usize) -> usize {
        let slot = Some(Slot {
            widget: Some(widget),
            node,
        });
        match self.free.pop() {
            Some(index) => {
                self.slots[index] = slot;
                index
            }
            None => {
                self.slots.push(slot);
                self.slots.len() - 1
            }
        }
    }

    fn remove(&mut self, id: WidgetId) -> Option<Slot> {
        let slot = self.slots.get_mut(id.index())?.take()?;
        self.free.push(id.index());
        Some(slot)
    }

    pub(crate) fn get(&self, id: WidgetId) -> Option<&dyn Widget> {
        self.slots.get(id.index())?.as_ref()?.widget.as_deref()
    }

    fn get_mut(&mut self, id: WidgetId) -> Option<&mut dyn Widget> {
        self.slots.get_mut(id.index())?.as_mut()?.widget.as_deref_mut()
    }

    fn node(&self, id: WidgetId) -> Option<usize> {
        Some(self.slots.get(id.index())?.as_ref()?.node)
    }

    /// Takes the widget out, with its node, so it can be called while the others are read.
    fn take(&mut self, id: WidgetId) -> Option<(Box<dyn Widget>, usize)> {
        let slot = self.slots.get_mut(id.index())?.as_mut()?;
        Some((slot.widget.take()?, slot.node))
    }

    fn put_back(&mut self, id: WidgetId, widget: Box<dyn Widget>) {
        if let Some(Some(slot)) = self.slots.get_mut(id.index()) {
            slot.widget = Some(widget);
        }
    }

    fn nodes(&self) -> impl Iterator<Item = usize> + '_ {
        self.slots.iter().flatten().map(|slot| slot.node)
    }
}

/// A UI drawn over the app's scenes, in a scene of its own (`scene`), which the app draws
/// after them (`FeatherApp::get_scene_to_render`).
///
/// The hud owns its widgets; the app keeps handles to them (`add`, `get`, `get_mut`). One of
/// them is the root (`set_root`), usually a layout, laid out over the whole window; the
/// widgets it places are laid out in turn. Widgets out of the root's tree are not drawn.
///
/// The hud's scene gets a camera of its own, made active, showing the window with the origin
/// at the top left corner, and a root node holding a node per widget. The app puts in it what
/// the widgets draw with: shaders, atlases, materials (e.g. by loading a scene file).
///
/// The app tells the hud the window's size (`set_window_size`) and gives it the mouse
/// (`mouse_move`, `mouse_down`, `mouse_up`, `mouse_wheel`, `mouse_left`, in window pixels);
/// those return whether the UI took the event, so the app knows whether to handle it too. Then
/// it reads what happened (`take_events`). It calls `update` once per frame, before drawing,
/// to lay the widgets out again if anything changed.
///
/// UI units are window pixels divided by the scale (`set_scale`).
pub struct Hud {
    scene: Scene,
    root_node: usize,
    camera: usize,
    square: usize,
    store: Store,
    root: Option<WidgetId>,
    scale: f32,
    window: (u32, u32),
    dirty: bool,
    capture: Option<WidgetId>,
    events: Vec<UiEvent>,
}

impl Hud {
    /// A hud drawing in `scene`, with no widgets yet. Adds to the scene a root node
    /// (`UI root`), the square mesh widgets draw rectangles with (`UI square`) and the camera
    /// (`UI camera`), which becomes the scene's active camera.
    pub fn new(mut scene: Scene) -> Result<Self> {
        let root_node = scene.create_root_node(Some("UI root".to_string()));
        let square = MeshBuilderPlane::new((0.0, 1.0), (0.0, 1.0), (1, 1))
            .build(&mut scene, Some("UI square".to_string()))?;
        let camera = scene.add_camera(
            Camera::orthographic(ScreenOrigin::TopLeft, ScreenUnits::Pixels).with_name("UI camera"),
        );
        scene.set_active_camera(camera)?;
        Ok(Self {
            scene,
            root_node,
            camera,
            square,
            store: Store::default(),
            root: None,
            scale: 1.0,
            window: (0, 0),
            dirty: true,
            capture: None,
            events: Vec::new(),
        })
    }

    /// The scene the hud draws in, for the app to draw (`FeatherApp::get_scene_to_render`) and
    /// to add resources to.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    /// The node every widget's node is under.
    pub fn root_node(&self) -> usize {
        self.root_node
    }

    /// Adds a widget, not shown until it is in the root's tree. Returns its handle.
    pub fn add<T: Widget>(&mut self, widget: T) -> Result<WidgetId<T>> {
        let node = self.scene.create_node(None, self.root_node)?;
        self.scene.get_node_mut(node).unwrap().set_visible(false);
        self.dirty = true;
        Ok(WidgetId::new(self.store.add(Box::new(widget), node)))
    }

    /// Removes a widget and its nodes. Widgets it placed (a layout's children) stay, out of the
    /// tree. Handles to it no longer work.
    pub fn remove(&mut self, id: impl Into<WidgetId>) -> Result<()> {
        let id = id.into();
        let Some(slot) = self.store.remove(id) else {
            return Ok(());
        };
        if self.capture == Some(id) {
            self.capture = None;
        }
        if self.root == Some(id) {
            self.root = None;
        }
        self.dirty = true;
        self.scene.remove_node(slot.node)
    }

    /// The widget `id`, if it is still in the hud.
    pub fn get<T: Widget>(&self, id: WidgetId<T>) -> Option<&T> {
        let widget: &dyn Any = self.store.get(id.untyped())?;
        widget.downcast_ref()
    }

    /// The widget `id`, to change it. The hud lays the widgets out again at the next `update`.
    pub fn get_mut<T: Widget>(&mut self, id: WidgetId<T>) -> Option<&mut T> {
        self.dirty = true;
        let widget: &mut dyn Any = self.store.get_mut(id.untyped())?;
        widget.downcast_mut()
    }

    /// Any widget, whatever its type.
    pub fn widget(&self, id: impl Into<WidgetId>) -> Option<&dyn Widget> {
        self.store.get(id.into())
    }

    /// The node of widget `id`: at the top left corner of its rectangle, hidden when the widget
    /// isn't drawn.
    pub fn node(&self, id: impl Into<WidgetId>) -> Option<usize> {
        self.store.node(id.into())
    }

    /// The widget laid out over the whole window (`None`: nothing is shown).
    pub fn set_root(&mut self, root: Option<WidgetId>) {
        self.root = root;
        self.dirty = true;
    }

    pub fn root(&self) -> Option<WidgetId> {
        self.root
    }

    /// How many window pixels a UI unit is (default 1). E.g. `height / 768` lays the UI out
    /// as if the window were 768 pixels high, so it grows with the window.
    pub fn set_scale(&mut self, scale: f32) {
        if scale > 0.0 && scale != self.scale {
            self.scale = scale;
            self.dirty = true;
        }
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The size of the window's drawing area, in pixels.
    pub fn set_window_size(&mut self, width: u32, height: u32) {
        if (width, height) != self.window {
            self.window = (width, height);
            self.dirty = true;
        }
    }

    pub fn window_size(&self) -> (u32, u32) {
        self.window
    }

    /// The window's size in UI units.
    pub fn size(&self) -> (f32, f32) {
        (
            self.window.0 as f32 / self.scale,
            self.window.1 as f32 / self.scale,
        )
    }

    /// Lays the widgets out, if anything changed since the last time.
    pub fn update(&mut self) -> Result<()> {
        if self.dirty {
            self.layout()?;
        }
        Ok(())
    }

    /// What happened to the widgets since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.events)
    }

    /// The pointer moved to `(x, y)` (window pixels). Returns whether a widget has the press
    /// (it gets the move as a drag).
    pub fn mouse_move(&mut self, x: f32, y: f32) -> Result<bool> {
        self.update()?;
        let (x, y) = self.to_ui(x, y);
        if let Some(captured) = self.capture {
            self.with_event(captured, |w, cx| w.mouse_drag(x, y, cx))?;
            return Ok(true);
        }
        for id in self.visible_widgets() {
            self.with_event(id, |w, cx| w.mouse_move(x, y, cx))?;
        }
        Ok(false)
    }

    /// The pointer left the window: nothing is under it any more.
    pub fn mouse_left(&mut self) -> Result<()> {
        if self.capture.is_none() {
            for id in self.visible_widgets() {
                self.with_event(id, |w, cx| w.mouse_move(f32::INFINITY, f32::INFINITY, cx))?;
            }
        }
        Ok(())
    }

    /// A button was pressed at `(x, y)` (window pixels). Returns whether a widget took it; that
    /// widget then gets the moves and the release (`mouse_up`) whatever happens in between.
    pub fn mouse_down(&mut self, x: f32, y: f32) -> Result<bool> {
        self.update()?;
        if self.capture.is_some() {
            return Ok(true);
        }
        let (x, y) = self.to_ui(x, y);
        self.capture = self.hit(x, y, &mut |w, cx| w.mouse_down(x, y, cx))?;
        Ok(self.capture.is_some())
    }

    /// The button was released at `(x, y)` (window pixels). Returns whether a widget had the
    /// press.
    pub fn mouse_up(&mut self, x: f32, y: f32) -> Result<bool> {
        self.update()?;
        let (x, y) = self.to_ui(x, y);
        let Some(captured) = self.capture.take() else {
            return Ok(false);
        };
        self.with_event(captured, |w, cx| w.mouse_up(x, y, cx))?;
        Ok(true)
    }

    /// The wheel turned with the pointer at `(x, y)` (window pixels); see
    /// `FeatherApp::on_mouse_wheel` for the deltas. Returns whether a widget took it: the
    /// innermost one under the pointer that wants it.
    pub fn mouse_wheel(&mut self, delta_x: f32, delta_y: f32, x: f32, y: f32) -> Result<bool> {
        self.update()?;
        let (x, y) = self.to_ui(x, y);
        let hit = self.hit(x, y, &mut |w, cx| w.mouse_wheel(delta_x, delta_y, x, y, cx))?;
        Ok(hit.is_some())
    }

    /// Whether a widget has the press (between a `mouse_down` it took and the `mouse_up`).
    pub fn has_capture(&self) -> bool {
        self.capture.is_some()
    }

    fn to_ui(&self, x: f32, y: f32) -> (f32, f32) {
        (x / self.scale, y / self.scale)
    }

    /// Lays the root's tree out over the window: measures it (children first), then
    /// arranges it (parents first). Hides the nodes of widgets out of the tree.
    fn layout(&mut self) -> Result<()> {
        self.dirty = false;
        if self.window.1 > 0 {
            let height = self.window.1 as f32 / self.scale;
            if let Some(camera) = self.scene.get_camera_mut(self.camera) {
                camera.set_units(ScreenUnits::Height(height));
            }
        }
        let nodes: Vec<usize> = self.store.nodes().collect();
        for node in nodes {
            if let Some(node) = self.scene.get_node_mut(node) {
                node.set_visible(false);
            }
        }
        let Some(root) = self.root else {
            return Ok(());
        };
        for id in self.tree(root).into_iter().rev() {
            self.with_layout(id, |w, cx| w.measure(cx))?;
        }
        let (width, height) = self.size();
        let space = Rect::new(0.0, 0.0, width, height);
        self.arrange(root, space, true, &mut HashSet::new())
    }

    /// Gives widget `id` its place in `space`, then its children theirs.
    fn arrange(
        &mut self,
        id: WidgetId,
        space: Rect,
        parent_visible: bool,
        seen: &mut HashSet<WidgetId>,
    ) -> Result<()> {
        if !seen.insert(id) {
            log::warn!("Widget {id:?} is placed twice in the hud's tree; only the first counts");
            return Ok(());
        }
        let Some(node) = self.store.node(id) else {
            return Ok(());
        };
        let Some(widget) = self.store.get_mut(id) else {
            return Ok(());
        };
        if !widget.takes_space() {
            return Ok(());
        }
        let visible = parent_visible && widget.is_visible();
        let rect = widget.base().place(space);
        widget.base_mut().set_rect(rect);
        if let Some(node) = self.scene.get_node_mut(node) {
            node.set_transform(position_transform(rect.left, rect.top, 0.0));
            node.set_visible(visible);
        }
        let children = self.with_layout(id, |w, cx| w.arrange(cx))?;
        for (child, space) in children.unwrap_or_default() {
            self.arrange(child, space, visible, seen)?;
        }
        Ok(())
    }

    /// The root's tree, parents before children (each child after its earlier siblings'
    /// trees).
    fn tree(&self, root: WidgetId) -> Vec<WidgetId> {
        self.walk(root, |_| true)
    }

    /// The widgets drawn, in tree order.
    fn visible_widgets(&self) -> Vec<WidgetId> {
        match self.root {
            Some(root) => self.walk(root, |w| w.takes_space() && w.is_visible()),
            None => Vec::new(),
        }
    }

    /// The tree under `root` in order, without the subtrees of widgets `keep` rejects.
    fn walk(&self, root: WidgetId, keep: impl Fn(&dyn Widget) -> bool) -> Vec<WidgetId> {
        let mut order = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(widget) = self.store.get(id) else {
                continue;
            };
            if !keep(widget) {
                continue;
            }
            order.push(id);
            stack.extend(widget.children().into_iter().rev());
        }
        order
    }

    /// The innermost visible widget under `(x, y)` (UI units) for which `f` returns `true`,
    /// trying children before their parent, in order.
    fn hit(
        &mut self,
        x: f32,
        y: f32,
        f: &mut dyn FnMut(&mut dyn Widget, &mut EventContext) -> Result<bool>,
    ) -> Result<Option<WidgetId>> {
        match self.root {
            Some(root) => self.hit_in(root, x, y, f, &mut HashSet::new()),
            None => Ok(None),
        }
    }

    fn hit_in(
        &mut self,
        id: WidgetId,
        x: f32,
        y: f32,
        f: &mut dyn FnMut(&mut dyn Widget, &mut EventContext) -> Result<bool>,
        seen: &mut HashSet<WidgetId>,
    ) -> Result<Option<WidgetId>> {
        if !seen.insert(id) {
            return Ok(None);
        }
        let Some(widget) = self.store.get(id) else {
            return Ok(None);
        };
        if !widget.takes_space() || !widget.is_visible() {
            return Ok(None);
        }
        let rect = widget.base().rect();
        for child in widget.children() {
            if let Some(hit) = self.hit_in(child, x, y, f, seen)? {
                return Ok(Some(hit));
            }
        }
        if rect.contains(x, y) && self.with_event(id, |w, cx| f(w, cx))? == Some(true) {
            return Ok(Some(id));
        }
        Ok(None)
    }

    /// Calls `f` with widget `id` and a layout context. `None` if there is no such widget.
    fn with_layout<R>(
        &mut self,
        id: WidgetId,
        f: impl FnOnce(&mut dyn Widget, &mut LayoutContext) -> Result<R>,
    ) -> Result<Option<R>> {
        let Some((mut widget, node)) = self.store.take(id) else {
            return Ok(None);
        };
        let mut cx = LayoutContext {
            scene: &mut self.scene,
            anchor: node,
            square: self.square,
            store: &self.store,
        };
        let result = f(&mut *widget, &mut cx);
        self.store.put_back(id, widget);
        result.map(Some)
    }

    /// Calls `f` with widget `id` and an event context. `None` if there is no such widget.
    fn with_event<R>(
        &mut self,
        id: WidgetId,
        f: impl FnOnce(&mut dyn Widget, &mut EventContext) -> Result<R>,
    ) -> Result<Option<R>> {
        let Some((mut widget, node)) = self.store.take(id) else {
            return Ok(None);
        };
        let mut cx = EventContext {
            scene: &mut self.scene,
            anchor: node,
            id,
            events: &mut self.events,
            relayout: &mut self.dirty,
        };
        let result = f(&mut *widget, &mut cx);
        self.store.put_back(id, widget);
        result.map(Some)
    }
}
