//! Text drawn with a font atlas.
//!
//! A font atlas is an atlas of character images, as made by the font creator tool
//! (`tools/font-creator`) and packed by the atlas creator: every image has the same height
//! (the font's line, ascent to descent, with the baseline on the same row in all of them) and
//! is as wide as the character's advance, and is named after the character's code point in
//! decimal (`65` for `A`). The images are white, with the glyph's coverage in alpha.
//!
//! `TextBuilder` lays a text out. `Text` shows it on a node and updates it when the text
//! changes, in one of two ways (`TextMethod`, the two methods of the C++ feather's text
//! builder):
//!
//! - `OneMesh`: one mesh for the whole text, a quad per character, with the character's
//!   rectangle of the atlas baked into the texture coordinates. One draw per text; changing the
//!   text builds a new mesh.
//! - `NodePerGlyph`: a child node per character, all sharing one square mesh, each with its
//!   rectangle of the atlas in uniforms. A draw per character, but changing the text only
//!   updates nodes, and each character can be moved on its own.
//!
//! Both are drawn with a material whose texture is the whole atlas (`create_text_material`).

use std::sync::OnceLock;

use anyhow::{anyhow, Result};

use super::atlas::Atlas;
use super::math::{Mat4, Vec2, Vec3};
use super::mesh::Mesh;
use super::meshbuilderplane::MeshBuilderPlane;
use super::object::Object;
use super::scene::Scene;
use super::uniformname::UniformName;
use super::vertex::Vertex;

/// Where text is put horizontally relative to its origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HorizontalAlign {
    /// Lines start at the origin.
    Left,
    /// Lines are centred on the origin.
    Center,
    /// Lines end at the origin.
    Right,
    /// Lines start at the origin and, when wrapped (`TextBuilder::wrap`), lines broken by the
    /// wrapping are stretched to the wrap width by widening their spaces. Without wrapping it
    /// is the same as `Left`.
    Justify,
}

/// Where text is put vertically relative to its origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalAlign {
    /// The top of the first line is at the origin.
    Top,
    /// The text is centred on the origin.
    Center,
    /// The bottom of the last line is at the origin.
    Bottom,
}

/// The plane text is laid in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextPlane {
    /// The XY plane (z = 0), read from +Z: lines go along +X and follow each other down -Y.
    XY,
    /// The XZ plane (y = 0), read from +Y (e.g. text lying on a floor): lines go along +X and
    /// follow each other along +Z. It is the XY layout turned by -90° around X.
    XZ,
}

/// How `Text` draws text: see the module documentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextMethod {
    /// One mesh for the whole text.
    OneMesh,
    /// A node per character, sharing one square mesh.
    NodePerGlyph,
}

/// The size of laid-out text, in scene units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextSize {
    /// The width of the longest line.
    pub width: f32,
    /// From the top of the first line to the bottom of the last one.
    pub height: f32,
}

/// One line of laid-out text.
struct Line {
    chars: Vec<char>,
    /// The natural width, in scene units.
    width: f32,
    /// Whether the line was broken by wrapping, so it is stretched when justified.
    stretch: bool,
}

/// One character's quad, in the XY layout: `x` left to right, `y` bottom to top, and its
/// rectangle in the atlas (`v.0` is the top row).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Quad {
    x: (f32, f32),
    y: (f32, f32),
    u: (f32, f32),
    v: (f32, f32),
}

/// Lays out text with a font atlas and builds meshes for it.
///
/// Characters the atlas has no image for take a quarter of the line height and are not drawn.
/// `\n` starts a new line (a new paragraph, when wrapping); `\r` is ignored.
#[derive(Clone, Debug)]
pub struct TextBuilder {
    /// Handle of the font atlas in `Scene::atlases`.
    atlas: usize,
    height: f32,
    line_spacing: f32,
    horizontal: HorizontalAlign,
    vertical: VerticalAlign,
    plane: TextPlane,
    method: TextMethod,
    wrap: Option<f32>,
}

impl TextBuilder {
    /// A builder for the font atlas `atlas` (a handle in `Scene::atlases`): lines 1 unit high,
    /// starting at the origin and going down from it, in the XY plane, as one mesh.
    pub fn new(atlas: usize) -> Self {
        Self {
            atlas,
            height: 1.0,
            line_spacing: 1.0,
            horizontal: HorizontalAlign::Left,
            vertical: VerticalAlign::Top,
            plane: TextPlane::XY,
            method: TextMethod::OneMesh,
            wrap: None,
        }
    }

    /// The height of a line in scene units: every character's image is scaled to it.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The distance between the tops of neighbouring lines, in line heights (1 = lines touch).
    pub fn line_spacing(mut self, line_spacing: f32) -> Self {
        self.line_spacing = line_spacing;
        self
    }

    pub fn align(mut self, horizontal: HorizontalAlign, vertical: VerticalAlign) -> Self {
        self.horizontal = horizontal;
        self.vertical = vertical;
        self
    }

    pub fn plane(mut self, plane: TextPlane) -> Self {
        self.plane = plane;
        self
    }

    /// Breaks lines between words so they fit in `width` scene units (`None`: lines break only
    /// at `\n`). A word wider than `width` gets a line of its own.
    pub fn wrap(mut self, width: Option<f32>) -> Self {
        self.wrap = width;
        self
    }

    /// How `Text` draws the text (`TextBuilder::build_mesh` always builds one mesh).
    pub fn method(mut self, method: TextMethod) -> Self {
        self.method = method;
        self
    }

    pub fn get_method(&self) -> TextMethod {
        self.method
    }

    pub fn get_atlas(&self) -> usize {
        self.atlas
    }

    /// The size `text` takes. Fails if the font atlas is not in the scene.
    pub fn measure(&self, scene: &Scene, text: &str) -> Result<TextSize> {
        Ok(self.layout(self.font(scene)?, text).1)
    }

