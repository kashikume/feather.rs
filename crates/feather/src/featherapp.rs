use crate::scene::Scene;
use anyhow::Result;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// What an app built on the engine provides: its scenes, and what it does every frame and on
/// input.
///
/// Mouse positions are in pixels of the window's drawing area (physical pixels, like the
/// rendered image), from its top left corner, with Y going down.
pub trait FeatherApp {
    fn on_create(&mut self) -> Result<()>;
    fn on_render(&mut self) -> Result<()>;
    fn on_update(&mut self, time: f32) -> Result<()>;
    fn on_destroy(&mut self);

    fn get_num_scenes_to_render(&self) -> usize;
    /// Scene `scene_index` (`0..get_num_scenes_to_render()`), drawn through its active camera
    /// over the scenes before it.
    fn get_scene_to_render(&mut self, scene_index: usize) -> &mut Scene;

    /// Called when a key is pressed (not for auto-repeat). `key` is the physical key, so it
    /// doesn't depend on the keyboard layout.
    fn on_key_pressed(&mut self, _key: KeyCode) -> Result<()> {
        Ok(())
    }

    /// Called with the size of the window's drawing area in pixels: once before the first
    /// frame, whenever it changes (not while the window is minimized), and after the engine
    /// resumes from `Suspended` (Android: back from the background).
    fn on_resized(&mut self, _width: u32, _height: u32) -> Result<()> {
        Ok(())
    }

    /// Called when the mouse pointer moves over the window, to `(x, y)`.
    fn on_mouse_moved(&mut self, _x: f32, _y: f32) -> Result<()> {
        Ok(())
    }

    /// Called when the mouse pointer leaves the window.
    fn on_mouse_left(&mut self) -> Result<()> {
        Ok(())
    }

    /// Called when a mouse button is pressed (`pressed`) or released, with the pointer at
    /// `(x, y)`.
    fn on_mouse_button(
        &mut self,
        _button: MouseButton,
        _pressed: bool,
        _x: f32,
        _y: f32,
    ) -> Result<()> {
        Ok(())
    }

    /// Called when the mouse wheel turns, with the pointer at `(x, y)`. `delta_y` is in lines,
    /// positive when the wheel turns away from the user (scrolling up); `delta_x` is positive to
    /// the right. Touchpads, which report pixels, are converted at `PIXELS_PER_WHEEL_LINE`.
    fn on_mouse_wheel(&mut self, _delta_x: f32, _delta_y: f32, _x: f32, _y: f32) -> Result<()> {
        Ok(())
    }

    /// Text for the window's title bar. `None` leaves the title the window was created with.
    /// The engine applies this at startup and after each key press or mouse click.
    fn window_title(&self) -> Option<String> {
        None
    }

    /// How the engine renders: multisampling and sample-rate shading. Read once, when the
    /// engine starts.
    fn render_settings(&self) -> RenderSettings {
        RenderSettings::default()
    }

    /// Called every frame, once the engine has collected what to draw, with how much it draws
    /// (e.g. to show it on screen).
    fn on_frame_stats(&mut self, _stats: &FrameStats) {}
}

/// What the engine draws in a frame (`FeatherApp::on_frame_stats`): the stats of each scene,
/// and the time taken to collect the draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// The stats of every scene, by index (`FeatherApp::get_scene_to_render`). A scene without
    /// a camera isn't drawn: it has no draws, but its resources are counted.
    pub scenes: Vec<SceneStats>,
    /// How long the engine took to collect the draws of every scene and pack their uniforms, in
    /// seconds.
    pub collect_time: f32,
}

impl FrameStats {
    /// The stats of every scene added together.
    pub fn total(&self) -> SceneStats {
        SceneStats::sum(&self.scenes)
    }
}

/// What the engine draws of one scene in a frame, and the scene's resources (`FrameStats`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneStats {
    /// Draw calls in the main pass: the nodes drawn, one draw per node (the C++ engine's
    /// `nodesRendered`).
    pub draws: usize,
    /// Nodes left out of the main pass because their mesh is outside the camera's view
    /// (frustum culling, see `bounds`).
    pub culled: usize,
    /// Draw calls into the scene's shadow map.
    pub shadow_draws: usize,
    /// Nodes in the scene (`Scene::nodes`), visible or not.
    pub nodes: usize,
    /// Meshes in the scene (`Scene::meshes`).
    pub meshes: usize,
    /// Mesh buffers in the scene (`Scene::buffers`): the vertex and index buffers its meshes
    /// are packed into.
    pub mesh_buffers: usize,
    /// Atlases (textures) in the scene (`Scene::atlases`).
    pub atlases: usize,
}

impl SceneStats {
    /// `stats` added together.
    pub fn sum(stats: &[SceneStats]) -> SceneStats {
        stats.iter().fold(SceneStats::default(), |a, b| SceneStats {
            draws: a.draws + b.draws,
            culled: a.culled + b.culled,
            shadow_draws: a.shadow_draws + b.shadow_draws,
            nodes: a.nodes + b.nodes,
            meshes: a.meshes + b.meshes,
            mesh_buffers: a.mesh_buffers + b.mesh_buffers,
            atlases: a.atlases + b.atlases,
        })
    }
}

/// How the engine renders (`FeatherApp::render_settings`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderSettings {
    /// The most samples per pixel for multisample antialiasing (1, 2, 4, 8, 16, 32 or 64); the
    /// engine takes the largest count up to this that the device supports. 1 turns MSAA off.
    /// The default, 4, costs little on tile-based GPUs and is the usual balance on desktop.
    pub max_msaa_samples: u32,
    /// Sample-rate shading: with MSAA, run the fragment shader for several samples of a pixel
    /// (at least 20% of them), smoothing the inside of textured surfaces as well as their
    /// edges, at the cost of more shader work. Used only if the device supports it. Off by
    /// default.
    pub sample_shading: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            max_msaa_samples: 4,
            sample_shading: false,
        }
    }
}

/// How many pixels of touchpad scrolling make one line of `FeatherApp::on_mouse_wheel`.
pub const PIXELS_PER_WHEEL_LINE: f32 = 20.0;
