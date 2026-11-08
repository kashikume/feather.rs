use anyhow::{bail, Result};

use super::object::Object;

/// SPIR-V magic number (first word of every SPIR-V module).
const SPIRV_MAGIC: u32 = 0x0723_0203;

/// A shader program: compiled SPIR-V for the vertex and fragment stages.
pub struct Shader {
    name: Option<String>,
    handle: usize,
    vertex: Vec<u8>,
    fragment: Vec<u8>,
    uniform_model_name: Option<String>,
    uniform_view_name: Option<String>,
    uniform_proj_name: Option<String>,
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
    /// `include_bytes!(concat!(env!("OUT_DIR"), "/shaders/shader.vert.spv"))`.
    /// `uniform_model_name`, `uniform_view_name` and `uniform_proj_name` name the uniforms
    /// that receive the node's model matrix and the camera's view and projection matrices
    /// (`None` if the shader doesn't use one).
    /// Fails if either stage is not valid SPIR-V.
    pub fn new(
        name: Option<String>,
        vertex: &[u8],
        fragment: &[u8],
        uniform_model_name: Option<String>,
        uniform_view_name: Option<String>,
        uniform_proj_name: Option<String>,
    ) -> Result<Self> {
        check_spirv("vertex", vertex)?;
        check_spirv("fragment", fragment)?;
        Ok(Self {
            name,
            handle: usize::MAX,
            vertex: vertex.to_vec(),
            fragment: fragment.to_vec(),
            uniform_model_name,
            uniform_view_name,
            uniform_proj_name,
        })
    }

    pub fn get_vertex(&self) -> &[u8] {
        &self.vertex
    }

    pub fn get_fragment(&self) -> &[u8] {
        &self.fragment
    }

    /// Name of the uniform that receives the node's model matrix, if the shader uses one.
    pub fn get_uniform_model_name(&self) -> Option<&str> {
        self.uniform_model_name.as_deref()
    }

    pub fn set_uniform_model_name(&mut self, name: Option<String>) -> &mut Self {
        self.uniform_model_name = name;
        self
    }

    /// Name of the uniform that receives the camera's view matrix, if the shader uses one.
    pub fn get_uniform_view_name(&self) -> Option<&str> {
        self.uniform_view_name.as_deref()
    }

    pub fn set_uniform_view_name(&mut self, name: Option<String>) -> &mut Self {
        self.uniform_view_name = name;
        self
    }

    /// Name of the uniform that receives the camera's projection matrix, if the shader uses one.
    pub fn get_uniform_proj_name(&self) -> Option<&str> {
        self.uniform_proj_name.as_deref()
    }

    pub fn set_uniform_proj_name(&mut self, name: Option<String>) -> &mut Self {
        self.uniform_proj_name = name;
        self
    }
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
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/shader.vert.spv"));
    pub(crate) const FRAG: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/shader.frag.spv"));

    #[test]
    fn new_keeps_code_and_name() {
        let shader = Shader::new(Some("basic".to_string()), VERT, FRAG, None, None, None).unwrap();
        assert_eq!(shader.get_name(), Some("basic".to_string()));
        assert_eq!(shader.get_handle(), usize::MAX);
        assert_eq!(shader.get_vertex(), VERT);
        assert_eq!(shader.get_fragment(), FRAG);
    }

    #[test]
    fn set_handle() {
        let mut shader = Shader::new(None, VERT, FRAG, None, None, None).unwrap();
        shader.set_handle(3);
        assert_eq!(shader.get_handle(), 3);
    }

    #[test]
    fn matrix_uniform_names_default_to_none() {
        let shader = Shader::new(None, VERT, FRAG, None, None, None).unwrap();
        assert_eq!(shader.get_uniform_model_name(), None);
        assert_eq!(shader.get_uniform_view_name(), None);
        assert_eq!(shader.get_uniform_proj_name(), None);
    }

    #[test]
    fn new_keeps_matrix_uniform_names() {
        let shader = Shader::new(
            None,
            VERT,
            FRAG,
            Some("model".to_string()),
            None,
            Some("proj".to_string()),
        )
        .unwrap();
        assert_eq!(shader.get_uniform_model_name(), Some("model"));
        assert_eq!(shader.get_uniform_view_name(), None);
        assert_eq!(shader.get_uniform_proj_name(), Some("proj"));
    }

    #[test]
    fn set_matrix_uniform_names() {
        let mut shader = Shader::new(None, VERT, FRAG, None, None, None).unwrap();
        shader
            .set_uniform_model_name(Some("model".to_string()))
            .set_uniform_view_name(Some("view".to_string()))
            .set_uniform_proj_name(Some("proj".to_string()));
        assert_eq!(shader.get_uniform_model_name(), Some("model"));
        assert_eq!(shader.get_uniform_view_name(), Some("view"));
        assert_eq!(shader.get_uniform_proj_name(), Some("proj"));

        shader.set_uniform_view_name(None);
        assert_eq!(shader.get_uniform_view_name(), None);
    }

    #[test]
    fn empty_code_fails() {
        assert!(Shader::new(None, &[], FRAG, None, None, None).is_err());
        assert!(Shader::new(None, VERT, &[], None, None, None).is_err());
    }

    #[test]
    fn size_not_multiple_of_four_fails() {
        let mut code = VERT.to_vec();
        code.push(0);
        assert!(Shader::new(None, &code, FRAG, None, None, None).is_err());
    }

    #[test]
    fn bad_magic_fails() {
        assert!(Shader::new(None, &[1, 2, 3, 4, 5, 6, 7, 8], FRAG, None, None, None).is_err());
    }
}
