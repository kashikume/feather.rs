mod cars;
mod geometry;
mod textures;
mod traffic;

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use anyhow::{anyhow, Result};
use cgmath::{Deg, InnerSpace, Rad};

use feather::assetsprovider::AssetsProvider;
use feather::atlas::Atlas;
use feather::camera::Camera;
use feather::math::{Mat4, Point3, Vec2, Vec3, Vec4};
use feather::orthographiccamera::{OrthographicCamera, ScreenOrigin, ScreenUnits};
use feather::perspectivecamera::PerspectiveCamera;
use feather::scene::Scene;
use feather::shadow::ShadowLight;
use feather::text::{create_text_material, HorizontalAlign, Text, TextBuilder, VerticalAlign};

use super::random::Random;
use super::testapp::Demo;
use geometry::MeshData;
use traffic::{signal, Axis, Car, Lane, Signal, Traffic, LANE_LENGTH};

/// Blocks along each side of the city.
const BLOCKS: usize = 5;
/// From a street's centre line to the next one.
const PITCH: f32 = 50.0;
/// A block's buildings: a square this wide.
const BLOCK: f32 = 36.0;
/// Half a road's width (two lanes), and a sidewalk's width.
const ROAD_HALF: f32 = 3.5;
const SIDEWALK: f32 = 3.5;
const CURB: f32 = 0.15;
const LANE_OFFSET: f32 = 1.75;
/// Facades repeat every bay (3 m) across and every floor (3.5 m) up.
const BAY: f32 = 3.0;
const FLOOR: f32 = 3.5;
/// The block (column, row) that is a park.
const PARK: (usize, usize) = (3, 1);

/// The camera's loop: around the streets this far from the centre, this high, this fast.
const ROUTE_HALF: f32 = 75.0;
const ROUTE_CORNER: f32 = 2.5;
const CAMERA_HEIGHT: f32 = 4.5;
const CAMERA_SPEED: f32 = 7.0;

/// The sun, and the area its shadows cover around the camera.
const SHADOW_RADIUS: f32 = 75.0;
const SHADOW_MAP_SIZE: u32 = 4096;

/// The overlay is laid out as if the window were this many pixels high.
const OVERLAY_HEIGHT: f32 = 768.0;
/// How often the frame rate shown is updated, in seconds.
const FPS_PERIOD: f32 = 0.5;

/// Longest simulation step; slower frames are split.
const MAX_STEP: f32 = 1.0 / 30.0;

/// The x (or y) of street `k` (0..=BLOCKS).
fn street(k: usize) -> f32 {
    (k as f32 - BLOCKS as f32 / 2.0) * PITCH
}

/// The x (or y) of the centre of block `i` (0..BLOCKS).
fn block_centre(i: usize) -> f32 {
    (i as f32 + 0.5 - BLOCKS as f32 / 2.0) * PITCH
}

/// A person walking around a block on its sidewalk.
struct Person {
    body: usize,
    legs: [usize; 2],
    centre: Vec2,
    /// Half the side of the square they walk around.
    half: f32,
    distance: f32,
    speed: f32,
    /// +1 counter-clockwise, -1 clockwise.
    direction: f32,
    height: f32,
}

/// Demo 15: a busy city. The camera glides through the streets of a city of 5 x 5 blocks:
/// buildings of four kinds, streets with lanes, crosswalks and traffic lights, sidewalks with
/// trees and lamps, a park, cars stopping at red lights and keeping their distance, and people
/// walking around the blocks. Everything is textured and casts shadows from the afternoon sun.
///
/// No image files: the textures are painted in code (`textures`), seamless, and repeat over
/// the surfaces (the texture sampler wraps). Their alpha says where a node's `tint` colours
/// them: walls, car paint and shirts vary, windows don't.
///
/// The static city is a few big meshes, one per texture (`geometry`), so it costs a few draws;
/// cars, people (a body and two swinging legs each) and traffic lights' lamps are nodes. The
/// shadow light follows the camera, snapped to the shadow map's texels so shadows don't
/// shimmer (`ShadowLight::directional_snapped`). The shader (`demo_015_city_shader.frag`) adds
/// light from the sky and the ground, darker street level, sky reflections on glass and fog
/// into the sky dome (`demo_015_city_sky_shader.frag`).
///
/// A second scene, an overlay seen by a flat camera, shows the frame rate
/// (`demo_015_city_overlay_scene.json`, its font rendered during the build from
/// `atlases/demo_015_city_atlas.font`).
pub struct CityDemo {
    scene: Scene,
    camera: PerspectiveCamera,
    sky: usize,
    traffic: Traffic,
    /// Each car's node and model.
    car_nodes: Vec<(usize, cars::CarModel)>,
    people: Vec<Person>,
    /// Traffic lights' glowing lamps, the streets they are for, and their lights' housings
    /// (where the lamp goes in front of the lens of the colour shown).
    lamps: Vec<(usize, Axis, Mat4)>,
    last_time: Option<f32>,
    /// The overlay in front of the city: the frame rate, in white over a soft shadow.
    overlay: Scene,
    overlay_camera: OrthographicCamera,
    fps_texts: [Text; 2],
    /// Frames counted since `fps_start`, for the frame rate shown.
    frames: u32,
    fps_start: Option<f32>,
}

