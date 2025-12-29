use anyhow::Result;

use crate::feather::featherapp::FeatherApp;
use crate::feather::scene::Scene;
use crate::feather::perspectivecamera::PerspectiveCamera;
use crate::feather::math::{Point3, Vec3};
use crate::feather::meshbuilderobjfile::MeshBuilderObjFile;

pub struct TestApp {
    scene: Scene,
    camera: PerspectiveCamera,
    root_node: usize,
    room_node: usize,
}

impl FeatherApp for TestApp {
    fn on_create(&mut self) -> Result<()> {
        Ok(())
    }

    fn on_render(&mut self) -> Result<()> {
        Ok(())
    }

    fn on_update(&mut self, time: usize) -> Result<()> {
        Ok(())
    }

    fn on_destroy(&mut self) {
    }
}

impl TestApp {
    pub fn new() -> Self {
        let mut scene = Scene::new();
        let root_node = scene.create_root_node(Some("Scene root".to_string()));
        let room_node = scene.create_node(Some("Room".to_string()), root_node);

        let mut camera = PerspectiveCamera::new();

        camera.set_fov(45.0)
            .set_near_far(0.1, 10.0)
            .set_view(Point3::new(2.0, 2.0, 2.0), Point3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
        
        let meshbuilderobjfile = MeshBuilderObjFile::new("resources/viking_room.obj");
        let room_mesh = meshbuilderobjfile.build(&mut scene).unwrap();

        scene.node_set_mesh(room_node, room_mesh).unwrap();

        //let meshbuildercuboid =
        //    meshbuildercuboid::MeshBuilderCuboid::new_same_walls((-0.5, 0.5), (-0.5, 0.5), (-0.5, 0.5));
        //let mesh2 = meshbuildercuboid.build(&mut data.scene)?;

        Self {
            scene,
            camera,
            root_node,
            room_node,
        }
    }
}
