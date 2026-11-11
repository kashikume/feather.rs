//! Tests of the hud with real widgets in a scene (no GPU needed).

use cgmath::SquareMatrix;
use feather::atlas::Atlas;
use feather::camera::ScreenUnits;
use feather::math::Mat4;
use feather::scene::Scene;
use feather::shader::ShaderUniformNames;
use feather::text::{create_text_material, TextBuilder};

use crate::*;

const VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/basic.vert.spv"));
const PANEL_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/panel.frag.spv"));
const TEXT_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/text.frag.spv"));

/// A hud in an 800 × 600 window, with materials for buttons and a font.
struct Fixture {
    hud: Hud,
    normal: usize,
    hover: usize,
    pressed: usize,
    font: usize,
    text: usize,
}

fn fixture() -> Fixture {
    let mut scene = Scene::new();
    let panel = scene
        .create_shader(None, VERT, PANEL_FRAG, ShaderUniformNames::default())
        .unwrap();
    let normal = scene.create_material(Some("normal".into()), panel).unwrap();
    let hover = scene.create_material(Some("hover".into()), panel).unwrap();
    let pressed = scene.create_material(Some("pressed".into()), panel).unwrap();

    // `A` is 8 × 16 pixels, the space 4 × 16.
    let mut atlas = Atlas::from_rgba(Some("font".into()), 64, 32, vec![0; 64 * 32 * 4]);
    atlas.set_metadata("65 0 0 8 16\n32 8 0 4 16").unwrap();
    let font = scene.add_atlas(atlas);
    let text_shader = scene
        .create_shader(None, VERT, TEXT_FRAG, ShaderUniformNames::default())
        .unwrap();
    let text = create_text_material(&mut scene, None, text_shader, font).unwrap();

    let mut hud = Hud::new(scene).unwrap();
    hud.set_window_size(800, 600);
    Fixture {
        hud,
        normal,
        hover,
        pressed,
        font,
        text,
    }
}

impl Fixture {
    fn button(&self) -> Button {
        Button::new()
            .material(self.normal)
            .hover_material(self.hover)
            .pressed_material(self.pressed)
    }

    fn label(&self, text: &str) -> Label {
        Label::new(TextBuilder::new(self.font).height(16.0), self.text, text)
    }

    fn rect(&self, id: impl Into<WidgetId>) -> Rect {
        self.hud.widget(id).unwrap().base().rect()
    }

    fn node_visible(&self, id: impl Into<WidgetId>) -> bool {
        let node = self.hud.node(id).unwrap();
        self.hud.scene().get_node(node).unwrap().is_visible()
    }

    fn node_transform(&self, id: impl Into<WidgetId>) -> Mat4 {
        let node = self.hud.node(id).unwrap();
        self.hud.scene().get_node(node).unwrap().get_transform()
    }

    fn material_of(&self, button: WidgetId<Button>) -> Option<usize> {
        let node = self.hud.get(button).unwrap().background_node().unwrap();
        self.hud.scene().get_node(node).unwrap().get_material()
    }

    /// Clicks at `(x, y)` (window pixels): press and release there.
    fn click(&mut self, x: f32, y: f32) -> bool {
        let taken = self.hud.mouse_down(x, y).unwrap();
        assert_eq!(self.hud.mouse_up(x, y).unwrap(), taken);
        taken
    }
}

#[test]
fn the_hud_has_its_own_camera_root_and_square() {
    let f = fixture();
    let scene = f.hud.scene();
    let camera = scene.active_camera().unwrap();
    assert_eq!(scene.find_camera("UI camera"), scene.active_camera_handle());
    assert_eq!(camera.get_view(), Mat4::identity());
    assert!(scene.find_mesh("UI square").is_some());
    assert!(scene.get_node(f.hud.root_node()).unwrap().is_root());
}