/// The afternoon sun: direction towards it.
fn sun() -> Vec3 {
    Vec3::new(-0.52, -0.38, 0.62).normalize()
}

impl CityDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        Self::with_random(assets, Random::from_seed(2024))
    }

    pub fn with_random(assets: &dyn AssetsProvider, mut random: Random) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_015_city_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let lit = find("shader `Lit`", scene.find_shader("Lit"))?;
        let city = find("node `City`", scene.find_node("City"))?;
        let sky = find("node `Sky`", scene.find_node("Sky"))?;

        // Textures and their materials: (texture, shininess, specular strength, reflectivity).
        let mut materials = HashMap::new();
        for texture in textures::all() {
            let name = texture.name;
            let atlas = scene.add_atlas(Atlas::from_rgba(
                Some(name.to_string()),
                texture.width,
                texture.height,
                texture.pixels,
            ));
            let (shininess, specular, reflectivity) = match name {
                "Glass facade" => (90.0, 0.9, 0.55),
                "Car" => (60.0, 0.7, 0.25),
                "Concrete facade" | "Stone facade" | "Brick facade" => (40.0, 0.25, 0.12),
                "Metal" => (30.0, 0.4, 0.1),
                "Leaves" | "Grass" => (6.0, 0.05, 0.0),
                _ => (12.0, 0.1, 0.0),
            };
            let material = scene.create_material(Some(name.to_string()), lit)?;
            let texture = scene.get_atlas(atlas).unwrap().get_full_texture();
            scene.material_set_texture(material, Some(texture))?;
            scene
                .get_material_mut(material)
                .unwrap()
                .uniforms_mut()
                .set("shininess", shininess as f32)
                .set("specularStrength", specular as f32)
                .set("reflectivity", reflectivity as f32);
            materials.insert(name, material);
        }

        // The sky: a dome around the camera, with the sun's colours.
        let dome = scene.add_mesh(geometry::sky_dome(32, 16).into_mesh("Sky dome"));
        scene.node_set_mesh(sky, dome)?;
        scene
            .get_node_mut(sky)
            .unwrap()
            .uniforms_mut()
            .set("sunDirection", sun());
        scene
            .get_node_mut(city)
            .unwrap()
            .uniforms_mut()
            .set("sunDirection", sun());

        let (overlay, fps_texts) = Self::overlay(assets)?;

        let mut demo = Self {
            scene,
            camera: PerspectiveCamera::new(),
            sky,
            traffic: Traffic {
                lanes: Vec::new(),
                cars: Vec::new(),
            },
            car_nodes: Vec::new(),
            people: Vec::new(),
            lamps: Vec::new(),
            last_time: None,
            overlay,
            overlay_camera: OrthographicCamera::new(
                ScreenOrigin::TopLeft,
                ScreenUnits::Height(OVERLAY_HEIGHT),
            ),
            fps_texts,
            frames: 0,
            fps_start: None,
        };
        demo.build_city(city, &materials, &mut random)?;
        demo.add_traffic(city, &materials, &mut random)?;
        demo.add_people(city, &materials, &mut random)?;
        demo.camera.set_fov(60.0).set_near_far(0.2, 520.0);
        demo.update(0.0)?;
        Ok(demo)
    }

    /// The overlay scene (`demo_015_city_overlay_scene.json`) with the frame rate's texts: a
    /// dark shadow, and the white text over it.
    fn overlay(assets: &dyn AssetsProvider) -> Result<(Scene, [Text; 2])> {
        let mut overlay = Scene::load(assets, "demo_015_city_overlay_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the overlay has no {what}"))
        };
        let font = find("atlas `Font`", overlay.find_atlas("Font"))?;
        let shader = find("shader `Text`", overlay.find_shader("Text"))?;
        let anchor = find("node `Frame rate`", overlay.find_node("Frame rate"))?;
        let material = create_text_material(&mut overlay, None, shader, font)?;
        let builder = TextBuilder::new(font)
            .height(30.0)
            .align(HorizontalAlign::Left, VerticalAlign::Top);
        // The shadow a little down and right, and behind (drawn first).
        let mut texts = Vec::new();
        for (offset, color) in [
            (Vec3::new(2.0, -2.0, -1.0), Vec4::new(0.0, 0.0, 0.0, 0.6)),
            (Vec3::new(0.0, 0.0, 0.0), Vec4::new(1.0, 1.0, 1.0, 1.0)),
        ] {
            let text = Text::create(&mut overlay, anchor, None, material, builder.clone(), "")?;
            let node = overlay.get_node_mut(text.get_node()).unwrap();
            node.set_transform(Mat4::from_translation(offset));
            node.uniforms_mut().set("color", color);
            texts.push(text);
        }
        let [shadow, text]: [Text; 2] = texts.try_into().ok().unwrap();
        Ok((overlay, [shadow, text]))
    }

    /// Adds `mesh` as a node with `material`, casting shadows.
    fn add_static(
        &mut self,
        parent: usize,
        name: &str,
        mesh: MeshData,
        material: usize,
    ) -> Result<usize> {
        let mesh = self.scene.add_mesh(mesh.into_mesh(name));
        let node = self.scene.create_node(Some(name.to_string()), parent)?;
        self.scene.node_set_mesh(node, mesh)?;
        self.scene.node_set_material(node, material)?;
        self.scene
            .get_node_mut(node)
            .unwrap()
            .set_casts_shadow(true);
        Ok(node)
    }

    /// The buildings, streets, sidewalks, park, trees, lamps and traffic lights.
    fn build_city(
        &mut self,
        city: usize,
        materials: &HashMap<&str, usize>,
        random: &mut Random,
    ) -> Result<()> {
        let styles = [
            "Glass facade",
            "Brick facade",
            "Concrete facade",
            "Stone facade",
        ];
        let mut facades: Vec<MeshData> = styles.iter().map(|_| MeshData::default()).collect();
        let (mut roofs, mut roads, mut crossings, mut crosswalks) = (
            MeshData::default(),
            MeshData::default(),
            MeshData::default(),
            MeshData::default(),
        );
        let (mut sidewalks, mut curbs, mut grass) = (
            MeshData::default(),
            MeshData::default(),
            MeshData::default(),
        );
        let (mut bark, mut leaves, mut metal) = (
            MeshData::default(),
            MeshData::default(),
            MeshData::default(),
        );
        let mut signals = MeshData::default();

        // Ground beyond the city.
        grass.top(
            Vec2::new(-600.0, -600.0),
            Vec2::new(600.0, 600.0),
            -0.02,
            4.0,
        );

        for bi in 0..BLOCKS {
            for bj in 0..BLOCKS {
                let (cx, cy) = (block_centre(bi), block_centre(bj));
                // The sidewalk: a slab around the block, curbs on its sides.
                let outer = BLOCK / 2.0 + SIDEWALK;
                let (lo, hi) = (
                    Vec2::new(cx - outer, cy - outer),
                    Vec2::new(cx + outer, cy + outer),
                );
                curbs.walls(lo.extend(0.0), hi.extend(CURB), Vec2::new(2.0, 2.0));
                if (bi, bj) == PARK {
                    // Sidewalk around a lawn with trees.
                    let inner = BLOCK / 2.0;
                    let lawn = (
                        Vec2::new(cx - inner, cy - inner),
                        Vec2::new(cx + inner, cy + inner),
                    );
                    for (a, b) in [
                        (lo, Vec2::new(hi.x, lawn.0.y)),
                        (Vec2::new(lo.x, lawn.1.y), hi),
                        (Vec2::new(lo.x, lawn.0.y), Vec2::new(lawn.0.x, lawn.1.y)),
                        (Vec2::new(lawn.1.x, lawn.0.y), Vec2::new(hi.x, lawn.1.y)),
                    ] {
                        sidewalks.top(a, b, CURB, 2.0);
                    }
                    grass.top(lawn.0, lawn.1, CURB, 4.0);
                    for _ in 0..14 {
                        let x = cx + random.range(-inner + 3.0, inner - 3.0);
                        let y = cy + random.range(-inner + 3.0, inner - 3.0);
                        tree(
                            &mut bark,
                            &mut leaves,
                            Vec3::new(x, y, CURB),
                            random.range(0.9, 1.4),
                            random,
                        );
                    }
                } else {
                    sidewalks.top(lo, hi, CURB, 2.0);
                    // Buildings: the block split into lots, whole bays wide.
                    let centre_distance = (cx * cx + cy * cy).sqrt();
                    let tallest = 5.0 + 15.0 * (1.0 - centre_distance / 150.0).max(0.0);
                    let xs = split_lots(random);
                    let ys = split_lots(random);
                    let (mut x0, half) = (cx - BLOCK / 2.0, BLOCK / 2.0);
                    for &w in &xs {
                        let mut y0 = cy - half;
                        for &d in &ys {
                            let style =
                                (random.next() * styles.len() as f32) as usize % styles.len();
                            let floors = (3.0 + random.next() * (tallest - 3.0)).round().max(3.0);
                            let min = Vec3::new(x0, y0, CURB);
                            let max = Vec3::new(x0 + w, y0 + d, CURB + floors * FLOOR);
                            facades[style].walls(min, max, Vec2::new(BAY, FLOOR));
                            roofs.top(min.truncate(), max.truncate(), max.z, 8.0);
                            // Some tall buildings have a narrower top.
                            if floors >= 8.0 && w >= 12.0 && d >= 12.0 && random.next() < 0.5 {
                                let more = (2.0 + random.next() * 6.0).round();
                                let inset = Vec3::new(BAY, BAY, 0.0);
                                let (tmin, tmax) = (
                                    Vec3::new(min.x, min.y, max.z) + inset,
                                    Vec3::new(max.x, max.y, max.z + more * FLOOR) - inset,
                                );
                                facades[style].walls(tmin, tmax, Vec2::new(BAY, FLOOR));
                                roofs.top(tmin.truncate(), tmax.truncate(), tmax.z, 8.0);
                            }
                            y0 += d;
                        }
                        x0 += w;
                    }
                }
                // Trees and lamps along the sidewalk, near the curb.
                let curb_line = BLOCK / 2.0 + SIDEWALK - 1.1;
                for side in 0..4 {
                    for k in 0..3 {
                        let along = -12.0 + 12.0 * k as f32;
                        let (dx, dy) = match side {
                            0 => (along, -curb_line),
                            1 => (curb_line, along),
                            2 => (-along, curb_line),
                            _ => (-curb_line, -along),
                        };
                        let p = Vec3::new(cx + dx, cy + dy, CURB);
                        if k == 1 {
                            lamp_post(&mut metal, p, side);
                        } else {
                            tree(&mut bark, &mut leaves, p, random.range(0.7, 0.85), random);
                        }
                    }
                }
            }
        }

        // Streets: road pieces between intersections, intersections, crosswalks.
        let far = LANE_LENGTH / 2.0;
        let mut edges = vec![-far];
        for k in 0..=BLOCKS {
            edges.push(street(k) - ROAD_HALF);
            edges.push(street(k) + ROAD_HALF);
        }
        edges.push(far);
        for k in 0..=BLOCKS {
            let s = street(k);
            for piece in edges.chunks(2) {
                let (a, b) = (piece[0], piece[1]);
                // Along X at y = s, and along Y at x = s.
                roads.strip(
                    Vec2::new(a, s - ROAD_HALF),
                    Vec2::new(b, s + ROAD_HALF),
                    0.0,
                    true,
                    7.0,
                );
                roads.strip(
                    Vec2::new(s - ROAD_HALF, a),
                    Vec2::new(s + ROAD_HALF, b),
                    0.0,
                    false,
                    7.0,
                );
            }
            for l in 0..=BLOCKS {
                let t = street(l);
                crossings.top(
                    Vec2::new(s - ROAD_HALF, t - ROAD_HALF),
                    Vec2::new(s + ROAD_HALF, t + ROAD_HALF),
                    0.0,
                    7.0,
                );
                // Crosswalks on the four roads leaving the intersection.
                let w = 3.0;
                let z = 0.005;
                crosswalks.strip(
                    Vec2::new(s + ROAD_HALF, t - ROAD_HALF),
                    Vec2::new(s + ROAD_HALF + w, t + ROAD_HALF),
                    z,
                    true,
                    w,
                );
                crosswalks.strip(
                    Vec2::new(s - ROAD_HALF - w, t - ROAD_HALF),
                    Vec2::new(s - ROAD_HALF, t + ROAD_HALF),
                    z,
                    true,
                    w,
                );
                crosswalks.strip(
                    Vec2::new(s - ROAD_HALF, t + ROAD_HALF),
                    Vec2::new(s + ROAD_HALF, t + ROAD_HALF + w),
                    z,
                    false,
                    w,
                );
                crosswalks.strip(
                    Vec2::new(s - ROAD_HALF, t - ROAD_HALF - w),
                    Vec2::new(s + ROAD_HALF, t - ROAD_HALF),
                    z,
                    false,
                    w,
                );
                // Traffic lights on the near right corner of every approach (traffic drives on
                // the right), facing the cars coming: (corner, street, facing).
                for (corner, axis, facing) in [
                    ((-1.0, -1.0), Axis::X, PI),
                    ((1.0, 1.0), Axis::X, 0.0),
                    ((1.0, -1.0), Axis::Y, -FRAC_PI_2),
                    ((-1.0, 1.0), Axis::Y, FRAC_PI_2),
                ] {
                    let base = Vec3::new(
                        s + corner.0 * (ROAD_HALF + 1.0),
                        t + corner.1 * (ROAD_HALF + 1.0),
                        CURB,
                    );
                    let head = traffic_light(&mut signals, &mut metal, base, facing);
                    let lamp = self.scene.create_node(None, city)?;
                    self.lamps.push((lamp, axis, head));
                }
            }
        }

        // The traffic lights' lamps: a glowing disc each, in front of the lens shown.
        let lamp_mesh = {
            let mut m = MeshData::default();
            let along_x = Mat4::from_angle_y(Deg(90.0));
            m.cylinder(
                along_x,
                LENS_RADIUS,
                0.025,
                16,
                [0.0, 0.0, 1.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
            );
            self.scene.add_mesh(m.into_mesh("Lamp"))
        };
        for &(lamp, _, _) in &self.lamps {
            self.scene.node_set_mesh(lamp, lamp_mesh)?;
            self.scene.node_set_material(lamp, materials["Metal"])?;
            self.scene
                .get_node_mut(lamp)
                .unwrap()
                .set_casts_shadow(true);
        }

        let statics = self.scene.create_node(Some("Static".into()), city)?;
        for (i, mesh) in facades.into_iter().enumerate() {
            if !mesh.is_empty() {
                let node = self.add_static(statics, styles[i], mesh, materials[styles[i]])?;
                // Each style in its own wall colour (glass facades stay neutral).
                let tint = [
                    Vec3::new(1.0, 1.0, 1.0),
                    Vec3::new(1.0, 0.92, 0.85),
                    Vec3::new(0.95, 0.97, 1.0),
                    Vec3::new(1.0, 0.96, 0.9),
                ][i];
                self.scene
                    .get_node_mut(node)
                    .unwrap()
                    .uniforms_mut()
                    .set("tint", tint);
            }
        }
        for (name, mesh, material) in [
            ("Roofs", roofs, "Roof"),
            ("Roads", roads, "Road"),
            ("Intersections", crossings, "Intersection"),
            ("Crosswalks", crosswalks, "Crosswalk"),
            ("Sidewalks", sidewalks, "Sidewalk"),
            ("Curbs", curbs, "Concrete"),
            ("Grass", grass, "Grass"),
            ("Tree trunks", bark, "Bark"),
            ("Tree tops", leaves, "Leaves"),
            ("Poles", metal, "Metal"),
            ("Traffic lights", signals, "Signal"),
        ] {
            self.add_static(statics, name, mesh, materials[material])?;
        }
        Ok(())
    }

    /// Two lanes on every street, with cars of all colours and a few vans.
    fn add_traffic(
        &mut self,
        city: usize,
        materials: &HashMap<&str, usize>,
        random: &mut Random,
    ) -> Result<()> {
        let crossings: Vec<f32> = (0..=BLOCKS).map(street).collect();
        let stop_back = ROAD_HALF + 3.6;
        for k in 0..=BLOCKS {
            let s = street(k);
            // Right-hand traffic: going +X on the -Y side.
            self.traffic.lanes.push(Lane::new(
                Axis::X,
                s - LANE_OFFSET,
                1.0,
                &crossings,
                stop_back,
            ));
            self.traffic.lanes.push(Lane::new(
                Axis::X,
                s + LANE_OFFSET,
                -1.0,
                &crossings,
                stop_back,
            ));
            self.traffic.lanes.push(Lane::new(
                Axis::Y,
                s + LANE_OFFSET,
                1.0,
                &crossings,
                stop_back,
            ));
            self.traffic.lanes.push(Lane::new(
                Axis::Y,
                s - LANE_OFFSET,
                -1.0,
                &crossings,
                stop_back,
            ));
        }
        // A mesh per model: sedans, hatchbacks, SUVs and taxis.
        let meshes: Vec<(cars::CarModel, usize)> = cars::CarModel::ALL
            .iter()
            .map(|&model| {
                let name = format!("{model:?}");
                (model, self.scene.add_mesh(model.mesh().into_mesh(&name)))
            })
            .collect();
        let cars = self.scene.create_node(Some("Cars".into()), city)?;
        let colours = [
            Vec3::new(0.7, 0.05, 0.05),
            Vec3::new(0.05, 0.12, 0.45),
            Vec3::new(0.85, 0.85, 0.85),
            Vec3::new(0.08, 0.08, 0.09),
            Vec3::new(0.5, 0.52, 0.55),
            Vec3::new(0.9, 0.6, 0.05),
            Vec3::new(0.1, 0.35, 0.18),
            Vec3::new(0.35, 0.05, 0.25),
        ];
        for lane in 0..self.traffic.lanes.len() {
            let count = 6;
            let spacing = LANE_LENGTH / count as f32;
            for c in 0..count {
                let distance = c as f32 * spacing + random.range(0.0, spacing - 15.0);
                let cruise = random.range(8.0, 13.0);
                self.traffic.cars.push(Car {
                    lane,
                    distance,
                    speed: cruise * 0.5,
                    cruise,
                });
                let pick = random.next();
                let (model, mesh) = meshes[match pick {
                    p if p < 0.45 => 0,
                    p if p < 0.7 => 1,
                    p if p < 0.9 => 2,
                    _ => 3,
                }];
                let node = self.scene.create_node(None, cars)?;
                self.scene.node_set_mesh(node, mesh)?;
                self.scene.node_set_material(node, materials["Car"])?;
                let colour = if model == cars::CarModel::Taxi {
                    Vec3::new(0.95, 0.72, 0.05)
                } else {
                    colours[(random.next() * colours.len() as f32) as usize % colours.len()]
                };
                let n = self.scene.get_node_mut(node).unwrap();
                n.set_casts_shadow(true);
                n.uniforms_mut().set("tint", colour);
                self.car_nodes.push((node, model));
            }
        }
        Ok(())
    }

    /// People walking around the blocks, on the sidewalks.
    fn add_people(
        &mut self,
        city: usize,
        materials: &HashMap<&str, usize>,
        random: &mut Random,
    ) -> Result<()> {
        let body = self
            .scene
            .add_mesh(geometry::person_body().into_mesh("Person"));
        let leg = self.scene.add_mesh(geometry::leg().into_mesh("Leg"));
        let group = self.scene.create_node(Some("People".into()), city)?;
        let shirts = [
            Vec3::new(0.8, 0.1, 0.1),
            Vec3::new(0.1, 0.3, 0.8),
            Vec3::new(0.95, 0.95, 0.95),
            Vec3::new(0.1, 0.1, 0.1),
            Vec3::new(0.9, 0.75, 0.1),
            Vec3::new(0.2, 0.6, 0.3),
            Vec3::new(0.6, 0.3, 0.7),
            Vec3::new(0.15, 0.55, 0.65),
        ];
        for bi in 0..BLOCKS {
            for bj in 0..BLOCKS {
                for _ in 0..5 {
                    let node = self.scene.create_node(None, group)?;
                    self.scene.node_set_mesh(node, body)?;
                    self.scene.node_set_material(node, materials["Person"])?;
                    let shirt =
                        shirts[(random.next() * shirts.len() as f32) as usize % shirts.len()];
                    let n = self.scene.get_node_mut(node).unwrap();
                    n.set_casts_shadow(true);
                    n.uniforms_mut().set("tint", shirt);
                    let mut legs = [0; 2];
                    for leg_node in &mut legs {
                        *leg_node = self.scene.create_node(None, node)?;
                        self.scene.node_set_mesh(*leg_node, leg)?;
                        self.scene
                            .node_set_material(*leg_node, materials["Person"])?;
                        self.scene
                            .get_node_mut(*leg_node)
                            .unwrap()
                            .set_casts_shadow(true);
                    }
                    let half = BLOCK / 2.0 + SIDEWALK / 2.0 + random.range(-1.2, 0.5);
                    self.people.push(Person {
                        body: node,
                        legs,
                        centre: Vec2::new(block_centre(bi), block_centre(bj)),
                        half,
                        distance: random.range(0.0, 8.0 * half),
                        speed: random.range(1.1, 1.6),
                        direction: if random.next() < 0.5 { 1.0 } else { -1.0 },
                        height: random.range(0.92, 1.08),
                    });
                }
            }
        }
        Ok(())
    }

    fn simulate(&mut self, time: f32, dt: f32) {
        self.traffic.step(time, dt);
        for person in &mut self.people {
            let loop_length = 8.0 * person.half;
            person.distance =
                (person.distance + person.speed * dt * person.direction).rem_euclid(loop_length);
        }
    }

    /// Puts cars, people, lamps, the camera, the sky and the shadow light where they are.
    fn place(&mut self, time: f32) {
        for (car, &(node, model)) in self.traffic.cars.iter().zip(&self.car_nodes) {
            let lane = &self.traffic.lanes[car.lane];
            // The car's middle is half its length behind the front bumper.
            let (x, y, heading) = lane.place(car.distance - model.length() / 2.0);
            self.scene.get_node_mut(node).unwrap().set_transform(
                Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_angle_z(Rad(heading)),
            );
        }

        for person in &self.people {
            let (p, heading) = square_loop(person.half, person.distance);
            let heading = if person.direction > 0.0 {
                heading
            } else {
                heading + PI
            };
            // Legs swing with each step; the body bobs twice a stride.
            let step = person.distance / 0.75 * PI;
            let swing = 0.45 * step.sin();
            let bob = 0.03 * step.sin().abs();
            let position = Vec3::new(person.centre.x + p.x, person.centre.y + p.y, CURB + bob);
            self.scene.get_node_mut(person.body).unwrap().set_transform(
                Mat4::from_translation(position)
                    * Mat4::from_angle_z(Rad(heading))
                    * Mat4::from_scale(person.height),
            );
            for (k, &leg) in person.legs.iter().enumerate() {
                let side = if k == 0 { -1.0 } else { 1.0 };
                self.scene.get_node_mut(leg).unwrap().set_transform(
                    Mat4::from_translation(Vec3::new(0.0, side * 0.09, 0.9 - bob))
                        * Mat4::from_angle_y(Rad(swing * side)),
                );
            }
        }

        for &(lamp, axis, head) in &self.lamps {
            let (lens, colour) = match signal(axis, time) {
                Signal::Red => (0, Vec3::new(4.0, 0.2, 0.12)),
                Signal::Yellow => (1, Vec3::new(4.0, 2.6, 0.15)),
                Signal::Green => (2, Vec3::new(0.15, 3.6, 0.8)),
            };
            let node = self.scene.get_node_mut(lamp).unwrap();
            node.set_transform(
                head * Mat4::from_translation(Vec3::new(LENS_FRONT, 0.0, LENS_HEIGHTS[lens])),
            );
            node.uniforms_mut().set("emissive", colour);
        }

        // The camera follows the route, looking a little way ahead along it.
        let distance = time * CAMERA_SPEED;
        let (eye, _) = route(distance);
        let (ahead, _) = route(distance + 18.0);
        let eye = Point3::new(eye.x, eye.y, CAMERA_HEIGHT);
        let target = Point3::new(ahead.x, ahead.y, CAMERA_HEIGHT - 1.2);
        self.camera.set_view(eye, target, Vec3::new(0.0, 0.0, 1.0));

        self.scene.get_node_mut(self.sky).unwrap().set_transform(
            Mat4::from_translation(Vec3::new(eye.x, eye.y, eye.z)) * Mat4::from_scale(480.0),
        );

        // Shadows around what the camera sees, moving by whole texels.
        let forward = (target - eye).normalize();
        let centre =
            eye + Vec3::new(forward.x, forward.y, 0.0).normalize() * (SHADOW_RADIUS * 0.45);
        let centre = Point3::new(centre.x, centre.y, 6.0);
        self.scene
            .set_shadow_light(Some(ShadowLight::directional_snapped(
                -sun(),
                centre,
                SHADOW_RADIUS,
                SHADOW_MAP_SIZE,
            )));
    }
}

