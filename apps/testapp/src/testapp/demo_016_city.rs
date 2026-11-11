//! Demo 16: the city again, but every model is an OBJ and every texture is a PNG packed into
//! an atlas at build time (`tools/generate_demo_016_city_assets.py`). Geometry is denser than
//! demo 15 (recessed windows, richer cars/people/trees).

mod traffic;

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use anyhow::{anyhow, Result};
use cgmath::{InnerSpace, Rad};

use feather::assetsprovider::AssetsProvider;
use feather::camera::Camera;
use feather::math::{Mat4, Point3, Vec2, Vec3};
use feather::perspectivecamera::PerspectiveCamera;
use feather::scene::Scene;
use feather::shadow::ShadowLight;

use super::random::Random;
use super::testapp::Demo;
use traffic::{signal, Axis, Car, Lane, Signal, Traffic, LANE_LENGTH};

const BLOCKS: usize = 5;
const PITCH: f32 = 50.0;
const BLOCK: f32 = 36.0;
const ROAD_HALF: f32 = 3.5;
const SIDEWALK: f32 = 3.5;
const CURB: f32 = 0.15;
const LANE_OFFSET: f32 = 1.75;
const ROUTE_HALF: f32 = 75.0;
const ROUTE_CORNER: f32 = 2.5;
const CAMERA_HEIGHT: f32 = 4.5;
const CAMERA_SPEED: f32 = 7.0;

const SHADOW_RADIUS: f32 = 75.0;
const SHADOW_MAP_SIZE: u32 = 4096;

const MAX_STEP: f32 = 1.0 / 30.0;

const LENS_HEIGHTS: [f32; 3] = [3.44, 3.1, 2.76];
const LENS_FRONT: f32 = 0.15;

fn street(k: usize) -> f32 {
    (k as f32 - BLOCKS as f32 / 2.0) * PITCH
}

fn block_centre(i: usize) -> f32 {
    (i as f32 + 0.5 - BLOCKS as f32 / 2.0) * PITCH
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CarModel {
    Sedan,
    Hatchback,
    Suv,
    Taxi,
}

impl CarModel {
    const ALL: [CarModel; 4] = [
        CarModel::Sedan,
        CarModel::Hatchback,
        CarModel::Suv,
        CarModel::Taxi,
    ];

    fn mesh_name(self) -> &'static str {
        match self {
            CarModel::Sedan => "sedan_mesh",
            CarModel::Hatchback => "hatchback_mesh",
            CarModel::Suv => "suv_mesh",
            CarModel::Taxi => "taxi_mesh",
        }
    }

    fn length(self) -> f32 {
        match self {
            CarModel::Sedan | CarModel::Taxi => 4.76,
            CarModel::Hatchback => 4.18,
            CarModel::Suv => 4.90,
        }
    }
}

struct Person {
    body: usize,
    legs: [usize; 2],
    centre: Vec2,
    half: f32,
    distance: f32,
    speed: f32,
    direction: f32,
    height: f32,
}

/// Demo 16: same busy city as demo 15, loaded from OBJ meshes and a PNG atlas.
pub struct CityAssetsDemo {
    scene: Scene,
    camera: PerspectiveCamera,
    sky: usize,
    traffic: Traffic,
    car_nodes: Vec<(usize, CarModel)>,
    people: Vec<Person>,
    lamps: Vec<(usize, Axis, Mat4)>,
    last_time: Option<f32>,
}

fn sun() -> Vec3 {
    Vec3::new(-0.52, -0.38, 0.62).normalize()
}

