use anyhow::Result;

use super::appdata::AppData;
use super::meshbuilderobjfile;

//================================================
// Model
//================================================

pub fn load_model(data: &mut AppData) -> Result<()> {
    let meshbuilderobjfile =
        meshbuilderobjfile::MeshBuilderObjFile::new("resources/viking_room.obj");
    let mesh = meshbuilderobjfile.build()?;
    data.mesh = mesh;
    Ok(())
}