#[test]
fn a_side_button_keeps_its_size_and_alignment_in_its_slot() {
    let mut f = fixture();
    let button = f.button().with_size(40.0, 100.0).with_margin(5.0);
    let button = f.hud.add(button.with_align(Align::End, Align::Center)).unwrap();
    let root = f.hud.add(LayoutNswe::new().with(Slot::East, button)).unwrap();
    f.hud.set_root(Some(root.into()));
    f.hud.set_scale(2.0);
    f.hud.update().unwrap();

    // 800 × 600 pixels at scale 2 is 400 × 300 units. East is 50 wide with the margins.
    assert_eq!(f.hud.size(), (400.0, 300.0));
    assert_eq!(f.rect(root), Rect::new(0.0, 0.0, 400.0, 300.0));
    assert_eq!(f.rect(button), Rect::new(355.0, 100.0, 395.0, 200.0));
    // The node is at the top left corner, Y up.
    assert_eq!(
        f.node_transform(button),
        Mat4::from_translation([355.0, -100.0, 0.0].into())
    );
    assert!(f.node_visible(button));
    let camera = f.hud.scene().active_camera().unwrap();
    assert_eq!(camera.get_units(), ScreenUnits::Height(300.0));

    // The window grows: the button follows the right edge.
    f.hud.set_window_size(1000, 600);
    f.hud.update().unwrap();
    assert_eq!(f.rect(button).left, 455.0);
}

#[test]
fn widgets_out_of_the_tree_or_gone_are_hidden() {
    let mut f = fixture();
    let west = f.hud.add(Spacer::new().with_size(30.0, 0.0)).unwrap();
    let center = f.hud.add(Spacer::new()).unwrap();
    let loose = f.hud.add(Spacer::new()).unwrap();
    let root = LayoutNswe::new().with(Slot::West, west).with(Slot::Center, center);
    let root = f.hud.add(root).unwrap();
    f.hud.set_root(Some(root.into()));
    f.hud.update().unwrap();
    assert!(f.node_visible(west) && f.node_visible(center));
    assert!(!f.node_visible(loose));
    assert_eq!(f.rect(center).left, 30.0);

    // Invisible: hidden, keeps its space.
    f.hud.get_mut(west).unwrap().base_mut().set_visibility(Visibility::Invisible);
    f.hud.update().unwrap();
    assert!(!f.node_visible(west));
    assert_eq!(f.rect(center).left, 30.0);

    // Gone: hidden, gives its space to the center.
    f.hud.get_mut(west).unwrap().base_mut().set_visibility(Visibility::Gone);
    f.hud.update().unwrap();
    assert!(!f.node_visible(west));
    assert_eq!(f.rect(center).left, 0.0);

    // Out of the tree.
    f.hud.get_mut(root).unwrap().set(Slot::Center, None);
    f.hud.update().unwrap();
    assert!(!f.node_visible(center));
}

#[test]
fn a_hidden_layout_hides_what_it_holds() {
    let mut f = fixture();
    let button = f.hud.add(f.button()).unwrap();
    let inner = f.hud.add(LayoutNswe::new().with(Slot::Center, button)).unwrap();
    let root = f.hud.add(LayoutNswe::new().with(Slot::Center, inner)).unwrap();
    f.hud.set_root(Some(root.into()));
    f.hud.update().unwrap();
    assert!(f.node_visible(button));

    f.hud.get_mut(inner).unwrap().base_mut().set_visibility(Visibility::Invisible);
    f.hud.update().unwrap();
    assert!(!f.node_visible(button));
    // Hidden widgets don't take clicks.
    assert!(!f.click(400.0, 300.0));
}

#[test]
fn buttons_report_clicks_released_over_them() {
    let mut f = fixture();
    let button = f.button().with_size(100.0, 50.0);
    let button = f.hud.add(button.with_align(Align::Start, Align::Start)).unwrap();
    f.hud.set_root(Some(button.into()));

    assert!(f.click(10.0, 10.0));
    assert_eq!(
        f.hud.take_events(),
        [UiEvent::Clicked {
            widget: button.into()
        }]
    );
    assert!(f.hud.take_events().is_empty());

    // Pressed, then released elsewhere: taken, but no click.
    assert!(f.hud.mouse_down(10.0, 10.0).unwrap());
    assert!(f.hud.mouse_move(300.0, 300.0).unwrap());
    assert!(f.hud.mouse_up(300.0, 300.0).unwrap());
    assert!(f.hud.take_events().is_empty());

    // Outside the button: not taken.
    assert!(!f.click(300.0, 300.0));
    assert!(f.hud.take_events().is_empty());
}