    /// Builds a mesh, named `name`, showing `text` and adds it to `scene`. Returns its handle
    /// (`None` when nothing would be drawn, e.g. for empty text or only spaces) and the size
    /// of the text. Fails if the font atlas is not in the scene.
    pub fn build_mesh(
        &self,
        scene: &mut Scene,
        text: &str,
        name: Option<String>,
    ) -> Result<(Option<usize>, TextSize)> {
        let (quads, size) = self.layout(self.font(scene)?, text);
        if quads.is_empty() {
            return Ok((None, size));
        }
        let (vertices, indices) = self.geometry(&quads);
        Ok((
            Some(scene.add_mesh(Mesh::new(name, vertices, indices))),
            size,
        ))
    }

    fn font<'a>(&self, scene: &'a Scene) -> Result<&'a Atlas> {
        scene
            .get_atlas(self.atlas)
            .ok_or_else(|| anyhow!("Font atlas {} not found", self.atlas))
    }

    /// The quads of the characters of `text` and the size of the text.
    fn layout(&self, font: &Atlas, text: &str) -> (Vec<Quad>, TextSize) {
        let (atlas_width, atlas_height) = (font.get_width() as f32, font.get_height() as f32);
        let mut name = String::new();
        // The width of `c` in scene units and its rectangle in the atlas, if it has one.
        let mut glyph = |c: char| {
            name.clear();
            name.push_str(&(c as u32).to_string());
            match font.get_region(&name) {
                Some(r) if r.height > 0 => {
                    let width = r.width as f32 * self.height / r.height as f32;
                    let u = (
                        r.x as f32 / atlas_width,
                        (r.x + r.width) as f32 / atlas_width,
                    );
                    let v = (
                        r.y as f32 / atlas_height,
                        (r.y + r.height) as f32 / atlas_height,
                    );
                    (width, Some((u, v)))
                }
                _ => (self.height / 4.0, None),
            }
        };

        let lines = self.lines(text, &mut |c| glyph(c).0);
        let justify_width = match (self.horizontal, self.wrap) {
            (HorizontalAlign::Justify, Some(width)) => Some(width),
            _ => None,
        };
        let longest = lines.iter().map(|l| l.width).fold(0.0, f32::max);
        let size = TextSize {
            width: match justify_width {
                Some(width) if lines.iter().any(|l| l.stretch) => longest.max(width),
                _ => longest,
            },
            height: self.height * (1.0 + (lines.len() - 1) as f32 * self.line_spacing),
        };

        let top = match self.vertical {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Center => size.height / 2.0,
            VerticalAlign::Bottom => size.height,
        };
        let mut quads = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let line_top = top - index as f32 * self.height * self.line_spacing;
            let mut x = match self.horizontal {
                HorizontalAlign::Left | HorizontalAlign::Justify => 0.0,
                HorizontalAlign::Center => -line.width / 2.0,
                HorizontalAlign::Right => -line.width,
            };
            // Justified lines share the room left over among their spaces.
            let spaces = line.chars.iter().filter(|&&c| c == ' ').count();
            let space_extra = match justify_width {
                Some(width) if line.stretch && spaces > 0 && line.width < width => {
                    (width - line.width) / spaces as f32
                }
                _ => 0.0,
            };
            for &c in &line.chars {
                let (advance, rect) = glyph(c);
                if let (Some((u, v)), false) = (rect, c.is_whitespace()) {
                    quads.push(Quad {
                        x: (x, x + advance),
                        y: (line_top - self.height, line_top),
                        u,
                        v,
                    });
                }
                x += advance;
                if c == ' ' {
                    x += space_extra;
                }
            }
        }
        (quads, size)
    }

    /// The lines of `text`: split at `\n` (without `\r`) and, with `wrap`, broken between
    /// words to fit in it. `advance` gives a character's width.
    fn lines(&self, text: &str, advance: &mut dyn FnMut(char) -> f32) -> Vec<Line> {
        let width_of = |chars: &[char], advance: &mut dyn FnMut(char) -> f32| -> f32 {
            chars.iter().map(|&c| advance(c)).sum()
        };
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let chars: Vec<char> = paragraph.chars().filter(|&c| c != '\r').collect();
            let Some(max_width) = self.wrap else {
                let width = width_of(&chars, advance);
                lines.push(Line {
                    chars,
                    width,
                    stretch: false,
                });
                continue;
            };
            let space = advance(' ');
            let mut line = Line {
                chars: Vec::new(),
                width: 0.0,
                stretch: false,
            };
            for word in chars.split(|&c| c == ' ') {
                let word_width = width_of(word, advance);
                if !line.chars.is_empty() && line.width + space + word_width > max_width {
                    // The line is full: it is stretched when justified.
                    line.stretch = true;
                    lines.push(std::mem::replace(
                        &mut line,
                        Line {
                            chars: Vec::new(),
                            width: 0.0,
                            stretch: false,
                        },
                    ));
                } else if !line.chars.is_empty() {
                    line.chars.push(' ');
                    line.width += space;
                }
                line.chars.extend_from_slice(word);
                line.width += word_width;
            }
            // The last line of a paragraph keeps its natural width.
            lines.push(line);
        }
        lines
    }

    /// The transform of a glyph node showing `quad`: it scales and moves the glyph mesh
    /// (`GLYPH_MESH_NAME`) onto the quad, in the builder's plane.
    fn glyph_transform(&self, quad: &Quad) -> Mat4 {
        let plane = match self.plane {
            TextPlane::XY => Mat4::from_scale(1.0),
            TextPlane::XZ => Mat4::from_angle_x(cgmath::Deg(-90.0)),
        };
        plane
            * Mat4::from_translation(Vec3::new(quad.x.0, quad.y.1, 0.0))
            * Mat4::from_nonuniform_scale(quad.x.1 - quad.x.0, quad.y.1 - quad.y.0, 1.0)
    }

    /// Vertices and indices of `quads` in the builder's plane.
    fn geometry(&self, quads: &[Quad]) -> (Vec<Vertex>, Vec<u32>) {
        let (position, normal): (fn(f32, f32) -> Vec3, Vec3) = match self.plane {
            TextPlane::XY => (|x, y| Vec3::new(x, y, 0.0), Vec3::new(0.0, 0.0, 1.0)),
            TextPlane::XZ => (|x, y| Vec3::new(x, 0.0, -y), Vec3::new(0.0, 1.0, 0.0)),
        };
        let mut vertices = Vec::with_capacity(quads.len() * 4);
        let mut indices = Vec::with_capacity(quads.len() * 6);
        for q in quads {
            let first = vertices.len() as u32;
            // Top left, top right, bottom left, bottom right.
            for (x, y, u, v) in [
                (q.x.0, q.y.1, q.u.0, q.v.0),
                (q.x.1, q.y.1, q.u.1, q.v.0),
                (q.x.0, q.y.0, q.u.0, q.v.1),
                (q.x.1, q.y.0, q.u.1, q.v.1),
            ] {
                vertices.push(Vertex::new(position(x, y), normal, Vec2::new(u, v)));
            }
            // Counter-clockwise seen from the front.
            let [tl, tr, bl, br] = [0, 1, 2, 3].map(|i| first + i);
            indices.extend([bl, br, tr, bl, tr, tl]);
        }
        (vertices, indices)
    }
}

