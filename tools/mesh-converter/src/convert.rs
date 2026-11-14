//! From a parsed OBJ to `.fmesh` meshes: triangulation, the automatic precision of each
//! stream, normals, merging identical vertices.

use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use feather_fmesh::{fixed_to_f32, MeshData, Stream};

use super::decimal::Decimal;
use super::normals::{self, Normals, Triangle};
use super::obj::{Corner, Number, Obj};
use super::{Converted, Settings, Stats};

/// A value of a stream before the precision is chosen.
#[derive(Clone, Copy)]
struct Entry {
    /// The exact value, if known.
    decimal: Option<Decimal>,
    /// The value a reader must get back.
    reference: f32,
}

impl From<Number> for Entry {
    fn from(number: Number) -> Self {
        Entry {
            decimal: number.decimal,
            reference: number.value,
        }
    }
}

impl From<Decimal> for Entry {
    fn from(decimal: Decimal) -> Self {
        Entry {
            decimal: Some(decimal),
            reference: decimal.to_f32(),
        }
    }
}

/// How a stream is stored: integers with some decimals, or floats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scale {
    Fixed(u32),
    Float,
}

/// The precision of a stream: the most decimals of its values, if every value then fits an
/// `i32` and decodes back to exactly its reference float. Otherwise `Float`, with the reason.
fn choose_scale(entries: &[Entry], max_decimals: u32) -> (Scale, Option<String>) {
    let mut decimals = 0;
    for entry in entries {
        match entry.decimal {
            Some(decimal) => decimals = decimals.max(decimal.decimals),
            None => {
                return (
                    Scale::Float,
                    Some("some values aren't plain decimal numbers".into()),
                )
            }
        }
    }
    if decimals > max_decimals {
        return (
            Scale::Float,
            Some(format!(
                "{decimals} decimals, more than the maximum {max_decimals} (--max-decimals)"
            )),
        );
    }
    for entry in entries {
        let n = entry.decimal.unwrap().scaled(decimals);
        match n {
            Some(n) if i32::try_from(n).is_ok() => {
                // `==` so that -0.0 matches 0.0: the sign of zero isn't kept.
                if fixed_to_f32(n, decimals as u8) != entry.reference {
                    return (
                        Scale::Float,
                        Some(format!(
                            "{} with {decimals} decimals can't be read back exactly",
                            entry.reference
                        )),
                    );
                }
            }
            _ => {
                return (
                    Scale::Float,
                    Some(format!(
                        "values too large for 32-bit integers with {decimals} decimals"
                    )),
                )
            }
        }
    }
    (Scale::Fixed(decimals), None)
}

/// The value stored for `entry`, as an integer: the scaled value, or the float's bits (with
/// -0.0 as 0.0).
fn key(scale: Scale, entry: Entry) -> i64 {
    match scale {
        Scale::Fixed(decimals) => entry.decimal.unwrap().scaled(decimals).unwrap(),
        Scale::Float => (entry.reference + 0.0).to_bits() as i64,
    }
}

fn stream(scale: Scale, keys: Vec<i64>) -> Stream {
    match scale {
        Scale::Fixed(decimals) => Stream::Fixed {
            values: keys.into_iter().map(|k| k as i32).collect(),
            decimals: decimals as u8,
        },
        Scale::Float => Stream::Float(keys.into_iter().map(|k| f32::from_bits(k as u32)).collect()),
    }
}

/// A stream's values scaled: one key per component of each source item (an OBJ position, UV,
/// normal...).
struct Scaled<const N: usize> {
    scale: Scale,
    keys: Vec<[i64; N]>,
    references: Vec<[f32; N]>,
}

fn scale_items<const N: usize>(
    name: &str,
    items: &[[Entry; N]],
    max_decimals: u32,
    warnings: &mut Vec<String>,
) -> Scaled<N> {
    let flat: Vec<Entry> = items.iter().flatten().copied().collect();
    let (scale, reason) = choose_scale(&flat, max_decimals);
    if let Some(reason) = reason {
        warnings.push(format!("{name} stored as f32: {reason}"));
    }
    Scaled {
        scale,
        keys: items
            .iter()
            .map(|item| item.map(|e| key(scale, e)))
            .collect(),
        references: items.iter().map(|item| item.map(|e| e.reference)).collect(),
    }
}

/// The OBJ's UV `uv` (or none) as stored: V flipped to `1 - v` unless `flip_v` is false.
fn uv_entries(obj: &Obj, uv: Option<u32>, flip_v: bool) -> [Entry; 2] {
    let [u, v] = uv.map_or([Number::ZERO; 2], |i| obj.uvs[i as usize]);
    let v = if flip_v {
        match v.decimal.and_then(Decimal::one_minus) {
            Some(flipped) => Entry::from(flipped),
            None => Entry {
                decimal: None,
                reference: (1.0 - v.to_f64()) as f32,
            },
        }
    } else {
        Entry::from(v)
    };
    [Entry::from(u), v]
}

