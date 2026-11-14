//! The camera a scene is seen through: where it is, where it looks, and how it projects the
//! scene onto the screen, in perspective or flat (orthographic).
//!
//! Cameras are scene objects (`Scene::add_camera`); a scene is drawn through its active camera
//! (`Scene::set_active_camera`).

use std::sync::OnceLock;

use cgmath::Deg;

use super::math::{Mat4, Point3, Vec3};
use super::object::Object;

/// How a camera projects the scene onto the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    /// Farther things look smaller, within the camera's field of view (`Camera::set_fov`).
    Perspective,
    /// No perspective: things keep their size whatever their distance. The part of the scene
    /// shown depends on `ScreenOrigin` and `ScreenUnits`. For 2D scenes.
    Orthographic,
}

/// Where the origin of an orthographic camera's view is on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenOrigin {
    /// At the top left corner: the screen spans x `0..width` and y `-height..0`.
    TopLeft,
    /// In the middle: x `-width/2..width/2`, y `-height/2..height/2`.
    Center,
    /// At the bottom left corner: x `0..width`, y `0..height`.
    BottomLeft,
}

/// How many scene units an orthographic camera shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScreenUnits {
    /// One unit per pixel of the rendered image: things keep their size in pixels, and more of
    /// the scene shows in a bigger window.
    Pixels,
    /// The screen is this many units high, and as wide as the image's aspect ratio makes it:
    /// the scene is scaled with the window (and with display scaling).
    Height(f32),
}

/// A camera, perspective or orthographic.
///
/// Both kinds have a position (`eye`), a point they look at (`target`), an up direction and a
/// depth range (`near`..`far`, along the view direction). A perspective camera also has a
/// vertical field of view; an orthographic one a `ScreenOrigin` and `ScreenUnits`. A camera
/// keeps the settings of both kinds, so `set_projection` can switch back and forth without
/// losing them; the settings of the other kind just have no effect.
///
/// The renderer sets the size of the rendered image every frame (`set_screen_dimention`).
/// View and projection matrices are computed when asked for and cached until a setter changes
/// them.
#[derive(Clone, Debug)]
pub struct Camera {
    name: Option<String>,
    handle: usize,
    projection: Projection,
    fov: f32,
    origin: ScreenOrigin,
    units: ScreenUnits,
    near: f32,
    far: f32,
    eye_position: Point3,
    target_position: Point3,
    up_vector: Vec3,
    screen_width: u32,
    screen_height: u32,
    view_matrix: OnceLock<Mat4>,
    projection_matrix: OnceLock<Mat4>,
}

impl Object for Camera {
    fn get_name(&self) -> Option<String> {
        self.name.clone()
    }

    fn set_handle(&mut self, handle: usize) {
        self.handle = handle;
    }

    fn get_handle(&self) -> usize {
        self.handle
    }
}

/// Vulkan's clip space has Y down and depth 0..1; cgmath's has Y up and depth -1..1.
#[rustfmt::skip]
const VULKAN_CORRECTION: Mat4 = Mat4::new(
    1.0,  0.0, 0.0, 0.0,
    0.0, -1.0, 0.0, 0.0,
    0.0,  0.0, 0.5, 0.0,
    0.0,  0.0, 0.5, 1.0,
);

impl Camera {
    /// A perspective camera: 45 degree field of view, depth 0.1..10, at (2, 2, 2) looking at
    /// the origin with Y up.
    pub fn perspective() -> Self {
        Self {
            name: None,
            handle: usize::MAX,
            projection: Projection::Perspective,
            fov: 45.0,
            origin: ScreenOrigin::Center,
            units: ScreenUnits::Pixels,
            near: 0.1,
            far: 10.0,
            eye_position: Point3::new(2.0, 2.0, 2.0),
            target_position: Point3::new(0.0, 0.0, 0.0),
            up_vector: Vec3::new(0.0, 1.0, 0.0),
            screen_width: 1920,
            screen_height: 1080,
            view_matrix: OnceLock::new(),
            projection_matrix: OnceLock::new(),
        }
    }

