//! A Wavefront OBJ parser keeping every number as written (see [`Number`]).
//!
//! Reads `v`, `vt`, `vn`, `f`, `o`, `g` and `s`; skips comments, `mtllib` and `usemtl`
//! quietly, and other statements (`l`, `p`, curves...) with a note.

use anyhow::{anyhow, bail, Result};

use super::decimal::Decimal;

/// A number of the OBJ file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Number {
    /// The exact value, unless the text isn't a plain decimal number (see [`Decimal::parse`]).
    pub decimal: Option<Decimal>,
    /// The text parsed as an `f32`, as `tobj` (and so `MeshBuilderObjFile`) reads it.
    pub value: f32,
}

impl Number {
    pub const ZERO: Number = Number {
        decimal: Some(Decimal::ZERO),
        value: 0.0,
    };

    fn parse(text: &str) -> Option<Number> {
        Some(Number {
            decimal: Decimal::parse(text),
            value: text.parse().ok()?,
        })
    }

    /// The value as an `f64`, as exact as possible.
    pub fn to_f64(self) -> f64 {
        self.decimal.map_or(self.value as f64, Decimal::to_f64)
    }
}

/// One corner of a face: indices (from 0) into the OBJ's positions, UVs and normals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corner {
    pub position: u32,
    pub uv: Option<u32>,
    pub normal: Option<u32>,
}

/// A face (polygon) of the OBJ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    /// At least 3.
    pub corners: Vec<Corner>,
    /// Index into [`Obj::objects`] of the last `o` before the face.
    pub object: Option<u32>,
    /// Index into [`Obj::groups`] of the last `g` before the face.
    pub group: Option<u32>,
    /// The smoothing group (`s`): 0 for `s off` / `s 0`, 1 before any `s`.
    pub smoothing: u32,
    /// The line of the face, from 1.
    pub line: usize,
}

/// The contents of an OBJ file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Obj {
    pub positions: Vec<[Number; 3]>,
    pub uvs: Vec<[Number; 2]>,
    pub normals: Vec<[Number; 3]>,
    pub faces: Vec<Face>,
    /// Names of the `o` statements, in order.
    pub objects: Vec<String>,
    /// Names of the `g` statements, in order.
    pub groups: Vec<String>,
    /// Whether there are `s` statements. Without any, every face is in smoothing group 1.
    pub has_smoothing: bool,
    /// Lines skipped, with their number: statements the converter doesn't use.
    pub skipped: Vec<(usize, String)>,
}

impl Obj {
    pub fn parse(text: &str) -> Result<Obj> {
        let mut obj = Obj::default();
        let mut object = None;
        let mut group = None;
        // Faces before any `s` are smoothed together, as if under `s 1`.
        let mut smoothing = 1;

        for (number, line) in text.lines().enumerate() {
            let number = number + 1;
            let at = |e: anyhow::Error| anyhow!("line {number}: {e}");
            let line = line.trim();
            let mut tokens = line.split_ascii_whitespace();
            let Some(keyword) = tokens.next() else {
                continue;
            };
            let rest = || line[keyword.len()..].trim().to_string();
            match keyword {
                "v" => obj.positions.push(numbers::<3>(tokens, 3).map_err(at)?),
                "vt" => obj.uvs.push(numbers::<2>(tokens, 1).map_err(at)?),
                "vn" => obj.normals.push(numbers::<3>(tokens, 3).map_err(at)?),
                "f" => {
                    let corners = tokens
                        .map(|corner| obj.corner(corner))
                        .collect::<Result<Vec<_>>>()
                        .map_err(at)?;
                    if corners.len() < 3 {
                        return Err(at(anyhow!("a face needs at least 3 corners")));
                    }
                    obj.faces.push(Face {
                        corners,
                        object,
                        group,
                        smoothing,
                        line: number,
                    });
                }
                "o" => {
                    obj.objects.push(rest());
                    object = Some(obj.objects.len() as u32 - 1);
                }
                "g" => {
                    obj.groups.push(rest());
                    group = Some(obj.groups.len() as u32 - 1);
                }
                "s" => {
                    obj.has_smoothing = true;
                    smoothing = match tokens.next() {
                        Some("off") | None => 0,
                        Some(group) => group.parse().unwrap_or(1),
                    };
                }
                "mtllib" | "usemtl" => {}
                _ if keyword.starts_with('#') => {}
                _ => obj.skipped.push((number, line.to_string())),
            }
        }

        obj.check_indices()?;
        Ok(obj)
    }

    /// Parses a face corner, `v`, `v/t`, `v//n` or `v/t/n`, with indices from 1 or negative
    /// (relative to the end of the lists read so far).
    fn corner(&self, text: &str) -> Result<Corner> {
        let mut parts = text.split('/');
        let mut index = |count: usize, what: &str| -> Result<Option<u32>> {
            match parts.next() {
                None | Some("") => Ok(None),
                Some(part) => {
                    let i: i64 = part
                        .parse()
                        .map_err(|_| anyhow!("invalid {what} index in \"{text}\""))?;
                    let resolved = match i {
                        0 => bail!("{what} index 0 in \"{text}\" (indices start at 1)"),
                        i if i > 0 => i - 1,
                        i => count as i64 + i,
                    };
                    if resolved < 0 || resolved > u32::MAX as i64 {
                        bail!("{what} index {i} in \"{text}\" is out of range");
                    }
                    Ok(Some(resolved as u32))
                }
            }
        };
        let position = index(self.positions.len(), "position")?
            .ok_or_else(|| anyhow!("corner \"{text}\" has no position"))?;
        let uv = index(self.uvs.len(), "texture coordinate")?;
        let normal = index(self.normals.len(), "normal")?;
        if parts.next().is_some() {
            bail!("invalid corner \"{text}\"");
        }
        Ok(Corner {
            position,
            uv,
            normal,
        })
    }

