use std::f32::consts::TAU;

use anyhow::{anyhow, Result};

use feather::assetsprovider::AssetsProvider;
use feather::math::{Mat4, Vec3};
use feather::scene::Scene;

use super::random::Random;
use super::testapp::Demo;

/// Number of balls.
const BALLS: usize = 12;
/// Half the size of the floor plate along X and Y; its top is at z = 0.
const FLOOR_HALF: f32 = 4.0;
/// Downward acceleration, units per second squared.
const GRAVITY: f32 = 9.8;
/// Share of the vertical speed a ball keeps when it bounces.
const RESTITUTION: f32 = 0.72;
/// A bounce always sends a ball up at least this fast, so balls never come to rest.
const MIN_BOUNCE_SPEED: f32 = 3.5;
/// Below this height a ball has left the screen, and starts again from the top.
const BOTTOM: f32 = -9.0;
/// Height of a shadow disc's centre above the floor's top, so it isn't hidden by the floor.
const SHADOW_LIFT: f32 = 0.004;

/// Longest step of the simulation, in seconds; longer frames are split.
const MAX_STEP: f32 = 1.0 / 120.0;

/// One ball and its motion.
struct Ball {
    node: usize,
    /// The ball's shadow: a flat disc on the floor below it.
    shadow: usize,
    radius: f32,
    position: Vec3,
    velocity: Vec3,
}

/// Demo 6: balls bouncing on a floor, in a loop. No textures: every object has its own colour.
///
/// The scene file has the floor plate (a flat cuboid with its top at z = 0), the ball mesh, the
/// shader and its materials, and the empty `Balls` and `Shadows` nodes; this demo adds a node
/// per ball, each with its own `color` uniform (the floor's is in the scene file), and moves
/// them.
///
/// Each ball drops from above, bounces on the floor and drifts sideways until it passes the
/// floor's edge, falls out of the screen and starts again from above.
///
/// The engine has no shadows, so each ball gets a dark disc on the floor below it (the ball mesh
/// squashed flat), smaller the higher the ball is. The discs are transparent nodes, so the
/// floor shows through them (their material's `opacity` is below 1) and the engine draws them
/// after the floor and the balls, the farthest first. They inherit their `color` from the
/// `Shadows` node.
pub struct BouncingBallsDemo {
    scene: Scene,
    balls: Vec<Ball>,
    random: Random,
    /// Simulated time, in seconds since the demo started.
    time: f32,
}

impl BouncingBallsDemo {
    pub fn new(assets: &dyn AssetsProvider) -> Result<Self> {
        Self::with_random(assets, Random::from_clock())
    }

    fn with_random(assets: &dyn AssetsProvider, mut random: Random) -> Result<Self> {
        let mut scene = Scene::load(assets, "demo_006_bouncing_balls_scene.json")?;
        let find = |what: &str, handle: Option<usize>| {
            handle.ok_or_else(|| anyhow!("the scene has no {what}"))
        };
        let balls_node = find("node `Balls`", scene.find_node("Balls"))?;
        let shadows_node = find("node `Shadows`", scene.find_node("Shadows"))?;
        let shadow_material = find("material `Shadow`", scene.find_material("Shadow"))?;
        let mesh = find("mesh `Ball`", scene.find_mesh("Ball"))?;
        let material = find("material `Glossy`", scene.find_material("Glossy"))?;

        let mut balls = Vec::with_capacity(BALLS);
        for i in 0..BALLS {
            let node = scene.create_node(Some(format!("Ball {}", i + 1)), balls_node)?;
            scene.node_set_mesh(node, mesh)?;
            scene.node_set_material(node, material)?;
            // Hues spread evenly around the colour wheel, so every ball has its own colour.
            let hue = i as f32 / BALLS as f32;
            scene
                .get_node_mut(node)
                .unwrap()
                .uniforms_mut()
                .set("color", hsv_to_rgb(hue, 0.75, 0.95));

            let shadow = scene.create_node(Some(format!("Shadow {}", i + 1)), shadows_node)?;
            scene.node_set_mesh(shadow, mesh)?;
            scene.node_set_material(shadow, shadow_material)?;
            scene.get_node_mut(shadow).unwrap().set_transparent(true);

            let mut ball = Ball {
                node,
                shadow,
                radius: random.range(0.25, 0.5),
                position: Vec3::new(0.0, 0.0, 0.0),
                velocity: Vec3::new(0.0, 0.0, 0.0),
            };
            respawn(&mut ball, &mut random);
            // Spread the first drops over time: some balls start higher.
            ball.position.z += random.range(0.0, 5.0);
            balls.push(ball);
        }

        // The scene file's camera: in front of the floor and above it, looking at its middle.
        let mut demo = Self {
            scene,
            balls,
            random,
            time: 0.0,
        };
        demo.place_balls();
        Ok(demo)
    }

