use anyhow::{bail, Result};

use super::math::{Vec2, Vec3};
use super::mesh::Mesh;
use super::scene::Scene;
use super::vertex::Vertex;

/// A flat rectangle in the XY plane (z = 0), facing +Z, split into a grid of
/// `segments.0 x segments.1` cells so that vertex shaders can bend it.
///
/// Texture coordinates cover the whole texture the way an image is seen from +Z: `u` goes from
/// 0 at `x.0` to 1 at `x.1`, and `v` from 0 at the top edge (`y.1`) to 1 at the bottom (`y.0`).
pub struct MeshBuilderPlane {
    x: (f32, f32),
    y: (f32, f32),
    segments: (u32, u32),
}

impl MeshBuilderPlane {
    /// A rectangle spanning `x.0..x.1` and `y.0..y.1`, split into `segments.0` columns and
    /// `segments.1` rows.
    pub fn new(x: (f32, f32), y: (f32, f32), segments: (u32, u32)) -> Self {
        Self { x, y, segments }
    }

    /// Builds the mesh, named `name`, adds it to `scene` and returns its handle. Fails if a
    /// segment count is 0 or the rectangle is empty.
    pub fn build(self, scene: &mut Scene, name: Option<String>) -> Result<usize> {
        let (columns, rows) = self.segments;
        if columns == 0 || rows == 0 {
            bail!("a plane needs at least 1 segment in each direction, got {columns}x{rows}");
        }
        if self.x.0 == self.x.1 || self.y.0 == self.y.1 {
            bail!("a plane can't be empty: x {:?}, y {:?}", self.x, self.y);
        }
        // More vertices than u32 indices can address.
        let count = (columns as u64 + 1) * (rows as u64 + 1);
        if count > u32::MAX as u64 {
            bail!("a plane with {columns}x{rows} segments has too many vertices");
        }

        let normal = Vec3::new(0.0, 0.0, 1.0);
        let mut vertices = Vec::with_capacity(count as usize);
        for row in 0..=rows {
            // Row 0 is the top edge.
            let v = row as f32 / rows as f32;
            let y = self.y.1 + (self.y.0 - self.y.1) * v;
            for column in 0..=columns {
                let u = column as f32 / columns as f32;
                let x = self.x.0 + (self.x.1 - self.x.0) * u;
                vertices.push(Vertex::new(Vec3::new(x, y, 0.0), normal, Vec2::new(u, v)));
            }
        }

        let mut indices = Vec::with_capacity((columns * rows * 6) as usize);
        let at = |column: u32, row: u32| row * (columns + 1) + column;
        for row in 0..rows {
            for column in 0..columns {
                let top_left = at(column, row);
                let top_right = at(column + 1, row);
                let bottom_left = at(column, row + 1);
                let bottom_right = at(column + 1, row + 1);
                // Counter-clockwise seen from +Z (when x.0 < x.1 and y.0 < y.1).
                indices.extend([bottom_left, bottom_right, top_right]);
                indices.extend([bottom_left, top_right, top_left]);
            }
        }

        Ok(scene.add_mesh(Mesh::new(name, vertices, indices)))
    }
}

impl Default for MeshBuilderPlane {
    /// A 1x1 square centred on the origin, as one cell.
    fn default() -> Self {
        Self::new((-0.5, 0.5), (-0.5, 0.5), (1, 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::Object;

    fn build(builder: MeshBuilderPlane) -> (Vec<Vertex>, Vec<u32>) {
        let mut scene = Scene::new();
        let mesh = builder
            .build(&mut scene, Some("plane".to_string()))
            .unwrap();
        let mesh = scene.get_mesh(mesh).unwrap();
        assert_eq!(mesh.get_name(), Some("plane".to_string()));
        (mesh.vertices.clone(), mesh.indices.clone())
    }

    #[test]
    fn default_is_one_unit_square() {
        let (vertices, indices) = build(MeshBuilderPlane::default());
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 6);
        let corners: Vec<_> = vertices
            .iter()
            .map(|v| (v.pos.x, v.pos.y, v.tex_coord.x, v.tex_coord.y))
            .collect();
        assert_eq!(
            corners,
            [
                (-0.5, 0.5, 0.0, 0.0),
                (0.5, 0.5, 1.0, 0.0),
                (-0.5, -0.5, 0.0, 1.0),
                (0.5, -0.5, 1.0, 1.0),
            ]
        );
        for v in &vertices {
            assert_eq!((v.pos.z, v.normal), (0.0, Vec3::new(0.0, 0.0, 1.0)));
        }
    }

    #[test]
    fn grid_has_a_vertex_per_corner_and_two_triangles_per_cell() {
        let (vertices, indices) = build(MeshBuilderPlane::new((0.0, 3.0), (-2.0, 0.0), (3, 4)));
        assert_eq!(vertices.len(), 4 * 5);
        assert_eq!(indices.len(), 3 * 4 * 6);
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
        // Top left and bottom right corners.
        assert_eq!(vertices[0].pos, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(vertices[19].pos, Vec3::new(3.0, -2.0, 0.0));
        assert_eq!(vertices[19].tex_coord, Vec2::new(1.0, 1.0));
    }

    #[test]
    fn triangles_face_positive_z() {
        let (vertices, indices) = build(MeshBuilderPlane::new((0.0, 2.0), (0.0, 1.0), (2, 3)));
        for triangle in indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| vertices[triangle[k] as usize].pos);
            let normal = (b - a).cross(c - a);
            assert!(normal.z > 0.0, "{triangle:?} is not counter-clockwise");
        }
    }

    #[test]
    fn invalid_planes_fail() {
        let mut scene = Scene::new();
        for builder in [
            MeshBuilderPlane::new((0.0, 1.0), (0.0, 1.0), (0, 1)),
            MeshBuilderPlane::new((0.0, 1.0), (0.0, 1.0), (1, 0)),
            MeshBuilderPlane::new((1.0, 1.0), (0.0, 1.0), (1, 1)),
            MeshBuilderPlane::new((0.0, 1.0), (2.0, 2.0), (1, 1)),
        ] {
            assert!(builder.build(&mut scene, None).is_err());
        }
        assert_eq!(scene.meshes.iter().count(), 0);
    }
}
