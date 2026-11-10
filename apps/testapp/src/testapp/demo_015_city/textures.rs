//! The city's textures, generated in code: seamless (they tile), with the tint mask in alpha
//! (1 where the node's `tint` colours the texture, 0 where it doesn't, e.g. windows).

/// An RGBA image being painted, colours in sRGB 0..1.
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pixels: Vec<[f32; 4]>,
}

impl Canvas {
    pub fn new(width: usize, height: usize, color: [f32; 3]) -> Self {
        Self {
            width,
            height,
            pixels: vec![[color[0], color[1], color[2], 1.0]; width * height],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> [f32; 4] {
        self.pixels[y * self.width + x]
    }

    pub fn set(&mut self, x: usize, y: usize, color: [f32; 4]) {
        self.pixels[y * self.width + x] = color;
    }

    /// Calls `f(x, y, u, v)` for every pixel, `u` and `v` in 0..1, and stores what it returns.
    pub fn paint(&mut self, mut f: impl FnMut(usize, usize, f32, f32, [f32; 4]) -> [f32; 4]) {
        for y in 0..self.height {
            for x in 0..self.width {
                let u = (x as f32 + 0.5) / self.width as f32;
                let v = (y as f32 + 0.5) / self.height as f32;
                let old = self.get(x, y);
                self.set(x, y, f(x, y, u, v, old));
            }
        }
    }

    /// Fills the rectangle `u0..u1` x `v0..v1` (in 0..1) with `color` and tint mask `mask`.
    pub fn rect(&mut self, u0: f32, v0: f32, u1: f32, v1: f32, color: [f32; 3], mask: f32) {
        self.paint(|_, _, u, v, old| {
            if u >= u0 && u < u1 && v >= v0 && v < v1 {
                [color[0], color[1], color[2], mask]
            } else {
                old
            }
        });
    }

    /// Multiplies the colour by `1 + amount * (noise - 0.5)`: surface variation.
    pub fn grain(&mut self, noise: &Noise, scale: usize, amount: f32) {
        let (w, h) = (self.width, self.height);
        self.paint(|x, y, _, _, c| {
            let n = noise.fbm(
                x as f32 * scale as f32 / w as f32,
                y as f32 * scale as f32 / h as f32,
                scale,
                4,
            );
            let k = 1.0 + amount * (n - 0.5);
            [c[0] * k, c[1] * k, c[2] * k, c[3]]
        });
    }

    /// 8-bit RGBA, row by row.
    pub fn into_rgba(self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8))
            .collect()
    }
}

/// Seamless value noise: its lattice wraps, so a texture made with a period that divides the
/// texture tiles without seams.
pub struct Noise {
    seed: u32,
}

impl Noise {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    fn hash(&self, x: i64, y: i64) -> f32 {
        let mut h = (x as u32).wrapping_mul(0x8da6_b343)
            ^ (y as u32).wrapping_mul(0xd816_3841)
            ^ self.seed.wrapping_mul(0xcb1a_b31f);
        h ^= h >> 13;
        h = h.wrapping_mul(0x5bd1_e995);
        h ^= h >> 15;
        (h & 0xff_ffff) as f32 / 0x100_0000 as f32
    }

    /// Smooth noise in 0..1 at (`x`, `y`) on a lattice of `period` cells that wraps.
    pub fn value(&self, x: f32, y: f32, period: usize) -> f32 {
        let p = period.max(1) as i64;
        let (xi, yi) = (x.floor() as i64, y.floor() as i64);
        let (fx, fy) = (x - x.floor(), y - y.floor());
        let s = |t: f32| t * t * (3.0 - 2.0 * t);
        let (sx, sy) = (s(fx), s(fy));
        let at = |i: i64, j: i64| self.hash(i.rem_euclid(p), j.rem_euclid(p));
        let top = at(xi, yi) * (1.0 - sx) + at(xi + 1, yi) * sx;
        let bottom = at(xi, yi + 1) * (1.0 - sx) + at(xi + 1, yi + 1) * sx;
        top * (1.0 - sy) + bottom * sy
    }