/// The names of the `float` uniforms a text shader declares for the rectangle of a character in
/// the font atlas, used with `TextMethod::NodePerGlyph`. The shader maps its texture
/// coordinates with `mix(vec2(umin, vmin), vec2(umax, vmax), uv)`.
pub const GLYPH_RECT_UNIFORMS: [&str; 4] = ["umin", "umax", "vmin", "vmax"];

/// `GLYPH_RECT_UNIFORMS` as ids, interned once: glyph nodes set them on every text change.
fn glyph_rect_ids() -> [UniformName; 4] {
    static IDS: OnceLock<[UniformName; 4]> = OnceLock::new();
    *IDS.get_or_init(|| GLYPH_RECT_UNIFORMS.map(UniformName::new))
}

/// The name of the mesh shared by the glyph nodes of `TextMethod::NodePerGlyph`: a 1x1 square
/// from (0, -1) to (1, 0) in the XY plane, so its top left corner is at the origin.
pub const GLYPH_MESH_NAME: &str = "Text glyph quad";

/// Creates a material for drawing text with shader `shader` from the font atlas `atlas`. Its
/// texture is the whole atlas: one-mesh text carries the characters' rectangles in its texture
/// coordinates, and glyph nodes in their uniforms. If the shader declares the
/// `GLYPH_RECT_UNIFORMS`, the material gives them the whole atlas (0..1), so the same shader
/// draws both kinds of text. Fails if the shader or the atlas is not in the scene.
pub fn create_text_material(
    scene: &mut Scene,
    name: Option<String>,
    shader: usize,
    atlas: usize,
) -> Result<usize> {
    let texture = scene
        .get_atlas(atlas)
        .ok_or_else(|| anyhow!("Font atlas {atlas} not found"))?
        .get_full_texture();
    let material = scene.create_material(name, shader)?;
    scene.material_set_texture(material, Some(texture))?;
    let layout = scene.get_shader(shader).unwrap().get_layout();
    if GLYPH_RECT_UNIFORMS.iter().all(|u| layout.find(u).is_some()) {
        let uniforms = scene.get_material_mut(material).unwrap().uniforms_mut();
        for (name, value) in glyph_rect_ids().into_iter().zip([0.0f32, 1.0, 0.0, 1.0]) {
            uniforms.set(name, value);
        }
    }
    Ok(material)
}

/// Text shown by a node, which can be changed while the scene is shown. How it is drawn
/// depends on the builder's `TextMethod`:
///
/// - `OneMesh`: the node is transparent and has the text's mesh. Changing the text builds a new
///   mesh and removes the old one (`Scene::remove_mesh`).
/// - `NodePerGlyph`: the node has a transparent child per drawn character (`get_glyph_nodes`,
///   in the text's order), sharing one square mesh and the material, with the character's
///   rectangle in the `GLYPH_RECT_UNIFORMS` and its place and size in its transform. Changing
///   the text reuses the child nodes, adding or removing (`Scene::remove_node`) the difference;
///   no meshes are built. The material's shader must declare the rectangle uniforms and must
///   not have the engine fill them from the material's texture (`ShaderUniformNames::umin`
///   ...), since the engine's values would win over the nodes'.
///
/// Uniforms set on the node (e.g. a colour) reach the glyph nodes too.
pub struct Text {
    node: usize,
    material: usize,
    builder: TextBuilder,
    text: String,
    mesh: Option<usize>,
    glyphs: Vec<usize>,
    size: TextSize,
}

impl Text {
    /// Creates a node named `name` under `parent` showing `text` laid out by `builder`, drawn
    /// with `material` (see `create_text_material`). Fails if the parent, the material or the
    /// font atlas is not in the scene, or the material's shader can't draw glyph nodes when the
    /// builder asks for them.
    pub fn create(
        scene: &mut Scene,
        parent: usize,
        name: Option<String>,
        material: usize,
        builder: TextBuilder,
        text: &str,
    ) -> Result<Self> {
        check_material(scene, material, &builder)?;
        let node = scene.create_node(name, parent)?;
        scene.node_set_material(node, material)?;
        let mut result = Self {
            node,
            material,
            builder,
            text: String::new(),
            mesh: None,
            glyphs: Vec::new(),
            size: TextSize::default(),
        };
        result.rebuild(scene, text)?;
        Ok(result)
    }