impl Demo for CityDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
    }

    fn scene_count(&self) -> usize {
        2
    }

    fn scene_at(&mut self, index: usize) -> &mut Scene {
        match index {
            0 => &mut self.scene,
            _ => &mut self.overlay,
        }
    }

    fn camera_at(&mut self, index: usize) -> &mut dyn Camera {
        match index {
            0 => &mut self.camera,
            _ => &mut self.overlay_camera,
        }
    }

    fn update(&mut self, time: f32) -> Result<()> {
        let mut dt = (time - self.last_time.unwrap_or(time)).clamp(0.0, 0.5);
        self.last_time = Some(time);
        let mut t = time - dt;
        while dt > 0.0 {
            let step = dt.min(MAX_STEP);
            t += step;
            self.simulate(t, step);
            dt -= step;
        }
        self.place(time);

        // The frame rate, counted over half a second.
        let start = *self.fps_start.get_or_insert(time);
        self.frames += 1;
        if time - start >= FPS_PERIOD {
            let fps = self.frames as f32 / (time - start);
            let text = format!("{fps:.0} fps");
            for shown in &mut self.fps_texts {
                shown.set_text(&mut self.overlay, &text)?;
            }
            self.frames = 0;
            self.fps_start = Some(time);
        }
        Ok(())
    }
}