    /// Moves every ball by `dt` seconds: gravity, bounces on the floor, and a new start from
    /// the top for balls that left the screen.
    fn step(&mut self, dt: f32) {
        for ball in &mut self.balls {
            step_ball(ball, dt, &mut self.random);
        }
    }

    /// Puts every ball's node where the simulation has it, scaled to its radius (the mesh is a
    /// ball of radius 1), and its shadow on the floor below it.
    fn place_balls(&mut self) {
        for ball in &self.balls {
            let transform = Mat4::from_translation(ball.position) * Mat4::from_scale(ball.radius);
            self.scene
                .get_node_mut(ball.node)
                .unwrap()
                .set_transform(transform);

            // Only over the floor and above it; it shrinks as the ball rises.
            let p = ball.position;
            let visible =
                p.x.abs() <= FLOOR_HALF && p.y.abs() <= FLOOR_HALF && p.z >= ball.radius - 0.01;
            let height = (p.z - ball.radius).max(0.0);
            let size = ball.radius * (1.0 - (height / 6.0).min(0.6));
            let shadow = self.scene.get_node_mut(ball.shadow).unwrap();
            shadow.set_visible(visible);
            shadow.set_transform(
                Mat4::from_translation(Vec3::new(p.x, p.y, SHADOW_LIFT))
                    * Mat4::from_nonuniform_scale(size, size, SHADOW_LIFT * 0.5),
            );
        }
    }
}

/// Moves one ball by `dt` seconds: gravity, a bounce on the floor, and a new start from the top
/// once it has left the screen.
fn step_ball(ball: &mut Ball, dt: f32, random: &mut Random) {
    ball.velocity.z -= GRAVITY * dt;
    ball.position += ball.velocity * dt;

    let over_floor = ball.position.x.abs() <= FLOOR_HALF && ball.position.y.abs() <= FLOOR_HALF;
    // Only balls coming down onto the floor's top bounce; a ball that fell past the edge stays
    // below the floor even if it drifts back under it.
    let touches_top = ball.position.z < ball.radius && ball.position.z > ball.radius - 0.5;
    if over_floor && touches_top && ball.velocity.z < 0.0 {
        ball.position.z = ball.radius;
        ball.velocity.z = (-ball.velocity.z * RESTITUTION).max(MIN_BOUNCE_SPEED);
    }

    if ball.position.z < BOTTOM {
        respawn(ball, random);
    }
}

/// Starts a ball again: above the middle of the floor, moving sideways in a random direction,
/// fast enough to reach the floor's edge after a few bounces.
fn respawn(ball: &mut Ball, random: &mut Random) {
    ball.position = Vec3::new(
        random.range(-2.0, 2.0),
        random.range(-2.0, 2.0),
        random.range(4.0, 6.0),
    );
    let direction = random.range(0.0, TAU);
    let speed = random.range(1.2, 2.8);
    ball.velocity = Vec3::new(
        speed * direction.cos(),
        speed * direction.sin(),
        random.range(0.0, 2.0),
    );
}

