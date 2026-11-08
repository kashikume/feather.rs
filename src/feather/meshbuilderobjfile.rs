use std::collections::HashMap;

use super::{
    assetsprovider::AssetsProvider,
    math::{Vec2, Vec3},
    mesh::Mesh,
    scene::Scene,
    vertex::Vertex,
};
use anyhow::{Context, Result};

/// Builds a mesh from a Wavefront OBJ file. Materials (`.mtl`) are ignored.
pub struct MeshBuilderObjFile {
    obj: Vec<u8>,
}

impl MeshBuilderObjFile {
    /// Reads the OBJ asset `asset` (e.g. `"viking_room.obj"`) from `assets`.
    pub fn new(assets: &dyn AssetsProvider, asset: &str) -> Result<Self> {
        Ok(Self::from_bytes(assets.get_asset(asset)?))
    }

    /// Uses OBJ data already in memory.
    pub fn from_bytes(obj: Vec<u8>) -> Self {
        Self { obj }
    }

    pub fn build(self, scene: &mut Scene) -> Result<usize> {
        let (models, _) = tobj::load_obj_buf(
            &mut self.obj.as_slice(),
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |_| Ok(Default::default()),
        )
        .context("Cannot parse OBJ")?;

        // Vertices / Indices

        let mut unique_vertices = HashMap::new();
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for model in &models {
            for index in &model.mesh.indices {
                let pos_offset = (3 * index) as usize;
                let tex_coord_offset = (2 * index) as usize;
                let normals_offset = (3 * index) as usize;

                let vertex = Vertex::new(
                    Vec3 {
                        x: model.mesh.positions[pos_offset],
                        y: model.mesh.positions[pos_offset + 1],
                        z: model.mesh.positions[pos_offset + 2],
                    },
                    Vec3 {
                        x: model.mesh.normals[normals_offset],
                        y: model.mesh.normals[normals_offset + 1],
                        z: model.mesh.normals[normals_offset + 2],
                    },
                    Vec2 {
                        x: model.mesh.texcoords[tex_coord_offset],
                        y: 1.0 - model.mesh.texcoords[tex_coord_offset + 1],
                    },
                );

                if let Some(index) = unique_vertices.get(&vertex) {
                    indices.push(*index as u32);
                } else {
                    let index = vertices.len();
                    unique_vertices.insert(vertex, index);
                    vertices.push(vertex);
                    indices.push(index as u32);
                }
            }
        }

        let mesh = Mesh::new(None, vertices, indices);
        Ok(scene.add_mesh(mesh))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feather::assetsprovider::tests::test_resources;

    /// One triangle with texture coordinates and normals.
    const TRIANGLE: &str = "v 0 0 0
v 1 0 0
v 0 1 0
vt 0 0
vt 1 0
vt 0 1
vn 0 0 1
f 1/1/1 2/2/1 3/3/1
";

    #[test]
    fn builds_mesh_from_bytes() {
        let mut scene = Scene::new();
        let mesh = MeshBuilderObjFile::from_bytes(TRIANGLE.as_bytes().to_vec())
            .build(&mut scene)
            .unwrap();

        let mesh = scene.get_mesh(mesh).unwrap();
        assert_eq!(mesh.gen_num_vertexes(), 3);
        assert_eq!(mesh.gen_num_indexes(), 3);
    }

    #[test]
    fn shared_vertices_are_merged() {
        // Two triangles of a quad sharing two vertices.
        let quad = "v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
vt 0 0
vt 1 0
vt 1 1
vt 0 1
vn 0 0 1
f 1/1/1 2/2/1 3/3/1
f 1/1/1 3/3/1 4/4/1
";
        let mut scene = Scene::new();
        let mesh = MeshBuilderObjFile::from_bytes(quad.as_bytes().to_vec())
            .build(&mut scene)
            .unwrap();

        let mesh = scene.get_mesh(mesh).unwrap();
        assert_eq!(mesh.gen_num_vertexes(), 4);
        assert_eq!(mesh.gen_num_indexes(), 6);
    }

    #[test]
    fn loads_asset() {
        let mut scene = Scene::new();
        let mesh = MeshBuilderObjFile::new(&*test_resources(), "viking_room.obj")
            .unwrap()
            .build(&mut scene)
            .unwrap();

        let mesh = scene.get_mesh(mesh).unwrap();
        assert!(mesh.gen_num_vertexes() > 0);
        assert!(mesh.gen_num_indexes() > 0);
        assert_eq!(mesh.gen_num_indexes() % 3, 0);
    }

    #[test]
    fn missing_asset_fails() {
        assert!(MeshBuilderObjFile::new(&*test_resources(), "does_not_exist.obj").is_err());
    }

    #[test]
    fn invalid_obj_fails() {
        let mut scene = Scene::new();
        let result = MeshBuilderObjFile::from_bytes(b"f 1 2 3
".to_vec()).build(&mut scene);
        assert!(result.is_err());
    }
}