    /// Handle of the node showing the text: move it, give it uniforms (e.g. a colour), hide it.
    pub fn get_node(&self) -> usize {
        self.node
    }

    /// The nodes of the drawn characters, in the text's order, with `TextMethod::NodePerGlyph`
    /// (empty with `OneMesh`). Their transforms place them; they are set again when the text
    /// changes.
    pub fn get_glyph_nodes(&self) -> &[usize] {
        &self.glyphs
    }

    pub fn get_text(&self) -> &str {
        &self.text
    }

    /// The size of the text, in the node's units.
    pub fn get_size(&self) -> TextSize {
        self.size
    }

    pub fn get_builder(&self) -> &TextBuilder {
        &self.builder
    }

    /// Removes the text from the scene: its node, with its glyph nodes, and its mesh. The
    /// glyph mesh and the material stay, for other texts.
    pub fn remove(mut self, scene: &mut Scene) -> Result<()> {
        if let Some(mesh) = self.mesh.take() {
            scene.remove_mesh(mesh)?;
        }
        scene.remove_node(self.node)
    }

    /// Shows `text` instead. Does nothing if the text is the same.
    pub fn set_text(&mut self, scene: &mut Scene, text: &str) -> Result<()> {
        if text == self.text {
            return Ok(());
        }
        self.rebuild(scene, text)
    }

    /// Lays the text out again with `builder` (e.g. another height, alignment or method).
    pub fn set_builder(&mut self, scene: &mut Scene, builder: TextBuilder) -> Result<()> {
        check_material(scene, self.material, &builder)?;
        self.builder = builder;
        let text = std::mem::take(&mut self.text);
        self.rebuild(scene, &text)
    }

    fn rebuild(&mut self, scene: &mut Scene, text: &str) -> Result<()> {
        let name = scene
            .get_node(self.node)
            .ok_or_else(|| anyhow!("Text node {} not found", self.node))?
            .get_name();
        let (quads, size) = self.builder.layout(self.builder.font(scene)?, text);
        match self.builder.method {
            TextMethod::OneMesh => {
                self.set_glyph_count(scene, 0)?;
                let mesh = if quads.is_empty() {
                    None
                } else {
                    let (vertices, indices) = self.builder.geometry(&quads);
                    Some(scene.add_mesh(Mesh::new(name, vertices, indices)))
                };
                self.set_mesh(scene, mesh)?;
            }
            TextMethod::NodePerGlyph => {
                self.set_mesh(scene, None)?;
                self.set_glyph_count(scene, quads.len())?;
                for (&glyph, quad) in self.glyphs.iter().zip(&quads) {
                    let node = scene.get_node_mut(glyph).unwrap();
                    node.set_transform(self.builder.glyph_transform(quad));
                    let rect = [quad.u.0, quad.u.1, quad.v.0, quad.v.1];
                    for (name, value) in glyph_rect_ids().into_iter().zip(rect) {
                        node.uniforms_mut().set(name, value);
                    }
                }
            }
        }
        self.size = size;
        self.text = text.to_string();
        Ok(())
    }

    /// Replaces the node's mesh (one-mesh text) with `mesh`, removing the old one.
    fn set_mesh(&mut self, scene: &mut Scene, mesh: Option<usize>) -> Result<()> {
        if let Some(old) = self.mesh.take() {
            scene.remove_mesh(old)?;
        }
        let node = scene.get_node_mut(self.node).unwrap();
        // One-mesh text is drawn by the node itself; glyph nodes are drawn by themselves.
        node.set_transparent(mesh.is_some());
        match mesh {
            Some(mesh) => node.set_mesh(mesh),
            None => node.remove_mesh(),
        }
        self.mesh = mesh;
        Ok(())
    }

    /// Adds or removes glyph nodes so there are `count`.
    fn set_glyph_count(&mut self, scene: &mut Scene, count: usize) -> Result<()> {
        while self.glyphs.len() > count {
            scene.remove_node(self.glyphs.pop().unwrap())?;
        }
        if self.glyphs.len() < count {
            let mesh = glyph_mesh(scene)?;
            while self.glyphs.len() < count {
                let glyph = scene.create_node(None, self.node)?;
                scene.node_set_mesh(glyph, mesh)?;
                scene.node_set_material(glyph, self.material)?;
                scene.get_node_mut(glyph).unwrap().set_transparent(true);
                self.glyphs.push(glyph);
            }
        }
        Ok(())
    }
}

/// Fails if `material` doesn't exist or, when `builder` asks for glyph nodes, if its shader
/// can't take each node's character rectangle (see `Text`).
fn check_material(scene: &Scene, material: usize, builder: &TextBuilder) -> Result<()> {
    let material = scene
        .get_material(material)
        .ok_or_else(|| anyhow!("Material {material} not found"))?;
    if builder.method != TextMethod::NodePerGlyph {
        return Ok(());
    }
    let shader = scene
        .get_shader(material.get_shader())
        .ok_or_else(|| anyhow!("Shader {} not found", material.get_shader()))?;
    let shader_name = shader
        .get_name()
        .unwrap_or_else(|| shader.get_handle().to_string());
    if let Some(missing) = GLYPH_RECT_UNIFORMS
        .iter()
        .find(|u| shader.get_layout().find(u).is_none())
    {
        anyhow::bail!(
            "Shader {shader_name} can't draw text with a node per glyph: it has no `{missing}` uniform"
        );
    }
    if shader.get_uniform_umin_name().is_some()
        || shader.get_uniform_umax_name().is_some()
        || shader.get_uniform_vmin_name().is_some()
        || shader.get_uniform_vmax_name().is_some()
    {
        anyhow::bail!(
            "Shader {shader_name} can't draw text with a node per glyph: the engine fills its \
             texture rectangle uniforms from the material, over the glyph nodes' values"
        );
    }
    Ok(())
}