/// A colour from hue, saturation and value, each in `0.0..=1.0`.
fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> Vec3 {
    let channel = |n: f32| {
        let k = (n + hue * 6.0) % 6.0;
        value - value * saturation * (k.min(4.0 - k).clamp(0.0, 1.0))
    };
    Vec3::new(channel(5.0), channel(3.0), channel(1.0))
}

impl Demo for BouncingBallsDemo {
    fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    fn update(&mut self, time: f32) -> Result<()> {
        // Catch up with `time` in short steps, so fast balls don't pass through the floor.
        // After a long pause (e.g. a moved window), skip ahead instead of simulating it all.
        let mut remaining = (time - self.time).clamp(0.0, 0.25);
        self.time = time;
        while remaining > 0.0 {
            let dt = remaining.min(MAX_STEP);
            self.step(dt);
            remaining -= dt;
        }
        self.place_balls();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_primaries() {
        let close = |a: Vec3, b: Vec3| (a - b).x.abs() + (a - b).y.abs() + (a - b).z.abs() < 1e-5;
        assert!(close(hsv_to_rgb(0.0, 1.0, 1.0), Vec3::new(1.0, 0.0, 0.0)));
        assert!(close(
            hsv_to_rgb(1.0 / 3.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0)
        ));
        assert!(close(
            hsv_to_rgb(2.0 / 3.0, 1.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0)
        ));
        assert!(close(hsv_to_rgb(0.5, 0.0, 0.5), Vec3::new(0.5, 0.5, 0.5)));
    }

    fn ball(position: Vec3, velocity: Vec3) -> Ball {
        Ball {
            node: 0,
            shadow: 0,
            radius: 0.5,
            position,
            velocity,
        }
    }

    /// Simulates `balls` for `seconds`, calling `check` after every step.
    fn simulate(mut balls: Vec<Ball>, seconds: f32, mut check: impl FnMut(&Ball)) -> Vec<Ball> {
        let mut random = Random::from_seed(3);
        let steps = (seconds / MAX_STEP) as usize;
        for _ in 0..steps {
            for ball in &mut balls {
                step_ball(ball, MAX_STEP, &mut random);
                check(ball);
            }
        }
        balls
    }

    #[test]
    fn ball_over_the_floor_never_sinks_into_it() {
        // Dropped straight down in the middle: it keeps bouncing.
        let balls = vec![ball(Vec3::new(0.0, 0.0, 6.0), Vec3::new(0.0, 0.0, 0.0))];
        simulate(balls, 20.0, |b| {
            assert!(b.position.z >= b.radius - 1e-4, "{:?}", b.position);
        });
    }

    #[test]
    fn ball_leaves_the_floor_and_starts_again() {
        // Moving sideways: it passes the edge, falls, and starts again from above.
        let balls = vec![ball(Vec3::new(3.0, 0.0, 3.0), Vec3::new(2.0, 0.0, 0.0))];
        let mut fell_below_floor = false;
        let mut started_again = false;
        simulate(balls, 15.0, |b| {
            fell_below_floor |= b.position.z < -2.0;
            started_again |= fell_below_floor && b.position.z > 4.0;
        });
        assert!(fell_below_floor && started_again);
    }

    #[test]
    fn respawn_starts_above_the_floor_moving_sideways() {
        let mut random = Random::from_seed(9);
        let mut b = ball(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        for _ in 0..100 {
            respawn(&mut b, &mut random);
            assert!(b.position.z >= 4.0);
            assert!(b.position.x.abs() <= 2.0 && b.position.y.abs() <= 2.0);
            let sideways = (b.velocity.x * b.velocity.x + b.velocity.y * b.velocity.y).sqrt();
            assert!(sideways >= 1.2 - 1e-4, "{sideways}");
        }
    }
}
