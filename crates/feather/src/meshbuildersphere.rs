use std::f32::consts::{PI, TAU};

use anyhow::{bail, Result};

use super::math::{Vec2, Vec3};
use super::mesh::Mesh;
use super::scene::Scene;
use super::vertex::Vertex;

/// Radius of `MeshBuilderSphere::default()`: a ball of diameter 1, like the default cuboid.
pub const DEFAULT_RADIUS: f32 = 0.5;
/// Segments of `MeshBuilderSphere::default()`.
pub const DEFAULT_SEGMENTS: u32 = 48;
/// Rings of `MeshBuilderSphere::default()`.
pub const DEFAULT_RINGS: u32 = 24;

/// A ball (UV sphere) centred on the origin, with its poles on the Z axis.
///
/// The surface is split into `segments` slices around the Z axis and `rings` bands from pole to
/// pole. Texture coordinates wrap an equirectangular image (such as a world map) around it:
/// `u` goes from 0 to 1 eastwards, starting at -X (longitude -180° when +X is longitude 0),
/// and `v` from 0 at the north pole (+Z) to 1 at the south pole. Normals point outward.
pub struct MeshBuilderSphere {
    radius: f32,
    segments: u32,
    rings: u32,
}

impl MeshBuilderSphere {
    /// A ball of `radius`, split into `segments` slices around the Z axis (at least 3) and
    /// `rings` bands from pole to pole (at least 2).
    pub fn new(radius: f32, segments: u32, rings: u32) -> Self {
        Self {
            radius,
            segments,
            rings,
        }
    }

    /// Builds the mesh, named `name`, adds it to `scene` and returns its handle. Fails if the
    /// radius isn't positive or there are too few segments or rings.
    pub fn build(self, scene: &mut Scene, name: Option<String>) -> Result<usize> {
        let (segments, rings) = (self.segments, self.rings);
        if !(self.radius > 0.0) {
            bail!("a sphere needs a positive radius, got {}", self.radius);
        }
        if segments < 3 || rings < 2 {
            bail!("a sphere needs at least 3 segments and 2 rings, got {segments} and {rings}");
        }
        let count = (segments as u64 + 1) * (rings as u64 + 1);
        if count > u32::MAX as u64 {
            bail!("a sphere with {segments} segments and {rings} rings has too many vertices");
        }

        // One vertex per (ring, segment) corner; the first and last column share positions
        // but not texture coordinates (the seam), and every corner of a pole row is the pole.
        let mut vertices = Vec::with_capacity(count as usize);
        for ring in 0..=rings {
            let v = ring as f32 / rings as f32;
            // 0 at the north pole, PI at the south pole.
            let polar = v * PI;
            for segment in 0..=segments {
                let u = segment as f32 / segments as f32;
                // Longitude -180° (u = 0) is -X; it grows counter-clockwise seen from +Z.
                let azimuth = u * TAU + PI;
                let normal = Vec3::new(
                    polar.sin() * azimuth.cos(),
                    polar.sin() * azimuth.sin(),
                    polar.cos(),
                );
                vertices.push(Vertex::new(normal * self.radius, normal, Vec2::new(u, v)));
            }
        }

        let mut indices = Vec::new();
        let at = |ring: u32, segment: u32| ring * (segments + 1) + segment;
        for ring in 0..rings {
            for segment in 0..segments {
                // Seen from outside, north up: `top_left` is west of `top_right`.
                let top_left = at(ring, segment);
                let top_right = at(ring, segment + 1);
                let bottom_left = at(ring + 1, segment);
                let bottom_right = at(ring + 1, segment + 1);
                // Counter-clockwise seen from outside. The top row of the first ring and the
                // bottom row of the last are points (the poles), so one triangle of each of
                // their cells would have no area.
                if ring != 0 {
                    indices.extend([bottom_left, top_right, top_left]);
                }
                if ring != rings - 1 {
                    indices.extend([bottom_left, bottom_right, top_right]);
                }
            }
        }

        Ok(scene.add_mesh(Mesh::new(name, vertices, indices)))
    }
}

