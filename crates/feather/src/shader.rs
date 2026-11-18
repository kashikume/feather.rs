use anyhow::{bail, Context, Result};

use super::material::MAX_MATERIAL_TEXTURES;
use super::object::Object;
use super::shaderlayout::{ShaderLayout, UniformLocation, UniformType};
use super::uniformname::UniformName;

/// SPIR-V magic number (first word of every SPIR-V module).
const SPIRV_MAGIC: u32 = 0x0723_0203;

/// Names of the four `float` uniforms that receive the edges of a texture's rectangle in its
/// atlas, in UV space (`Texture::get_u_min` ...). A shader maps its texture coordinates into
/// the rectangle with `mix(vec2(umin, vmin), vec2(umax, vmax), uv)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureRectNames {
    pub umin: String,
    pub umax: String,
    pub vmin: String,
    pub vmax: String,
}

impl TextureRectNames {
    pub fn new(umin: &str, umax: &str, vmin: &str, vmax: &str) -> Self {
        Self {
            umin: umin.to_string(),
            umax: umax.to_string(),
            vmin: vmin.to_string(),
            vmax: vmax.to_string(),
        }
    }

    fn names(&self) -> [&str; 4] {
        [&self.umin, &self.umax, &self.vmin, &self.vmax]
    }
}

/// A sampler that samples a texture slot of the material, and optionally the uniforms that
/// receive that texture's rectangle (without them the shader samples the whole atlas).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureSamplerNames {
    pub sampler: String,
    pub rect: Option<TextureRectNames>,
}

/// Names of the uniforms and samplers the engine fills for every draw, as the shader calls
/// them. Leave a name `None` when the shader doesn't use that value.
///
/// - `model`, `view`, `projection`: the node's model matrix and the camera's view and
///   projection matrices; each must be a `mat4` in the shader.
/// - `texture_samplers`: the samplers that sample the material's textures, at most
///   `MAX_MATERIAL_TEXTURES`: entry `i` samples the atlas of the texture in slot `i`, and its
///   `rect` uniforms (each a `float`) get that texture's rectangle. **Every sampler of the
///   shader** except `shadow_map` must be listed.
/// - `shadow_matrix`: the view-projection matrix of the scene's shadow light (identity without
///   one), a `mat4`; `shadow_map`: the sampler that samples the scene's shadow map. See
///   `shadow`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShaderUniformNames {
    pub model: Option<String>,
    pub view: Option<String>,
    pub projection: Option<String>,
    pub texture_samplers: Vec<TextureSamplerNames>,
    pub shadow_matrix: Option<String>,
    pub shadow_map: Option<String>,
}

impl ShaderUniformNames {
    /// Only the three matrices.
    pub fn matrices(model: &str, view: &str, projection: &str) -> Self {
        Self {
            model: Some(model.to_string()),
            view: Some(view.to_string()),
            projection: Some(projection.to_string()),
            ..Self::default()
        }
    }

    /// Adds a texture sampler: it samples the next texture slot of the material (the first
    /// call slot 0, the next slot 1, ...), and `rect`, when given, names the uniforms that
    /// receive that texture's rectangle.
    pub fn with_texture_sampler(mut self, sampler: &str, rect: Option<TextureRectNames>) -> Self {
        self.texture_samplers.push(TextureSamplerNames {
            sampler: sampler.to_string(),
            rect,
        });
        self
    }

    /// Adds the shadow light's matrix uniform and the shadow map's sampler (see `shadow`).
    pub fn with_shadow(mut self, matrix: &str, map: &str) -> Self {
        self.shadow_matrix = Some(matrix.to_string());
        self.shadow_map = Some(map.to_string());
        self
    }

