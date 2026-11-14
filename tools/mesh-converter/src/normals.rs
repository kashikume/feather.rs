//! Normals computed from the triangles: `--flat-normals` and `--soft-normals`.

use std::collections::HashMap;

/// Where the normals come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Normals {
    /// The OBJ's `vn`; every corner must have one.
    #[default]
    FromObj,
    /// Each triangle's own normal on its three corners.
    Flat,
    /// At each corner, the average of the normals of the triangles around its position (in the
    /// same smoothing group), weighted by their angles at that position.
    Soft,
}

/// The decimals computed normals are stored with.
pub const DECIMALS: u32 = 4;

/// A triangle for normal computation.
pub struct Triangle {
    /// Corner positions, counter-clockwise seen from the front.
    pub points: [[f64; 3]; 3],
    /// Identifies each corner's position: equal keys are the same point.
    pub keys: [[i64; 3]; 3],
    /// The OBJ smoothing group; 0 means not smoothed.
    pub smoothing: u32,
}

/// Normals per triangle corner (three per triangle, unit length), and how many triangles
/// had no normal (zero area).
pub struct Computed {
    pub normals: Vec<[f64; 3]>,
    pub degenerate: usize,
}

/// The normal used where there is none: a degenerate triangle draws nothing, so it's never
/// seen.
const FALLBACK: [f64; 3] = [0.0, 0.0, 1.0];

