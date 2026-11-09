use anyhow::Result;

/// Platform-independent access to the application's assets (textures, meshes, ...).
///
/// Each platform has its own implementation:
/// - Windows: `WindowsAssetsProvider`, reading files from a directory next to the executable,
/// - Android: not implemented yet (assets will come from the APK through `AAssetManager`).
///
/// Use `create_assets_provider` to get the implementation for the current platform.
pub trait AssetsProvider {
    /// Reads the asset `name` and returns its contents. `name` is a path relative to the
    /// assets root, using `/` as separator, e.g. `"demo_000_viking_room_atlas.png"` or
    /// `"textures/wall.png"`.
    fn get_asset(&self, name: &str) -> Result<Vec<u8>>;
}

/// Creates the assets provider for the current platform.
///
/// On Windows `dir` is the assets directory relative to the executable's directory
/// (`build.rs` copies `resources/` to `assets/` there).
#[cfg(windows)]
pub fn create_assets_provider(dir: &str) -> Result<Box<dyn AssetsProvider>> {
    Ok(Box::new(
        super::windowsassetsprovider::WindowsAssetsProvider::new(dir)?,
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;

    use anyhow::{Context, Result};

    use super::AssetsProvider;

    /// Assets kept in memory, for tests.
    #[derive(Default)]
    pub(crate) struct MemoryAssetsProvider {
        pub(crate) assets: HashMap<String, Vec<u8>>,
    }

    impl AssetsProvider for MemoryAssetsProvider {
        fn get_asset(&self, name: &str) -> Result<Vec<u8>> {
            self.assets
                .get(name)
                .cloned()
                .with_context(|| format!("asset {name} not found"))
        }
    }

    /// Provider reading the crate's `testdata/` directory.
    pub(crate) fn test_resources() -> Box<dyn AssetsProvider> {
        Box::new(
            crate::windowsassetsprovider::WindowsAssetsProvider::from_path(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/testdata"
            )),
        )
    }

    #[test]
    fn memory_provider_returns_stored_assets() {
        let mut provider = MemoryAssetsProvider::default();
        provider.assets.insert("a/b.bin".to_string(), vec![1, 2, 3]);
        assert_eq!(provider.get_asset("a/b.bin").unwrap(), vec![1, 2, 3]);
        assert!(provider.get_asset("missing").is_err());
    }
}
