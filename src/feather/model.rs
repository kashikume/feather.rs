use std::collections::HashMap;

use std::fs::File;

use std::io::BufReader;

use anyhow::Result;
use cgmath::{vec2, vec3};

use super::appdata::AppData;
use super::vertex::Vertex;

//================================================
// Model
//================================================

pub fn load_model(data: &mut AppData) -> Result<()> {
    // Model

    let mut reader = BufReader::new(File::open("resources/viking_room.obj")?);

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

    for model in &models {
        for index in &model.mesh.indices {
            let pos_offset = (3 * index) as usize;
            let tex_coord_offset = (2 * index) as usize;
            let normals_offset = (3 * index) as usize;

            let vertex = Vertex::new(
                vec3(
                    model.mesh.positions[pos_offset],
                    model.mesh.positions[pos_offset + 1],
                    model.mesh.positions[pos_offset + 2],
                ),
                vec3(
                    model.mesh.normals[normals_offset],
                    model.mesh.normals[normals_offset + 1],
                    model.mesh.normals[normals_offset + 2],
                ),
                vec2(
                    model.mesh.texcoords[tex_coord_offset],
                    1.0 - model.mesh.texcoords[tex_coord_offset + 1],
                ),
            );

            if let Some(index) = unique_vertices.get(&vertex) {
                data.indices.push(*index as u32);
            } else {
                let index = data.vertices.len();
                unique_vertices.insert(vertex, index);
                data.vertices.push(vertex);
                data.indices.push(index as u32);
            }
        }
    }

    Ok(())
}