/// Computes the normals of `triangles`' corners. `mode` must be `Flat` or `Soft`.
pub fn compute(mode: Normals, triangles: &[Triangle]) -> Computed {
    let face_normals: Vec<Option<[f64; 3]>> = triangles.iter().map(face_normal).collect();
    let degenerate = face_normals.iter().filter(|n| n.is_none()).count();

    let normals = match mode {
        Normals::FromObj => panic!("normals::compute needs Flat or Soft"),
        Normals::Flat => face_normals
            .iter()
            .flat_map(|n| [n.unwrap_or(FALLBACK); 3])
            .collect(),
        Normals::Soft => {
            let mut sums: HashMap<(u32, [i64; 3]), [f64; 3]> = HashMap::new();
            for (triangle, normal) in triangles.iter().zip(&face_normals) {
                let Some(normal) = normal else { continue };
                if triangle.smoothing == 0 {
                    continue;
                }
                for k in 0..3 {
                    let weight = corner_angle(triangle, k);
                    let sum = sums
                        .entry((triangle.smoothing, triangle.keys[k]))
                        .or_insert([0.0; 3]);
                    for c in 0..3 {
                        sum[c] += normal[c] * weight;
                    }
                }
            }
            let sums = &sums;
            triangles
                .iter()
                .zip(&face_normals)
                .flat_map(|(triangle, normal)| {
                    let own = normal.unwrap_or(FALLBACK);
                    (0..3)
                        .map(move |k| {
                            if triangle.smoothing == 0 {
                                return own;
                            }
                            // Opposite faces can cancel out: keep the triangle's own normal then.
                            sums.get(&(triangle.smoothing, triangle.keys[k]))
                                .and_then(|&sum| normalize(sum))
                                .unwrap_or(own)
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        }
    };
    Computed {
        normals,
        degenerate,
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(v, v).sqrt();
    (length > 0.0 && length.is_finite()).then(|| [v[0] / length, v[1] / length, v[2] / length])
}

/// `normalize((b - a) × (c - a))`, `None` for a triangle of zero area.
fn face_normal(triangle: &Triangle) -> Option<[f64; 3]> {
    let [a, b, c] = triangle.points;
    normalize(cross(sub(b, a), sub(c, a)))
}

/// The triangle's angle at corner `k`, in radians.
fn corner_angle(triangle: &Triangle, k: usize) -> f64 {
    let p = triangle.points;
    let (Some(e1), Some(e2)) = (
        normalize(sub(p[(k + 1) % 3], p[k])),
        normalize(sub(p[(k + 2) % 3], p[k])),
    ) else {
        return 0.0;
    };
    dot(e1, e2).clamp(-1.0, 1.0).acos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle(points: [[f64; 3]; 3], smoothing: u32) -> Triangle {
        let key = |p: [f64; 3]| p.map(|v| (v * 1000.0).round() as i64);
        Triangle {
            points,
            keys: points.map(key),
            smoothing,
        }
    }

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|c| (a[c] - b[c]).abs() < 1e-9)
    }

    #[test]
    fn flat_normals_follow_the_winding() {
        let up = triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 1);
        let down = triangle([[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]], 1);
        let computed = compute(Normals::Flat, &[up, down]);
        assert_eq!(computed.normals[..3], [[0.0, 0.0, 1.0]; 3]);
        assert_eq!(computed.normals[3..], [[0.0, 0.0, -1.0]; 3]);
        assert_eq!(computed.degenerate, 0);
    }

    #[test]
    fn soft_normals_average_by_angle() {
        // Two faces of a roof meeting at the ridge (y = 0, z = 1), sloping 45 degrees.
        let left = triangle([[0.0, -1.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]], 1);
        let right = triangle([[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]], 1);
        let computed = compute(Normals::Soft, &[left, right]);
        // On the ridge: straight up.
        assert!(
            close(computed.normals[1], [0.0, 0.0, 1.0]),
            "{:?}",
            computed.normals[1]
        );
        assert!(close(computed.normals[3], [0.0, 0.0, 1.0]));
        // At the eaves: each face's own normal.
        let h = std::f64::consts::FRAC_1_SQRT_2;
        assert!(
            close(computed.normals[0], [0.0, -h, h]),
            "{:?}",
            computed.normals[0]
        );
        assert!(close(computed.normals[5], [0.0, h, h]));
    }

    #[test]
    fn splitting_a_face_doesnt_change_soft_normals() {
        // A flat quad as 2 triangles, and its neighbour at 90 degrees as 1 or 4 triangles: the
        // weights by angle make the shared corner's normal the same.
        let floor = [
            triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]], 1),
            triangle([[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]], 1),
        ];
        let wall = |split: bool| {
            if split {
                // The same wall as a fan around its corner (0, 0, 0).
                vec![
                    triangle([[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.5, 1.0]], 1),
                    triangle([[0.0, 0.0, 0.0], [0.0, 0.5, 1.0], [0.0, 1.0, 1.0]], 1),
                    triangle([[0.0, 0.0, 0.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0]], 1),
                ]
            } else {
                vec![
                    triangle([[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 1.0]], 1),
                    triangle([[0.0, 0.0, 0.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0]], 1),
                ]
            }
        };
        let normal_at_origin = |split| {
            let mut triangles: Vec<Triangle> =
                floor.iter().map(|t| triangle(t.points, 1)).collect();
            triangles.extend(wall(split));
            compute(Normals::Soft, &triangles).normals[0]
        };
        assert!(close(normal_at_origin(false), normal_at_origin(true)));
    }

    #[test]
    fn smoothing_groups_separate_faces() {
        let left = [[0.0, -1.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]];
        let right = [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]];
        let h = std::f64::consts::FRAC_1_SQRT_2;
        // Different groups: each keeps its own normal on the ridge.
        let computed = compute(Normals::Soft, &[triangle(left, 1), triangle(right, 2)]);
        assert!(close(computed.normals[1], [0.0, -h, h]));
        assert!(close(computed.normals[3], [0.0, h, h]));
        // Group 0 (s off): flat.
        let computed = compute(Normals::Soft, &[triangle(left, 0), triangle(right, 0)]);
        assert!(close(computed.normals[1], [0.0, -h, h]));
    }

    #[test]
    fn degenerate_triangles() {
        let line = triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], 1);
        let point = triangle([[1.0, 1.0, 1.0]; 3], 1);
        let computed = compute(Normals::Flat, &[line, point]);
        assert_eq!(computed.degenerate, 2);
        assert_eq!(computed.normals, vec![FALLBACK; 6]);
        let line = triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], 1);
        let computed = compute(Normals::Soft, &[line]);
        assert_eq!(computed.normals, vec![FALLBACK; 3]);
    }
}