    /// An orthographic camera for 2D scenes, with its origin at `origin`, showing `units`. It
    /// shows the XY plane looking down -Z from the origin, with Y up and X to the right, like
    /// the rest of the engine: its view is the identity. Objects between z = -1000 and
    /// z = 1000 are visible; the nearer ones (greater z) are drawn over the farther ones.
    pub fn orthographic(origin: ScreenOrigin, units: ScreenUnits) -> Self {
        Self {
            projection: Projection::Orthographic,
            origin,
            units,
            near: -1000.0,
            far: 1000.0,
            eye_position: Point3::new(0.0, 0.0, 0.0),
            target_position: Point3::new(0.0, 0.0, -1.0),
            up_vector: Vec3::new(0.0, 1.0, 0.0),
            ..Self::perspective()
        }
    }

    /// Names the camera, for `Scene::find_camera`.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn get_projection_kind(&self) -> Projection {
        self.projection
    }

    /// Switches between perspective and orthographic. The settings of both kinds are kept.
    pub fn set_projection(&mut self, projection: Projection) -> &mut Self {
        if projection != self.projection {
            self.projection = projection;
            self.projection_matrix = OnceLock::new();
        }
        self
    }

    /// The projection matrix, for Vulkan's clip space (y down, depth 0..1).
    pub fn get_projection(&self) -> Mat4 {
        *self
            .projection_matrix
            .get_or_init(|| self.compute_projection())
    }

    fn compute_projection(&self) -> Mat4 {
        VULKAN_CORRECTION
            * match self.projection {
                Projection::Perspective => cgmath::perspective(
                    Deg(self.fov),
                    self.screen_width as f32 / self.screen_height.max(1) as f32,
                    self.near,
                    self.far,
                ),
                Projection::Orthographic => {
                    let (left, right, bottom, top) = self.orthographic_area();
                    cgmath::ortho(left, right, bottom, top, self.near, self.far)
                }
            }
    }

    /// The view matrix: from world space to the camera's, where it looks down -Z.
    pub fn get_view(&self) -> Mat4 {
        *self.view_matrix.get_or_init(|| {
            Mat4::look_at_rh(self.eye_position, self.target_position, self.up_vector)
        })
    }

    /// Called by the renderer with the size of the image being rendered.
    pub fn set_screen_dimention(&mut self, width: u32, height: u32) -> &mut Self {
        if width != self.screen_width || height != self.screen_height {
            self.screen_width = width;
            self.screen_height = height;
            self.projection_matrix = OnceLock::new();
        }
        self
    }

    pub fn get_screen_dimention(&self) -> (u32, u32) {
        (self.screen_width, self.screen_height)
    }

    // --- perspective ---

    pub fn get_fov(&self) -> f32 {
        self.fov
    }

    /// The vertical field of view in degrees, for a perspective camera.
    pub fn set_fov(&mut self, fov: f32) -> &mut Self {
        if fov != self.fov {
            self.fov = fov;
            self.invalidate_projection_of(Projection::Perspective);
        }
        self
    }

    // --- orthographic ---

    pub fn get_origin(&self) -> ScreenOrigin {
        self.origin
    }

    /// Where the origin is on the screen, for an orthographic camera.
    pub fn set_origin(&mut self, origin: ScreenOrigin) -> &mut Self {
        if origin != self.origin {
            self.origin = origin;
            self.invalidate_projection_of(Projection::Orthographic);
        }
        self
    }

    pub fn get_units(&self) -> ScreenUnits {
        self.units
    }

    /// How many scene units the screen shows, for an orthographic camera.
    pub fn set_units(&mut self, units: ScreenUnits) -> &mut Self {
        if units != self.units {
            self.units = units;
            self.invalidate_projection_of(Projection::Orthographic);
        }
        self
    }

    /// The width and height of the screen in scene units, for an orthographic camera (`None`
    /// for a perspective one).
    pub fn get_visible_size(&self) -> Option<(f32, f32)> {
        (self.projection == Projection::Orthographic).then(|| self.orthographic_size())
    }

    /// The visible area, (left, right, bottom, top) in scene units around the view direction,
    /// for an orthographic camera (`None` for a perspective one).
    pub fn get_visible_area(&self) -> Option<(f32, f32, f32, f32)> {
        (self.projection == Projection::Orthographic).then(|| self.orthographic_area())
    }

    fn orthographic_size(&self) -> (f32, f32) {
        let (width, height) = (self.screen_width as f32, self.screen_height.max(1) as f32);
        match self.units {
            ScreenUnits::Pixels => (width, height),
            ScreenUnits::Height(units) => (units * width / height, units),
        }
    }

    fn orthographic_area(&self) -> (f32, f32, f32, f32) {
        let (width, height) = self.orthographic_size();
        match self.origin {
            ScreenOrigin::TopLeft => (0.0, width, -height, 0.0),
            ScreenOrigin::Center => (-width / 2.0, width / 2.0, -height / 2.0, height / 2.0),
            ScreenOrigin::BottomLeft => (0.0, width, 0.0, height),
        }
    }

    // --- both ---

    pub fn get_near(&self) -> f32 {
        self.near
    }

    pub fn get_far(&self) -> f32 {
        self.far
    }

    /// The visible depth range along the view direction. Near may be negative (behind the
    /// eye) for an orthographic camera.
    pub fn set_near_far(&mut self, near: f32, far: f32) -> &mut Self {
        if near != self.near || far != self.far {
            self.near = near;
            self.far = far;
            self.projection_matrix = OnceLock::new();
        }
        self
    }

    pub fn set_near(&mut self, near: f32) -> &mut Self {
        self.set_near_far(near, self.far)
    }

    pub fn set_far(&mut self, far: f32) -> &mut Self {
        self.set_near_far(self.near, far)
    }

    pub fn get_eye_position(&self) -> Point3 {
        self.eye_position
    }

    pub fn get_target_position(&self) -> Point3 {
        self.target_position
    }

    pub fn get_up_vector(&self) -> Vec3 {
        self.up_vector
    }

    /// Puts the camera at `eye_position`, looking at `target_position`, with `up_vector` up.
    pub fn set_view(
        &mut self,
        eye_position: Point3,
        target_position: Point3,
        up_vector: Vec3,
    ) -> &mut Self {
        if eye_position != self.eye_position
            || target_position != self.target_position
            || up_vector != self.up_vector
        {
            self.eye_position = eye_position;
            self.target_position = target_position;
            self.up_vector = up_vector;
            self.view_matrix = OnceLock::new();
        }
        self
    }

    pub fn set_eye_position(&mut self, eye_position: Point3) -> &mut Self {
        self.set_view(eye_position, self.target_position, self.up_vector)
    }

    pub fn set_target_position(&mut self, target_position: Point3) -> &mut Self {
        self.set_view(self.eye_position, target_position, self.up_vector)
    }

    pub fn set_up_vector(&mut self, up_vector: Vec3) -> &mut Self {
        self.set_view(self.eye_position, self.target_position, up_vector)
    }

    /// Drops the cached projection if the camera has `kind` of projection: settings of the
    /// other kind don't change it.
    fn invalidate_projection_of(&mut self, kind: Projection) {
        if self.projection == kind {
            self.projection_matrix = OnceLock::new();
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::perspective()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec4;
    use cgmath::{InnerSpace, SquareMatrix};

    const EPS: f32 = 1e-5;

    fn assert_close(a: f32, b: f32) {
        assert!((a - b).abs() < EPS, "{a} != {b}");
    }

    fn assert_vec_close(a: Vec3, b: Vec3) {
        assert!((a - b).magnitude() < EPS, "{a:?} != {b:?}");
    }

    fn assert_mat_close(a: Mat4, b: Mat4) {
        let a: &[f32; 16] = a.as_ref();
        let b: &[f32; 16] = b.as_ref();
        for i in 0..16 {
            assert_close(a[i], b[i]);
        }
    }

    /// Transforms a point with `m` and does the perspective divide.
    fn project(m: Mat4, p: Vec3) -> Vec3 {
        let clip = m * Vec4::new(p.x, p.y, p.z, 1.0);
        clip.truncate() / clip.w
    }

    /// Where `p` ends up in Vulkan's normalized device coordinates.
    fn to_screen(camera: &Camera, p: Vec3) -> Vec3 {
        project(camera.get_projection() * camera.get_view(), p)
    }

    // --- construction ---

    #[test]
    fn perspective_has_expected_defaults() {
        let camera = Camera::perspective();
        assert_eq!(camera.get_projection_kind(), Projection::Perspective);
        assert_eq!(camera.fov, 45.0);
        assert_eq!(camera.near, 0.1);
        assert_eq!(camera.far, 10.0);
        assert_eq!(camera.screen_width, 1920);
        assert_eq!(camera.screen_height, 1080);
        assert_eq!(camera.eye_position, Point3::new(2.0, 2.0, 2.0));
        assert_eq!(camera.target_position, Point3::new(0.0, 0.0, 0.0));
        assert_eq!(camera.up_vector, Vec3::new(0.0, 1.0, 0.0));
        assert!(camera.projection_matrix.get().is_none());
        assert!(camera.view_matrix.get().is_none());
        assert_eq!(camera.get_name(), None);
    }

    #[test]
    fn default_is_perspective() {
        let a = Camera::perspective();
        let b = Camera::default();
        assert_eq!(b.get_projection_kind(), Projection::Perspective);
        assert_mat_close(a.get_projection(), b.get_projection());
        assert_mat_close(a.get_view(), b.get_view());
    }

    #[test]
    fn orthographic_view_is_identity() {
        let camera = Camera::orthographic(ScreenOrigin::Center, ScreenUnits::Pixels);
        assert_eq!(camera.get_projection_kind(), Projection::Orthographic);
        assert_mat_close(camera.get_view(), Mat4::identity());
        assert_eq!((camera.near, camera.far), (-1000.0, 1000.0));
    }

    #[test]
    fn with_name_names_the_camera() {
        let camera = Camera::perspective().with_name("Main");
        assert_eq!(camera.get_name().as_deref(), Some("Main"));
    }

    // --- perspective projection ---

    #[test]
    fn projection_matches_formula() {
        let mut camera = Camera::perspective();
        camera
            .set_fov(60.0)
            .set_near_far(0.5, 100.0)
            .set_screen_dimention(800, 600);
        let expected =
            VULKAN_CORRECTION * cgmath::perspective(Deg(60.0), 800.0 / 600.0, 0.5, 100.0);
        assert_mat_close(camera.get_projection(), expected);
    }

    #[test]
    fn projection_maps_near_and_far_to_vulkan_depth_range() {
        let mut camera = Camera::perspective();
        camera.set_near_far(0.5, 50.0);
        let proj = camera.get_projection();

        // The camera looks down -Z in view space; Vulkan depth goes from 0 (near) to 1 (far).
        assert_close(project(proj, Vec3::new(0.0, 0.0, -0.5)).z, 0.0);
        assert_close(project(proj, Vec3::new(0.0, 0.0, -50.0)).z, 1.0);

        let middle = project(proj, Vec3::new(0.0, 0.0, -5.0)).z;
        assert!(middle > 0.0 && middle < 1.0);
    }

    #[test]
    fn projection_flips_y_for_vulkan() {
        let camera = Camera::perspective();
        let proj = camera.get_projection();

        // Up in view space is down in Vulkan clip space (Y points down).
        assert!(project(proj, Vec3::new(0.0, 1.0, -5.0)).y < 0.0);
        assert!(project(proj, Vec3::new(0.0, -1.0, -5.0)).y > 0.0);
        // X is not flipped.
        assert!(project(proj, Vec3::new(1.0, 0.0, -5.0)).x > 0.0);
    }

    #[test]
    fn projection_edges_of_fov_map_to_clip_edges() {
        let mut camera = Camera::perspective();
        camera.set_fov(90.0).set_screen_dimention(200, 100);
        let proj = camera.get_projection();

        // With a 90 degree vertical FOV, a point at distance d and height d is at the top edge.
        assert_close(project(proj, Vec3::new(0.0, 3.0, -3.0)).y, -1.0);
        // Horizontally the view is wider by the aspect ratio (2:1).
        assert_close(project(proj, Vec3::new(6.0, 0.0, -3.0)).x, 1.0);
    }

    #[test]
    fn projection_depends_on_aspect_ratio() {
        let mut camera = Camera::perspective();
        camera.set_screen_dimention(100, 100);
        let square = camera.get_projection();
        camera.set_screen_dimention(200, 100);
        let wide = camera.get_projection();

        // Wider screen: X is scaled down by the aspect ratio, Y stays the same.
        assert_close(wide.x.x, square.x.x / 2.0);
        assert_close(wide.y.y, square.y.y);
    }

    #[test]
    fn perspective_has_no_visible_area() {
        let camera = Camera::perspective();
        assert_eq!(camera.get_visible_size(), None);
        assert_eq!(camera.get_visible_area(), None);
    }

    // --- orthographic projection ---

    #[test]
    fn pixels_from_the_top_left() {
        let mut camera = Camera::orthographic(ScreenOrigin::TopLeft, ScreenUnits::Pixels);
        camera.set_screen_dimention(800, 600);
        assert_eq!(camera.get_visible_area(), Some((0.0, 800.0, -600.0, 0.0)));
        // Vulkan: x -1 on the left, y -1 at the top.
        assert_vec_close(
            to_screen(&camera, Vec3::new(0.0, 0.0, 0.0)),
            Vec3::new(-1.0, -1.0, 0.5),
        );
        assert_vec_close(
            to_screen(&camera, Vec3::new(800.0, -600.0, 0.0)),
            Vec3::new(1.0, 1.0, 0.5),
        );
    }

    #[test]
    fn fixed_height_keeps_the_aspect_ratio() {
        let mut camera = Camera::orthographic(ScreenOrigin::Center, ScreenUnits::Height(10.0));
        camera.set_screen_dimention(1600, 800);
        assert_eq!(camera.get_visible_size(), Some((20.0, 10.0)));
        assert_eq!(camera.get_visible_area(), Some((-10.0, 10.0, -5.0, 5.0)));
        assert_vec_close(
            to_screen(&camera, Vec3::new(10.0, 5.0, 0.0)),
            Vec3::new(1.0, -1.0, 0.5),
        );

        camera.set_origin(ScreenOrigin::BottomLeft);
        assert_eq!(camera.get_visible_area(), Some((0.0, 20.0, 0.0, 10.0)));
    }

    #[test]
    fn nearer_objects_have_smaller_depth() {
        let mut camera = Camera::orthographic(ScreenOrigin::Center, ScreenUnits::Pixels);
        camera.set_near_far(-100.0, 100.0);
        let near = to_screen(&camera, Vec3::new(0.0, 0.0, 100.0)).z;
        let far = to_screen(&camera, Vec3::new(0.0, 0.0, -100.0)).z;
        assert_close(near, 0.0);
        assert_close(far, 1.0);
    }

    #[test]
    fn orthographic_view_pans_the_scene() {
        let mut camera = Camera::orthographic(ScreenOrigin::Center, ScreenUnits::Height(10.0));
        camera.set_screen_dimention(1000, 1000);
        // Looking at (3, 4) instead of the origin: (3, 4) is now in the middle.
        camera.set_view(
            Point3::new(3.0, 4.0, 0.0),
            Point3::new(3.0, 4.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        assert_vec_close(
            to_screen(&camera, Vec3::new(3.0, 4.0, 0.0)),
            Vec3::new(0.0, 0.0, 0.5),
        );
        // The right edge is 5 units to the right of it.
        assert_close(to_screen(&camera, Vec3::new(8.0, 4.0, 0.0)).x, 1.0);
    }

    // --- switching projection ---

    #[test]
    fn switching_projection_keeps_settings() {
        let mut camera = Camera::perspective();
        camera
            .set_fov(70.0)
            .set_origin(ScreenOrigin::TopLeft)
            .set_units(ScreenUnits::Height(5.0));
        let perspective = camera.get_projection();

        camera.set_projection(Projection::Orthographic);
        assert!(camera.projection_matrix.get().is_none());
        assert_eq!(
            camera.get_visible_size(),
            Some((5.0 * 1920.0 / 1080.0, 5.0))
        );
        assert_ne!(camera.get_projection(), perspective);

        camera.set_projection(Projection::Perspective);
        assert_eq!(camera.get_fov(), 70.0);
        assert_eq!(camera.get_origin(), ScreenOrigin::TopLeft);
        assert_mat_close(camera.get_projection(), perspective);
    }

    #[test]
    fn settings_of_the_other_kind_keep_the_projection() {
        let mut camera = Camera::perspective();
        let before = camera.get_projection();
        camera
            .set_origin(ScreenOrigin::TopLeft)
            .set_units(ScreenUnits::Height(3.0));
        assert_eq!(camera.projection_matrix.get().copied(), Some(before));

        let mut camera = Camera::orthographic(ScreenOrigin::Center, ScreenUnits::Pixels);
        let before = camera.get_projection();
        camera.set_fov(80.0);
        assert_eq!(camera.projection_matrix.get().copied(), Some(before));
    }

    // --- view ---

    #[test]
    fn view_matches_look_at() {
        let mut camera = Camera::perspective();
        let eye = Point3::new(1.0, 2.0, 3.0);
        let target = Point3::new(-1.0, 0.0, 0.5);
        let up = Vec3::new(0.0, 0.0, 1.0);
        camera.set_view(eye, target, up);

        assert_mat_close(camera.get_view(), Mat4::look_at_rh(eye, target, up));
    }

    #[test]
    fn view_puts_eye_at_origin_and_target_in_front() {
        let mut camera = Camera::perspective();
        camera.set_view(
            Point3::new(0.0, 0.0, 5.0),
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let view = camera.get_view();

        let eye = project(view, Vec3::new(0.0, 0.0, 5.0));
        assert_close(eye.magnitude(), 0.0);

        let target = project(view, Vec3::new(0.0, 0.0, 0.0));
        assert_vec_close(target, Vec3::new(0.0, 0.0, -5.0));
    }

    #[test]
    fn view_is_invertible() {
        let camera = Camera::perspective();
        assert!(camera.get_view().invert().is_some());
    }

    #[test]
    fn target_projects_to_screen_center() {
        let mut camera = Camera::perspective();
        camera.set_view(
            Point3::new(3.0, -2.0, 4.0),
            Point3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        let p = to_screen(&camera, Vec3::new(1.0, 1.0, 1.0));
        assert_close(p.x, 0.0);
        assert_close(p.y, 0.0);
        assert!(p.z > 0.0 && p.z < 1.0);
    }

    // --- caching ---

    #[test]
    fn matrices_are_cached_after_first_get() {
        let camera = Camera::perspective();
        let proj = camera.get_projection();
        let view = camera.get_view();

        assert_eq!(camera.projection_matrix.get().copied(), Some(proj));
        assert_eq!(camera.view_matrix.get().copied(), Some(view));
        assert_eq!(camera.get_projection(), proj);
        assert_eq!(camera.get_view(), view);
    }

    /// Checks that `change` drops the cached projection but keeps the cached view.
    fn assert_invalidates_projection_only(mut camera: Camera, change: impl FnOnce(&mut Camera)) {
        let before = camera.get_projection();
        camera.get_view();

        change(&mut camera);

        assert!(camera.projection_matrix.get().is_none());
        assert!(camera.view_matrix.get().is_some());
        assert_ne!(camera.get_projection(), before);
    }

    /// Checks that `change` drops the cached view but keeps the cached projection.
    fn assert_invalidates_view_only(change: impl FnOnce(&mut Camera)) {
        let mut camera = Camera::perspective();
        camera.get_projection();
        let before = camera.get_view();

        change(&mut camera);

        assert!(camera.view_matrix.get().is_none());
        assert!(camera.projection_matrix.get().is_some());
        assert_ne!(camera.get_view(), before);
    }

    #[test]
    fn projection_setters_invalidate_projection() {
        let perspective = Camera::perspective;
        assert_invalidates_projection_only(perspective(), |c| {
            c.set_fov(60.0);
        });
        assert_invalidates_projection_only(perspective(), |c| {
            c.set_near(0.5);
        });
        assert_invalidates_projection_only(perspective(), |c| {
            c.set_far(50.0);
        });
        assert_invalidates_projection_only(perspective(), |c| {
            c.set_near_far(0.5, 50.0);
        });
        assert_invalidates_projection_only(perspective(), |c| {
            c.set_screen_dimention(800, 800);
        });

        let orthographic = || Camera::orthographic(ScreenOrigin::Center, ScreenUnits::Pixels);
        assert_invalidates_projection_only(orthographic(), |c| {
            c.set_origin(ScreenOrigin::TopLeft);
        });
        assert_invalidates_projection_only(orthographic(), |c| {
            c.set_units(ScreenUnits::Height(10.0));
        });
        assert_invalidates_projection_only(orthographic(), |c| {
            c.set_near_far(-10.0, 10.0);
        });
        assert_invalidates_projection_only(orthographic(), |c| {
            c.set_screen_dimention(800, 800);
        });
    }

    #[test]
    fn view_setters_invalidate_view() {
        assert_invalidates_view_only(|c| {
            c.set_eye_position(Point3::new(5.0, 0.0, 0.0));
        });
        assert_invalidates_view_only(|c| {
            c.set_target_position(Point3::new(1.0, 0.0, 0.0));
        });
        assert_invalidates_view_only(|c| {
            c.set_up_vector(Vec3::new(0.0, 0.0, 1.0));
        });
        assert_invalidates_view_only(|c| {
            c.set_view(
                Point3::new(5.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            );
        });
    }

    #[test]
    fn setting_same_values_keeps_cache() {
        let mut camera = Camera::perspective();
        camera.get_projection();
        camera.get_view();

        camera
            .set_projection(Projection::Perspective)
            .set_fov(45.0)
            .set_near(0.1)
            .set_far(10.0)
            .set_near_far(0.1, 10.0)
            .set_screen_dimention(1920, 1080)
            .set_origin(ScreenOrigin::Center)
            .set_units(ScreenUnits::Pixels)
            .set_eye_position(Point3::new(2.0, 2.0, 2.0))
            .set_target_position(Point3::new(0.0, 0.0, 0.0))
            .set_up_vector(Vec3::new(0.0, 1.0, 0.0))
            .set_view(
                Point3::new(2.0, 2.0, 2.0),
                Point3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            );

        assert!(camera.projection_matrix.get().is_some());
        assert!(camera.view_matrix.get().is_some());
    }

    #[test]
    fn individual_setters_update_fields() {
        let mut camera = Camera::perspective();
        camera
            .set_near(0.2)
            .set_far(20.0)
            .set_eye_position(Point3::new(1.0, 1.0, 1.0))
            .set_target_position(Point3::new(0.0, 1.0, 0.0))
            .set_up_vector(Vec3::new(0.0, 0.0, 1.0));

        assert_eq!(camera.get_near(), 0.2);
        assert_eq!(camera.get_far(), 20.0);
        assert_eq!(camera.get_eye_position(), Point3::new(1.0, 1.0, 1.0));
        assert_eq!(camera.get_target_position(), Point3::new(0.0, 1.0, 0.0));
        assert_eq!(camera.get_up_vector(), Vec3::new(0.0, 0.0, 1.0));
    }
}
