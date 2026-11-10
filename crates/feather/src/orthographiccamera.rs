use cgmath::SquareMatrix;

use super::camera::Camera;
use super::math::Mat4;

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

/// A flat camera for 2D scenes: it shows the XY plane without perspective, looking down -Z,
/// with Y up and X to the right, like the rest of the engine. Objects between z = `-depth` and
/// z = `depth` are visible; the nearer ones (greater z) are drawn over the farther ones.
///
/// What part of the plane it shows depends on its `ScreenOrigin` and `ScreenUnits`, and on the
/// size of the rendered image, which the renderer sets every frame (`set_screen_dimention`).
pub struct OrthographicCamera {
    origin: ScreenOrigin,
    units: ScreenUnits,
    depth: f32,
    screen_width: u32,
    screen_height: u32,
}

impl OrthographicCamera {
    /// A camera with its origin at `origin`, showing `units`, for objects between z = -1000
    /// and z = 1000.
    pub fn new(origin: ScreenOrigin, units: ScreenUnits) -> Self {
        Self {
            origin,
            units,
            depth: 1000.0,
            screen_width: 1920,
            screen_height: 1080,
        }
    }

    /// Shows objects between z = `-depth` and z = `depth`.
    pub fn set_depth(&mut self, depth: f32) -> &mut Self {
        self.depth = depth;
        self
    }

    pub fn set_origin(&mut self, origin: ScreenOrigin) -> &mut Self {
        self.origin = origin;
        self
    }

    pub fn set_units(&mut self, units: ScreenUnits) -> &mut Self {
        self.units = units;
        self
    }

    /// The width and height of the screen in scene units.
    pub fn get_visible_size(&self) -> (f32, f32) {
        let (width, height) = (self.screen_width as f32, self.screen_height.max(1) as f32);
        match self.units {
            ScreenUnits::Pixels => (width, height),
            ScreenUnits::Height(units) => (units * width / height, units),
        }
    }

    /// The visible area: (left, right, bottom, top) in scene units.
    pub fn get_visible_area(&self) -> (f32, f32, f32, f32) {
        let (width, height) = self.get_visible_size();
        match self.origin {
            ScreenOrigin::TopLeft => (0.0, width, -height, 0.0),
            ScreenOrigin::Center => (-width / 2.0, width / 2.0, -height / 2.0, height / 2.0),
            ScreenOrigin::BottomLeft => (0.0, width, 0.0, height),
        }
    }
}

impl Camera for OrthographicCamera {
    fn get_projection(&self) -> Mat4 {
        let (left, right, bottom, top) = self.get_visible_area();
        // Vulkan's clip space has Y down and depth 0..1; cgmath's has Y up and depth -1..1.
        #[rustfmt::skip]
        let correction = Mat4::new(
            1.0,  0.0, 0.0, 0.0,
            0.0, -1.0, 0.0, 0.0,
            0.0,  0.0, 0.5, 0.0,
            0.0,  0.0, 0.5, 1.0,
        );
        correction * cgmath::ortho(left, right, bottom, top, -self.depth, self.depth)
    }

    fn get_view(&self) -> Mat4 {
        Mat4::identity()
    }

    fn set_screen_dimention(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Vec3, Vec4};

    /// Where `p` ends up in Vulkan's normalized device coordinates.
    fn project(camera: &OrthographicCamera, p: Vec3) -> Vec3 {
        let clip = camera.get_projection() * camera.get_view() * Vec4::new(p.x, p.y, p.z, 1.0);
        clip.truncate() / clip.w
    }

    fn assert_close(a: Vec3, b: Vec3) {
        assert!(
            (a - b).x.abs() < 1e-5 && (a - b).y.abs() < 1e-5 && (a - b).z.abs() < 1e-5,
            "{a:?} != {b:?}"
        );
    }

    #[test]
    fn pixels_from_the_top_left() {
        let mut camera = OrthographicCamera::new(ScreenOrigin::TopLeft, ScreenUnits::Pixels);
        camera.set_screen_dimention(800, 600);
        assert_eq!(camera.get_visible_area(), (0.0, 800.0, -600.0, 0.0));
        // Vulkan: x -1 on the left, y -1 at the top.
        assert_close(
            project(&camera, Vec3::new(0.0, 0.0, 0.0)),
            Vec3::new(-1.0, -1.0, 0.5),
        );
        assert_close(
            project(&camera, Vec3::new(800.0, -600.0, 0.0)),
            Vec3::new(1.0, 1.0, 0.5),
        );
    }

    #[test]
    fn fixed_height_keeps_the_aspect_ratio() {
        let mut camera = OrthographicCamera::new(ScreenOrigin::Center, ScreenUnits::Height(10.0));
        camera.set_screen_dimention(1600, 800);
        assert_eq!(camera.get_visible_size(), (20.0, 10.0));
        assert_eq!(camera.get_visible_area(), (-10.0, 10.0, -5.0, 5.0));
        assert_close(
            project(&camera, Vec3::new(10.0, 5.0, 0.0)),
            Vec3::new(1.0, -1.0, 0.5),
        );

        camera.set_origin(ScreenOrigin::BottomLeft);
        assert_eq!(camera.get_visible_area(), (0.0, 20.0, 0.0, 10.0));
    }

    #[test]
    fn nearer_objects_have_smaller_depth() {
        let mut camera = OrthographicCamera::new(ScreenOrigin::Center, ScreenUnits::Pixels);
        camera.set_depth(100.0);
        let near = project(&camera, Vec3::new(0.0, 0.0, 100.0)).z;
        let far = project(&camera, Vec3::new(0.0, 0.0, -100.0)).z;
        assert!(
            (near - 0.0).abs() < 1e-5 && (far - 1.0).abs() < 1e-5,
            "{near} {far}"
        );
    }
}
