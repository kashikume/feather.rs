//! The city's cars: bodies and cabins extruded from side profiles, with wheels, bumpers and
//! mirrors, textured from the car texture (`car_regions`).

use cgmath::{Deg, InnerSpace};

use feather::math::{Mat4, Vec2, Vec3};

use super::geometry::MeshData;
use super::textures::car_regions::*;

/// The kinds of cars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarModel {
    Sedan,
    Hatchback,
    Suv,
    /// A yellow sedan with a sign on its roof.
    Taxi,
}

/// A car's shape: side profiles (x forward, z up) and sizes.
struct Shape {
    /// The lower body, around its outline.
    body: &'static [(f32, f32)],
    body_half_width: f32,
    /// The cabin, around its outline; it narrows from `cabin_half_width.0` at its bottom to
    /// `.1` at its top.
    cabin: &'static [(f32, f32)],
    cabin_half_width: (f32, f32),
    wheel_radius: f32,
}

const SEDAN: Shape = Shape {
    body: &[
        (2.3, 0.34),
        (2.39, 0.52),
        (2.34, 0.76),
        (1.45, 0.9),
        (-1.55, 0.95),
        (-2.22, 0.92),
        (-2.32, 0.7),
        (-2.28, 0.34),
    ],
    body_half_width: 0.89,
    cabin: &[(1.38, 0.9), (0.38, 1.4), (-0.75, 1.43), (-1.52, 0.95)],
    cabin_half_width: (0.8, 0.66),
    wheel_radius: 0.33,
};

const HATCHBACK: Shape = Shape {
    body: &[
        (2.0, 0.33),
        (2.08, 0.52),
        (2.03, 0.74),
        (1.2, 0.89),
        (-1.85, 0.94),
        (-2.0, 0.9),
        (-2.03, 0.6),
        (-1.98, 0.33),
    ],
    body_half_width: 0.85,
    cabin: &[(1.15, 0.89), (0.18, 1.42), (-1.55, 1.44), (-1.95, 0.94)],
    cabin_half_width: (0.78, 0.65),
    wheel_radius: 0.31,
};

const SUV: Shape = Shape {
    body: &[
        (2.35, 0.42),
        (2.43, 0.66),
        (2.37, 0.98),
        (1.5, 1.1),
        (-2.18, 1.13),
        (-2.36, 1.08),
        (-2.38, 0.74),
        (-2.33, 0.42),
    ],
    body_half_width: 0.95,
    cabin: &[(1.45, 1.1), (0.52, 1.72), (-1.9, 1.74), (-2.3, 1.13)],
    cabin_half_width: (0.88, 0.78),
    wheel_radius: 0.39,
};

impl CarModel {
    pub const ALL: [CarModel; 4] = [
        CarModel::Sedan,
        CarModel::Hatchback,
        CarModel::Suv,
        CarModel::Taxi,
    ];