impl CityAssetsDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        Self::with_random(assets, Random::from_seed(2024))
    }

    pub fn with_random(assets: &dyn AssetsProvider, mut random: Random) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_016_city_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let city = find("node `City`", scene.find_node("City"))?;
        let sky = find("node `Sky`", scene.find_node("Sky"))?;
        let statics = find("node `Static`", scene.find_node("Static"))?;

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

        let material = |scene: &Scene, name: &str| -> Result<usize> {
            scene
                .find_material(name)
                .ok_or_else(|| anyhow!("the scene has no material `{name}`"))
        };
        let mesh = |scene: &Scene, name: &str| -> Result<usize> {
            scene
                .find_mesh(name)
                .ok_or_else(|| anyhow!("the scene has no mesh `{name}`"))
        };

        let facade_tints = [
            ("glass_facade", Vec3::new(1.0, 1.0, 1.0)),
            ("brick_facade", Vec3::new(1.0, 0.92, 0.85)),
            ("concrete_facade", Vec3::new(0.95, 0.97, 1.0)),
            ("stone_facade", Vec3::new(1.0, 0.96, 0.9)),
        ];
        for (name, tint) in facade_tints {
            let node = scene.create_node(Some(name.to_string()), statics)?;
            scene.node_set_mesh(node, mesh(&scene, &format!("{name}_mesh"))?)?;
            scene.node_set_material(node, material(&scene, name)?)?;
            let n = scene.get_node_mut(node).unwrap();
            n.set_casts_shadow(true);
            n.uniforms_mut().set("tint", tint);
        }
        for (node_name, mesh_name, material_name) in [
            ("Roofs", "roofs_mesh", "roof"),
            ("Roads", "roads_mesh", "road"),
            ("Intersections", "intersections_mesh", "intersection"),
            ("Crosswalks", "crosswalks_mesh", "crosswalk"),
            ("Sidewalks", "sidewalks_mesh", "sidewalk"),
            ("Curbs", "curbs_mesh", "concrete"),
            ("Grass", "grass_mesh", "grass"),
            ("Tree trunks", "tree_trunks_mesh", "bark"),
            ("Tree tops", "tree_tops_mesh", "leaves"),
            ("Poles", "poles_mesh", "metal"),
            ("Traffic lights", "traffic_lights_mesh", "signal"),
        ] {
            let node = scene.create_node(Some(node_name.to_string()), statics)?;
            scene.node_set_mesh(node, mesh(&scene, mesh_name)?)?;
            scene.node_set_material(node, material(&scene, material_name)?)?;
            scene
                .get_node_mut(node)
                .unwrap()
                .set_casts_shadow(true);
        }

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
        };
        demo.add_lamps(city)?;
        demo.add_traffic(city, &mut random)?;
        demo.add_people(city, &mut random)?;
        demo.camera.set_fov(60.0).set_near_far(0.2, 520.0);
        demo.update(0.0)?;
        Ok(demo)
    }

    /// Traffic-light lamp discs, one per approach, driven every frame.
    fn add_lamps(&mut self, city: usize) -> Result<()> {
        let lamp_mesh = self
            .scene
            .find_mesh("lamp_mesh")
            .ok_or_else(|| anyhow!("the scene has no mesh `lamp_mesh`"))?;
        let metal = self
            .scene
            .find_material("metal")
            .ok_or_else(|| anyhow!("the scene has no material `metal`"))?;
        for k in 0..=BLOCKS {
            let s = street(k);
            for l in 0..=BLOCKS {
                let t = street(l);
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
                    let head = Mat4::from_translation(base) * Mat4::from_angle_z(Rad(facing));
                    let lamp = self.scene.create_node(None, city)?;
                    self.scene.node_set_mesh(lamp, lamp_mesh)?;
                    self.scene.node_set_material(lamp, metal)?;
                    self.scene
                        .get_node_mut(lamp)
                        .unwrap()
                        .set_casts_shadow(true);
                    self.lamps.push((lamp, axis, head));
                }
            }
        }
        Ok(())
    }

    fn add_traffic(&mut self, city: usize, random: &mut Random) -> Result<()> {
        let crossings: Vec<f32> = (0..=BLOCKS).map(street).collect();
        let stop_back = ROAD_HALF + 3.6;
        for k in 0..=BLOCKS {
            let s = street(k);
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

        let meshes: Result<Vec<(CarModel, usize)>> = CarModel::ALL
            .iter()
            .map(|&model| {
                let handle = self
                    .scene
                    .find_mesh(model.mesh_name())
                    .ok_or_else(|| anyhow!("the scene has no mesh `{}`", model.mesh_name()))?;
                Ok((model, handle))
            })
            .collect();
        let meshes = meshes?;
        let car_material = self
            .scene
            .find_material("car")
            .ok_or_else(|| anyhow!("the scene has no material `car`"))?;
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
                self.scene.node_set_material(node, car_material)?;
                let colour = if model == CarModel::Taxi {
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

    fn add_people(&mut self, city: usize, random: &mut Random) -> Result<()> {
        let body = self
            .scene
            .find_mesh("person_mesh")
            .ok_or_else(|| anyhow!("the scene has no mesh `person_mesh`"))?;
        let leg = self
            .scene
            .find_mesh("leg_mesh")
            .ok_or_else(|| anyhow!("the scene has no mesh `leg_mesh`"))?;
        let person_material = self
            .scene
            .find_material("person")
            .ok_or_else(|| anyhow!("the scene has no material `person`"))?;
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
                    self.scene.node_set_material(node, person_material)?;
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
                            .node_set_material(*leg_node, person_material)?;
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

    fn place(&mut self, time: f32) {
        for (car, &(node, model)) in self.traffic.cars.iter().zip(&self.car_nodes) {
            let lane = &self.traffic.lanes[car.lane];
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

        let distance = time * CAMERA_SPEED;
        let (eye, _) = route(distance);
        let (ahead, _) = route(distance + 18.0);
        let eye = Point3::new(eye.x, eye.y, CAMERA_HEIGHT);
        let target = Point3::new(ahead.x, ahead.y, CAMERA_HEIGHT - 1.2);
        self.camera.set_view(eye, target, Vec3::new(0.0, 0.0, 1.0));

        self.scene.get_node_mut(self.sky).unwrap().set_transform(
            Mat4::from_translation(Vec3::new(eye.x, eye.y, eye.z)) * Mat4::from_scale(480.0),
        );

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

impl Demo for CityAssetsDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn camera(&mut self) -> &mut dyn Camera {
        &mut self.camera
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

        Ok(())
    }
}

fn square_loop(half: f32, distance: f32) -> (Vec2, f32) {
    rounded_square(half, 1.0, distance)
}

fn route(distance: f32) -> (Vec2, f32) {
    rounded_square(ROUTE_HALF, ROUTE_CORNER, distance)
}

fn rounded_square(half: f32, radius: f32, distance: f32) -> (Vec2, f32) {
    let straight = 2.0 * (half - radius);
    let arc = FRAC_PI_2 * radius;
    let side = straight + arc;
    let d = distance.rem_euclid(4.0 * side);
    let k = (d / side).floor() as i32;
    let local = d - k as f32 * side;
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
    let turn = k as f32 * FRAC_PI_2;
    let (s, c) = turn.sin_cos();
    let rotated = Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c);
    (rotated, (heading + turn).rem_euclid(TAU))
}
