use anyhow::Result;

use super::appdata::AppData;
use super::{meshbuildercuboid, meshbuilderobjfile};

//================================================
// Model
//================================================

pub fn load_model(data: &mut AppData) -> Result<()> {
    let meshbuilderobjfile =
        meshbuilderobjfile::MeshBuilderObjFile::new("resources/viking_room.obj");
    let mesh = meshbuilderobjfile.build(&mut data.scene)?;

    let meshbuildercuboid =
        meshbuildercuboid::MeshBuilderCuboid::new_same_walls((-0.5, 0.5), (-0.5, 0.5), (-0.5, 0.5));
    let mesh2 = meshbuildercuboid.build(&mut data.scene)?;

    data.mesh = mesh;
    Ok(())
}