    fn shape(self) -> &'static Shape {
        match self {
            CarModel::Sedan | CarModel::Taxi => &SEDAN,
            CarModel::Hatchback => &HATCHBACK,
            CarModel::Suv => &SUV,
        }
    }

    /// From the rear bumper to the front one.
    pub fn length(self) -> f32 {
        let xs = self.shape().body.iter().map(|p| p.0);
        xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min)
    }

    /// The car's mesh: facing +X, its middle at the origin, wheels on z = 0.
    pub fn mesh(self) -> MeshData {
        let shape = self.shape();
        let mut m = MeshData::default();
        let w = shape.body_half_width;
        extrude(&mut m, shape.body, |_| w, Part::Body);
        let (bottom, top) = shape.cabin_half_width;
        let (z0, z1) = z_range(shape.cabin);
        extrude(
            &mut m,
            shape.cabin,
            |z| bottom + (top - bottom) * (z - z0) / (z1 - z0),
            Part::Cabin,
        );

        // Bumpers, black plastic, a little wider than the body.
        let (front, rear) = x_range(shape.body);
        let (low, _) = z_range(shape.body);
        let bumper = |m: &mut MeshData, x0: f32, x1: f32| {
            m.box_in(
                Mat4::from_scale(1.0),
                Vec3::new(x0, -w - 0.02, low + 0.04),
                Vec3::new(x1, w + 0.02, low + 0.2),
                inset(PLASTIC),
            );
        };
        bumper(&mut m, front - 0.12, front + 0.06);
        bumper(&mut m, rear - 0.06, rear + 0.12);

        // Side mirrors at the bottom of the windshield, in the body's colour.
        let (mx, mz) = shape.cabin[0];
        for side in [-1.0f32, 1.0] {
            let y0 = side * (bottom + 0.02);
            let y1 = side * (bottom + 0.2);
            m.box_in(
                Mat4::from_scale(1.0),
                Vec3::new(mx - 0.32, y0.min(y1), mz + 0.08),
                Vec3::new(mx - 0.18, y0.max(y1), mz + 0.2),
                inset(PAINT),
            );
        }

        // Wheels: tyres and rims, flush with the body's sides, at the wheel wells.
        let length = front - rear;
        let r = shape.wheel_radius;
        for &fraction in &WHEELS {
            let x = front - fraction * length;
            for side in [-1.0f32, 1.0] {
                // Cylinders are built along +Z: turn them to point out of the car's side.
                let out = Mat4::from_angle_x(Deg(-90.0 * side));
                let tyre = Mat4::from_translation(Vec3::new(x, side * (w - 0.2), r)) * out;
                m.cylinder(tyre, r, 0.23, 20, inset(TIRE), inset(PLASTIC));
                let rim = Mat4::from_translation(Vec3::new(x, side * (w + 0.03), r)) * out;
                m.cylinder(rim, r * 0.7, 0.012, 20, inset(PLASTIC), inset(RIM));
            }
        }

        if self == CarModel::Taxi {
            let (_, roof) = z_range(shape.cabin);
            let x = (shape.cabin[1].0 + shape.cabin[2].0) / 2.0;
            m.box_in(
                Mat4::from_scale(1.0),
                Vec3::new(x - 0.1, -0.32, roof),
                Vec3::new(x + 0.1, 0.32, roof + 0.2),
                inset(TAXI_SIGN),
            );
        }
        m
    }
}

/// The parts of a car built by `extrude`, textured differently.
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Body,
    Cabin,
}

/// `region` shrunk by a few texels, so mipmaps don't bleed in neighbouring regions.
fn inset(region: [f32; 4]) -> [f32; 4] {
    let pad = 0.004;
    [
        region[0] + pad,
        region[1] + pad,
        region[2] - pad,
        region[3] - pad,
    ]
}

/// Maps (x, y) in 0..1 into `region`.
fn within(region: [f32; 4], x: f32, y: f32) -> Vec2 {
    let r = inset(region);
    Vec2::new(
        r[0] + (r[2] - r[0]) * x.clamp(0.0, 1.0),
        r[1] + (r[3] - r[1]) * y.clamp(0.0, 1.0),
    )
}

fn x_range(profile: &[(f32, f32)]) -> (f32, f32) {
    let xs = profile.iter().map(|p| p.0);
    (
        xs.clone().fold(f32::MIN, f32::max),
        xs.fold(f32::MAX, f32::min),
    )
}

fn z_range(profile: &[(f32, f32)]) -> (f32, f32) {
    let zs = profile.iter().map(|p| p.1);
    (
        zs.clone().fold(f32::MAX, f32::min),
        zs.fold(f32::MIN, f32::max),
    )
}

