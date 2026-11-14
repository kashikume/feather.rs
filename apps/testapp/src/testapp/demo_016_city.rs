//! Demo 16: the city again, but every model is an OBJ and every texture is a PNG packed into
//! an atlas at build time (`tools/generate_demo_016_city_assets.py`). Geometry is denser than
//! demo 15 (recessed windows, richer people/trees). The vehicles are low-poly models imported
//! from an OBJ, with a texture of their own (`vehicles.png`).

mod traffic;

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use anyhow::{anyhow, Result};
use cgmath::{InnerSpace, Rad};
use winit::event::MouseButton;

use feather::assetsprovider::AssetsProvider;
use feather::math::{Mat4, Point3, Vec2, Vec3};
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

/// A vehicle model: its mesh, its length (as the importer prints it), how often it comes up
/// (the weights add up to 1) and the colours its body is painted in (sRGB; the first is the
/// model's own).
struct CarModel {
    mesh: &'static str,
    length: f32,
    weight: f32,
    paints: &'static [[f32; 3]],
}

const MILK_WHITE: [f32; 3] = [0.94, 0.92, 0.88];
const SILVER: [f32; 3] = [0.74, 0.76, 0.79];
const NAVY: [f32; 3] = [0.08, 0.13, 0.36];
const SKY_BLUE: [f32; 3] = [0.60, 0.80, 0.93];
const RED_ORANGE: [f32; 3] = [0.93, 0.32, 0.17];
const PINK: [f32; 3] = [0.95, 0.68, 0.72];
const MINT: [f32; 3] = [0.52, 0.80, 0.66];
const YELLOW: [f32; 3] = [0.97, 0.82, 0.30];
const RED: [f32; 3] = [0.78, 0.12, 0.16];
const GREEN: [f32; 3] = [0.16, 0.42, 0.30];
const CHARCOAL: [f32; 3] = [0.16, 0.16, 0.18];

const CAR_MODELS: [CarModel; 8] = [
    CarModel {
        mesh: "hatchback_mesh",
        length: 3.82,
        weight: 0.2,
        paints: &[SKY_BLUE, PINK, YELLOW, MINT, RED_ORANGE],
    },
    CarModel {
        mesh: "sedan_mesh",
        length: 5.10,
        weight: 0.2,
        paints: &[NAVY, RED, CHARCOAL, GREEN, SILVER],
    },
    CarModel {
        mesh: "suv_mesh",
        length: 4.65,
        weight: 0.14,
        paints: &[RED_ORANGE, MINT, YELLOW, NAVY, MILK_WHITE],
    },
    CarModel {
        mesh: "van_mesh",
        length: 5.05,
        weight: 0.12,
        paints: &[MILK_WHITE, SILVER, SKY_BLUE],
    },
    CarModel {
        mesh: "delivery_mesh",
        length: 4.76,
        weight: 0.1,
        paints: &[PINK, MINT, YELLOW, SKY_BLUE],
    },
    CarModel {
        mesh: "truck_mesh",
        length: 9.59,
        weight: 0.1,
        paints: &[[0.20, 0.20, 0.36], RED, GREEN, MILK_WHITE],
    },
    CarModel {
        mesh: "pickup_mesh",
        length: 4.81,
        weight: 0.08,
        paints: &[MILK_WHITE, SILVER],
    },
    CarModel {
        mesh: "bus_mesh",
        length: 7.67,
        weight: 0.06,
        paints: &[[0.97, 0.63, 0.12]],
    },
];

/// An sRGB colour as the linear `tint` the shader multiplies the texture with.
fn linear(srgb: [f32; 3]) -> Vec3 {
    Vec3::new(srgb[0].powf(2.2), srgb[1].powf(2.2), srgb[2].powf(2.2))
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
    /// The cameras, in `View` order (named as the views in the scene file).
    cameras: [usize; View::ALL.len()],
    /// What the active camera shows.
    view: View,
    /// The car the `Chase` camera follows (an index into `traffic.cars`).
    chased: usize,
    /// Where the left button (or a finger) went down, while it is down.
    press: Option<(f32, f32)>,
    /// The shorter side of the window, in pixels: how far a tap may move is a part of it.
    window_side: f32,
    sky: usize,
    traffic: Traffic,
    car_nodes: Vec<usize>,
    people: Vec<Person>,
    lamps: Vec<([usize; 2], Axis, Mat4)>,
    last_time: Option<f32>,
}

/// The demo's cameras. A tap (or a click) switches to the next one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    /// Gliding through the streets, along `route`.
    Street,
    /// Fixed, above the intersection at the park.
    Crossroads,
    /// Behind a car, following it; another car each time.
    Chase,
    /// Circling high over the city.
    Above,
}

impl View {
    const ALL: [View; 4] = [View::Street, View::Crossroads, View::Chase, View::Above];

    /// Also the camera's name in the scene file.
    fn name(self) -> &'static str {
        match self {
            View::Street => "Street",
            View::Crossroads => "Crossroads",
            View::Chase => "Chase",
            View::Above => "Above",
        }
    }

    fn next(self) -> View {
        View::ALL[(self as usize + 1) % View::ALL.len()]
    }
}