    /// Fails if a name doesn't exist in `layout` or has the wrong type, if there are too many
    /// texture samplers, or if a sampler of the shader is neither a texture sampler nor the
    /// shadow map.
    fn check(&self, layout: &ShaderLayout) -> Result<()> {
        let matrices = [
            &self.model,
            &self.view,
            &self.projection,
            &self.shadow_matrix,
        ];
        for uniform in matrices.into_iter().flatten() {
            check_uniform_type(layout, uniform, UniformType::Mat4)?;
        }
        let has_sampler = |name: &str| layout.samplers.iter().any(|s| s.name == name);
        if let Some(map) = &self.shadow_map {
            if !has_sampler(map) {
                bail!("the shader has no sampler `{map}` for the shadow map");
            }
        }
        if self.texture_samplers.len() > MAX_MATERIAL_TEXTURES {
            bail!(
                "{} texture samplers, at most {MAX_MATERIAL_TEXTURES} (one per material texture slot)",
                self.texture_samplers.len()
            );
        }
        for (slot, entry) in self.texture_samplers.iter().enumerate() {
            let sampler = entry.sampler.as_str();
            if !has_sampler(sampler) {
                bail!("the shader has no sampler `{sampler}` (texture sampler {slot})");
            }
            if self.shadow_map.as_deref() == Some(sampler) {
                bail!("sampler `{sampler}` is both a texture sampler and the shadow map sampler");
            }
            if self.texture_samplers[..slot]
                .iter()
                .any(|e| e.sampler == sampler)
            {
                bail!("sampler `{sampler}` is listed twice in the texture samplers");
            }
            for uniform in entry.rect.iter().flat_map(|r| r.names()) {
                check_uniform_type(layout, uniform, UniformType::Float)?;
            }
        }
        for sampler in &layout.samplers {
            let name = sampler.name.as_str();
            let listed = self.shadow_map.as_deref() == Some(name)
                || self.texture_samplers.iter().any(|e| e.sampler == name);
            if !listed {
                bail!(
                    "sampler `{name}` is not listed in the texture samplers (nor is it the shadow map sampler)"
                );
            }
        }
        Ok(())
    }

    /// The names as ids, for the per-draw work.
    pub fn ids(&self) -> ShaderUniformIds {
        let id = |name: &Option<String>| name.as_deref().map(UniformName::new);
        let mut ids = ShaderUniformIds {
            model: id(&self.model),
            view: id(&self.view),
            projection: id(&self.projection),
            shadow_matrix: id(&self.shadow_matrix),
            shadow_map: id(&self.shadow_map),
            ..ShaderUniformIds::default()
        };
        for (slot, entry) in self
            .texture_samplers
            .iter()
            .take(MAX_MATERIAL_TEXTURES)
            .enumerate()
        {
            ids.texture_samplers[slot] = Some(UniformName::new(&entry.sampler));
            ids.texture_rects[slot] = entry.rect.as_ref().map(|r| TextureRectIds {
                umin: UniformName::new(&r.umin),
                umax: UniformName::new(&r.umax),
                vmin: UniformName::new(&r.vmin),
                vmax: UniformName::new(&r.vmax),
            });
        }
        ids
    }
}

/// `TextureRectNames` as interned ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureRectIds {
    pub umin: UniformName,
    pub umax: UniformName,
    pub vmin: UniformName,
    pub vmax: UniformName,
}

/// `ShaderUniformNames` as interned ids (`ShaderUniformNames::ids`), so packing a draw doesn't
/// compare strings. Texture samplers and their rectangles are indexed by material texture slot
/// (`None` past the shader's last texture sampler).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShaderUniformIds {
    pub model: Option<UniformName>,
    pub view: Option<UniformName>,
    pub projection: Option<UniformName>,
    pub texture_samplers: [Option<UniformName>; MAX_MATERIAL_TEXTURES],
    pub texture_rects: [Option<TextureRectIds>; MAX_MATERIAL_TEXTURES],
    pub shadow_matrix: Option<UniformName>,
    pub shadow_map: Option<UniformName>,
}

/// A shader program: compiled SPIR-V for the vertex and fragment stages.
pub struct Shader {
    name: Option<String>,
    handle: usize,
    vertex: Vec<u8>,
    fragment: Vec<u8>,
    /// Resources the shader uses, reflected from the SPIR-V.
    layout: ShaderLayout,
    uniform_names: ShaderUniformNames,
    /// `uniform_names` as ids.
    uniform_ids: ShaderUniformIds,
}

impl Object for Shader {
    fn get_name(&self) -> Option<String> {
        self.name.clone()
    }

    fn set_handle(&mut self, handle: usize) {
        self.handle = handle;
    }

    fn get_handle(&self) -> usize {
        self.handle
    }
}

impl Shader {
    /// Creates a shader from SPIR-V bytecode, e.g. from
    /// `include_bytes!(concat!(env!("OUT_DIR"), "/shaders/demo_000_viking_room_shader.vert.spv"))`.
    /// `uniform_names` names the uniforms the engine fills (see `ShaderUniformNames`); each
    /// named uniform must exist in the shader and have the right type.
    ///
    /// Fails if either stage is not valid SPIR-V, if the shader uses resources the engine
    /// doesn't support (see `ShaderLayout`), or if a uniform name is wrong.
    pub fn new(
        name: Option<String>,
        vertex: &[u8],
        fragment: &[u8],
        uniform_names: ShaderUniformNames,
    ) -> Result<Self> {
        check_spirv("vertex", vertex)?;
        check_spirv("fragment", fragment)?;
        let layout = ShaderLayout::reflect(vertex, fragment).with_context(|| {
            format!(
                "Cannot read the layout of shader {}",
                name.as_deref().unwrap_or("<unnamed>")
            )
        })?;
        uniform_names.check(&layout)?;
        Ok(Self {
            name,
            handle: usize::MAX,
            vertex: vertex.to_vec(),
            fragment: fragment.to_vec(),
            layout,
            uniform_ids: uniform_names.ids(),
            uniform_names,
        })
    }

