use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::assetsprovider::AssetsProvider;

/// Windows `AssetsProvider`: reads assets as files from a directory next to the executable.
pub struct WindowsAssetsProvider {
    base_dir: PathBuf,
}

impl WindowsAssetsProvider {
    /// Creates a provider for `dir`, relative to the directory of the running executable.
    pub fn new(dir: impl AsRef<Path>) -> Result<Self> {
        let exe = std::env::current_exe().context("failed to get executable path")?;
        let exe_dir = exe
            .parent()
            .context("executable path has no parent directory")?;
        Ok(Self::from_path(exe_dir.join(dir)))
    }

    /// Creates a provider reading from `base_dir` as given (not relative to the executable).
    pub fn from_path(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    pub fn get_base_dir(&self) -> &Path {
        &self.base_dir
    }
}

impl AssetsProvider for WindowsAssetsProvider {
    fn get_asset(&self, name: &str) -> Result<Vec<u8>> {
        let path = self.base_dir.join(name);
        fs::read(&path).with_context(|| format!("failed to read asset {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resources() -> WindowsAssetsProvider {
        WindowsAssetsProvider::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata"))
    }

    fn expected(name: &str) -> Vec<u8> {
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("testdata")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn new_is_relative_to_executable() {
        let provider = WindowsAssetsProvider::new("assets").unwrap();
        let exe = std::env::current_exe().unwrap();
        assert_eq!(
            provider.get_base_dir(),
            exe.parent().unwrap().join("assets")
        );
    }

    #[test]
    fn get_asset_reads_file() {
        let bytes = resources().get_asset("atlas_512.png").unwrap();
        assert_eq!(bytes, expected("atlas_512.png"));
    }

    #[test]
    fn get_asset_through_trait_object() {
        let provider: Box<dyn AssetsProvider> = Box::new(resources());
        assert_eq!(
            provider.get_asset("quad.obj").unwrap(),
            expected("quad.obj")
        );
    }

    #[test]
    fn missing_asset_fails() {
        let err = resources().get_asset("does_not_exist.png").unwrap_err();
        assert!(err.to_string().contains("does_not_exist.png"));
    }

    #[test]
    fn create_assets_provider_uses_executable_dir() {
        // Test executables live in target/<profile>/deps, which has no assets/ directory,
        // so only check that the provider is created and reports missing files.
        let provider = crate::assetsprovider::create_assets_provider("assets").unwrap();
        assert!(provider.get_asset("does_not_exist.png").is_err());
    }
}
