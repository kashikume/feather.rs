//! Building the city's meshes: quads, boxes, cylinders and ellipsoids gathered into one mesh,
//! with texture coordinates either in world tiles (for repeating textures) or within a region
//! of a texture.

use std::f32::consts::{PI, TAU};

use cgmath::{InnerSpace, Matrix, SquareMatrix, Transform};

use feather::math::{Mat4, Point3, Vec2, Vec3};
use feather::mesh::Mesh;
use feather::vertex::Vertex;

use super::textures::person_regions;

/// Vertices and indices being gathered into a mesh.
#[derive(Default)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn into_mesh(self, name: &str) -> Mesh {
        Mesh::new(Some(name.to_string()), self.vertices, self.indices)
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Splits the triangles into square cells of the ground, `size` wide with a corner at
    /// `origin` (x, y), by where each triangle's centre is; returns the cells' meshes, each
    /// with its cell's (column, row), in order. So a big mesh becomes pieces that frustum
    /// culling can leave out one by one.
    pub fn split_into_cells(self, origin: Vec2, size: f32) -> Vec<((i32, i32), MeshData)> {
        let mut cells: std::collections::BTreeMap<(i32, i32), (MeshData, Vec<u32>)> =
            Default::default();
        for triangle in self.indices.chunks_exact(3) {
            let centre = triangle
                .iter()
                .map(|&i| self.vertices[i as usize].pos)
                .fold(Vec3::new(0.0, 0.0, 0.0), |sum, p| sum + p)
                / 3.0;
            let cell = (
                ((centre.x - origin.x) / size).floor() as i32,
                ((centre.y - origin.y) / size).floor() as i32,
            );
            let (mesh, remap) = cells
                .entry(cell)
                .or_insert_with(|| (MeshData::default(), vec![u32::MAX; self.vertices.len()]));
            for &i in triangle {
                if remap[i as usize] == u32::MAX {
                    remap[i as usize] = mesh.vertices.len() as u32;
                    mesh.vertices.push(self.vertices[i as usize]);
                }
                mesh.indices.push(remap[i as usize]);
            }
        }
        cells
            .into_iter()
            .map(|(cell, (mesh, _))| (cell, mesh))
            .collect()
    }

    /// A flat quad; corners counter-clockwise seen from the front: bottom left, bottom right,
    /// top right, top left.
    pub fn quad(&mut self, corners: [Vec3; 4], uvs: [Vec2; 4]) {
        let normal = (corners[1] - corners[0])
            .cross(corners[3] - corners[0])
            .normalize();
        let first = self.vertices.len() as u32;
        for (p, uv) in corners.into_iter().zip(uvs) {
            self.vertices.push(Vertex::new(p, normal, uv));
        }
        self.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    /// A flat polygon (convex, or star-shaped around its centre) facing `outward`: fanned
    /// from its centre, whatever the order of its points.
    pub fn polygon(&mut self, points: &[Vec3], uvs: &[Vec2], outward: Vec3) {
        let count = points.len() as f32;
        let centre = points.iter().fold(Vec3::new(0.0, 0.0, 0.0), |a, &p| a + p) / count;
        let centre_uv = uvs.iter().fold(Vec2::new(0.0, 0.0), |a, &t| a + t) / count;
        // Newell's normal, turned to face `outward`.
        let mut normal = Vec3::new(0.0, 0.0, 0.0);
        for i in 0..points.len() {
            normal += points[i].cross(points[(i + 1) % points.len()]);
        }
        let flip = normal.dot(outward) < 0.0;
        let normal = if flip { -normal } else { normal }.normalize();
        let first = self.vertices.len() as u32;
        self.vertices.push(Vertex::new(centre, normal, centre_uv));
        for (&p, &uv) in points.iter().zip(uvs) {
            self.vertices.push(Vertex::new(p, normal, uv));
        }
        let n = points.len() as u32;
        for i in 0..n {
            let (a, b) = (first + 1 + i, first + 1 + (i + 1) % n);
            if flip {
                self.indices.extend([first, b, a]);
            } else {
                self.indices.extend([first, a, b]);
            }
        }
    }

    /// The four walls of a box from `min` to `max`, with texture coordinates in tiles: `u`
    /// along a wall in units of `tile.x`, `v` down from the top in units of `tile.y`.
    pub fn walls(&mut self, min: Vec3, max: Vec3, tile: Vec2) {
        let corners = [
            Vec3::new(min.x, min.y, 0.0),
            Vec3::new(max.x, min.y, 0.0),
            Vec3::new(max.x, max.y, 0.0),
            Vec3::new(min.x, max.y, 0.0),
        ];
        let height = (max.z - min.z) / tile.y;
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            let width = (b - a).magnitude() / tile.x;
            self.quad(
                [
                    Vec3::new(a.x, a.y, min.z),
                    Vec3::new(b.x, b.y, min.z),
                    Vec3::new(b.x, b.y, max.z),
                    Vec3::new(a.x, a.y, max.z),
                ],
                [
                    Vec2::new(0.0, height),
                    Vec2::new(width, height),
                    Vec2::new(width, 0.0),
                    Vec2::new(0.0, 0.0),
                ],
            );
        }
    }

    /// A horizontal rectangle facing up at height `z`, with texture coordinates in world tiles
    /// of `tile` (`u` along X, `v` along -Y), so neighbouring pieces continue each other.
    pub fn top(&mut self, min: Vec2, max: Vec2, z: f32, tile: f32) {
        let uv = |x: f32, y: f32| Vec2::new(x / tile, -y / tile);
        self.quad(
            [
                Vec3::new(min.x, min.y, z),
                Vec3::new(max.x, min.y, z),
                Vec3::new(max.x, max.y, z),
                Vec3::new(min.x, max.y, z),
            ],
            [
                uv(min.x, min.y),
                uv(max.x, min.y),
                uv(max.x, max.y),
                uv(min.x, max.y),
            ],
        );
    }

    /// A horizontal rectangle at height `z` along X (`along_x`) or Y, with the texture once
    /// across it (`u`) and repeating along it (`v`) every `tile`.
    pub fn strip(&mut self, min: Vec2, max: Vec2, z: f32, along_x: bool, tile: f32) {
        let c = [
            Vec3::new(min.x, min.y, z),
            Vec3::new(max.x, min.y, z),
            Vec3::new(max.x, max.y, z),
            Vec3::new(min.x, max.y, z),
        ];
        let uvs = if along_x {
            // Across is Y (u), along is X (v).
            let len = (max.x - min.x) / tile;
            [
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, len),
                Vec2::new(1.0, len),
                Vec2::new(1.0, 0.0),
            ]
        } else {
            let len = (max.y - min.y) / tile;
            [
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, len),
                Vec2::new(1.0, len),
            ]
        };
        self.quad(c, uvs);
    }

    /// A box from `min` to `max` mapped into one texture region on every face.
    pub fn box_in(&mut self, transform: Mat4, min: Vec3, max: Vec3, region: [f32; 4]) {
        let r = region;
        let uv = [
            Vec2::new(r[0], r[3]),
            Vec2::new(r[2], r[3]),
            Vec2::new(r[2], r[1]),
            Vec2::new(r[0], r[1]),
        ];
        for face in box_faces(min, max) {
            self.quad(face.map(|p| transform_point(transform, p)), uv);
        }
    }

    /// A cylinder around `transform`'s Z axis, from z = 0 to `height`, its side mapped into
    /// `side` and both caps into `cap`.
    pub fn cylinder(
        &mut self,
        transform: Mat4,
        radius: f32,
        height: f32,
        segments: u32,
        side: [f32; 4],
        cap: [f32; 4],
    ) {
        let normal_matrix = normal_matrix(transform);
        let first = self.vertices.len() as u32;
        for i in 0..=segments {
            let a = i as f32 / segments as f32 * TAU;
            let n = Vec3::new(a.cos(), a.sin(), 0.0);
            let u = side[0] + (side[2] - side[0]) * i as f32 / segments as f32;
            for (z, v) in [(0.0, side[3]), (height, side[1])] {
                self.vertices.push(Vertex::new(
                    transform_point(transform, n * radius + Vec3::new(0.0, 0.0, z)),
                    (normal_matrix * n.extend(0.0)).truncate().normalize(),
                    Vec2::new(u, v),
                ));
            }
        }
        for i in 0..segments {
            let (b0, t0, b1, t1) = (
                first + 2 * i,
                first + 2 * i + 1,
                first + 2 * i + 2,
                first + 2 * i + 3,
            );
            self.indices.extend([b0, b1, t1, b0, t1, t0]);
        }
        // Caps: fans around their centres.
        for (z, up) in [(height, 1.0f32), (0.0, -1.0)] {
            let n = (normal_matrix * Vec3::new(0.0, 0.0, up).extend(0.0))
                .truncate()
                .normalize();
            let centre = self.vertices.len() as u32;
            let cu = (cap[0] + cap[2]) / 2.0;
            let cv = (cap[1] + cap[3]) / 2.0;
            let (ru, rv) = ((cap[2] - cap[0]) / 2.0, (cap[3] - cap[1]) / 2.0);
            self.vertices.push(Vertex::new(
                transform_point(transform, Vec3::new(0.0, 0.0, z)),
                n,
                Vec2::new(cu, cv),
            ));
            for i in 0..=segments {
                let a = i as f32 / segments as f32 * TAU;
                let p = Vec3::new(a.cos() * radius, a.sin() * radius, z);
                self.vertices.push(Vertex::new(
                    transform_point(transform, p),
                    n,
                    Vec2::new(cu + ru * a.cos(), cv + rv * a.sin()),
                ));
            }
            for i in 0..segments {
                let (a, b) = (centre + 1 + i, centre + 2 + i);
                if up > 0.0 {
                    self.indices.extend([centre, a, b]);
                } else {
                    self.indices.extend([centre, b, a]);
                }
            }
        }
    }

    /// An ellipsoid with `radii`, around `transform`'s origin, mapped into `region`.
    pub fn ellipsoid(
        &mut self,
        transform: Mat4,
        radii: Vec3,
        segments: u32,
        rings: u32,
        region: [f32; 4],
    ) {
        let normal_matrix = normal_matrix(transform);
        let first = self.vertices.len() as u32;
        for ring in 0..=rings {
            let theta = PI * ring as f32 / rings as f32;
            for seg in 0..=segments {
                let phi = TAU * seg as f32 / segments as f32;
                let unit = Vec3::new(
                    theta.sin() * phi.cos(),
                    theta.sin() * phi.sin(),
                    theta.cos(),
                );
                let p = Vec3::new(unit.x * radii.x, unit.y * radii.y, unit.z * radii.z);
                let n = Vec3::new(unit.x / radii.x, unit.y / radii.y, unit.z / radii.z).normalize();
                let u = region[0] + (region[2] - region[0]) * seg as f32 / segments as f32;
                let v = region[1] + (region[3] - region[1]) * ring as f32 / rings as f32;
                self.vertices.push(Vertex::new(
                    transform_point(transform, p),
                    (normal_matrix * n.extend(0.0)).truncate().normalize(),
                    Vec2::new(u, v),
                ));
            }
        }
        let row = segments + 1;
        for ring in 0..rings {
            for seg in 0..segments {
                let a = first + ring * row + seg;
                let (b, c) = (a + 1, a + row);
                let d = c + 1;
                if ring > 0 {
                    self.indices.extend([a, c, b]);
                }
                if ring < rings - 1 {
                    self.indices.extend([b, c, d]);
                }
            }
        }
    }
}