/// Splits a block's side (12 bays) into 1 to 3 lots, whole bays each, in metres.
fn split_lots(random: &mut Random) -> Vec<f32> {
    let options: [&[u32]; 7] = [
        &[12],
        &[6, 6],
        &[4, 8],
        &[7, 5],
        &[4, 4, 4],
        &[3, 5, 4],
        &[5, 3, 4],
    ];
    let choice = options[(random.next() * options.len() as f32) as usize % options.len()];
    choice.iter().map(|&b| b as f32 * BAY).collect()
}

/// A tree standing at `base`: a trunk and a few clumps of leaves.
fn tree(bark: &mut MeshData, leaves: &mut MeshData, base: Vec3, size: f32, random: &mut Random) {
    let full = [0.0, 0.0, 1.0, 1.0];
    let trunk = Mat4::from_translation(base) * Mat4::from_scale(size);
    bark.cylinder(trunk, 0.14, 3.2, 8, [0.0, 0.0, 1.0, 2.0], full);
    for _ in 0..3 {
        let offset = Vec3::new(
            random.range(-0.7, 0.7),
            random.range(-0.7, 0.7),
            random.range(3.4, 4.4),
        );
        let radii = Vec3::new(
            random.range(1.3, 1.8),
            random.range(1.3, 1.8),
            random.range(1.1, 1.5),
        );
        leaves.ellipsoid(
            trunk * Mat4::from_translation(offset),
            radii,
            12,
            8,
            [0.0, 0.0, 3.0, 2.0],
        );
    }
}

