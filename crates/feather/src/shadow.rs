//! Shadows: the light that casts them.
//!
//! A scene with a `ShadowLight` (`Scene::set_shadow_light`) is first drawn from the light into
//! a shadow map, a depth image: only the nodes that cast shadows (`Node::set_casts_shadow`),
//! with a built-in shader. Shaders that receive shadows name a `mat4` uniform for the light's
//! view-projection matrix and a sampler for the shadow map (`ShaderUniformNames::with_shadow`);
//! the engine fills both. A fragment is in shadow when its depth seen from the light is greater
//! than the depth stored in the map at its place:
//!
//! ```glsl
//! vec4 light = lightViewProj * worldPosition;     // in the vertex shader
//! vec3 p = light.xyz / light.w;                    // in the fragment shader
//! float lit = p.z - bias <= texture(shadowMap, p.xy * 0.5 + 0.5).r ? 1.0 : 0.0;
//! ```
//!
//! The map is cleared to depth 1 (lit), and is sampled with nearest filtering and a white
//! border, so places outside it are lit too. Scenes without a light give such shaders a 1x1
//! map that is lit everywhere.

use cgmath::{EuclideanSpace, InnerSpace};

use super::math::{Mat4, Point3, Vec3};

/// The default width and height of a shadow map, in pixels.
pub const DEFAULT_SHADOW_MAP_SIZE: u32 = 2048;

/// A light casting shadows: where shadows are computed from, and how finely.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowLight {
    /// Projection × view of the light, mapping world positions to Vulkan clip space (x and y
    /// in -1..1, y down, depth 0..1), like a camera's. The shadow map covers its view volume.
    pub view_projection: Mat4,
    /// Width and height of the shadow map, in pixels.
    pub map_size: u32,
}

impl ShadowLight {
    /// A light with the view-projection matrix `view_projection` (see the field).
    pub fn from_matrix(view_projection: Mat4) -> Self {
        Self {
            view_projection,
            map_size: DEFAULT_SHADOW_MAP_SIZE,
        }
    }

    /// A directional light, like the sun: parallel rays going in `direction` (towards the
    /// scene, need not be normalized), casting the shadows of everything within `radius` of
    /// `center`.
    pub fn directional(direction: Vec3, center: Point3, radius: f32) -> Self {
        let direction = direction.normalize();
        let eye = center - direction * (2.0 * radius);
        // Any up vector not parallel to the light.
        let up = if direction.z.abs() < 0.9 {
            Vec3::new(0.0, 0.0, 1.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let view = Mat4::look_at_rh(eye, center, up);
        let projection = vulkan_correction()
            * cgmath::ortho(-radius, radius, -radius, radius, radius, 3.0 * radius);
        Self::from_matrix(projection * view)
    }

    /// Like `directional`, with a `map_size` shadow map, and with `center` moved to the nearest
    /// point that is a whole number of shadow map texels from the origin, across the light.
    /// When the centre follows something moving (e.g. the camera), shadows then don't shimmer:
    /// the map moves by whole texels, so every caster keeps covering the same texels.
    pub fn directional_snapped(
        direction: Vec3,
        center: Point3,
        radius: f32,
        map_size: u32,
    ) -> Self {
        let map_size = map_size.max(1);
        let forward = direction.normalize();
        let up = if forward.z.abs() < 0.9 {
            Vec3::new(0.0, 0.0, 1.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        // The light's view axes, as `Mat4::look_at_rh` makes them.
        let side = forward.cross(up).normalize();
        let up = side.cross(forward);
        let texel = 2.0 * radius / map_size as f32;
        let c = center.to_vec();
        let snap = |v: f32| (v / texel).round() * texel;
        let snapped = side * snap(c.dot(side)) + up * snap(c.dot(up)) + forward * c.dot(forward);
        Self::directional(direction, Point3::from_vec(snapped), radius).with_map_size(map_size)
    }

    /// The same light with a `size` x `size` shadow map.
    pub fn with_map_size(mut self, size: u32) -> Self {
        self.map_size = size.max(1);
        self
    }

    /// Where `p` ends up in Vulkan's normalized device coordinates, seen from the light.
    #[cfg(test)]
    fn project(&self, p: Point3) -> Vec3 {
        let clip = self.view_projection * p.to_homogeneous();
        clip.truncate() / clip.w
    }
}

/// Maps cgmath's clip space (y up, depth -1..1) to Vulkan's (y down, depth 0..1).
fn vulkan_correction() -> Mat4 {
    #[rustfmt::skip]
    let correction = Mat4::new(
        1.0,  0.0, 0.0, 0.0,
        0.0, -1.0, 0.0, 0.0,
        0.0,  0.0, 0.5, 0.0,
        0.0,  0.0, 0.5, 1.0,
    );
    correction
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn directional_light_covers_the_sphere_around_its_centre() {
        let light =
            ShadowLight::directional(Vec3::new(1.0, 0.5, -2.0), Point3::new(1.0, 2.0, 0.0), 5.0);
        assert_eq!(light.map_size, DEFAULT_SHADOW_MAP_SIZE);
        // The centre is in the middle of the map, halfway in depth.
        let centre = light.project(Point3::new(1.0, 2.0, 0.0));
        assert!(
            close(centre.x, 0.0) && close(centre.y, 0.0) && close(centre.z, 0.5),
            "{centre:?}"
        );

        // Along the light, nearer the light means less depth; across it, the same depth.
        let d = Vec3::new(1.0, 0.5, -2.0).normalize();
        let nearer = light.project(Point3::new(1.0, 2.0, 0.0) - d * 4.0);
        let farther = light.project(Point3::new(1.0, 2.0, 0.0) + d * 4.0);
        assert!(nearer.z < centre.z && centre.z < farther.z);
        assert!(close(nearer.x, 0.0) && close(nearer.y, 0.0));
        assert!(nearer.z >= 0.0 && farther.z <= 1.0);
    }

    #[test]
    fn snapped_lights_move_by_whole_texels() {
        let direction = Vec3::new(-1.0, 0.7, -1.2);
        let light = |x: f32| {
            ShadowLight::directional_snapped(direction, Point3::new(x, 3.0, 0.0), 8.0, 256)
        };
        let a = light(0.0);
        assert_eq!(a.map_size, 256);
        // Wherever the centre is, a fixed point lands on the same place within a texel.
        let texel_in_ndc = 2.0 / 256.0;
        let fraction = |l: &ShadowLight| {
            let p = l.project(Point3::new(1.0, 2.0, 0.5));
            (
                (p.x / texel_in_ndc).rem_euclid(1.0),
                (p.y / texel_in_ndc).rem_euclid(1.0),
            )
        };
        let (fx, fy) = fraction(&a);
        for x in [0.013, 0.31, 1.77, -4.2] {
            let (gx, gy) = fraction(&light(x));
            let same = |f: f32, g: f32| (f - g).abs() < 1e-2 || (1.0 - (f - g).abs()) < 1e-2;
            assert!(same(fx, gx) && same(fy, gy), "{x}: {fx},{fy} vs {gx},{gy}");
        }
    }

    #[test]
    fn straight_down_light_works() {
        let light = ShadowLight::directional(Vec3::new(0.0, 0.0, -1.0), Point3::origin(), 2.0)
            .with_map_size(512);
        assert_eq!(light.map_size, 512);
        let edge = light.project(Point3::new(2.0, 0.0, 0.0));
        assert!(close(edge.x.abs().max(edge.y.abs()), 1.0), "{edge:?}");
    }
}