/// The six faces of a box, each counter-clockwise from outside.
fn box_faces(min: Vec3, max: Vec3) -> [[Vec3; 4]; 6] {
    let p = |x: bool, y: bool, z: bool| {
        Vec3::new(
            if x { max.x } else { min.x },
            if y { max.y } else { min.y },
            if z { max.z } else { min.z },
        )
    };
    [
        [
            p(false, false, false),
            p(true, false, false),
            p(true, false, true),
            p(false, false, true),
        ], // -Y
        [
            p(true, false, false),
            p(true, true, false),
            p(true, true, true),
            p(true, false, true),
        ], // +X
        [
            p(true, true, false),
            p(false, true, false),
            p(false, true, true),
            p(true, true, true),
        ], // +Y
        [
            p(false, true, false),
            p(false, false, false),
            p(false, false, true),
            p(false, true, true),
        ], // -X
        [
            p(false, false, true),
            p(true, false, true),
            p(true, true, true),
            p(false, true, true),
        ], // +Z
        [
            p(false, true, false),
            p(true, true, false),
            p(true, false, false),
            p(false, false, false),
        ], // -Z
    ]
}

fn transform_point(m: Mat4, p: Vec3) -> Vec3 {
    m.transform_point(Point3::new(p.x, p.y, p.z))
        .to_homogeneous()
        .truncate()
}