    /// Several octaves of `value`, each twice finer and half as strong.
    pub fn fbm(&self, x: f32, y: f32, period: usize, octaves: u32) -> f32 {
        let (mut sum, mut weight, mut total, mut scale) = (0.0, 1.0, 0.0, 1.0);
        for _ in 0..octaves {
            sum += self.value(x * scale, y * scale, period * scale as usize) * weight;
            total += weight;
            weight *= 0.5;
            scale *= 2.0;
        }
        sum / total
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// A generated texture: its name and its 8-bit RGBA pixels.
pub struct Texture {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

fn finish(name: &'static str, canvas: Canvas) -> Texture {
    Texture {
        name,
        width: canvas.width as u32,
        height: canvas.height as u32,
        pixels: canvas.into_rgba(),
    }
}

/// Window glass: dark, bluish, brighter at the top (the sky's reflection), not tinted.
fn glass(u: f32, v: f32, noise: &Noise) -> [f32; 4] {
    let sky = mix([0.42, 0.55, 0.68], [0.12, 0.17, 0.24], v.powf(0.7));
    let streak = 0.06 * noise.value(u * 3.0 + v * 6.0, v * 2.0, 64);
    [sky[0] + streak, sky[1] + streak, sky[2] + streak, 0.0]
}

/// One bay of a glass curtain wall: 3 m wide, one 3.5 m floor high.
fn facade_glass() -> Texture {
    let noise = Noise::new(11);
    let mut c = Canvas::new(256, 256, [0.75, 0.77, 0.8]);
    c.paint(|_, _, u, v, _| {
        let frame = u < 0.03 || u > 0.97 || v < 0.03;
        if frame {
            [0.78, 0.8, 0.83, 1.0]
        } else if v > 0.78 {
            // Spandrel panel between floors.
            [0.22, 0.24, 0.28, 1.0]
        } else if (u - 0.5).abs() < 0.012 {
            [0.7, 0.72, 0.75, 1.0]
        } else {
            glass(u, v / 0.78, &noise)
        }
    });
    c.grain(&noise, 8, 0.08);
    finish("Glass facade", c)
}

/// Brick wall with one window per bay.
fn facade_brick() -> Texture {
    let noise = Noise::new(12);
    let mut c = Canvas::new(256, 256, [0.6, 0.3, 0.22]);
    let (rows, cols) = (24.0, 6.0);
    c.paint(|_, _, u, v, _| {
        let row = (v * rows).floor();
        let offset = if row as i32 % 2 == 0 { 0.0 } else { 0.5 };
        let bu = (u * cols + offset).fract();
        let bv = (v * rows).fract();
        let mortar = bu < 0.05 || bv < 0.12;
        let shade = 0.8 + 0.4 * noise.value((u * cols + offset).floor() * 7.3, row * 3.1, 64);
        if mortar {
            [0.72, 0.68, 0.62, 1.0]
        } else {
            [0.62 * shade, 0.32 * shade, 0.24 * shade, 1.0]
        }
    });
    // The window: a white frame, glass, and a stone sill.
    c.rect(0.24, 0.16, 0.76, 0.76, [0.92, 0.92, 0.9], 0.0);
    c.paint(|_, _, u, v, old| {
        if u > 0.27 && u < 0.73 && v > 0.19 && v < 0.73 {
            if (u - 0.5).abs() < 0.012 || (v - 0.4).abs() < 0.012 {
                [0.92, 0.92, 0.9, 0.0]
            } else {
                glass((u - 0.27) / 0.46, (v - 0.19) / 0.54, &noise)
            }
        } else {
            old
        }
    });
    c.rect(0.21, 0.76, 0.79, 0.8, [0.8, 0.78, 0.72], 0.0);
    c.grain(&noise, 16, 0.12);
    finish("Brick facade", c)
}

/// Concrete panels with a band of windows.
fn facade_concrete() -> Texture {
    let noise = Noise::new(13);
    let mut c = Canvas::new(256, 256, [0.78, 0.77, 0.74]);
    c.paint(|_, _, u, v, old| {
        if (0.18..0.66).contains(&v) {
            let mullion = u < 0.02 || u > 0.98 || (u - 0.5).abs() < 0.015;
            if mullion {
                [0.3, 0.3, 0.32, 0.0]
            } else {
                glass(u, (v - 0.18) / 0.48, &noise)
            }
        } else if (v - 0.85).abs() < 0.008 || u < 0.006 || u > 0.994 {
            [0.6, 0.6, 0.58, 1.0]
        } else {
            old
        }
    });
    c.grain(&noise, 16, 0.18);
    finish("Concrete facade", c)
}

/// Pale stone with grooves and a tall window.
fn facade_stone() -> Texture {
    let noise = Noise::new(14);
    let mut c = Canvas::new(256, 256, [0.86, 0.8, 0.68]);
    c.paint(|_, _, u, v, old| {
        if (v * 7.0).fract() < 0.06 {
            [0.7, 0.64, 0.54, 1.0]
        } else if u > 0.3 && u < 0.7 && v > 0.1 && v < 0.82 {
            if u < 0.33 || u > 0.67 || v < 0.13 {
                [0.25, 0.22, 0.2, 0.0]
            } else {
                glass((u - 0.33) / 0.34, (v - 0.13) / 0.69, &noise)
            }
        } else {
            old
        }
    });
    c.grain(&noise, 16, 0.15);
    finish("Stone facade", c)
}

/// Flat roof: gravel and concrete, 8 m a tile.
fn roof() -> Texture {
    let noise = Noise::new(15);
    let mut c = Canvas::new(256, 256, [0.48, 0.47, 0.45]);
    c.paint(|x, y, u, v, _| {
        let n = noise.fbm(u * 8.0, v * 8.0, 8, 5);
        let speck = noise.value(x as f32 * 0.9, y as f32 * 0.9, 230);
        let k = 0.75 + 0.5 * n + 0.15 * (speck - 0.5);
        [0.5 * k, 0.49 * k, 0.47 * k, 1.0]
    });
    finish("Roof", c)
}

/// Asphalt: dark grey with fine grain and faint patches.
fn asphalt_at(noise: &Noise, x: usize, y: usize, u: f32, v: f32) -> [f32; 3] {
    let n = noise.fbm(u * 4.0, v * 4.0, 4, 5);
    let speck = noise.value(x as f32 * 0.7, y as f32 * 0.7, 179);
    let k = 0.8 + 0.35 * n + 0.2 * (speck - 0.5);
    [0.2 * k, 0.2 * k, 0.21 * k]
}

/// A road, 7 m across (`u`) and 7 m along (`v`): edge lines and a dashed centre line.
fn road() -> Texture {
    let noise = Noise::new(16);
    let mut c = Canvas::new(256, 256, [0.2, 0.2, 0.2]);
    c.paint(|x, y, u, v, _| {
        let a = asphalt_at(&noise, x, y, u, v);
        let edge = (u - 0.05).abs() < 0.008 || (u - 0.95).abs() < 0.008;
        let centre = (u - 0.5).abs() < 0.01 && v < 0.43;
        if edge || centre {
            let worn = 0.85 + 0.15 * noise.value(u * 40.0, v * 40.0, 40);
            [0.86 * worn, 0.86 * worn, 0.82 * worn, 1.0]
        } else {
            [a[0], a[1], a[2], 1.0]
        }
    });
    finish("Road", c)
}

fn intersection() -> Texture {
    let noise = Noise::new(16);
    let mut c = Canvas::new(256, 256, [0.2, 0.2, 0.2]);
    c.paint(|x, y, u, v, _| {
        let a = asphalt_at(&noise, x, y, u, v);
        [a[0], a[1], a[2], 1.0]
    });
    finish("Intersection", c)
}

/// Zebra stripes across a road (`u` across it), on asphalt.
fn crosswalk() -> Texture {
    let noise = Noise::new(17);
    let mut c = Canvas::new(256, 256, [0.2, 0.2, 0.2]);
    c.paint(|x, y, u, v, _| {
        let a = asphalt_at(&noise, x, y, u, v);
        let stripe = (u * 7.0).fract() < 0.5 && v > 0.08 && v < 0.92;
        if stripe {
            let worn = 0.8 + 0.2 * noise.value(u * 30.0, v * 30.0, 30);
            [0.88 * worn, 0.88 * worn, 0.85 * worn, 1.0]
        } else {
            [a[0], a[1], a[2], 1.0]
        }
    });
    finish("Crosswalk", c)
}

/// Sidewalk pavers: 4 x 4 slabs of 0.5 m.
fn sidewalk() -> Texture {
    let noise = Noise::new(18);
    let mut c = Canvas::new(256, 256, [0.7, 0.68, 0.64]);
    c.paint(|_, _, u, v, _| {
        let (su, sv) = ((u * 4.0).fract(), (v * 4.0).fract());
        let slab = noise.value((u * 4.0).floor() * 5.7, (v * 4.0).floor() * 3.3, 64);
        if su < 0.03 || sv < 0.03 {
            [0.45, 0.44, 0.42, 1.0]
        } else {
            let k = 0.9 + 0.2 * slab;
            [0.72 * k, 0.7 * k, 0.66 * k, 1.0]
        }
    });
    c.grain(&noise, 16, 0.12);
    finish("Sidewalk", c)
}

fn concrete() -> Texture {
    let noise = Noise::new(19);
    let mut c = Canvas::new(128, 128, [0.66, 0.65, 0.62]);
    c.grain(&noise, 8, 0.25);
    finish("Concrete", c)
}

fn grass() -> Texture {
    let noise = Noise::new(20);
    let mut c = Canvas::new(256, 256, [0.3, 0.45, 0.2]);
    c.paint(|x, y, u, v, _| {
        let n = noise.fbm(u * 6.0, v * 6.0, 6, 5);
        let blade = noise.value(x as f32 * 1.3, y as f32 * 0.6, 333);
        let k = 0.7 + 0.5 * n + 0.25 * (blade - 0.5);
        let dry = noise.fbm(u * 2.0 + 3.0, v * 2.0, 2, 3);
        let base = mix(
            [0.26, 0.46, 0.17],
            [0.45, 0.47, 0.22],
            (dry - 0.45).max(0.0) * 2.5,
        );
        [base[0] * k, base[1] * k, base[2] * k, 1.0]
    });
    finish("Grass", c)
}

fn bark() -> Texture {
    let noise = Noise::new(21);
    let mut c = Canvas::new(128, 128, [0.35, 0.26, 0.18]);
    c.paint(|_, _, u, v, _| {
        let n = noise.fbm(u * 16.0, v * 2.0, 16, 4);
        let k = 0.6 + 0.8 * n;
        [0.36 * k, 0.27 * k, 0.19 * k, 1.0]
    });
    finish("Bark", c)
}

fn leaves() -> Texture {
    let noise = Noise::new(22);
    let mut c = Canvas::new(256, 256, [0.2, 0.4, 0.15]);
    c.paint(|x, y, u, v, _| {
        let clump = noise.fbm(u * 8.0, v * 8.0, 8, 4);
        let leaf = noise.value(x as f32 * 0.5, y as f32 * 0.5, 128);
        let k = 0.45 + 0.8 * clump * (0.7 + 0.6 * leaf);
        let tone = mix([0.16, 0.36, 0.1], [0.36, 0.5, 0.16], leaf);
        [tone[0] * k, tone[1] * k, tone[2] * k, 1.0]
    });
    finish("Leaves", c)
}

fn metal() -> Texture {
    let noise = Noise::new(23);
    let mut c = Canvas::new(64, 64, [0.25, 0.27, 0.28]);
    c.grain(&noise, 4, 0.2);
    finish("Metal", c)
}

/// Where each part of a car is in the car texture, as (u0, v0, u1, v1).
pub mod car_regions {
    pub const PAINT: [f32; 4] = [0.01, 0.01, 0.49, 0.49];
    pub const GLASS: [f32; 4] = [0.51, 0.01, 0.99, 0.49];
    pub const FRONT: [f32; 4] = [0.01, 0.51, 0.49, 0.74];
    pub const REAR: [f32; 4] = [0.01, 0.76, 0.49, 0.99];
    pub const TIRE: [f32; 4] = [0.51, 0.51, 0.74, 0.99];
    pub const HUB: [f32; 4] = [0.76, 0.51, 0.99, 0.99];
}

/// Everything a car is painted with, in regions (`car_regions`). Paint is tinted per car.
fn car() -> Texture {
    use car_regions::*;
    let noise = Noise::new(24);
    let mut c = Canvas::new(512, 512, [0.0, 0.0, 0.0]);
    let inside = |r: [f32; 4], u: f32, v: f32| -> Option<(f32, f32)> {
        (u >= r[0] - 0.01 && u < r[2] + 0.01 && v >= r[1] - 0.01 && v < r[3] + 0.01)
            .then(|| ((u - r[0]) / (r[2] - r[0]), (v - r[1]) / (r[3] - r[1])))
    };
    c.paint(|_, _, u, v, _| {
        if let Some((_, y)) = inside(PAINT, u, v) {
            // Paint, a bit lighter at the top, a dark trim strip at the bottom.
            if y > 0.86 {
                [0.12, 0.12, 0.13, 0.0]
            } else {
                let k = 0.95 - 0.15 * y;
                [k, k, k, 1.0]
            }
        } else if let Some((x, y)) = inside(GLASS, u, v) {
            let g = glass(x, y, &noise);
            [g[0] * 0.7, g[1] * 0.7, g[2] * 0.75, 0.0]
        } else if let Some((x, y)) = inside(FRONT, u, v) {
            let lamp = (x < 0.22 || x > 0.78) && y < 0.42;
            let grille = (0.3..0.7).contains(&x) && (0.3..0.62).contains(&y);
            if lamp {
                [0.95, 0.95, 0.9, 0.0]
            } else if grille {
                let slat = ((y * 20.0).fract() < 0.5) as i32 as f32;
                [
                    0.08 + 0.06 * slat,
                    0.08 + 0.06 * slat,
                    0.09 + 0.06 * slat,
                    0.0,
                ]
            } else if y > 0.75 {
                [0.12, 0.12, 0.13, 0.0]
            } else {
                [0.9, 0.9, 0.9, 1.0]
            }
        } else if let Some((x, y)) = inside(REAR, u, v) {
            let lamp = (x < 0.24 || x > 0.76) && y < 0.4;
            let plate = (0.38..0.62).contains(&x) && (0.45..0.65).contains(&y);
            if lamp {
                [0.75, 0.05, 0.04, 0.0]
            } else if plate {
                [0.92, 0.9, 0.8, 0.0]
            } else if y > 0.75 {
                [0.12, 0.12, 0.13, 0.0]
            } else {
                [0.9, 0.9, 0.9, 1.0]
            }
        } else if let Some((_, y)) = inside(TIRE, u, v) {
            let tread = ((y * 24.0).fract() < 0.5) as i32 as f32;
            [
                0.06 + 0.03 * tread,
                0.06 + 0.03 * tread,
                0.06 + 0.03 * tread,
                0.0,
            ]
        } else if let Some((x, y)) = inside(HUB, u, v) {
            let r = ((x - 0.5).powi(2) + (y - 0.5).powi(2)).sqrt();
            if r < 0.3 {
                [0.7, 0.71, 0.73, 0.0]
            } else {
                [0.07, 0.07, 0.07, 0.0]
            }
        } else {
            [0.1, 0.1, 0.1, 0.0]
        }
    });
    c.grain(&noise, 16, 0.05);
    finish("Car", c)
}

/// Where each part of a person is in the person texture, as (u0, v0, u1, v1).
pub mod person_regions {
    pub const SKIN: [f32; 4] = [0.02, 0.02, 0.48, 0.48];
    pub const HAIR: [f32; 4] = [0.52, 0.02, 0.98, 0.48];
    pub const SHIRT: [f32; 4] = [0.02, 0.52, 0.48, 0.98];
    pub const PANTS: [f32; 4] = [0.52, 0.52, 0.98, 0.73];
    pub const SHOES: [f32; 4] = [0.52, 0.77, 0.98, 0.98];
}

/// Skin, hair, a shirt (tinted per person), jeans and shoes.
fn person() -> Texture {
    use person_regions::*;
    let noise = Noise::new(25);
    let mut c = Canvas::new(256, 256, [0.5, 0.5, 0.5]);
    let regions: [([f32; 4], [f32; 3], f32); 5] = [
        (SKIN, [0.86, 0.66, 0.52], 0.0),
        (HAIR, [0.2, 0.13, 0.08], 0.0),
        (SHIRT, [0.92, 0.92, 0.92], 1.0),
        (PANTS, [0.18, 0.25, 0.42], 0.0),
        (SHOES, [0.12, 0.1, 0.09], 0.0),
    ];
    for (r, color, mask) in regions {
        c.rect(
            r[0] - 0.02,
            r[1] - 0.02,
            r[2] + 0.02,
            r[3] + 0.02,
            color,
            mask,
        );
    }
    c.grain(&noise, 16, 0.15);
    finish("Person", c)
}

/// Every texture of the city.
pub fn all() -> Vec<Texture> {
    vec![
        facade_glass(),
        facade_brick(),
        facade_concrete(),
        facade_stone(),
        roof(),
        road(),
        intersection(),
        crosswalk(),
        sidewalk(),
        concrete(),
        grass(),
        bark(),
        leaves(),
        metal(),
        car(),
        person(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_tiles_seamlessly() {
        let noise = Noise::new(1);
        for i in 0..20 {
            let y = i as f32 * 0.37;
            assert!((noise.value(0.0, y, 8) - noise.value(8.0, y, 8)).abs() < 1e-6);
            assert!((noise.fbm(y, 0.0, 4, 3) - noise.fbm(y, 4.0, 4, 3)).abs() < 1e-6);
        }
    }

    #[test]
    fn textures_have_their_sizes_and_tint_masks() {
        for texture in all() {
            assert_eq!(
                texture.pixels.len(),
                (texture.width * texture.height * 4) as usize
            );
        }
        // Car paint is tinted, car glass isn't.
        let car = car();
        let alpha = |u: f32, v: f32| {
            let (x, y) = (
                (u * car.width as f32) as usize,
                (v * car.height as f32) as usize,
            );
            car.pixels[(y * car.width as usize + x) * 4 + 3]
        };
        assert_eq!(alpha(0.2, 0.2), 255);
        assert_eq!(alpha(0.7, 0.2), 0);
    }
}
