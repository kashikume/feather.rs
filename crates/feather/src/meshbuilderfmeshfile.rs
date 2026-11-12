use std::time::Instant;

use super::{
    assetsprovider::AssetsProvider,
    math::{Vec2, Vec3},
    mesh::Mesh,
    scene::Scene,
    vertex::Vertex,
};
use anyhow::{Context, Result};

/// Builds a mesh from an `.fmesh` file, feather's binary mesh made from an OBJ by the
/// `mesh-converter` tool (see `doc/fmesh-format.md`). The mesh is the one
/// `MeshBuilderObjFile` makes from the OBJ, without parsing any text.
pub struct MeshBuilderFmeshFile {
    bytes: Vec<u8>,
    /// For messages: the asset's name, if read from one.
    asset: Option<String>,
}

impl MeshBuilderFmeshFile {
    /// Reads the asset `asset` (e.g. `"demo_000_viking_room_mesh.fmesh"`) from `assets`.
    pub fn new(assets: &dyn AssetsProvider, asset: &str) -> Result<Self> {
        Ok(Self {
            bytes: assets.get_asset(asset)?,
            asset: Some(asset.to_string()),
        })
    }

    /// Uses `.fmesh` data already in memory.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes, asset: None }
    }

    /// Builds the mesh, named `name`, adds it to `scene` and returns its handle.
    pub fn build(self, scene: &mut Scene, name: Option<String>) -> Result<usize> {
        let start = Instant::now();
        let what = self.asset.as_deref().unwrap_or("mesh data");
        let file = feather_fmesh::read(&self.bytes).with_context(|| format!("Cannot read {what}"))?;

        let vertices: Vec<Vertex> = (0..file.vertex_count())
            .map(|i| {
                let (p, n, t) = (&file.positions[3 * i..], &file.normals[3 * i..], &file.uvs[2 * i..]);
                Vertex::new(
                    Vec3 { x: p[0], y: p[1], z: p[2] },
                    Vec3 { x: n[0], y: n[1], z: n[2] },
                    Vec2 { x: t[0], y: t[1] },
                )
            })
            .collect();
        log::debug!(
            "{what}: {} vertices, {} triangles, {} bytes{}, read in {:.2} ms",
            vertices.len(),
            file.indices.len() / 3,
            self.bytes.len(),
            if file.header.compressed() { " (deflated)" } else { "" },
            start.elapsed().as_secs_f64() * 1000.0
        );

        let mesh = Mesh::new(name, vertices, file.indices);
        Ok(scene.add_mesh(mesh))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assetsprovider::tests::test_resources;
    use crate::meshbuilderobjfile::MeshBuilderObjFile;
    use crate::object::Object;

    #[test]
    fn loads_asset() {
        let mut scene = Scene::new();
        let mesh = MeshBuilderFmeshFile::new(&*test_resources(), "triangle.fmesh")
            .unwrap()
            .build(&mut scene, Some("triangle".to_string()))
            .unwrap();
        let mesh = scene.get_mesh(mesh).unwrap();
        assert_eq!(mesh.get_name(), Some("triangle".to_string()));
        assert_eq!(mesh.get_indices(), [0, 1, 2]);
        let v = mesh.get_vertices();
        assert_eq!((v[1].pos.x, v[1].pos.y, v[1].pos.z), (1.5, 0.0, 0.0));
        assert_eq!((v[2].normal.x, v[2].normal.y, v[2].normal.z), (0.0, 0.0, 1.0));
        // V flipped: the OBJ's (0, 1) is (0, 0).
        assert_eq!((v[2].tex_coord.x, v[2].tex_coord.y), (0.0, 0.0));
    }

    #[test]
    fn missing_asset_fails() {
        assert!(MeshBuilderFmeshFile::new(&*test_resources(), "does_not_exist.fmesh").is_err());
    }

    #[test]
    fn invalid_data_fails() {
        let mut scene = Scene::new();
        let error = MeshBuilderFmeshFile::from_bytes(b"v 0 0 0\n".to_vec())
            .build(&mut scene, None)
            .unwrap_err();
        assert!(format!("{error:#}").contains("not an .fmesh file"), "{error:#}");
    }

    /// Converts every OBJ of the test app and checks that the `.fmesh` gives the same mesh as
    /// the OBJ: the same triangles, with the same values at every corner. (Corner by corner,
    /// because the OBJ loader keeps `-0.0` and `0.0` apart while the converter merges them, so
    /// vertex counts can differ.) V may be one ulp apart: the OBJ loader computes `1 - v` in
    /// `f32`, the converter exactly.
    #[test]
    fn same_mesh_as_the_obj_for_every_test_app_obj() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/testapp/resources");
        let mut objs: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "obj"))
            .collect();
        objs.sort();
        assert!(!objs.is_empty());

        for path in objs {
            let text = std::fs::read_to_string(&path).unwrap();
            let fmesh = mesh_converter::convert(&text, &mesh_converter::Settings::default())
                .unwrap()
                .meshes
                .remove(0)
                .written
                .bytes;

            let mut scene = Scene::new();
            let from_obj = MeshBuilderObjFile::from_bytes(text.into_bytes()).build(&mut scene, None).unwrap();
            let from_fmesh = MeshBuilderFmeshFile::from_bytes(fmesh).build(&mut scene, None).unwrap();
            let (a, b) = (scene.get_mesh(from_obj).unwrap(), scene.get_mesh(from_fmesh).unwrap());

            let name = path.file_name().unwrap().to_string_lossy();
            assert_eq!(a.get_indices().len(), b.get_indices().len(), "{name}");
            assert!(b.get_vertices().len() <= a.get_vertices().len(), "{name}");
            for (&i, &j) in a.get_indices().iter().zip(b.get_indices()) {
                let (u, v) = (a.get_vertices()[i as usize], b.get_vertices()[j as usize]);
                let v_close = (u.tex_coord.y - v.tex_coord.y).abs() <= u.tex_coord.y.abs().max(1.0) * f32::EPSILON;
                assert!(
                    u.pos == v.pos && u.normal == v.normal && u.tex_coord.x == v.tex_coord.x && v_close,
                    "{name}: {u:?} != {v:?}"
                );
            }
        }
    }
}
