use std::ffi::CString;
use std::io::Read;

use anyhow::{Context, Result};

use super::assetsprovider::AssetsProvider;
use super::AndroidApp;

/// Android `AssetsProvider`: reads assets packed into the APK (its `assets/` directory), through
/// Android's `AAssetManager`.
///
/// The Rust form of the C++ engine's `AssetManager(AAssetManager*)`. Build it in `android_main`
/// from the `AndroidApp` it receives, and give it to the app.
pub struct AndroidAssetsProvider {
    app: AndroidApp,
}

impl AndroidAssetsProvider {
    pub fn new(app: &AndroidApp) -> Self {
        Self { app: app.clone() }
    }
}

impl AssetsProvider for AndroidAssetsProvider {
    fn get_asset(&self, name: &str) -> Result<Vec<u8>> {
        let path = CString::new(name).with_context(|| format!("bad asset name {name:?}"))?;
        let mut asset = self
            .app
            .asset_manager()
            .open(&path)
            .with_context(|| format!("asset {name} not found in the APK"))?;
        let mut bytes = Vec::with_capacity(asset.length());
        asset
            .read_to_end(&mut bytes)
            .with_context(|| format!("failed to read asset {name}"))?;
        Ok(bytes)
    }
}