    pub fn get_vertex(&self) -> &[u8] {
        &self.vertex
    }

    pub fn get_fragment(&self) -> &[u8] {
        &self.fragment
    }

    /// Uniform blocks, push constants and samplers the shader uses.
    pub fn get_layout(&self) -> &ShaderLayout {
        &self.layout
    }

    /// Names of the uniforms the engine fills.
    pub fn get_uniform_names(&self) -> &ShaderUniformNames {
        &self.uniform_names
    }

    /// Ids of the uniforms the engine fills.
    pub fn get_uniform_ids(&self) -> &ShaderUniformIds {
        &self.uniform_ids
    }

    /// Changes the names of the uniforms the engine fills. Fails, keeping the old names, if a
    /// name doesn't exist in the shader or has the wrong type.
    pub fn set_uniform_names(&mut self, uniform_names: ShaderUniformNames) -> Result<()> {
        uniform_names.check(&self.layout)?;
        self.uniform_ids = uniform_names.ids();
        self.uniform_names = uniform_names;
        Ok(())
    }

    /// Name of the uniform that receives the node's model matrix, if the shader uses one.
    pub fn get_uniform_model_name(&self) -> Option<&str> {
        self.uniform_names.model.as_deref()
    }

    /// Name of the uniform that receives the camera's view matrix, if the shader uses one.
    pub fn get_uniform_view_name(&self) -> Option<&str> {
        self.uniform_names.view.as_deref()
    }

    /// Name of the uniform that receives the camera's projection matrix, if the shader uses one.
    pub fn get_uniform_proj_name(&self) -> Option<&str> {
        self.uniform_names.projection.as_deref()
    }

    /// The samplers that sample the material's textures, in slot order, with their rectangle
    /// uniforms.
    pub fn get_texture_samplers(&self) -> &[TextureSamplerNames] {
        &self.uniform_names.texture_samplers
    }

    /// Name of the uniform that receives the shadow light's view-projection matrix.
    pub fn get_uniform_shadow_matrix_name(&self) -> Option<&str> {
        self.uniform_names.shadow_matrix.as_deref()
    }

    /// Name of the sampler that samples the shadow map.
    pub fn get_shadow_map_sampler_name(&self) -> Option<&str> {
        self.uniform_names.shadow_map.as_deref()
    }
}

fn check_uniform_type(layout: &ShaderLayout, name: &str, expected: UniformType) -> Result<()> {
    let ty = match layout.find(name) {
        Some(UniformLocation::Block { member, .. }) => member.ty,
        Some(UniformLocation::PushConstant(member)) => member.ty,
        Some(UniformLocation::Sampler(_)) => {
            bail!("uniform `{name}` is a sampler, not a {expected:?}")
        }
        None => bail!("the shader has no uniform `{name}`"),
    };
    if ty != expected {
        bail!("uniform `{name}` is {ty:?}, not a {expected:?}");
    }
    Ok(())
}

