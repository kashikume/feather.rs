use std::{collections::HashMap, fs::File, io::BufReader, rc::Rc};

use super::{
    math::{Vec2, Vec3},
    mesh::Mesh,
    scene::Scene,
    vertex::Vertex,
};
use anyhow::Result;

pub struct MeshBuilderObjFile {
    file_name: String,
}

impl MeshBuilderObjFile {
    pub fn new(file_name: &str) -> Self {
        Self {
            file_name: file_name.to_string(),
        }
    }

    pub fn build(self, scene: &mut Scene) -> Result<Rc<Mesh>> {
        let mut reader = BufReader::new(File::open(&self.file_name)?);

        let (models, _) = tobj::load_obj_buf(
            &mut reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |_| Ok(Default::default()),
        )?;

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

        let mesh = Rc::new( Mesh::new(&mut scene.id_gen, vertices, indices) );
        scene.add_mesh(mesh.clone());
        Ok(mesh)
    }
}