/// The square mesh of glyph nodes (`GLYPH_MESH_NAME`), built the first time it is needed.
fn glyph_mesh(scene: &mut Scene) -> Result<usize> {
    if let Some(mesh) = scene.find_mesh(GLYPH_MESH_NAME) {
        return Ok(mesh);
    }
    MeshBuilderPlane::new((0.0, 1.0), (-1.0, 0.0), (1, 1))
        .build(scene, Some(GLYPH_MESH_NAME.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader::tests::{FRAG, VERT};
    use crate::shader::ShaderUniformNames;

    /// A 64x32 font atlas: `A` (65) is 8x16 pixels at (0, 0), the space (32) 4x16 at (8, 0)
    /// and `i` (105) 2x16 at (12, 16).
    fn font(scene: &mut Scene) -> usize {
        let mut atlas = Atlas::from_rgba(Some("font".to_string()), 64, 32, vec![0; 64 * 32 * 4]);
        atlas
            .set_metadata("65 0 0 8 16\n32 8 0 4 16\n105 12 16 2 16")
            .unwrap();
        scene.add_atlas(atlas)
    }

    fn layout(builder: &TextBuilder, scene: &Scene, text: &str) -> (Vec<Quad>, TextSize) {
        builder.layout(scene.get_atlas(builder.get_atlas()).unwrap(), text)
    }

    #[test]
    fn characters_are_scaled_to_the_line_height_and_follow_each_other() {
        let mut scene = Scene::new();
        let builder = TextBuilder::new(font(&mut scene)).height(2.0);
        let (quads, size) = layout(&builder, &scene, "A i");

        // The space takes its place but is not drawn.
        assert_eq!(quads.len(), 2);
        assert_eq!(quads[0].x, (0.0, 1.0));
        assert_eq!(quads[0].y, (-2.0, 0.0));
        assert_eq!(quads[0].u, (0.0, 8.0 / 64.0));
        assert_eq!(quads[0].v, (0.0, 0.5));
        assert_eq!(quads[1].x, (1.5, 1.75));
        assert_eq!(quads[1].u, (12.0 / 64.0, 14.0 / 64.0));
        assert_eq!(quads[1].v, (0.5, 1.0));
        assert_eq!(
            size,
            TextSize {
                width: 1.75,
                height: 2.0
            }
        );
    }

    #[test]
    fn unknown_characters_take_a_quarter_of_the_height() {
        let mut scene = Scene::new();
        let builder = TextBuilder::new(font(&mut scene)).height(2.0);
        let (quads, size) = layout(&builder, &scene, "?A");
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].x, (0.5, 1.5));
        assert_eq!(size.width, 1.5);
    }

    #[test]
    fn lines_are_aligned() {
        let mut scene = Scene::new();
        let atlas = font(&mut scene);
        // Lines 1 unit high: "AA" is 1 wide, "A" 0.5.
        let lines = |h, v, spacing| {
            let builder = TextBuilder::new(atlas).align(h, v).line_spacing(spacing);
            let (quads, size) = layout(&builder, &scene, "AA\r\nA");
            (
                quads.iter().map(|q| (q.x.0, q.y.1)).collect::<Vec<_>>(),
                size,
            )
        };

        let (left_top, size) = lines(HorizontalAlign::Left, VerticalAlign::Top, 1.0);
        assert_eq!(left_top, [(0.0, 0.0), (0.5, 0.0), (0.0, -1.0)]);
        assert_eq!(
            size,
            TextSize {
                width: 1.0,
                height: 2.0
            }
        );

        let (centre, _) = lines(HorizontalAlign::Center, VerticalAlign::Center, 1.0);
        assert_eq!(centre, [(-0.5, 1.0), (0.0, 1.0), (-0.25, 0.0)]);

        let (right_bottom, size) = lines(HorizontalAlign::Right, VerticalAlign::Bottom, 1.5);
        assert_eq!(right_bottom, [(-1.0, 2.5), (-0.5, 2.5), (-0.5, 1.0)]);
        assert_eq!(size.height, 2.5);
    }

    #[test]
    fn meshes_have_a_quad_per_drawn_character_facing_the_reader() {
        let mut scene = Scene::new();
        let atlas = font(&mut scene);
        for (plane, normal) in [
            (TextPlane::XY, Vec3::new(0.0, 0.0, 1.0)),
            (TextPlane::XZ, Vec3::new(0.0, 1.0, 0.0)),
        ] {
            let builder = TextBuilder::new(atlas).plane(plane);
            let (mesh, _) = builder.build_mesh(&mut scene, "A A\nA", None).unwrap();
            let mesh = scene.get_mesh(mesh.unwrap()).unwrap();
            assert_eq!(mesh.vertices.len(), 12);
            assert_eq!(mesh.indices.len(), 18);
            for triangle in mesh.indices.chunks(3) {
                let [a, b, c] = [0, 1, 2].map(|k| mesh.vertices[triangle[k] as usize].pos);
                use cgmath::InnerSpace;
                assert!((b - a).cross(c - a).dot(normal) > 0.0, "{plane:?}");
            }
            // The second line is further down the page.
            let last = mesh.vertices[8].pos;
            match plane {
                TextPlane::XY => assert_eq!((last.y, last.z), (-1.0, 0.0)),
                TextPlane::XZ => assert_eq!((last.y, last.z), (0.0, 1.0)),
            }
        }
    }

    #[test]
    fn nothing_to_draw_builds_no_mesh() {
        let mut scene = Scene::new();
        let builder = TextBuilder::new(font(&mut scene));
        let (mesh, size) = builder.build_mesh(&mut scene, "  ", None).unwrap();
        assert_eq!(mesh, None);
        assert_eq!(size.width, 0.5);
        assert_eq!(scene.meshes.iter().count(), 0);
        assert!(TextBuilder::new(99)
            .build_mesh(&mut scene, "A", None)
            .is_err());
    }

    #[test]
    fn text_replaces_its_mesh() {
        let mut scene = Scene::new();
        let atlas = font(&mut scene);
        let root = scene.create_root_node(None);
        let shader = scene
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
            .unwrap();
        let material = create_text_material(&mut scene, None, shader, atlas).unwrap();
        assert_eq!(
            scene.get_material(material).unwrap().get_texture(),
            Some(&crate::texture::Texture::full(atlas))
        );

        let builder = TextBuilder::new(atlas);
        let mut text = Text::create(
            &mut scene,
            root,
            Some("Label".into()),
            material,
            builder,
            "A",
        )
        .unwrap();
        let node = text.get_node();
        assert!(scene.get_node(node).unwrap().is_transparent());
        let first = scene.get_node(node).unwrap().get_mesh().unwrap();
        assert_eq!(scene.get_mesh(first).unwrap().vertices.len(), 4);
        scene.build_missing_mesh_buffers().unwrap();

        text.set_text(&mut scene, "AiA").unwrap();
        assert_eq!(text.get_text(), "AiA");
        assert_eq!(text.get_size().width, 1.125);
        let second = scene.get_node(node).unwrap().get_mesh().unwrap();
        assert_eq!(scene.get_mesh(second).unwrap().vertices.len(), 12);
        assert_eq!(scene.meshes.iter().count(), 1);
        assert_eq!(scene.buffers.iter().count(), 0);

        text.set_text(&mut scene, "").unwrap();
        assert_eq!(scene.get_node(node).unwrap().get_mesh(), None);
        assert_eq!(scene.meshes.iter().count(), 0);

        text.set_builder(&mut scene, TextBuilder::new(atlas).height(2.0))
            .unwrap();
        text.set_text(&mut scene, "A").unwrap();
        assert_eq!(
            text.get_size(),
            TextSize {
                width: 1.0,
                height: 2.0
            }
        );
    }

    /// A scene with the test font and a text material drawing with a shader that has the glyph
    /// rectangle uniforms. Returns (scene, root, atlas, material).
    fn glyph_scene() -> (Scene, usize, usize, usize) {
        let mut scene = Scene::new();
        let atlas = font(&mut scene);
        let root = scene.create_root_node(None);
        let shader = scene
            .create_shader(
                Some("Text".into()),
                VERT,
                crate::shader::tests::TEXT_FRAG,
                ShaderUniformNames::default(),
            )
            .unwrap();
        let material = create_text_material(&mut scene, None, shader, atlas).unwrap();
        (scene, root, atlas, material)
    }

    fn uniform(scene: &Scene, node: usize, name: &str) -> f32 {
        match scene.get_node(node).unwrap().uniforms().get(name) {
            Some(crate::uniform::UniformValue::Float(v)) => *v,
            other => panic!("{name}: {other:?}"),
        }
    }

    #[test]
    fn text_materials_show_the_whole_atlas_to_rectangle_uniforms() {
        let (scene, _, _, material) = glyph_scene();
        let uniforms = scene.get_material(material).unwrap().uniforms();
        for (name, value) in GLYPH_RECT_UNIFORMS.into_iter().zip([0.0f32, 1.0, 0.0, 1.0]) {
            assert_eq!(
                uniforms.get(name),
                Some(&crate::uniform::UniformValue::Float(value))
            );
        }
    }

    #[test]
    fn glyph_nodes_place_the_characters() {
        let (mut scene, root, atlas, material) = glyph_scene();
        let builder = TextBuilder::new(atlas)
            .height(2.0)
            .method(TextMethod::NodePerGlyph);
        let text = Text::create(&mut scene, root, None, material, builder, "A i").unwrap();

        let node = scene.get_node(text.get_node()).unwrap();
        assert_eq!(node.get_mesh(), None);
        assert!(!node.is_transparent());
        assert_eq!(
            text.get_size(),
            TextSize {
                width: 1.75,
                height: 2.0
            }
        );

        // A node per drawn character (not the space), in order, sharing the glyph mesh.
        let glyphs = text.get_glyph_nodes();
        assert_eq!(glyphs.len(), 2);
        let mesh = scene.find_mesh(GLYPH_MESH_NAME).unwrap();
        for &glyph in glyphs {
            let node = scene.get_node(glyph).unwrap();
            assert_eq!(node.get_parent(), Some(text.get_node()));
            assert_eq!(node.get_mesh(), Some(mesh));
            assert_eq!(node.get_material(), Some(material));
            assert!(node.is_transparent());
        }

        // `i` (2x16 at (12, 16) in the 64x32 atlas) is 0.25 wide, from x = 1.5, top at 0.
        let i = glyphs[1];
        let rect = GLYPH_RECT_UNIFORMS.map(|u| uniform(&scene, i, u));
        assert_eq!(rect, [12.0 / 64.0, 14.0 / 64.0, 0.5, 1.0]);
        let transform = scene.get_node(i).unwrap().get_transform();
        let corner = |x, y| {
            let p = transform * crate::math::Vec4::new(x, y, 0.0, 1.0);
            (p.x, p.y, p.z)
        };
        assert_eq!(corner(0.0, 0.0), (1.5, 0.0, 0.0));
        assert_eq!(corner(1.0, -1.0), (1.75, -2.0, 0.0));
    }

    #[test]
    fn glyph_nodes_lie_in_the_xz_plane() {
        let (mut scene, root, atlas, material) = glyph_scene();
        let builder = TextBuilder::new(atlas)
            .method(TextMethod::NodePerGlyph)
            .plane(TextPlane::XZ);
        let text = Text::create(&mut scene, root, None, material, builder, "A\nA").unwrap();
        let second = scene.get_node(text.get_glyph_nodes()[1]).unwrap();
        // The second line's bottom right corner: further along +Z, on y = 0.
        let p = second.get_transform() * crate::math::Vec4::new(1.0, -1.0, 0.0, 1.0);
        assert!((p.x - 0.5).abs() < 1e-6 && p.y.abs() < 1e-6 && (p.z - 2.0).abs() < 1e-6);
        // Facing +Y.
        let normal = second.get_transform() * crate::math::Vec4::new(0.0, 0.0, 1.0, 0.0);
        assert!(normal.y > 0.99, "{normal:?}");
    }

    #[test]
    fn changing_glyph_text_reuses_nodes_and_builds_no_meshes() {
        let (mut scene, root, atlas, material) = glyph_scene();
        let builder = TextBuilder::new(atlas).method(TextMethod::NodePerGlyph);
        let mut text = Text::create(&mut scene, root, None, material, builder, "AAA").unwrap();
        let first: Vec<usize> = text.get_glyph_nodes().to_vec();
        let meshes = scene.meshes.iter().count();

        text.set_text(&mut scene, "iA").unwrap();
        assert_eq!(text.get_glyph_nodes(), &first[..2]);
        assert!(
            scene.get_node(first[2]).is_none(),
            "extra glyph node not removed"
        );
        assert_eq!(uniform(&scene, first[0], "umin"), 12.0 / 64.0);
        assert_eq!(scene.meshes.iter().count(), meshes);

        text.set_text(&mut scene, "AAAA").unwrap();
        assert_eq!(text.get_glyph_nodes().len(), 4);
        assert_eq!(&text.get_glyph_nodes()[..2], &first[..2]);
        assert_eq!(scene.meshes.iter().count(), meshes);
    }

    #[test]
    fn switching_methods_replaces_glyph_nodes_and_meshes() {
        let (mut scene, root, atlas, material) = glyph_scene();
        let builder = TextBuilder::new(atlas).method(TextMethod::NodePerGlyph);
        let mut text = Text::create(&mut scene, root, None, material, builder, "AA").unwrap();
        let glyphs = text.get_glyph_nodes().to_vec();

        text.set_builder(&mut scene, TextBuilder::new(atlas))
            .unwrap();
        assert!(text.get_glyph_nodes().is_empty());
        assert!(glyphs.iter().all(|&g| scene.get_node(g).is_none()));
        let node = scene.get_node(text.get_node()).unwrap();
        assert!(node.get_mesh().is_some() && node.is_transparent());

        let builder = TextBuilder::new(atlas).method(TextMethod::NodePerGlyph);
        text.set_builder(&mut scene, builder).unwrap();
        assert_eq!(text.get_glyph_nodes().len(), 2);
        assert_eq!(scene.get_node(text.get_node()).unwrap().get_mesh(), None);
        // Only the glyph quad is left.
        assert_eq!(scene.meshes.iter().count(), 1);
    }

    #[test]
    fn glyph_nodes_need_a_shader_taking_the_rectangle_from_nodes() {
        let mut scene = Scene::new();
        let atlas = font(&mut scene);
        let root = scene.create_root_node(None);
        let builder = TextBuilder::new(atlas).method(TextMethod::NodePerGlyph);

        // No rectangle uniforms.
        let plain = scene
            .create_shader(None, VERT, FRAG, ShaderUniformNames::default())
            .unwrap();
        let material = create_text_material(&mut scene, None, plain, atlas).unwrap();
        let err = Text::create(&mut scene, root, None, material, builder.clone(), "A");
        assert!(format!("{:#}", err.err().unwrap()).contains("no `umin` uniform"));

        // Rectangle uniforms filled by the engine from the material's texture.
        let names = ShaderUniformNames::default().with_texture_rect("umin", "umax", "vmin", "vmax");
        let filled = scene
            .create_shader(None, VERT, crate::shader::tests::TEXT_FRAG, names)
            .unwrap();
        let material = create_text_material(&mut scene, None, filled, atlas).unwrap();
        let err = Text::create(&mut scene, root, None, material, builder.clone(), "A");
        assert!(format!("{:#}", err.err().unwrap()).contains("engine fills"));

        // One mesh works with either.
        Text::create(
            &mut scene,
            root,
            None,
            material,
            TextBuilder::new(atlas),
            "A",
        )
        .unwrap();
    }

    #[test]
    fn wrapping_breaks_lines_between_words() {
        let mut scene = Scene::new();
        // Lines 1 high: `A` is 0.5 wide, the space 0.25, `i` 0.125.
        let builder = TextBuilder::new(font(&mut scene)).wrap(Some(1.6));
        let lines = |text: &str| {
            let (quads, size) = layout(&builder, &scene, text);
            let mut tops: Vec<f32> = quads.iter().map(|q| q.y.1).collect();
            tops.dedup();
            (tops.len(), size)
        };
        // "AA AA" is 2.25 wide: it doesn't fit, so it takes two lines.
        let (count, size) = lines("AA AA");
        assert_eq!(count, 2);
        assert_eq!(
            size,
            TextSize {
                width: 1.0,
                height: 2.0
            }
        );
        // "AA i" is 1.375: one line.
        assert_eq!(lines("AA i").0, 1);
        // A new paragraph still starts a new line, and a word too long has a line of its own.
        assert_eq!(lines("A\nAAAA i").0, 3);
    }

    #[test]
    fn justified_lines_fill_the_wrap_width_except_the_last() {
        let mut scene = Scene::new();
        let atlas = font(&mut scene);
        let builder = TextBuilder::new(atlas)
            .wrap(Some(2.0))
            .align(HorizontalAlign::Justify, VerticalAlign::Top);
        // "A A A A A": 5 x 0.5 + 4 x 0.25 = 3.5 > 2; "A A A" (2.0) fits on the first line.
        let (quads, size) = layout(&builder, &scene, "A A A A A");
        assert_eq!(size.width, 2.0);
        let first: Vec<_> = quads.iter().filter(|q| q.y.1 == 0.0).collect();
        let second: Vec<_> = quads.iter().filter(|q| q.y.1 == -1.0).collect();
        assert_eq!((first.len(), second.len()), (3, 2));
        // The first line is already full width; the last line is not stretched.
        assert_eq!(first[2].x.1, 2.0);
        assert_eq!(
            second.iter().map(|q| q.x.0).collect::<Vec<_>>(),
            [0.0, 0.75]
        );

        // A line with room left over is stretched to the full width.
        let builder = builder.wrap(Some(2.5));
        let (quads, _) = layout(&builder, &scene, "A A A A A");
        let first: Vec<_> = quads.iter().filter(|q| q.y.1 == 0.0).collect();
        assert_eq!(first.len(), 3);
        assert!((first[2].x.1 - 2.5).abs() < 1e-6, "{:?}", first[2].x);

        // Without wrapping, justified is left aligned.
        let builder = TextBuilder::new(atlas).align(HorizontalAlign::Justify, VerticalAlign::Top);
        let (quads, size) = layout(&builder, &scene, "A A");
        assert_eq!((quads[0].x.0, size.width), (0.0, 1.25));
    }

    #[test]
    fn removing_text_removes_its_node_and_mesh() {
        let (mut scene, root, atlas, material) = glyph_scene();
        let one_mesh = Text::create(
            &mut scene,
            root,
            None,
            material,
            TextBuilder::new(atlas),
            "A",
        )
        .unwrap();
        let builder = TextBuilder::new(atlas).method(TextMethod::NodePerGlyph);
        let glyphs = Text::create(&mut scene, root, None, material, builder, "AA").unwrap();
        let glyph_nodes = glyphs.get_glyph_nodes().to_vec();
        let (node, mesh) = (
            one_mesh.get_node(),
            scene
                .get_node(one_mesh.get_node())
                .unwrap()
                .get_mesh()
                .unwrap(),
        );

        one_mesh.remove(&mut scene).unwrap();
        assert!(scene.get_node(node).is_none());
        assert!(scene.get_mesh(mesh).is_none());

        let node = glyphs.get_node();
        glyphs.remove(&mut scene).unwrap();
        assert!(scene.get_node(node).is_none());
        assert!(glyph_nodes.iter().all(|&g| scene.get_node(g).is_none()));
        assert!(scene.find_mesh(GLYPH_MESH_NAME).is_some());
        assert_eq!(scene.get_node(root).unwrap().get_children().len(), 0);
    }

    /// Not a test: times the per-node uniform work for 2,000 glyph nodes (40 texts of 50
    /// characters with `NodePerGlyph`), without a GPU. Run it in release:
    /// `cargo test --release -p feather -- --ignored uniform_benchmark --nocapture`.
    #[test]
    #[ignore]
    fn uniform_benchmark() {
        use crate::math::Mat4;
        use crate::uniformdata::{engine_uniforms, pack_draw, UniformArena, UniformValues};
        use std::time::Instant;

        const LINES: usize = 40;
        const ROUNDS: u32 = 50;
        let mut scene = Scene::new();
        let atlas = font(&mut scene);
        let root = scene.create_root_node(None);
        scene
            .get_node_mut(root)
            .unwrap()
            .uniforms_mut()
            .set("color", crate::math::Vec4::new(1.0, 1.0, 1.0, 1.0));
        let shader = scene
            .create_shader(
                Some("Text".into()),
                VERT,
                crate::shader::tests::TEXT_FRAG,
                ShaderUniformNames::matrices("model", "view", "proj"),
            )
            .unwrap();
        let material = create_text_material(&mut scene, None, shader, atlas).unwrap();
        let builder = TextBuilder::new(atlas).method(TextMethod::NodePerGlyph);
        // 50 characters (spaces have no node), each different from the other round's.
        let line = |round: u32| if round % 2 == 0 { "Ai" } else { "iA" }.repeat(25);
        let mut texts: Vec<Text> = (0..LINES)
            .map(|_| {
                Text::create(&mut scene, root, None, material, builder.clone(), &line(0)).unwrap()
            })
            .collect();
        scene.build_missing_mesh_buffers().unwrap();
        scene.update_nodes();
        assert_eq!(scene.draw_items(None).len(), LINES * 50);

        let ms = |start: Instant| start.elapsed().as_secs_f64() * 1000.0 / ROUNDS as f64;

        // Setting the glyphs' rectangle uniforms (and transforms), every character changing.
        let start = Instant::now();
        for round in 1..=ROUNDS {
            let text = line(round);
            for t in &mut texts {
                t.set_text(&mut scene, &text).unwrap();
            }
        }
        let set_text = ms(start);

        // Uniform inheritance and global transforms.
        let start = Instant::now();
        for _ in 0..ROUNDS {
            scene.update_nodes();
        }
        let update_nodes = ms(start);

        // Packing every draw's uniforms, as `collect_draws` does.
        let shader = scene.get_shader(shader).unwrap();
        let items = scene.draw_items(None);
        let identity = Mat4::from_scale(1.0);
        let mut issues = Vec::new();
        let start = Instant::now();
        for _ in 0..ROUNDS {
            let mut arena = UniformArena::new(256);
            for item in &items {
                let node = scene.get_node(item.node).unwrap();
                let texture = scene.get_material(item.material).unwrap().get_texture();
                let engine = engine_uniforms(
                    shader.get_uniform_ids(),
                    item.model,
                    identity,
                    identity,
                    identity,
                    texture,
                );
                let values = UniformValues { engine: &engine, node: node.dynamic_uniforms() };
                pack_draw(shader.get_layout(), &values, &mut arena, &mut issues);
                issues.clear();
            }
        }
        let pack = ms(start);

        println!(
            "{} glyph nodes, ms per frame: set_text {set_text:.3}, update_nodes {update_nodes:.3}, \
             pack {pack:.3}",
            items.len()
        );
    }
}