#[test]
fn buttons_show_hover_and_press() {
    let mut f = fixture();
    let button = f.button().with_size(100.0, 50.0);
    let button = f.hud.add(button.with_align(Align::Start, Align::Start)).unwrap();
    f.hud.set_root(Some(button.into()));
    f.hud.update().unwrap();
    assert_eq!(f.material_of(button), Some(f.normal));

    assert!(!f.hud.mouse_move(10.0, 10.0).unwrap());
    assert_eq!(f.material_of(button), Some(f.hover));
    f.hud.mouse_down(10.0, 10.0).unwrap();
    assert_eq!(f.material_of(button), Some(f.pressed));
    // Dragged off while pressed: no longer lit.
    f.hud.mouse_move(300.0, 300.0).unwrap();
    assert_eq!(f.material_of(button), Some(f.normal));
    f.hud.mouse_move(10.0, 10.0).unwrap();
    assert_eq!(f.material_of(button), Some(f.pressed));
    f.hud.mouse_up(10.0, 10.0).unwrap();
    assert_eq!(f.material_of(button), Some(f.hover));
    f.hud.mouse_left().unwrap();
    assert_eq!(f.material_of(button), Some(f.normal));
}

#[test]
fn clicks_reach_buttons_in_nested_layouts() {
    let mut f = fixture();
    let button = f.hud.add(f.button().with_size(50.0, 50.0)).unwrap();
    let inner = f.hud.add(LayoutNswe::new().with(Slot::West, button)).unwrap();
    let root = f.hud.add(LayoutNswe::new().with(Slot::Center, inner)).unwrap();
    f.hud.set_root(Some(root.into()));

    assert!(f.click(10.0, 300.0));
    assert_eq!(
        f.hud.take_events(),
        [UiEvent::Clicked {
            widget: button.into()
        }]
    );
    // Layouts themselves don't take clicks.
    assert!(!f.click(400.0, 300.0));
}

#[test]
fn mouse_positions_are_divided_by_the_scale() {
    let mut f = fixture();
    let button = f.button().with_size(100.0, 50.0);
    let button = f.hud.add(button.with_align(Align::Start, Align::Start)).unwrap();
    f.hud.set_root(Some(button.into()));
    f.hud.set_scale(4.0);
    // The button is 400 × 200 pixels.
    assert!(f.click(390.0, 190.0));
    assert!(!f.click(410.0, 190.0));
}

#[test]
fn labels_are_as_big_as_their_text() {
    let mut f = fixture();
    let label = f.label("AA A").with_align(Align::Start, Align::End);
    let label = f.hud.add(label).unwrap();
    let root = f.hud.add(LayoutNswe::new().with(Slot::South, label)).unwrap();
    f.hud.set_root(Some(root.into()));
    f.hud.update().unwrap();

    // 8 + 8 + 4 + 8 wide, a line high: at the bottom left corner.
    assert_eq!(f.rect(label), Rect::new(0.0, 584.0, 28.0, 600.0));
    let text = f.hud.get(label).unwrap().text_node().unwrap();
    assert_eq!(
        f.hud.scene().get_node(text).unwrap().get_parent(),
        f.hud.node(label)
    );

    f.hud.get_mut(label).unwrap().set_text("A");
    f.hud.update().unwrap();
    assert_eq!(f.rect(label), Rect::new(0.0, 584.0, 8.0, 600.0));
    assert_eq!(f.hud.get(label).unwrap().text(), "A");
}

#[test]
fn button_labels_are_centered() {
    let mut f = fixture();
    let button = f.button().label(f.label("A")).with_size(48.0, 96.0);
    let button = f.hud.add(button.with_align(Align::Start, Align::Start)).unwrap();
    f.hud.set_root(Some(button.into()));
    f.hud.update().unwrap();
    let text = f
        .hud
        .get(button)
        .unwrap()
        .get_label()
        .unwrap()
        .text_node()
        .unwrap();
    let transform = f.hud.scene().get_node(text).unwrap().get_transform();
    // (48 - 8) / 2 across, (96 - 16) / 2 down.
    assert_eq!(transform.w.x, 20.0);
    assert_eq!(transform.w.y, -40.0);
}