/// One mesh (all the OBJ, or one object of it with `--split`).
pub(crate) fn convert_faces(
    obj: &Obj,
    faces: &[usize],
    settings: &Settings,
    object: Option<String>,
) -> Result<Converted> {
    let mut warnings = Vec::new();
    let mut notes = Vec::new();

    // Triangles, as fans around each face's first corner (like tobj).
    let mut triangles: Vec<[Corner; 3]> = Vec::new();
    let mut smoothing = Vec::new();
    for &f in faces {
        let face = &obj.faces[f];
        let corners = &face.corners;
        if corners.len() > 3 {
            notes.push(format!(
                "line {}: polygon of {} corners split into {} triangles",
                face.line,
                corners.len(),
                corners.len() - 2
            ));
        }
        for i in 1..corners.len() - 1 {
            triangles.push([corners[0], corners[i], corners[i + 1]]);
            smoothing.push(if obj.has_smoothing { face.smoothing } else { 1 });
        }
        if settings.normals == Normals::FromObj {
            if let Some(corner) = corners.iter().find(|c| c.normal.is_none()) {
                bail!(
                    "line {}: corner of position {} has no normal; give the OBJ normals, or use \
                     --flat-normals or --soft-normals",
                    face.line,
                    corner.position + 1
                );
            }
        }
    }

    // Positions: the ones the triangles use.
    let used_positions = used(
        triangles.iter().flatten().map(|c| c.position as usize),
        obj.positions.len(),
    );
    let position_items: Vec<[Entry; 3]> = used_positions
        .iter()
        .map(|&i| obj.positions[i].map(Entry::from))
        .collect();
    let positions = scale_items(
        "positions",
        &position_items,
        settings.max_decimals,
        &mut warnings,
    );
    let position_slot = slots(&used_positions, obj.positions.len());

    // UVs: the ones used, and (last) the one of corners without any.
    let uv_slot_count = obj.uvs.len() + 1;
    let uv_index = |c: &Corner| c.uv.map_or(obj.uvs.len(), |i| i as usize);
    let used_uvs = used(triangles.iter().flatten().map(uv_index), uv_slot_count);
    let uv_items: Vec<[Entry; 2]> = used_uvs
        .iter()
        .map(|&i| {
            uv_entries(
                obj,
                (i < obj.uvs.len()).then_some(i as u32),
                settings.flip_v,
            )
        })
        .collect();
    let uvs = scale_items(
        "texture coordinates",
        &uv_items,
        settings.max_decimals,
        &mut warnings,
    );
    let uv_slot = slots(&used_uvs, uv_slot_count);

    // Normals: one item per OBJ normal used, or one per triangle corner when computed.
    let (normals, normal_slot_of_corner, computed_normals): (
        Scaled<3>,
        Vec<usize>,
        Option<Vec<[f64; 3]>>,
    ) = if settings.normals == Normals::FromObj {
        let used_normals = used(
            triangles
                .iter()
                .flatten()
                .map(|c| c.normal.unwrap() as usize),
            obj.normals.len(),
        );
        let items: Vec<[Entry; 3]> = used_normals
            .iter()
            .map(|&i| obj.normals[i].map(Entry::from))
            .collect();
        let scaled = scale_items("normals", &items, settings.max_decimals, &mut warnings);
        let slot = slots(&used_normals, obj.normals.len());
        let of_corner = triangles
            .iter()
            .flatten()
            .map(|c| slot[c.normal.unwrap() as usize])
            .collect();
        (scaled, of_corner, None)
    } else {
        if !obj.normals.is_empty() {
            warnings.push(format!(
                "the OBJ's {} normals are replaced by computed ones",
                obj.normals.len()
            ));
        }
        let normal_triangles: Vec<Triangle> = triangles
            .iter()
            .zip(&smoothing)
            .map(|(corners, &smoothing)| Triangle {
                points: corners.map(|c| obj.positions[c.position as usize].map(Number::to_f64)),
                keys: corners.map(|c| positions.keys[position_slot[c.position as usize]]),
                smoothing,
            })
            .collect();
        let computed = normals::compute(settings.normals, &normal_triangles);
        if computed.degenerate > 0 {
            warnings.push(format!(
                "{} triangles have no area, so no normal: given (0, 0, 1)",
                computed.degenerate
            ));
        }
        let scale = 10f64.powi(normals::DECIMALS as i32);
        let items: Vec<[Entry; 3]> = computed
            .normals
            .iter()
            .map(|n| {
                n.map(|v| Entry::from(Decimal::new((v * scale).round() as i64, normals::DECIMALS)))
            })
            .collect();
        let scaled = scale_items("normals", &items, normals::DECIMALS, &mut warnings);
        (scaled, (0..items.len()).collect(), Some(computed.normals))
    };

    // Vertices: identical ones merged, in the order of first use.
    let mut vertex_of_key: HashMap<[i64; 8], u32> = HashMap::new();
    let mut vertex_keys: Vec<[i64; 8]> = Vec::new();
    let mut indices = Vec::with_capacity(triangles.len() * 3);
    let mut expected = Vec::with_capacity(triangles.len() * 3);
    for (k, corner) in triangles.iter().flatten().enumerate() {
        let p = position_slot[corner.position as usize];
        let n = normal_slot_of_corner[k];
        let t = uv_slot[uv_index(corner)];
        let [px, py, pz] = positions.keys[p];
        let [nx, ny, nz] = normals.keys[n];
        let [tu, tv] = uvs.keys[t];
        let key = [px, py, pz, nx, ny, nz, tu, tv];
        let index = *vertex_of_key.entry(key).or_insert_with(|| {
            vertex_keys.push(key);
            vertex_keys.len() as u32 - 1
        });
        indices.push(index);

        let normal = match &computed_normals {
            Some(computed) => computed[k].map(|v| v as f32),
            None => normals.references[n],
        };
        let [px, py, pz] = positions.references[p];
        let [tu, tv] = uvs.references[t];
        expected.push([px, py, pz, normal[0], normal[1], normal[2], tu, tv]);
    }

    // The vertices' keys, component `columns` of each.
    let interleave = |columns: &[usize]| -> Vec<i64> {
        vertex_keys
            .iter()
            .flat_map(|k| columns.iter().map(|&c| k[c]))
            .collect()
    };
    let mesh = MeshData {
        positions: stream(positions.scale, interleave(&[0, 1, 2])),
        normals: stream(normals.scale, interleave(&[3, 4, 5])),
        uvs: stream(uvs.scale, interleave(&[6, 7])),
        indices,
    };

    let written = feather_fmesh::write(
        &mesh,
        &feather_fmesh::WriteOptions {
            encoding: settings.encoding,
            compression: settings.compression,
            ..Default::default()
        },
    )
    .map_err(|e| anyhow!("{e}"))?;

    let stats = Stats {
        triangles: triangles.len(),
        corners: triangles.len() * 3,
        vertices: vertex_keys.len(),
    };
    Ok(Converted {
        object,
        mesh,
        written,
        stats,
        warnings,
        notes,
        expected,
        normals_tolerance: if computed_normals.is_some() {
            1e-4
        } else {
            0.0
        },
    })
}