fn normal_matrix(m: Mat4) -> Mat4 {
    m.invert().unwrap_or(Mat4::identity()).transpose()
}

/// A person's body without legs, facing +X, about 1.75 m tall standing on z = 0 (the legs,
/// `leg`, hang from the hips at z = 0.9 and y = +-0.09).
pub fn person_body() -> MeshData {
    use person_regions::*;
    let mut m = MeshData::default();
    let at = |x: f32, y: f32, z: f32| Mat4::from_translation(Vec3::new(x, y, z));
    let parts: [(Mat4, Vec3, [f32; 4]); 8] = [
        (at(0.0, 0.0, 1.645), Vec3::new(0.1, 0.09, 0.115), SKIN),
        (at(-0.012, 0.0, 1.685), Vec3::new(0.105, 0.098, 0.09), HAIR),
        (at(0.0, 0.0, 1.49), Vec3::new(0.05, 0.05, 0.07), SKIN),
        (at(0.0, 0.0, 1.3), Vec3::new(0.11, 0.18, 0.18), SHIRT),
        (at(0.0, 0.0, 1.08), Vec3::new(0.095, 0.15, 0.14), SHIRT),
        (at(0.0, 0.0, 0.93), Vec3::new(0.1, 0.16, 0.1), PANTS),
        (at(0.0, -0.19, 1.4), Vec3::new(0.065, 0.065, 0.06), SHIRT),
        (at(0.0, 0.19, 1.4), Vec3::new(0.065, 0.065, 0.06), SHIRT),
    ];
    for (t, r, region) in parts {
        m.ellipsoid(t, r, 14, 8, region);
    }
    for side in [-1.0f32, 1.0] {
        let tilt = |deg: f32| Mat4::from_angle_x(cgmath::Deg(deg * side));
        m.ellipsoid(
            at(0.0, side * 0.235, 1.23) * tilt(9.0),
            Vec3::new(0.05, 0.048, 0.16),
            10,
            8,
            SHIRT,
        );
        m.ellipsoid(
            at(0.01, side * 0.28, 0.95) * tilt(7.0),
            Vec3::new(0.042, 0.04, 0.14),
            10,
            8,
            SKIN,
        );
        m.ellipsoid(
            at(0.015, side * 0.305, 0.765) * tilt(5.0),
            Vec3::new(0.045, 0.03, 0.07),
            8,
            6,
            SKIN,
        );
    }
    m
}