impl Default for MeshBuilderSphere {
    /// A ball of diameter 1 (like the default cuboid), with 48 segments and 24 rings.
    fn default() -> Self {
        Self::new(DEFAULT_RADIUS, DEFAULT_SEGMENTS, DEFAULT_RINGS)
    }
}

#[cfg(test)]
mod tests {
    use cgmath::InnerSpace;

    use super::*;

    fn build(builder: MeshBuilderSphere) -> (Vec<Vertex>, Vec<u32>) {
        let mut scene = Scene::new();
        let mesh = builder.build(&mut scene, None).unwrap();
        let mesh = scene.get_mesh(mesh).unwrap();
        (mesh.vertices.clone(), mesh.indices.clone())
    }

    #[test]
    fn vertices_lie_on_the_sphere_with_outward_normals() {
        let (vertices, indices) = build(MeshBuilderSphere::new(2.0, 8, 4));
        assert_eq!(vertices.len(), 9 * 5);
        // Every cell has 2 triangles, except the cells touching a pole.
        assert_eq!(indices.len(), (8 * 4 * 2 - 8 * 2) * 3);
        for v in &vertices {
            assert!((v.pos.magnitude() - 2.0).abs() < 1e-5, "{:?}", v.pos);
            assert!((v.normal - v.pos / 2.0).magnitude() < 1e-6, "{v:?}");
        }
    }

    #[test]
    fn triangles_face_outward() {
        let (vertices, indices) = build(MeshBuilderSphere::new(1.0, 12, 6));
        for triangle in indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| vertices[triangle[k] as usize]);
            let facing = (b.pos - a.pos).cross(c.pos - a.pos);
            assert!(facing.magnitude() > 1e-6, "{triangle:?} has no area");
            let centre = (a.pos + b.pos + c.pos) / 3.0;
            assert!(facing.dot(centre) > 0.0, "{triangle:?} faces inward");
        }
    }

    #[test]
    fn texture_wraps_like_a_world_map() {
        let (vertices, _) = build(MeshBuilderSphere::new(1.0, 4, 2));
        let find = |u: f32, v: f32| {
            vertices
                .iter()
                .find(|vertex| vertex.tex_coord == Vec2::new(u, v))
                .unwrap()
                .pos
        };
        let close = |a: Vec3, b: Vec3| (a - b).magnitude() < 1e-6;
        // North pole at the top of the image, south pole at the bottom.
        assert!(close(find(0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)));
        assert!(close(find(0.0, 1.0), Vec3::new(0.0, 0.0, -1.0)));
        // On the equator: -180° at -X, -90° at -Y, 0° at +X, 90° at +Y, 180° at -X again.
        assert!(close(find(0.0, 0.5), Vec3::new(-1.0, 0.0, 0.0)));
        assert!(close(find(0.25, 0.5), Vec3::new(0.0, -1.0, 0.0)));
        assert!(close(find(0.5, 0.5), Vec3::new(1.0, 0.0, 0.0)));
        assert!(close(find(0.75, 0.5), Vec3::new(0.0, 1.0, 0.0)));
        assert!(close(find(1.0, 0.5), Vec3::new(-1.0, 0.0, 0.0)));
    }

    #[test]
    fn default_has_diameter_one() {
        let (vertices, _) = build(MeshBuilderSphere::default());
        assert_eq!(vertices.len(), 49 * 25);
        for v in &vertices {
            assert!((v.pos.magnitude() - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn invalid_spheres_fail() {
        let mut scene = Scene::new();
        for builder in [
            MeshBuilderSphere::new(0.0, 8, 4),
            MeshBuilderSphere::new(-1.0, 8, 4),
            MeshBuilderSphere::new(f32::NAN, 8, 4),
            MeshBuilderSphere::new(1.0, 2, 4),
            MeshBuilderSphere::new(1.0, 8, 1),
        ] {
            assert!(builder.build(&mut scene, None).is_err());
        }
        assert_eq!(scene.meshes.iter().count(), 0);
    }
}
