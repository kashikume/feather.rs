use std::cell::Cell;
use cgmath::Deg;

use super::math::{Mat4, Vec3, Point3};
use super::camera::Camera;

pub struct PerspectiveCamera {
    projection: Cell<Option<Mat4>>,
    view: Cell<Option<Mat4>>,
    fov: f32,
    near: f32,
    far: f32,
    screen_width: u32,
    screen_height: u32,
    eye_position: Point3,
    target_position: Point3,
    up_vector: Vec3,
}

impl Camera for PerspectiveCamera {
    fn set_screen_dimention(&mut self, width: u32, height: u32) {
        PerspectiveCamera::set_screen_dimention(self, width, height);
    }

    fn get_projection(&self) -> Mat4 {
        match self.projection.get() {
            Some(projection) => projection,
            None => {
                #[rustfmt::skip]
                let correction = Mat4::new(
                    1.0,  0.0,       0.0, 0.0,
                    0.0, -1.0,       0.0, 0.0,
                    0.0,  0.0, 1.0 / 2.0, 0.0,
                    0.0,  0.0, 1.0 / 2.0, 1.0,
                );
        
                let projection = correction
                    * cgmath::perspective(
                        Deg(self.fov),
                        self.screen_width as f32
                            / self.screen_height as f32,
                        self.near,
                        self.far,
                    );
                self.projection.set(Some(projection));
                projection
            }
        }
    }

    fn get_view(&self) -> Mat4 {
        match self.view.get() {
            Some(view) => view,
            None => {
                let view = Mat4::look_at_rh(self.eye_position, self.target_position , self.up_vector);
                self.view.set(Some(view));
                view
            }
        }
    }
}

impl PerspectiveCamera {

    pub fn new() -> Self {
        Self {
            projection: std::cell::Cell::new(None),
            view: std::cell::Cell::new(None),
            fov: 45.0,
            near: 0.1,
            far: 10.0,
            screen_width: 1920,
            screen_height: 1080,
            eye_position: Point3::new(2.0, 2.0, 2.0),
            target_position: Point3::new(0.0, 0.0, 0.0),
            up_vector: Vec3::new(0.0, 1.0, 0.0),
        }
    }

    pub fn set_screen_dimention(&mut self, width: u32, height: u32) -> &mut Self {
        if width != self.screen_width || height != self.screen_height {
            self.screen_width = width;
            self.screen_height = height;
            self.projection.set(None);
        }
        self
    }

    pub fn set_fov(&mut self, fov: f32) -> &mut Self {
        if fov != self.fov {
            self.fov = fov;
            self.projection.set(None);
        }
        self
    }

    pub fn set_near_far(&mut self, near: f32, far: f32) -> &mut Self {
        if near != self.near || far != self.far {
            self.near = near;
            self.far = far;
            self.projection.set(None);
        }
        self
    }

    pub fn set_near(&mut self, near: f32) -> &mut Self {
        if near != self.near {
            self.near = near;
            self.projection.set(None);
        }
        self
    }
    
    pub fn set_far(&mut self, far: f32) -> &mut Self {
        if far != self.far {
            self.far = far;
            self.projection.set(None);
        }
        self
    }

    pub fn set_view(&mut self, eye_position: Point3, target_position: Point3, up_vector: Vec3) -> &mut Self {
        if eye_position != self.eye_position || target_position != self.target_position || up_vector != self.up_vector {
            self.eye_position = eye_position;
            self.target_position = target_position;
            self.up_vector = up_vector;
            self.view.set(None);
        }
        self
    }

    pub fn set_eye_position(&mut self, eye_position: Point3) -> &mut Self {
        if eye_position != self.eye_position {
            self.eye_position = eye_position;
            self.view.set(None);
        }
        self
    }

    pub fn set_target_position(&mut self, target_position: Point3) -> &mut Self {
        if target_position != self.target_position {
            self.target_position = target_position;
            self.view.set(None);
        }
        self
    }

    pub fn set_up_vector(&mut self, up_vector: Vec3) -> &mut Self {
        if up_vector != self.up_vector {
            self.up_vector = up_vector;
            self.view.set(None);
        }
        self
    }
}