/// A street lamp at `base` near the curb of a block's side `side` (0: -Y, 1: +X, 2: +Y,
/// 3: -X), its arm reaching out over the road.
fn lamp_post(metal: &mut MeshData, base: Vec3, side: usize) {
    let full = [0.0, 0.0, 1.0, 1.0];
    let pole = Mat4::from_translation(base);
    metal.cylinder(pole, 0.08, 6.5, 8, full, full);
    // The arm is built along +Y; turn it towards the road: -Y, +X, +Y, -X.
    let turn = Mat4::from_angle_z(Deg(180.0 + 90.0 * side as f32));
    metal.box_in(
        pole * turn,
        Vec3::new(-0.06, -0.06, 6.3),
        Vec3::new(0.06, 2.6, 6.42),
        full,
    );
    metal.box_in(
        pole * turn,
        Vec3::new(-0.17, 2.0, 6.14),
        Vec3::new(0.17, 2.75, 6.32),
        full,
    );
}

/// The heights of a traffic light's red, yellow and green lenses above its base, how far in
/// front of the housing's centre they are, and their radius.
const LENS_HEIGHTS: [f32; 3] = [3.44, 3.1, 2.76];
const LENS_FRONT: f32 = 0.175;
const LENS_RADIUS: f32 = 0.11;

