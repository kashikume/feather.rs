use anyhow::{bail, Context, Result};

use super::object::Object;
use super::shaderlayout::{ShaderLayout, UniformLocation, UniformType};

/// SPIR-V magic number (first word of every SPIR-V module).
const SPIRV_MAGIC: u32 = 0x0723_0203;

/// Names of the uniforms the engine fills for every draw, as the shader calls them. Leave a
/// name `None` when the shader doesn't use that value.
///
/// - `model`, `view`, `projection`: the node's model matrix and the camera's view and
///   projection matrices; each must be a `mat4` in the shader.
/// - `umin`, `umax`, `vmin`, `vmax`: the edges of the rectangle of the material's texture in
///   its atlas, in UV space; each must be a `float` in the shader.
/// - `shadow_matrix`: the view-projection matrix of the scene's shadow light (identity without
///   one), a `mat4`; `shadow_map`: the sampler that samples the scene's shadow map instead of
///   the material's texture. See `shadow`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShaderUniformNames {
    pub model: Option<String>,
    pub view: Option<String>,
    pub projection: Option<String>,
    pub umin: Option<String>,
    pub umax: Option<String>,
    pub vmin: Option<String>,
    pub vmax: Option<String>,
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

    /// Adds the four texture rectangle uniforms.
    pub fn with_texture_rect(mut self, umin: &str, umax: &str, vmin: &str, vmax: &str) -> Self {
        self.umin = Some(umin.to_string());
        self.umax = Some(umax.to_string());
        self.vmin = Some(vmin.to_string());
        self.vmax = Some(vmax.to_string());
        self
    }

    /// Adds the shadow light's matrix uniform and the shadow map's sampler (see `shadow`).
    pub fn with_shadow(mut self, matrix: &str, map: &str) -> Self {
        self.shadow_matrix = Some(matrix.to_string());
        self.shadow_map = Some(map.to_string());
        self
    }

    /// Each name with the type it must have in the shader.
    fn typed(&self) -> [(&Option<String>, UniformType); 8] {
        [
            (&self.model, UniformType::Mat4),
            (&self.view, UniformType::Mat4),
            (&self.projection, UniformType::Mat4),
            (&self.umin, UniformType::Float),
            (&self.umax, UniformType::Float),
            (&self.vmin, UniformType::Float),
            (&self.vmax, UniformType::Float),
            (&self.shadow_matrix, UniformType::Mat4),
        ]
    }

    /// Fails if a name doesn't exist in `layout` or has the wrong type.
    fn check(&self, layout: &ShaderLayout) -> Result<()> {
        for (uniform, ty) in self.typed() {
            if let Some(uniform) = uniform {
                check_uniform_type(layout, uniform, ty)?;
            }
        }
        if let Some(map) = &self.shadow_map {
            if !layout.samplers.iter().any(|s| &s.name == map) {
                bail!("the shader has no sampler `{map}` for the shadow map");
            }
        }
        Ok(())
    }
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

    /// Changes the names of the uniforms the engine fills. Fails, keeping the old names, if a
    /// name doesn't exist in the shader or has the wrong type.
    pub fn set_uniform_names(&mut self, uniform_names: ShaderUniformNames) -> Result<()> {
        uniform_names.check(&self.layout)?;
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

    /// Name of the uniform that receives the left edge (`u` min) of the texture's rectangle.
    pub fn get_uniform_umin_name(&self) -> Option<&str> {
        self.uniform_names.umin.as_deref()
    }

    /// Name of the uniform that receives the right edge (`u` max) of the texture's rectangle.
    pub fn get_uniform_umax_name(&self) -> Option<&str> {
        self.uniform_names.umax.as_deref()
    }

    /// Name of the uniform that receives the top edge (`v` min) of the texture's rectangle.
    pub fn get_uniform_vmin_name(&self) -> Option<&str> {
        self.uniform_names.vmin.as_deref()
    }

    /// Name of the uniform that receives the bottom edge (`v` max) of the texture's rectangle.
    pub fn get_uniform_vmax_name(&self) -> Option<&str> {
        self.uniform_names.vmax.as_deref()
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
    const TYPES_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/types.vert.spv"));
    const TYPES_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/types.frag.spv"));

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
            ..ShaderUniformNames::default()
        }
    }

    #[test]
    fn new_keeps_code_and_name() {
        let names = ShaderUniformNames::default();
        let shader = Shader::new(Some("basic".to_string()), VERT, FRAG, names).unwrap();
        assert_eq!(shader.get_name(), Some("basic".to_string()));
        assert_eq!(shader.get_handle(), usize::MAX);
        assert_eq!(shader.get_vertex(), VERT);
        assert_eq!(shader.get_fragment(), FRAG);
    }

    #[test]
    fn set_handle() {
        let mut shader = new(VERT, FRAG, ShaderUniformNames::default()).unwrap();
        shader.set_handle(3);
        assert_eq!(shader.get_handle(), 3);
    }

    #[test]
    fn uniform_names_default_to_none() {
        let shader = new(VERT, FRAG, ShaderUniformNames::default()).unwrap();
        assert_eq!(shader.get_uniform_names(), &ShaderUniformNames::default());
        assert_eq!(shader.get_uniform_model_name(), None);
        assert_eq!(shader.get_uniform_view_name(), None);
        assert_eq!(shader.get_uniform_proj_name(), None);
        assert_eq!(shader.get_uniform_umin_name(), None);
        assert_eq!(shader.get_uniform_umax_name(), None);
        assert_eq!(shader.get_uniform_vmin_name(), None);
        assert_eq!(shader.get_uniform_vmax_name(), None);
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
        let mut shader = new(VERT, FRAG, ShaderUniformNames::default()).unwrap();
        shader
            .set_uniform_names(ShaderUniformNames::matrices("model", "view", "proj"))
            .unwrap();
        assert_eq!(shader.get_uniform_model_name(), Some("model"));
        assert_eq!(shader.get_uniform_view_name(), Some("view"));
        assert_eq!(shader.get_uniform_proj_name(), Some("proj"));

        // A wrong name is rejected and the old names stay.
        assert!(shader
            .set_uniform_names(matrices(None, Some("viewx"), None))
            .is_err());
        assert_eq!(shader.get_uniform_view_name(), Some("view"));

        shader.set_uniform_names(matrices(None, None, None)).unwrap();
        assert_eq!(shader.get_uniform_view_name(), None);
    }

    #[test]
    fn texture_rect_uniform_names_must_be_floats() {
        // `time` is a float in types.vert/types.frag, `tint` a vec4.
        let names = ShaderUniformNames::default().with_texture_rect("time", "time", "time", "time");
        let shader = new(TYPES_VERT, TYPES_FRAG, names).unwrap();
        assert_eq!(shader.get_uniform_umin_name(), Some("time"));
        assert_eq!(shader.get_uniform_umax_name(), Some("time"));
        assert_eq!(shader.get_uniform_vmin_name(), Some("time"));
        assert_eq!(shader.get_uniform_vmax_name(), Some("time"));

        let wrong = ShaderUniformNames {
            vmin: Some("tint".to_string()),
            ..ShaderUniformNames::default()
        };
        let err = new(TYPES_VERT, TYPES_FRAG, wrong).err().unwrap();
        assert!(err.to_string().contains("tint"), "{err}");

        let missing = ShaderUniformNames {
            umax: Some("u_max".to_string()),
            ..ShaderUniformNames::default()
        };
        assert!(new(TYPES_VERT, TYPES_FRAG, missing).is_err());
    }

    #[test]
    fn new_reflects_layout() {
        let shader = new(VERT, FRAG, ShaderUniformNames::default()).unwrap();
        assert_eq!(shader.get_layout(), &ShaderLayout::reflect(VERT, FRAG).unwrap());
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
        assert!(new(&[1, 2, 3, 4, 5, 6, 7, 8], FRAG, ShaderUniformNames::default()).is_err());
    }
}