#[test]
fn removed_widgets_lose_their_nodes() {
    let mut f = fixture();
    let button = f.hud.add(f.button()).unwrap();
    f.hud.set_root(Some(button.into()));
    f.hud.update().unwrap();
    let node = f.hud.node(button).unwrap();
    f.hud.remove(button).unwrap();
    assert!(f.hud.get(button).is_none());
    assert!(f.hud.scene().get_node(node).is_none());
    assert_eq!(f.hud.root(), None);
}

fn scrollbar_fixture() -> (Fixture, WidgetId<Scrollbar>) {
    let mut f = fixture();
    // A vertical scrollbar 20 wide on the right, 600 high.
    let mut bar = Scrollbar::vertical()
        .track_material(f.normal)
        .knob_material(f.normal)
        .knob_hover_material(f.hover)
        .with_size(20.0, 0.0);
    bar.set_range(0.0, 300.0);
    bar.set_page(100.0);
    let bar = f.hud.add(bar).unwrap();
    let root = f.hud.add(LayoutNswe::new().with(Slot::East, bar)).unwrap();
    f.hud.set_root(Some(root.into()));
    f.hud.update().unwrap();
    (f, bar)
}

fn value(f: &Fixture, bar: WidgetId<Scrollbar>) -> f64 {
    f.hud.get(bar).unwrap().value()
}

#[test]
fn the_knob_shows_the_page_in_the_content() {
    let (f, bar) = scrollbar_fixture();
    let bar = f.hud.get(bar).unwrap();
    // A page of 100 in 400 of content: a quarter of the track.
    assert_eq!(bar.knob_size(600.0), 150.0);
    assert_eq!(bar.knob_position(600.0), 0.0);
    assert_eq!(bar.knob_rect(), Rect::new(0.0, 0.0, 20.0, 150.0));

    let mut other = Scrollbar::vertical();
    other.set_range(0.0, 1000.0);
    other.set_page(1.0);
    other.set_value(1000.0);
    // At least the minimum knob size, at the end.
    assert_eq!(other.knob_size(600.0), 10.0);
    assert_eq!(other.knob_position(600.0), 590.0);
    // Nothing to scroll: the knob is the track (no division by zero).
    other.set_range(5.0, 5.0);
    assert_eq!(other.knob_size(600.0), 600.0);
    assert_eq!(other.knob_position(600.0), 0.0);
    // A track shorter than the minimum knob.
    other.set_range(0.0, 10.0);
    assert_eq!(other.knob_size(4.0), 4.0);
}

#[test]
fn values_stay_in_range() {
    let mut bar = Scrollbar::horizontal();
    bar.set_range(10.0, 20.0);
    bar.set_value(25.0);
    assert_eq!(bar.value(), 20.0);
    bar.set_range(0.0, 5.0);
    assert_eq!(bar.value(), 5.0);
    bar.set_value(-1.0);
    assert_eq!(bar.value(), 0.0);
    bar.set_range(3.0, 1.0);
    assert_eq!((bar.min(), bar.max(), bar.value()), (3.0, 3.0, 3.0));
}

#[test]
fn clicking_the_track_pages() {
    let (mut f, bar) = scrollbar_fixture();
    // Below the knob (0..150 high): a page down.
    assert!(f.click(790.0, 500.0));
    assert_eq!(value(&f, bar), 100.0);
    assert_eq!(
        f.hud.take_events(),
        [UiEvent::ValueChanged {
            widget: bar.into(),
            value: 100.0
        }]
    );
    // The knob moved: 100 / 300 of the 450 free.
    f.hud.update().unwrap();
    assert_eq!(f.hud.get(bar).unwrap().knob_rect().top, 150.0);
    // Above it: back up.
    f.click(790.0, 10.0);
    assert_eq!(value(&f, bar), 0.0);
    // Already at the top: no change, no event.
    f.hud.take_events();
    f.click(790.0, 10.0);
    assert_eq!(value(&f, bar), 0.0);
    assert!(f.hud.take_events().is_empty());
}