/// How far a finger or the mouse may move between press and release for a tap, as a part of
/// the window's shorter side.
const TAP_SLOP: f32 = 0.04;
/// The `Chase` camera picks cars this near the middle of the city (along both axes), and
/// moves to another one when its car is past the edge of the city.
const CHASE_PICK: f32 = 80.0;
const CHASE_LEAVE: f32 = 125.0;
/// The `Above` camera's circle: radius, height and speed (radians per second).
const ABOVE_RADIUS: f32 = 95.0;
const ABOVE_HEIGHT: f32 = 60.0;
const ABOVE_SPEED: f32 = 0.05;

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
        let mut cameras = [0; View::ALL.len()];
        for (camera, view) in cameras.iter_mut().zip(View::ALL) {
            *camera = scene
                .find_camera(view.name())
                .ok_or_else(|| anyhow!("the scene has no camera `{}`", view.name()))?;
        }
        scene.set_active_camera(cameras[View::Street as usize])?;
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
            scene.get_node_mut(node).unwrap().set_casts_shadow(true);
        }

        let mut demo = Self {
            scene,
            cameras,
            view: View::Street,
            chased: 0,
            press: None,
            window_side: 768.0,
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
                    // Two discs: red and yellow light together before green.
                    let mut lamps = [0; 2];
                    for lamp in &mut lamps {
                        *lamp = self.scene.create_node(None, city)?;
                        self.scene.node_set_mesh(*lamp, lamp_mesh)?;
                        self.scene.node_set_material(*lamp, metal)?;
                        self.scene
                            .get_node_mut(*lamp)
                            .unwrap()
                            .set_casts_shadow(true);
                    }
                    self.lamps.push((lamps, axis, head));
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

        let meshes: Result<Vec<usize>> = CAR_MODELS
            .iter()
            .map(|model| {
                self.scene
                    .find_mesh(model.mesh)
                    .ok_or_else(|| anyhow!("the scene has no mesh `{}`", model.mesh))
            })
            .collect();
        let meshes = meshes?;
        let car_material = self
            .scene
            .find_material("vehicles")
            .ok_or_else(|| anyhow!("the scene has no material `vehicles`"))?;
        let cars = self.scene.create_node(Some("Cars".into()), city)?;
        for lane in 0..self.traffic.lanes.len() {
            let count = 6;
            let spacing = LANE_LENGTH / count as f32;
            for c in 0..count {
                let distance = c as f32 * spacing + random.range(0.0, spacing - 15.0);
                let mut pick = random.next();
                let kind = CAR_MODELS
                    .iter()
                    .position(|model| {
                        pick -= model.weight;
                        pick < 0.0
                    })
                    .unwrap_or(0);
                let model = &CAR_MODELS[kind];
                // Buses and trucks drive slower.
                let cruise = random.range(8.0, 13.0) * if model.length > 7.0 { 0.8 } else { 1.0 };
                self.traffic.cars.push(Car {
                    lane,
                    distance,
                    length: model.length,
                    speed: cruise * 0.5,
                    cruise,
                });
                let node = self.scene.create_node(None, cars)?;
                self.scene.node_set_mesh(node, meshes[kind])?;
                self.scene.node_set_material(node, car_material)?;
                let paint = model.paints
                    [(random.next() * model.paints.len() as f32) as usize % model.paints.len()];
                let n = self.scene.get_node_mut(node).unwrap();
                n.set_casts_shadow(true);
                n.uniforms_mut().set("tint", linear(paint));
                self.car_nodes.push(node);
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
                        self.scene.node_set_material(*leg_node, person_material)?;
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
        for (car, &node) in self.traffic.cars.iter().zip(&self.car_nodes) {
            let lane = &self.traffic.lanes[car.lane];
            let (x, y, heading) = lane.place(car.distance - car.length / 2.0);
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

        const RED_GLOW: (usize, Vec3) = (0, Vec3::new(4.0, 0.2, 0.12));
        const YELLOW_GLOW: (usize, Vec3) = (1, Vec3::new(4.0, 2.6, 0.15));
        const GREEN_GLOW: (usize, Vec3) = (2, Vec3::new(0.15, 3.6, 0.8));
        for &(lamps, axis, head) in &self.lamps {
            // The second lamp is lit only with red and yellow together.
            let (first, second) = match signal(axis, time) {
                Signal::Red => (RED_GLOW, None),
                Signal::Yellow => (YELLOW_GLOW, None),
                Signal::Green => (GREEN_GLOW, None),
                Signal::RedYellow => (RED_GLOW, Some(YELLOW_GLOW)),
            };
            for (lamp, glow) in lamps.into_iter().zip([Some(first), second]) {
                let node = self.scene.get_node_mut(lamp).unwrap();
                node.set_visible(glow.is_some());
                if let Some((lens, colour)) = glow {
                    node.set_transform(
                        head * Mat4::from_translation(Vec3::new(
                            LENS_FRONT,
                            0.0,
                            LENS_HEIGHTS[lens],
                        )),
                    );
                    node.uniforms_mut().set("emissive", colour);
                }
            }
        }

        let distance = time * CAMERA_SPEED;
        let (eye, _) = route(distance);
        let (ahead, _) = route(distance + 18.0);
        let eye = Point3::new(eye.x, eye.y, CAMERA_HEIGHT);
        let target = Point3::new(ahead.x, ahead.y, CAMERA_HEIGHT - 1.2);
        self.set_view(View::Street, eye, target);

        // Behind the chased car, a little above it, looking ahead of it.
        let (x, y, _) = self.car_place(self.chased);
        if x.abs().max(y.abs()) > CHASE_LEAVE {
            self.chased = self.pick_chased(self.chased);
        }
        let (x, y, heading) = self.car_place(self.chased);
        let forward = Vec3::new(heading.cos(), heading.sin(), 0.0);
        let back = self.traffic.cars[self.chased].length / 2.0 + 8.0;
        let car = Point3::new(x, y, 0.0);
        self.set_view(
            View::Chase,
            car - forward * back + Vec3::new(0.0, 0.0, 4.0),
            car + forward * 8.0 + Vec3::new(0.0, 0.0, 1.0),
        );

        let angle = time * ABOVE_SPEED;
        self.set_view(
            View::Above,
            Point3::new(
                ABOVE_RADIUS * angle.cos(),
                ABOVE_RADIUS * angle.sin(),
                ABOVE_HEIGHT,
            ),
            Point3::new(0.0, 0.0, 0.0),
        );

        follow_active_camera(&mut self.scene, self.sky);
    }

    fn set_view(&mut self, view: View, eye: Point3, target: Point3) {
        self.scene
            .get_camera_mut(self.cameras[view as usize])
            .unwrap()
            .set_view(eye, target, Vec3::new(0.0, 0.0, 1.0));
    }

    /// The middle of car `index` on the ground, and its heading.
    fn car_place(&self, index: usize) -> (f32, f32, f32) {
        let car = &self.traffic.cars[index];
        self.traffic.lanes[car.lane].place(car.distance - car.length / 2.0)
    }

    /// The first car after `after` (going round) that is near the middle of the city.
    fn pick_chased(&self, after: usize) -> usize {
        let count = self.traffic.cars.len();
        (1..=count)
            .map(|k| (after + k) % count)
            .find(|&index| {
                let (x, y, _) = self.car_place(index);
                x.abs().max(y.abs()) < CHASE_PICK
            })
            .unwrap_or(after)
    }

    /// Switches to the next camera; the `Chase` camera follows another car each time.
    fn next_view(&mut self) -> Result<()> {
        self.view = self.view.next();
        if self.view == View::Chase {
            self.chased = self.pick_chased(self.chased);
        }
        self.scene
            .set_active_camera(self.cameras[self.view as usize])?;
        if let Some(time) = self.last_time {
            self.place(time);
        }
        Ok(())
    }
}

/// Puts the sky dome around the scene's active camera and the shadows around what it sees,
/// moving by whole texels.
fn follow_active_camera(scene: &mut Scene, sky: usize) {
    let Some(camera) = scene.active_camera() else {
        return;
    };
    let (eye, target) = (camera.get_eye_position(), camera.get_target_position());
    scene.get_node_mut(sky).unwrap().set_transform(
        Mat4::from_translation(Vec3::new(eye.x, eye.y, eye.z)) * Mat4::from_scale(480.0),
    );

    // Low cameras see the streets ahead of them; high ones the ground around their target.
    let forward = target - eye;
    let centre = if eye.z > 20.0 {
        target
    } else {
        eye + Vec3::new(forward.x, forward.y, 0.0).normalize() * (SHADOW_RADIUS * 0.45)
    };
    let centre = Point3::new(centre.x, centre.y, 6.0);
    scene.set_shadow_light(Some(ShadowLight::directional_snapped(
        -sun(),
        centre,
        SHADOW_RADIUS,
        SHADOW_MAP_SIZE,
    )));
}

impl Demo for CityAssetsDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
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

    fn caption(&self) -> Option<&str> {
        Some(self.view.name())
    }

    fn on_resized(&mut self, width: u32, height: u32) -> Result<()> {
        self.window_side = width.min(height).max(1) as f32;
        Ok(())
    }

    /// A tap (or a click): press and release without moving far. Drags don't count.
    fn on_mouse_button(
        &mut self,
        button: MouseButton,
        pressed: bool,
        x: f32,
        y: f32,
    ) -> Result<()> {
        if button != MouseButton::Left {
            return Ok(());
        }
        if pressed {
            self.press = Some((x, y));
        } else if let Some((px, py)) = self.press.take() {
            if (x - px).hypot(y - py) <= TAP_SLOP * self.window_side {
                self.next_view()?;
            }
        }
        Ok(())
    }

    fn on_mouse_left(&mut self) -> Result<()> {
        // Touches end with the pointer leaving, after the release; a mouse leaving the window
        // while pressed shouldn't count as a tap later.
        self.press = None;
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