impl Default for PerspectiveCamera {
    fn default() -> Self {
        Self::new()
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

    // --- construction ---

    #[test]
    fn new_has_expected_defaults() {
        let camera = PerspectiveCamera::new();
        assert_eq!(camera.fov, 45.0);
        assert_eq!(camera.near, 0.1);
        assert_eq!(camera.far, 10.0);
        assert_eq!(camera.screen_width, 1920);
        assert_eq!(camera.screen_height, 1080);
        assert_eq!(camera.eye_position, Point3::new(2.0, 2.0, 2.0));
        assert_eq!(camera.target_position, Point3::new(0.0, 0.0, 0.0));
        assert_eq!(camera.up_vector, Vec3::new(0.0, 1.0, 0.0));
        assert!(camera.projection.get().is_none());
        assert!(camera.view.get().is_none());
    }

    #[test]
    fn default_matches_new() {
        let a = PerspectiveCamera::new();
        let b = PerspectiveCamera::default();
        assert_mat_close(a.get_projection(), b.get_projection());
        assert_mat_close(a.get_view(), b.get_view());
    }

    // --- projection ---

    #[test]
    fn projection_matches_formula() {
        let mut camera = PerspectiveCamera::new();
        camera
            .set_fov(60.0)
            .set_near_far(0.5, 100.0)
            .set_screen_dimention(800, 600);

        #[rustfmt::skip]
        let correction = Mat4::new(
            1.0,  0.0, 0.0, 0.0,
            0.0, -1.0, 0.0, 0.0,
            0.0,  0.0, 0.5, 0.0,
            0.0,  0.0, 0.5, 1.0,
        );
        let expected = correction * cgmath::perspective(Deg(60.0), 800.0 / 600.0, 0.5, 100.0);
        assert_mat_close(camera.get_projection(), expected);
    }

    #[test]
    fn projection_maps_near_and_far_to_vulkan_depth_range() {
        let mut camera = PerspectiveCamera::new();
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
        let camera = PerspectiveCamera::new();
        let proj = camera.get_projection();

        // Up in view space is down in Vulkan clip space (Y points down).
        assert!(project(proj, Vec3::new(0.0, 1.0, -5.0)).y < 0.0);
        assert!(project(proj, Vec3::new(0.0, -1.0, -5.0)).y > 0.0);
        // X is not flipped.
        assert!(project(proj, Vec3::new(1.0, 0.0, -5.0)).x > 0.0);
    }

    #[test]
    fn projection_edges_of_fov_map_to_clip_edges() {
        let mut camera = PerspectiveCamera::new();
        camera.set_fov(90.0).set_screen_dimention(200, 100);
        let proj = camera.get_projection();

        // With a 90 degree vertical FOV, a point at distance d and height d is at the top edge.
        assert_close(project(proj, Vec3::new(0.0, 3.0, -3.0)).y, -1.0);
        // Horizontally the view is wider by the aspect ratio (2:1).
        assert_close(project(proj, Vec3::new(6.0, 0.0, -3.0)).x, 1.0);
    }

    #[test]
    fn projection_depends_on_aspect_ratio() {
        let mut camera = PerspectiveCamera::new();
        camera.set_screen_dimention(100, 100);
        let square = camera.get_projection();
        camera.set_screen_dimention(200, 100);
        let wide = camera.get_projection();

        // Wider screen: X is scaled down by the aspect ratio, Y stays the same.
        assert_close(wide.x.x, square.x.x / 2.0);
        assert_close(wide.y.y, square.y.y);
    }

    // --- view ---

    #[test]
    fn view_matches_look_at() {
        let mut camera = PerspectiveCamera::new();
        let eye = Point3::new(1.0, 2.0, 3.0);
        let target = Point3::new(-1.0, 0.0, 0.5);
        let up = Vec3::new(0.0, 0.0, 1.0);
        camera.set_view(eye, target, up);

        assert_mat_close(camera.get_view(), Mat4::look_at_rh(eye, target, up));
    }

    #[test]
    fn view_puts_eye_at_origin_and_target_in_front() {
        let mut camera = PerspectiveCamera::new();
        camera.set_view(
            Point3::new(0.0, 0.0, 5.0),
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let view = camera.get_view();

        let eye = project(view, Vec3::new(0.0, 0.0, 5.0));
        assert_close(eye.magnitude(), 0.0);

        let target = project(view, Vec3::new(0.0, 0.0, 0.0));
        assert_close(target.x, 0.0);
        assert_close(target.y, 0.0);
        assert_close(target.z, -5.0);
    }

    #[test]
    fn view_is_invertible() {
        let camera = PerspectiveCamera::new();
        assert!(camera.get_view().invert().is_some());
    }

    #[test]
    fn target_projects_to_screen_center() {
        let mut camera = PerspectiveCamera::new();
        camera.set_view(
            Point3::new(3.0, -2.0, 4.0),
            Point3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        let p = project(
            camera.get_projection() * camera.get_view(),
            Vec3::new(1.0, 1.0, 1.0),
        );
        assert_close(p.x, 0.0);
        assert_close(p.y, 0.0);
        assert!(p.z > 0.0 && p.z < 1.0);
    }

    // --- caching ---

    #[test]
    fn matrices_are_cached_after_first_get() {
        let camera = PerspectiveCamera::new();
        let proj = camera.get_projection();
        let view = camera.get_view();

        assert_eq!(camera.projection.get(), Some(proj));
        assert_eq!(camera.view.get(), Some(view));
        assert_eq!(camera.get_projection(), proj);
        assert_eq!(camera.get_view(), view);
    }

    /// Checks that `change` drops the cached projection but keeps the cached view.
    fn assert_invalidates_projection_only(change: impl FnOnce(&mut PerspectiveCamera)) {
        let mut camera = PerspectiveCamera::new();
        let before = camera.get_projection();
        camera.get_view();

        change(&mut camera);

        assert!(camera.projection.get().is_none());
        assert!(camera.view.get().is_some());
        assert_ne!(camera.get_projection(), before);
    }

    /// Checks that `change` drops the cached view but keeps the cached projection.
    fn assert_invalidates_view_only(change: impl FnOnce(&mut PerspectiveCamera)) {
        let mut camera = PerspectiveCamera::new();
        camera.get_projection();
        let before = camera.get_view();

        change(&mut camera);

        assert!(camera.view.get().is_none());
        assert!(camera.projection.get().is_some());
        assert_ne!(camera.get_view(), before);
    }

    #[test]
    fn projection_setters_invalidate_projection() {
        assert_invalidates_projection_only(|c| {
            c.set_fov(60.0);
        });
        assert_invalidates_projection_only(|c| {
            c.set_near(0.5);
        });
        assert_invalidates_projection_only(|c| {
            c.set_far(50.0);
        });
        assert_invalidates_projection_only(|c| {
            c.set_near_far(0.5, 50.0);
        });
        assert_invalidates_projection_only(|c| {
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
        let mut camera = PerspectiveCamera::new();
        camera.get_projection();
        camera.get_view();

        camera
            .set_fov(45.0)
            .set_near(0.1)
            .set_far(10.0)
            .set_near_far(0.1, 10.0)
            .set_screen_dimention(1920, 1080)
            .set_eye_position(Point3::new(2.0, 2.0, 2.0))
            .set_target_position(Point3::new(0.0, 0.0, 0.0))
            .set_up_vector(Vec3::new(0.0, 1.0, 0.0))
            .set_view(
                Point3::new(2.0, 2.0, 2.0),
                Point3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            );

        assert!(camera.projection.get().is_some());
        assert!(camera.view.get().is_some());
    }

    #[test]
    fn individual_setters_update_fields() {
        let mut camera = PerspectiveCamera::new();
        camera
            .set_near(0.2)
            .set_far(20.0)
            .set_eye_position(Point3::new(1.0, 1.0, 1.0))
            .set_target_position(Point3::new(0.0, 1.0, 0.0))
            .set_up_vector(Vec3::new(0.0, 0.0, 1.0));

        assert_eq!(camera.near, 0.2);
        assert_eq!(camera.far, 20.0);
        assert_eq!(camera.eye_position, Point3::new(1.0, 1.0, 1.0));
        assert_eq!(camera.target_position, Point3::new(0.0, 1.0, 0.0));
        assert_eq!(camera.up_vector, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn works_through_camera_trait() {
        let camera = PerspectiveCamera::new();
        let as_trait: &dyn Camera = &camera;
        assert_eq!(as_trait.get_projection(), camera.get_projection());
        assert_eq!(as_trait.get_view(), camera.get_view());
    }
}
