//! Bounds and view frustums, for frustum culling.
//!
//! Every mesh has a box around its vertices in its own space (`Mesh::get_bounds`). A node is
//! drawn only when that box, moved by the node's global transform, may be inside the view of
//! the camera (`Frustum`); a shadow caster only when it may be inside the shadow light's view.
//! The test is conservative: a box that is partly in view, or that the test can't rule out, is
//! drawn, so nothing visible is ever left out.

use cgmath::InnerSpace;

use super::math::{Mat4, Vec3, Vec4};

/// An axis-aligned box: every point with `min <= p <= max` on each axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// The smallest box holding all `points`; `None` when there are none.
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let mut aabb = Self::new(first, first);
        for p in points {
            aabb.min = Vec3::new(
                aabb.min.x.min(p.x),
                aabb.min.y.min(p.y),
                aabb.min.z.min(p.z),
            );
            aabb.max = Vec3::new(
                aabb.max.x.max(p.x),
                aabb.max.y.max(p.y),
                aabb.max.z.max(p.z),
            );
        }
        Some(aabb)
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Half the box's size along each axis.
    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }
}

/// The six planes bounding what a view-projection matrix shows. Each plane `(a, b, c, d)` has
/// the inside where `a·x + b·y + c·z + d >= 0`, with `(a, b, c)` of unit length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// The frustum of `view_projection` (world space to Vulkan's clip space: `-w <= x <= w`,
    /// `-w <= y <= w`, `0 <= z <= w`). Works for perspective and orthographic projections.
    pub fn from_matrix(view_projection: &Mat4) -> Self {
        let m = view_projection;
        // Rows of the matrix (cgmath stores columns).
        let row = |i: usize| Vec4::new(m.x[i], m.y[i], m.z[i], m.w[i]);
        let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
        let planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2].map(|p| {
            let length = p.truncate().magnitude();
            if length > 0.0 {
                p / length
            } else {
                p
            }
        });
        Self { planes }
    }

    /// Whether the box `bounds`, in the space that `model` takes to world space, may be inside
    /// the frustum. `false` only when the box is entirely outside one of the planes. Exact for
    /// rotated and non-uniformly scaled boxes (the box itself is tested, not a box around it).
    pub fn intersects(&self, bounds: &Aabb, model: &Mat4) -> bool {
        let center = model * bounds.center().extend(1.0);
        let h = bounds.half_extents();
        // The box's axes in world space, scaled by its half-extents.
        let axes = [
            model.x.truncate() * h.x,
            model.y.truncate() * h.y,
            model.z.truncate() * h.z,
        ];
        self.planes.iter().all(|plane| {
            let n = plane.truncate();
            let distance = n.dot(center.truncate()) + plane.w;
            let reach: f32 = axes.iter().map(|axis| n.dot(*axis).abs()).sum();
            distance >= -reach
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Point3;
    use cgmath::{Deg, SquareMatrix};

    /// The same correction as the camera's: y down, depth 0..1.
    fn vulkan(projection: Mat4) -> Mat4 {
        #[rustfmt::skip]
        let correction = Mat4::new(
            1.0, 0.0, 0.0, 0.0,
            0.0, -1.0, 0.0, 0.0,
            0.0, 0.0, 0.5, 0.0,
            0.0, 0.0, 0.5, 1.0,
        );
        correction * projection
    }

    /// A camera at the origin looking down -Z, 90° field of view, square, near 1, far 100.
    fn perspective() -> Frustum {
        let proj = vulkan(cgmath::perspective(Deg(90.0), 1.0, 1.0, 100.0));
        let view = Mat4::look_at_rh(
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        Frustum::from_matrix(&(proj * view))
    }

    fn unit_box() -> Aabb {
        Aabb::new(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5))
    }

    fn at(x: f32, y: f32, z: f32) -> Mat4 {
        Mat4::from_translation(Vec3::new(x, y, z))
    }

    #[test]
    fn from_points_takes_the_extremes() {
        let aabb = Aabb::from_points([
            Vec3::new(1.0, -2.0, 3.0),
            Vec3::new(-1.0, 4.0, 0.0),
            Vec3::new(0.0, 0.0, 5.0),
        ])
        .unwrap();
        assert_eq!(aabb.min, Vec3::new(-1.0, -2.0, 0.0));
        assert_eq!(aabb.max, Vec3::new(1.0, 4.0, 5.0));
        assert_eq!(aabb.center(), Vec3::new(0.0, 1.0, 2.5));
        assert_eq!(aabb.half_extents(), Vec3::new(1.0, 3.0, 2.5));
    }

    #[test]
    fn from_no_points_is_none() {
        assert_eq!(Aabb::from_points([]), None);
    }

    #[test]
    fn box_in_front_is_inside() {
        assert!(perspective().intersects(&unit_box(), &at(0.0, 0.0, -10.0)));
    }

    #[test]
    fn box_behind_the_camera_is_outside() {
        assert!(!perspective().intersects(&unit_box(), &at(0.0, 0.0, 10.0)));
    }

    #[test]
    fn box_outside_each_side_is_outside() {
        let f = perspective();
        // At distance 10 the view is 10 to each side (90° field of view).
        for (x, y) in [(12.0, 0.0), (-12.0, 0.0), (0.0, 12.0), (0.0, -12.0)] {
            assert!(!f.intersects(&unit_box(), &at(x, y, -10.0)), "({x}, {y})");
        }
    }

    #[test]
    fn box_before_near_or_beyond_far_is_outside() {
        let f = perspective();
        assert!(!f.intersects(&unit_box(), &at(0.0, 0.0, -0.2)));
        assert!(!f.intersects(&unit_box(), &at(0.0, 0.0, -101.0)));
    }

    #[test]
    fn box_straddling_a_plane_is_inside() {
        let f = perspective();
        // Centre just outside the right plane, but the box reaches in.
        assert!(f.intersects(&unit_box(), &at(10.3, 0.0, -10.0)));
        // Across the near and far planes.
        assert!(f.intersects(&unit_box(), &at(0.0, 0.0, -0.8)));
        assert!(f.intersects(&unit_box(), &at(0.0, 0.0, -100.3)));
    }

    #[test]
    fn scaled_box_reaches_into_view() {
        let f = perspective();
        // Centre far outside to the right, but stretched along X it reaches in.
        let model = at(20.0, 0.0, -10.0) * Mat4::from_nonuniform_scale(25.0, 1.0, 1.0);
        assert!(f.intersects(&unit_box(), &model));
        // Stretched along Y instead it doesn't.
        let model = at(20.0, 0.0, -10.0) * Mat4::from_nonuniform_scale(1.0, 25.0, 1.0);
        assert!(!f.intersects(&unit_box(), &model));
    }

    #[test]
    fn rotated_box_is_tested_as_rotated() {
        let f = perspective();
        // A long thin box (20 along X) whose centre is 15 to the right, outside the view.
        let long = Aabb::new(Vec3::new(-10.0, -0.1, -0.1), Vec3::new(10.0, 0.1, 0.1));
        // Lying along X it reaches into the view...
        assert!(f.intersects(&long, &at(15.0, 0.0, -10.0)));
        // ...turned to point along Y it doesn't.
        let turned = at(15.0, 0.0, -10.0) * Mat4::from_angle_z(Deg(90.0));
        assert!(!f.intersects(&long, &turned));
    }

    #[test]
    fn orthographic_frustum() {
        // Like a UI camera: x 0..800, y 0..600, depth -1..1.
        let proj = vulkan(cgmath::ortho(0.0, 800.0, 0.0, 600.0, -1.0, 1.0));
        let f = Frustum::from_matrix(&proj);
        let glyph = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 20.0, 0.0));
        assert!(f.intersects(&glyph, &at(100.0, 100.0, 0.0)));
        assert!(f.intersects(&glyph, &at(-5.0, 100.0, 0.0)));
        assert!(!f.intersects(&glyph, &at(-11.0, 100.0, 0.0)));
        assert!(!f.intersects(&glyph, &at(100.0, 601.0, 0.0)));
        assert!(!f.intersects(&glyph, &at(100.0, 100.0, 2.0)));
    }

    #[test]
    fn flat_box_is_tested() {
        // A box with no thickness (a plane, a text quad) still works.
        let f = perspective();
        let flat = Aabb::new(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 0.0));
        assert!(f.intersects(&flat, &at(0.0, 0.0, -5.0)));
        assert!(!f.intersects(&flat, &at(0.0, 0.0, 5.0)));
    }

    #[test]
    fn identity_frustum_is_the_clip_volume() {
        let f = Frustum::from_matrix(&Mat4::identity());
        let small = Aabb::new(Vec3::new(-0.1, -0.1, 0.0), Vec3::new(0.1, 0.1, 0.1));
        assert!(f.intersects(&small, &at(0.0, 0.0, 0.5)));
        assert!(!f.intersects(&small, &at(0.0, 0.0, -0.5)));
        assert!(!f.intersects(&small, &at(2.0, 0.0, 0.5)));
    }
}