/// A traffic light standing at `base`, its lenses facing `facing` (radians from +X): a pole, a
/// back plate, a housing with red, yellow and green lenses under visors. Returns the housing's
/// transform: in it, the lenses face +X at `LENS_FRONT`, at `LENS_HEIGHTS`.
fn traffic_light(signals: &mut MeshData, metal: &mut MeshData, base: Vec3, facing: f32) -> Mat4 {
    use textures::signal_regions::*;
    let full = [0.0, 0.0, 1.0, 1.0];
    let head = Mat4::from_translation(base) * Mat4::from_angle_z(Rad(facing));
    metal.cylinder(Mat4::from_translation(base), 0.08, 2.6, 10, full, full);
    signals.box_in(
        head,
        Vec3::new(-0.2, -0.24, 2.5),
        Vec3::new(-0.17, 0.24, 3.7),
        HOUSING,
    );
    signals.box_in(
        head,
        Vec3::new(-0.17, -0.16, 2.58),
        Vec3::new(0.15, 0.16, 3.62),
        HOUSING,
    );
    let along_x = Mat4::from_angle_y(Deg(90.0));
    for (&z, region) in LENS_HEIGHTS.iter().zip([RED, YELLOW, GREEN]) {
        let lens = head * Mat4::from_translation(Vec3::new(0.15, 0.0, z)) * along_x;
        signals.cylinder(lens, LENS_RADIUS, 0.02, 16, region, region);
        // A visor shading the lens from above.
        signals.box_in(
            head,
            Vec3::new(0.15, -0.13, z + LENS_RADIUS),
            Vec3::new(0.34, 0.13, z + LENS_RADIUS + 0.02),
            HOUSING,
        );
    }
    head
}