/// A leg hanging from the hip at the origin, foot on z = -0.9 pointing +X.
pub fn leg() -> MeshData {
    use person_regions::*;
    let mut m = MeshData::default();
    let at = |x: f32, z: f32| Mat4::from_translation(Vec3::new(x, 0.0, z));
    m.ellipsoid(at(0.0, -0.22), Vec3::new(0.075, 0.07, 0.24), 10, 8, PANTS);
    m.ellipsoid(
        at(-0.01, -0.62),
        Vec3::new(0.055, 0.052, 0.22),
        10,
        8,
        PANTS,
    );
    m.ellipsoid(
        at(0.06, -0.865),
        Vec3::new(0.12, 0.045, 0.036),
        10,
        6,
        SHOES,
    );
    m
}

/// A sphere seen from inside, for the sky: radius 1, facing its centre.
pub fn sky_dome(segments: u32, rings: u32) -> MeshData {
    let mut m = MeshData::default();
    m.ellipsoid(
        Mat4::identity(),
        Vec3::new(1.0, 1.0, 1.0),
        segments,
        rings,
        [0.0, 0.0, 1.0, 1.0],
    );
    for triangle in m.indices.chunks_mut(3) {
        triangle.swap(1, 2);
    }
    for v in &mut m.vertices {
        v.normal = -v.normal;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether every triangle of `m` faces away from `inside` (its winding is counter-clockwise
    /// seen from outside).
    fn faces_out(m: &MeshData, inside: impl Fn(Vec3) -> Vec3) -> bool {
        m.indices.chunks(3).all(|t| {
            let [a, b, c] = [0, 1, 2].map(|k| m.vertices[t[k] as usize].pos);
            let n = (b - a).cross(c - a);
            n.magnitude() < 1e-9 || n.dot((a + b + c) / 3.0 - inside((a + b + c) / 3.0)) > 0.0
        })
    }

    #[test]
    fn split_into_cells_keeps_every_triangle() {
        let mut m = MeshData::default();
        // A box in cell (0, 0), one in cell (1, 0), and a quad in cell (-1, 2).
        m.walls(
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(3.0, 3.0, 2.0),
            Vec2::new(1.0, 1.0),
        );
        m.walls(
            Vec3::new(11.0, 1.0, 0.0),
            Vec3::new(13.0, 3.0, 2.0),
            Vec2::new(1.0, 1.0),
        );
        m.top(Vec2::new(-8.0, 21.0), Vec2::new(-6.0, 23.0), 0.0, 1.0);
        let (vertices, indices) = (m.vertices.clone(), m.indices.clone());
        let cells = m.split_into_cells(Vec2::new(0.0, 0.0), 10.0);
        let keys: Vec<_> = cells.iter().map(|(cell, _)| *cell).collect();
        assert_eq!(keys, [(-1, 2), (0, 0), (1, 0)]);
        // The same triangles, vertex for vertex.
        let triangles = |vertices: &[Vertex], indices: &[u32]| -> Vec<[f32; 9]> {
            let mut t: Vec<[f32; 9]> = indices
                .chunks_exact(3)
                .map(|tri| {
                    let p = |k: usize| vertices[tri[k] as usize].pos;
                    let (a, b, c) = (p(0), p(1), p(2));
                    [a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z]
                })
                .collect();
            t.sort_by(|a, b| a.partial_cmp(b).unwrap());
            t
        };
        let mut split = Vec::new();
        for (_, mesh) in &cells {
            assert!(mesh
                .indices
                .iter()
                .all(|&i| (i as usize) < mesh.vertices.len()));
            split.extend(triangles(&mesh.vertices, &mesh.indices));
        }
        split.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(split, triangles(&vertices, &indices));
    }

    #[test]
    fn walls_tile_by_bays_and_floors() {
        let mut m = MeshData::default();
        m.walls(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(6.0, 3.0, 7.0),
            Vec2::new(3.0, 3.5),
        );
        assert_eq!(m.indices.len(), 24);
        let max_u = m.vertices.iter().map(|v| v.tex_coord.x).fold(0.0, f32::max);
        let max_v = m.vertices.iter().map(|v| v.tex_coord.y).fold(0.0, f32::max);
        assert_eq!((max_u, max_v), (2.0, 2.0));
        assert!(faces_out(&m, |_| Vec3::new(3.0, 1.5, 3.5)));
    }

    #[test]
    fn solids_face_outwards_and_the_dome_inwards() {
        let mut m = MeshData::default();
        m.ellipsoid(
            Mat4::identity(),
            Vec3::new(1.0, 2.0, 0.5),
            12,
            8,
            [0.0, 0.0, 1.0, 1.0],
        );
        assert!(faces_out(&m, |_| Vec3::new(0.0, 0.0, 0.0)));

        let mut m = MeshData::default();
        m.cylinder(
            Mat4::identity(),
            1.0,
            2.0,
            12,
            [0.0; 4],
            [0.0, 0.0, 1.0, 1.0],
        );
        assert!(faces_out(&m, |p| Vec3::new(0.0, 0.0, p.z.clamp(0.1, 1.9))));

        let dome = sky_dome(12, 8);
        assert!(dome.indices.chunks(3).all(|t| {
            let [a, b, c] = [0, 1, 2].map(|k| dome.vertices[t[k] as usize].pos);
            let n = (b - a).cross(c - a);
            n.magnitude() < 1e-9 || n.dot(a + b + c) < 0.0
        }));
    }

    #[test]
    fn people_stand_on_the_ground() {
        let body = person_body();
        let top = body.vertices.iter().map(|v| v.pos.z).fold(0.0, f32::max);
        assert!((1.7..1.85).contains(&top), "{top}");
        let leg = leg();
        let bottom = leg.vertices.iter().map(|v| v.pos.z).fold(0.0, f32::min);
        assert!((bottom + 0.9).abs() < 0.01, "{bottom}");
    }
}