#[test]
fn dragging_the_knob_follows_the_pointer_to_both_ends() {
    let (mut f, bar) = scrollbar_fixture();
    // Take the knob 50 below its top, move 225 down: half of the 450 free.
    assert!(f.hud.mouse_down(790.0, 50.0).unwrap());
    assert!(f.hud.mouse_move(790.0, 275.0).unwrap());
    assert_eq!(value(&f, bar), 150.0);
    // Far past the end, and off the scrollbar: still dragging, at the end.
    f.hud.mouse_move(100.0, 5000.0).unwrap();
    assert_eq!(value(&f, bar), 300.0);
    f.hud.mouse_move(100.0, -5000.0).unwrap();
    assert_eq!(value(&f, bar), 0.0);
    f.hud.mouse_up(790.0, 50.0).unwrap();
    assert_eq!(value(&f, bar), 0.0);
    let events = f.hud.take_events();
    assert_eq!(events.len(), 3, "{events:?}");
    // Released: moving no longer drags.
    f.hud.mouse_move(790.0, 400.0).unwrap();
    assert_eq!(value(&f, bar), 0.0);
}

#[test]
fn the_knob_lights_up_under_the_pointer_and_while_dragged() {
    let (mut f, bar) = scrollbar_fixture();
    let knob_material = |f: &Fixture| {
        let scene = f.hud.scene();
        // The knob is the anchor's child with the knob's transform (the track starts at 0).
        let anchor = scene.get_node(f.hud.node(bar).unwrap()).unwrap();
        let knob = anchor
            .get_children()
            .iter()
            .map(|&n| scene.get_node(n).unwrap())
            .find(|n| n.get_transform().w.z > 0.0)
            .unwrap();
        knob.get_material()
    };
    assert_eq!(knob_material(&f), Some(f.normal));
    f.hud.mouse_move(790.0, 50.0).unwrap();
    assert_eq!(knob_material(&f), Some(f.hover));
    f.hud.mouse_down(790.0, 50.0).unwrap();
    f.hud.mouse_move(100.0, 50.0).unwrap();
    assert_eq!(knob_material(&f), Some(f.hover));
    f.hud.mouse_up(100.0, 50.0).unwrap();
    assert_eq!(knob_material(&f), Some(f.normal));
}

#[test]
fn the_wheel_scrolls_the_scrollbar_under_the_pointer() {
    let (mut f, bar) = scrollbar_fixture();
    // Towards the user: down, a tenth of the page per line.
    assert!(f.hud.mouse_wheel(0.0, -3.0, 790.0, 300.0).unwrap());
    assert_eq!(value(&f, bar), 30.0);
    f.hud.get_mut(bar).unwrap().set_wheel_step(Some(50.0));
    f.hud.mouse_wheel(0.0, 1.0, 790.0, 300.0).unwrap();
    assert_eq!(value(&f, bar), 0.0);
    // Not over it.
    assert!(!f.hud.mouse_wheel(0.0, -1.0, 100.0, 300.0).unwrap());
    assert_eq!(value(&f, bar), 0.0);
}

#[test]
fn scrollbars_hide_when_there_is_nothing_to_scroll() {
    let (mut f, bar) = scrollbar_fixture();
    assert!(f.node_visible(bar));
    f.hud.get_mut(bar).unwrap().set_range(0.0, 0.0);
    f.hud.update().unwrap();
    assert!(!f.node_visible(bar));
    assert!(!f.click(790.0, 10.0));

    // Forced visible.
    f.hud.get_mut(bar).unwrap().base_mut().set_visibility(Visibility::Visible);
    f.hud.update().unwrap();
    assert!(f.node_visible(bar));
    // Invisible: not drawn even though it could scroll (a bug in C++).
    f.hud.get_mut(bar).unwrap().set_range(0.0, 10.0);
    f.hud.get_mut(bar).unwrap().base_mut().set_visibility(Visibility::Invisible);
    f.hud.update().unwrap();
    assert!(!f.node_visible(bar));
}

#[test]
fn the_app_setting_the_value_is_not_an_event() {
    let (mut f, bar) = scrollbar_fixture();
    f.hud.get_mut(bar).unwrap().set_value(200.0);
    f.hud.update().unwrap();
    assert!(f.hud.take_events().is_empty());
    assert_eq!(f.hud.get(bar).unwrap().knob_rect().top, 300.0);
}