/// A point at `distance` along a square loop of half side `half` around the origin (rounded
/// at the corners by 1 m), counter-clockwise from the middle of the -Y side; and the heading.
fn square_loop(half: f32, distance: f32) -> (Vec2, f32) {
    rounded_square(half, 1.0, distance)
}

/// The camera's route: a rounded square around the middle of the city, through the streets.
fn route(distance: f32) -> (Vec2, f32) {
    rounded_square(ROUTE_HALF, ROUTE_CORNER, distance)
}

/// A point at `distance` along a square of half side `half` with corners rounded by `radius`,
/// counter-clockwise from the middle of the -Y side; and the heading (radians).
fn rounded_square(half: f32, radius: f32, distance: f32) -> (Vec2, f32) {
    let straight = 2.0 * (half - radius);
    let arc = FRAC_PI_2 * radius;
    let side = straight + arc;
    let d = distance.rem_euclid(4.0 * side);
    let k = (d / side).floor() as i32;
    let local = d - k as f32 * side;
    // Along the -Y side going +X, then rotate by k quarter turns.
    let (p, heading) = if local < straight / 2.0 {
        (Vec2::new(local, -half), 0.0)
    } else if local < straight / 2.0 + arc {
        let a = (local - straight / 2.0) / radius;
        let c = Vec2::new(half - radius, -half + radius);
        (c + Vec2::new(a.sin(), -a.cos()) * radius, a)
    } else {
        (
            Vec2::new(half, -half + radius + (local - straight / 2.0 - arc)),
            FRAC_PI_2,
        )
    };
    // Shift so the start is the middle of the side, and turn.
    let turn = k as f32 * FRAC_PI_2;
    let (s, c) = turn.sin_cos();
    let rotated = Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c);
    (rotated, (heading + turn).rem_euclid(TAU))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_square_goes_round_smoothly() {
        let total = 4.0 * (2.0 * (10.0 - 2.0) + FRAC_PI_2 * 2.0);
        let mut last = rounded_square(10.0, 2.0, 0.0).0;
        assert_eq!(last, Vec2::new(0.0, -10.0));
        for step in 1..=400 {
            let (p, _) = rounded_square(10.0, 2.0, step as f32 * total / 400.0);
            assert!(
                (p - last).magnitude() < total / 400.0 + 1e-3,
                "jump at {step}"
            );
            assert!(p.x.abs() <= 10.0 + 1e-4 && p.y.abs() <= 10.0 + 1e-4);
            last = p;
        }
        assert!((last - Vec2::new(0.0, -10.0)).magnitude() < 1e-3);
    }

    #[test]
    fn lots_fill_a_block() {
        let mut random = Random::from_seed(3);
        for _ in 0..50 {
            assert_eq!(split_lots(&mut random).iter().sum::<f32>(), BLOCK);
        }
    }
}