/// Extrudes the side `profile` (x, z) across the car, `half_width(z)` to each side: two side
/// faces and a face along every edge of the profile, textured by what they face.
fn extrude(m: &mut MeshData, profile: &[(f32, f32)], half_width: impl Fn(f32) -> f32, part: Part) {
    let (front, rear) = x_range(profile);
    let (low, high) = z_range(profile);
    let centre = profile
        .iter()
        .fold(Vec2::new(0.0, 0.0), |a, p| a + Vec2::new(p.0, p.1))
        / profile.len() as f32;

    // The sides: the car's length runs along the texture, front first.
    let side_region = if part == Part::Body {
        SIDE
    } else {
        SIDE_WINDOWS
    };
    for side in [-1.0f32, 1.0] {
        let points: Vec<Vec3> = profile
            .iter()
            .map(|&(x, z)| Vec3::new(x, side * half_width(z), z))
            .collect();
        let uvs: Vec<Vec2> = profile
            .iter()
            .map(|&(x, z)| {
                within(
                    side_region,
                    (front - x) / (front - rear),
                    (high - z) / (high - low),
                )
            })
            .collect();
        m.polygon(&points, &uvs, Vec3::new(0.0, side, 0.0));
    }

    // Where the faces looking forward and back are, for the front and rear textures.
    let edges: Vec<((f32, f32), (f32, f32), Vec2)> = (0..profile.len())
        .map(|i| {
            let (a, b) = (profile[i], profile[(i + 1) % profile.len()]);
            let along = Vec2::new(b.0 - a.0, b.1 - a.1);
            let mut normal = Vec2::new(along.y, -along.x).normalize();
            let middle = Vec2::new(a.0 + b.0, a.1 + b.1) / 2.0;
            if normal.dot(middle - centre) < 0.0 {
                normal = -normal;
            }
            (a, b, normal)
        })
        .collect();
    let facing_range = |sign: f32| {
        let zs = edges
            .iter()
            .filter(|e| e.2.x * sign > 0.6)
            .flat_map(|e| [e.0 .1, e.1 .1]);
        (
            zs.clone().fold(f32::MAX, f32::min),
            zs.fold(f32::MIN, f32::max),
        )
    };
    let (front_range, rear_range) = (facing_range(1.0), facing_range(-1.0));

    for &(a, b, normal) in &edges {
        let corners = [
            Vec3::new(a.0, -half_width(a.1), a.1),
            Vec3::new(b.0, -half_width(b.1), b.1),
            Vec3::new(b.0, half_width(b.1), b.1),
            Vec3::new(a.0, half_width(a.1), a.1),
        ];
        let outward = Vec3::new(normal.x, 0.0, normal.y);
        if normal.y < -0.6 {
            // Underneath: plain dark, and the cabin's bottom is hidden in the body.
            if part == Part::Body {
                m.polygon(&corners, &[within(PLASTIC, 0.5, 0.5); 4], outward);
            }
            continue;
        }
        let uvs: Vec<Vec2> = match part {
            Part::Body if normal.x.abs() > 0.6 => {
                // Front or rear, seen from in front of it.
                let (region, (z0, z1), flip) = if normal.x > 0.0 {
                    (FRONT, front_range, 1.0)
                } else {
                    (REAR, rear_range, -1.0)
                };
                corners
                    .iter()
                    .map(|p| {
                        within(
                            region,
                            0.5 - flip * p.y / (2.0 * half_width(p.z)),
                            (z1 - p.z) / (z1 - z0).max(1e-3),
                        )
                    })
                    .collect()
            }
            Part::Cabin if normal.x.abs() > 0.35 => {
                // Windshield and rear window.
                let ends = [0.0, 0.0, 1.0, 1.0];
                corners
                    .iter()
                    .zip(ends)
                    .map(|(p, along)| {
                        within(WINDSHIELD, 0.5 + p.y / (2.0 * half_width(p.z)), along)
                    })
                    .collect()
            }
            _ => [within(PAINT, 0.5, 0.5); 4].to_vec(),
        };
        m.polygon(&corners, &uvs, outward);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cars_face_outwards_and_stand_on_their_wheels() {
        for model in CarModel::ALL {
            let mesh = model.mesh();
            assert!(mesh.indices.len() > 300, "{model:?}");
            let lowest = mesh
                .vertices
                .iter()
                .map(|v| v.pos.z)
                .fold(f32::MAX, f32::min);
            assert!(lowest.abs() < 1e-3, "{model:?}: {lowest}");
            let length = model.length();
            assert!((3.9..4.9).contains(&length), "{model:?}: {length}");
            // Every triangle is wound the way its normals face.
            for t in mesh.indices.chunks(3) {
                let [a, b, c] = [0, 1, 2].map(|k| mesh.vertices[t[k] as usize]);
                let n = (b.pos - a.pos).cross(c.pos - a.pos);
                if n.magnitude() > 1e-9 {
                    assert!(
                        n.dot(a.normal + b.normal + c.normal) > 0.0,
                        "{model:?}: {t:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn suvs_are_taller_than_hatchbacks() {
        let top = |m: CarModel| {
            m.mesh()
                .vertices
                .iter()
                .map(|v| v.pos.z)
                .fold(0.0, f32::max)
        };
        assert!(top(CarModel::Suv) > top(CarModel::Hatchback) + 0.2);
        assert!(top(CarModel::Taxi) > top(CarModel::Sedan));
    }
}
