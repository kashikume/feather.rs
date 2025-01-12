use anyhow::{bail, Result};

use super::math::{Vec2, Vec3};
use super::mesh::Mesh;
use super::vertex::Vertex;

//   ^ y
//   |                    4
//   |
//   |               *-----------*
//   |              /|          /|
//   |             / |         / |
//   |            /  |    2   /  |
//   |           *-----------*   |
//   |           |   |       |   |   1
//   |        3  |   |       |   |
//   |           |   *-0-----|---*
//   |           |  /        |  /
//   |   z       | /         | /
//   |  /        |/          |/
//   | /         *-----------*
//   |/                5
//   -----------------------------------------------------------> x

pub struct MeshBuilderCuboid {
    x: (f32, f32),
    y: (f32, f32),
    z: (f32, f32),
    u: Vec<(f32, f32)>,
    v: Vec<(f32, f32)>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl MeshBuilderCuboid {
    pub fn new(
        x: (f32, f32),
        y: (f32, f32),
        z: (f32, f32),
        u: Option<Vec<(f32, f32)>>,
        v: Option<Vec<(f32, f32)>>,
    ) -> Self {
        Self {
            x,
            y,
            z,
            u: match u {
                Some(u) => {
                    if u.len() > 0 {
                        u
                    } else {
                        vec![(0.0, 1.0)]
                    }
                }
                None => vec![(0.0, 1.0)],
            },
            v: match v {
                Some(v) => {
                    if v.len() > 0 {
                        v
                    } else {
                        vec![(0.0, 1.0)]
                    }
                }
                None => vec![(0.0, 1.0)],
            },
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn new_same_walls(x: (f32, f32), y: (f32, f32), z: (f32, f32)) -> Self {
        Self::new(x, y, z, None, None)
    }

    pub fn build(mut self) -> Result<Mesh> {

        // 0
        self.add_wall(
            (self.x.0, self.x.1),
            (self.y.0, self.y.1),
            (self.z.0, self.z.0),
            self.u[0 % self.u.len()],
            self.v[0 % self.v.len()],
            (0.0, 0.0, 1.0),
        )?;

        // 1
        self.add_wall(
            (self.x.1, self.x.1),
            (self.y.0, self.y.1),
            (self.z.1, self.z.0),
            self.u[1 % self.u.len()],
            self.v[1 % self.v.len()],
            (1.0, 0.0, 0.0),
        )?;

        // 2
        self.add_wall(
            (self.x.1, self.x.0),
            (self.y.0, self.y.1),
            (self.z.1, self.z.1),
            self.u[2 % self.u.len()],
            self.v[2 % self.v.len()],
            (0.0, 0.0, -1.0),
        )?;

        // 3
        self.add_wall(
            (self.x.0, self.x.0),
            (self.y.1, self.y.0),
            (self.z.1, self.z.0),
            self.u[3 % self.u.len()],
            self.v[3 % self.v.len()],
            (-1.0, 0.0, 0.0),
        )?;

        // 4
        self.add_wall(
            (self.x.0, self.x.1),
            (self.y.0, self.y.0),
            (self.z.1, self.z.0),
            self.u[4 % self.u.len()],
            self.v[4 % self.v.len()],
            (0.0, 1.0, 0.0),
        )?;

        // 5
        self.add_wall(
            (self.x.1, self.x.0),
            (self.y.1, self.y.1),
            (self.z.1, self.z.0),
            self.u[5 % self.u.len()],
            self.v[5 % self.v.len()],
            (0.0, -1.0, 0.0),
        )?;

        Ok(Mesh::new(self.vertices, self.indices))
    }

    #[rustfmt::skip]
    fn add_wall(&mut self, x:(f32,f32), y:(f32,f32), z:(f32,f32), u:(f32,f32), v:(f32,f32), n:(f32, f32, f32) ) -> Result<()> {
        let start_pos = self.vertices.len() as u32;

        if z.0 == z.1 {
			self.vertices.push(Vertex::new(Vec3{x:x.0, y:y.0, z:z.0}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.0, y:v.0}));
			self.vertices.push(Vertex::new(Vec3{x:x.1, y:y.0, z:z.0}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.1, y:v.0}));
			self.vertices.push(Vertex::new(Vec3{x:x.1, y:y.1, z:z.0}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.1, y:v.1}));
			self.vertices.push(Vertex::new(Vec3{x:x.0, y:y.1, z:z.0}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.0, y:v.1}));
		}
		else if y.0 == y.1 {
			self.vertices.push(Vertex::new(Vec3{x:x.0, y:y.0, z:z.0}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.0, y:v.0}));
			self.vertices.push(Vertex::new(Vec3{x:x.1, y:y.0, z:z.0}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.1, y:v.0}));
			self.vertices.push(Vertex::new(Vec3{x:x.1, y:y.0, z:z.1}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.1, y:v.1}));
			self.vertices.push(Vertex::new(Vec3{x:x.0, y:y.0, z:z.1}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.0, y:v.1}));
		}
		else if x.0 == x.1 {
			self.vertices.push(Vertex::new(Vec3{x:x.0, y:y.0, z:z.0}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.0, y:v.0}));
			self.vertices.push(Vertex::new(Vec3{x:x.0, y:y.1, z:z.0}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.1, y:v.0}));
			self.vertices.push(Vertex::new(Vec3{x:x.0, y:y.1, z:z.1}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.1, y:v.1}));
			self.vertices.push(Vertex::new(Vec3{x:x.0, y:y.0, z:z.1}, Vec3{x:n.0, y:n.1, z:n.2}, Vec2{x:u.0, y:v.1}));
		}
		else {
			bail!("Invalid cuboid wall");
        }

        self.indices.push(start_pos + 0);
        self.indices.push(start_pos + 3);
        self.indices.push(start_pos + 1);

        self.indices.push(start_pos + 1);
        self.indices.push(start_pos + 3);
        self.indices.push(start_pos + 2);

        Ok(())
    }
}