fn check_spirv(stage: &str, code: &[u8]) -> Result<()> {
    if code.len() < 4 || code.len() % 4 != 0 {
        bail!(
            "{stage} shader is not SPIR-V: size {} is not a non-zero multiple of 4",
            code.len()
        );
    }
    let magic = u32::from_le_bytes([code[0], code[1], code[2], code[3]]);
    if magic != SPIRV_MAGIC {
        bail!("{stage} shader is not SPIR-V: bad magic number {magic:#010x}");
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const VERT: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/basic.vert.spv"));
    pub(crate) const FRAG: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/basic.frag.spv"));
    pub(crate) const TEXT_FRAG: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/text.frag.spv"));
    pub(crate) const TWO_TEXTURES_FRAG: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/two_textures.frag.spv"));
    const TYPES_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/types.vert.spv"));
    const TYPES_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/types.frag.spv"));

    /// Names for `VERT` + `FRAG`: no matrices, its one sampler `texSampler` samples slot 0.
    pub(crate) fn basic_names() -> ShaderUniformNames {
        ShaderUniformNames::default().with_texture_sampler("texSampler", None)
    }

    /// Names for `TYPES_VERT` + `TYPES_FRAG`: its one sampler `albedo` samples slot 0.
    fn types_names() -> ShaderUniformNames {
        ShaderUniformNames::default().with_texture_sampler("albedo", None)
    }

    fn two_textures_names() -> ShaderUniformNames {
        ShaderUniformNames::default()
            .with_texture_sampler(
                "colorMap",
                Some(TextureRectNames::new(
                    "colorUmin",
                    "colorUmax",
                    "colorVmin",
                    "colorVmax",
                )),
            )
            .with_texture_sampler(
                "normalMap",
                Some(TextureRectNames::new(
                    "normalUmin",
                    "normalUmax",
                    "normalVmin",
                    "normalVmax",
                )),
            )
    }

    fn new(vertex: &[u8], fragment: &[u8], names: ShaderUniformNames) -> Result<Shader> {
        Shader::new(None, vertex, fragment, names)
    }

    fn matrices(
        model: Option<&str>,
        view: Option<&str>,
        projection: Option<&str>,
    ) -> ShaderUniformNames {
        ShaderUniformNames {
            model: model.map(str::to_string),
            view: view.map(str::to_string),
            projection: projection.map(str::to_string),
            ..basic_names()
        }
    }

    #[test]
    fn new_keeps_code_and_name() {
        let shader = Shader::new(Some("basic".to_string()), VERT, FRAG, basic_names()).unwrap();
        assert_eq!(shader.get_name(), Some("basic".to_string()));
        assert_eq!(shader.get_handle(), usize::MAX);
        assert_eq!(shader.get_vertex(), VERT);
        assert_eq!(shader.get_fragment(), FRAG);
    }

    #[test]
    fn set_handle() {
        let mut shader = new(VERT, FRAG, basic_names()).unwrap();
        shader.set_handle(3);
        assert_eq!(shader.get_handle(), 3);
    }

    #[test]
    fn uniform_names_default_to_none() {
        let shader = new(VERT, FRAG, basic_names()).unwrap();
        assert_eq!(shader.get_uniform_names(), &basic_names());
        assert_eq!(shader.get_uniform_model_name(), None);
        assert_eq!(shader.get_uniform_view_name(), None);
        assert_eq!(shader.get_uniform_proj_name(), None);
        assert_eq!(
            shader.get_texture_samplers(),
            [TextureSamplerNames {
                sampler: "texSampler".to_string(),
                rect: None
            }]
        );
    }

    #[test]
    fn new_keeps_matrix_uniform_names() {
        let shader = new(VERT, FRAG, matrices(Some("model"), None, Some("proj"))).unwrap();
        assert_eq!(shader.get_uniform_model_name(), Some("model"));
        assert_eq!(shader.get_uniform_view_name(), None);
        assert_eq!(shader.get_uniform_proj_name(), Some("proj"));
    }

    #[test]
    fn set_uniform_names() {
        let mut shader = new(VERT, FRAG, basic_names()).unwrap();
        shader
            .set_uniform_names(
                ShaderUniformNames::matrices("model", "view", "proj")
                    .with_texture_sampler("texSampler", None),
            )
            .unwrap();
        assert_eq!(shader.get_uniform_model_name(), Some("model"));
        assert_eq!(shader.get_uniform_view_name(), Some("view"));
        assert_eq!(shader.get_uniform_proj_name(), Some("proj"));

        // A wrong name is rejected and the old names stay.
        assert!(shader
            .set_uniform_names(matrices(None, Some("viewx"), None))
            .is_err());
        assert_eq!(shader.get_uniform_view_name(), Some("view"));

        shader
            .set_uniform_names(matrices(None, None, None))
            .unwrap();
        assert_eq!(shader.get_uniform_view_name(), None);
    }

    #[test]
    fn texture_rect_uniform_names_must_be_floats() {
        // `time` is a float in types.vert/types.frag, `tint` a vec4.
        let rect = |vmin: &str| Some(TextureRectNames::new("time", "time", vmin, "time"));
        let names = ShaderUniformNames::default().with_texture_sampler("albedo", rect("time"));
        let shader = new(TYPES_VERT, TYPES_FRAG, names).unwrap();
        assert_eq!(
            shader.get_texture_samplers()[0].rect,
            Some(TextureRectNames::new("time", "time", "time", "time"))
        );

        let wrong = ShaderUniformNames::default().with_texture_sampler("albedo", rect("tint"));
        let err = new(TYPES_VERT, TYPES_FRAG, wrong).err().unwrap();
        assert!(err.to_string().contains("tint"), "{err}");

        let missing = ShaderUniformNames::default().with_texture_sampler("albedo", rect("v_min"));
        assert!(new(TYPES_VERT, TYPES_FRAG, missing).is_err());
        assert!(new(TYPES_VERT, TYPES_FRAG, types_names()).is_ok());
    }

    #[test]
    fn two_texture_samplers_with_their_rectangles() {
        let shader = new(VERT, TWO_TEXTURES_FRAG, two_textures_names()).unwrap();
        let samplers = shader.get_texture_samplers();
        assert_eq!(samplers.len(), 2);
        assert_eq!(samplers[0].sampler, "colorMap");
        assert_eq!(samplers[1].sampler, "normalMap");

        let ids = shader.get_uniform_ids();
        assert_eq!(ids.texture_samplers[0], Some(UniformName::new("colorMap")));
        assert_eq!(ids.texture_samplers[1], Some(UniformName::new("normalMap")));
        assert_eq!(ids.texture_samplers[2], None);
        assert_eq!(
            ids.texture_rects[1].map(|r| r.vmax),
            Some(UniformName::new("normalVmax"))
        );
        assert_eq!(ids.texture_rects[2], None);
    }

    #[test]
    fn every_sampler_must_be_a_texture_sampler_or_the_shadow_map() {
        // Not listed at all.
        let err = new(VERT, FRAG, ShaderUniformNames::default())
            .err()
            .unwrap();
        assert!(err.to_string().contains("texSampler"), "{err}");

        // One of two listed.
        let one = ShaderUniformNames::default().with_texture_sampler("colorMap", None);
        let err = new(VERT, TWO_TEXTURES_FRAG, one).err().unwrap();
        assert!(err.to_string().contains("normalMap"), "{err}");

        // The shadow map sampler needs no texture sampler entry.
        let shadow = ShaderUniformNames {
            shadow_map: Some("texSampler".to_string()),
            ..ShaderUniformNames::default()
        };
        assert!(new(VERT, FRAG, shadow).is_ok());
    }

    #[test]
    fn wrong_texture_samplers_fail() {
        let unknown = basic_names().with_texture_sampler("nope", None);
        let err = new(VERT, FRAG, unknown).err().unwrap();
        assert!(err.to_string().contains("nope"), "{err}");

        let twice = basic_names().with_texture_sampler("texSampler", None);
        let err = new(VERT, FRAG, twice).err().unwrap();
        assert!(err.to_string().contains("twice"), "{err}");

        let also_shadow = ShaderUniformNames {
            shadow_map: Some("texSampler".to_string()),
            ..basic_names()
        };
        let err = new(VERT, FRAG, also_shadow).err().unwrap();
        assert!(err.to_string().contains("shadow map"), "{err}");

        let mut too_many = two_textures_names();
        for _ in 0..MAX_MATERIAL_TEXTURES - 1 {
            too_many = too_many.with_texture_sampler("colorMap", None);
        }
        let err = new(VERT, TWO_TEXTURES_FRAG, too_many).err().unwrap();
        assert!(err.to_string().contains("at most"), "{err}");
    }

    #[test]
    fn new_reflects_layout() {
        let shader = new(VERT, FRAG, basic_names()).unwrap();
        assert_eq!(
            shader.get_layout(),
            &ShaderLayout::reflect(VERT, FRAG).unwrap()
        );
        assert!(shader.get_layout().find("texSampler").is_some());
    }

    #[test]
    fn unknown_matrix_uniform_name_fails() {
        let err = new(VERT, FRAG, matrices(Some("mdl"), None, None))
            .err()
            .unwrap();
        assert!(err.to_string().contains("mdl"), "{err}");
    }

    #[test]
    fn matrix_uniform_name_must_be_mat4() {
        assert!(new(VERT, FRAG, matrices(None, Some("texSampler"), None)).is_err());
    }

    #[test]
    fn empty_code_fails() {
        assert!(new(&[], FRAG, ShaderUniformNames::default()).is_err());
        assert!(new(VERT, &[], ShaderUniformNames::default()).is_err());
    }

    #[test]
    fn size_not_multiple_of_four_fails() {
        let mut code = VERT.to_vec();
        code.push(0);
        assert!(new(&code, FRAG, ShaderUniformNames::default()).is_err());
    }

    #[test]
    fn bad_magic_fails() {
        assert!(new(
            &[1, 2, 3, 4, 5, 6, 7, 8],
            FRAG,
            ShaderUniformNames::default()
        )
        .is_err());
    }
}