    /// Every corner must point at a position, UV and normal of the file (positive indices may
    /// point forward, so this is checked at the end).
    fn check_indices(&self) -> Result<()> {
        for face in &self.faces {
            for corner in &face.corners {
                let check = |index: Option<u32>, count: usize, what: &str| match index {
                    Some(i) if i as usize >= count => Err(anyhow!(
                        "line {}: {what} {} doesn't exist (the file has {count})",
                        face.line,
                        i + 1
                    )),
                    _ => Ok(()),
                };
                check(Some(corner.position), self.positions.len(), "position")?;
                check(corner.uv, self.uvs.len(), "texture coordinate")?;
                check(corner.normal, self.normals.len(), "normal")?;
            }
        }
        Ok(())
    }
}

/// The first `N` numbers of a statement, which must have at least `required` of them (the
/// missing ones are 0, the extra ones ignored: `w` of `v` and `vt`, colours after `v`).
fn numbers<'a, const N: usize>(
    tokens: impl Iterator<Item = &'a str>,
    required: usize,
) -> Result<[Number; N]> {
    let mut numbers = [Number::ZERO; N];
    let mut count = 0;
    for token in tokens.take(N) {
        numbers[count] =
            Number::parse(token).ok_or_else(|| anyhow!("invalid number \"{token}\""))?;
        count += 1;
    }
    if count < required {
        bail!("{required} numbers expected, {count} found");
    }
    Ok(numbers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corner(position: u32, uv: Option<u32>, normal: Option<u32>) -> Corner {
        Corner {
            position,
            uv,
            normal,
        }
    }

    #[test]
    fn parses_statements() {
        let obj = Obj::parse(
            "# comment
mtllib x.mtl
o First
v 1 2 3
v 4.5 5 6 1.0
v 7 8 9 0.5 0.5 0.5
vt 0.25 0.5
vt 0.75
vn 0 0 1
usemtl red
s off
f 1/1/1 2/2/1 3/1/1
g Group A
s 2
f 1//1 2//1 3//1
l 1 2
",
        )
        .unwrap();
        assert_eq!(obj.positions.len(), 3);
        assert_eq!(obj.positions[1][0].decimal, Decimal::parse("4.5"));
        assert_eq!(obj.positions[1][0].value, 4.5);
        assert_eq!(obj.uvs[1][1], Number::ZERO);
        assert_eq!(obj.objects, ["First"]);
        assert_eq!(obj.groups, ["Group A"]);
        assert!(obj.has_smoothing);
        assert_eq!(obj.faces.len(), 2);
        assert_eq!(obj.faces[0].corners[1], corner(1, Some(1), Some(0)));
        assert_eq!(
            (
                obj.faces[0].object,
                obj.faces[0].group,
                obj.faces[0].smoothing
            ),
            (Some(0), None, 0)
        );
        assert_eq!(
            (
                obj.faces[1].group,
                obj.faces[1].smoothing,
                obj.faces[1].line
            ),
            (Some(0), 2, 15)
        );
        assert_eq!(obj.faces[1].corners[0], corner(0, None, Some(0)));
        assert_eq!(obj.skipped, [(16, "l 1 2".to_string())]);
    }

    #[test]
    fn parses_corner_forms_and_negative_indices() {
        let obj = Obj::parse(
            "v 0 0 0
v 1 0 0
v 0 1 0
v 1 1 0
vt 0 0
vn 0 0 1
f 1 2 3 4
f -4/-1 -3/-1/-1 -2//-1
",
        )
        .unwrap();
        assert_eq!(obj.faces[0].corners.len(), 4);
        assert_eq!(obj.faces[0].corners[3], corner(3, None, None));
        assert_eq!(obj.faces[1].corners[0], corner(0, Some(0), None));
        assert_eq!(obj.faces[1].corners[1], corner(1, Some(0), Some(0)));
        assert_eq!(obj.faces[1].corners[2], corner(2, None, Some(0)));
        assert!(!obj.has_smoothing);
        assert_eq!(obj.faces[0].smoothing, 1);
    }

    #[test]
    fn reports_errors_with_line_numbers() {
        let error = |text: &str| Obj::parse(text).unwrap_err().to_string();
        assert!(error("v 0 0\n").starts_with("line 1:"));
        assert!(error("v 0 0 x\n").contains("invalid number"));
        assert!(error("v 0 0 0\nf 1 1\n").contains("at least 3 corners"));
        assert!(error("v 0 0 0\nf 1 2 0\n").contains("index 0"));
        assert!(error("v 0 0 0\nf 1 2 3\n").contains("position 2 doesn't exist"));
        assert!(error("v 0 0 0\nf 1/1 1 1\n").contains("texture coordinate 1 doesn't exist"));
        assert!(error("v 0 0 0\nf -2 1 1\n").contains("out of range"));
        assert!(error("v 0 0 0\nf 1/1/1/1 1 1\n").contains("invalid corner"));
        assert!(error("v 0 0 0\nf /1 1 1\n").contains("no position"));
    }

    #[test]
    fn keeps_values_that_arent_decimals() {
        let obj = Obj::parse("v nan 1e50 -inf\n").unwrap();
        assert_eq!(obj.positions[0][0].decimal, None);
        assert!(obj.positions[0][0].value.is_nan());
        assert_eq!(obj.positions[0][1].decimal, None);
        assert_eq!(obj.positions[0][1].value, f32::INFINITY);
    }
}