/// The distinct values of `items` (each below `count`), sorted.
fn used(items: impl Iterator<Item = usize>, count: usize) -> Vec<usize> {
    let mut is_used = vec![false; count];
    for i in items {
        is_used[i] = true;
    }
    (0..count).filter(|&i| is_used[i]).collect()
}

/// For each of `count` items, its position in `used` (`usize::MAX` if not used).
fn slots(used: &[usize], count: usize) -> Vec<usize> {
    let mut slots = vec![usize::MAX; count];
    for (slot, &i) in used.iter().enumerate() {
        slots[i] = slot;
    }
    slots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(texts: &[&str]) -> Vec<Entry> {
        texts
            .iter()
            .map(|t| Entry {
                decimal: Decimal::parse(t),
                reference: t.parse().unwrap(),
            })
            .collect()
    }

    #[test]
    fn scale_is_the_most_decimals() {
        assert_eq!(
            choose_scale(&entries(&["0.5", "1.25", "-3"]), 9),
            (Scale::Fixed(2), None)
        );
        assert_eq!(
            choose_scale(&entries(&["1.0000", "0.00000", "-0.0"]), 9),
            (Scale::Fixed(0), None)
        );
        assert_eq!(
            choose_scale(&entries(&["1.5e-07"]), 9),
            (Scale::Fixed(8), None)
        );
        assert_eq!(choose_scale(&[], 9), (Scale::Fixed(0), None));
    }

    #[test]
    fn scale_falls_back_to_floats() {
        let float = |texts: &[&str], max| choose_scale(&entries(texts), max).0 == Scale::Float;
        // More decimals than allowed.
        assert!(float(&["0.1000000015"], 9));
        assert!(!float(&["0.1000000015"], 10));
        // Too large for i32 once scaled.
        assert!(float(&["150.123456789"], 18));
        assert!(!float(&["2.147483647"], 18));
        assert!(float(&["2.147483648"], 18));
        // Not a decimal.
        assert!(float(&["nan"], 9));
    }

    #[test]
    fn keys_merge_zero_signs() {
        let zero = entries(&["0", "-0.0"]);
        assert_eq!(key(Scale::Fixed(3), zero[0]), key(Scale::Fixed(3), zero[1]));
        assert_eq!(key(Scale::Float, zero[0]), key(Scale::Float, zero[1]));
    }
}
