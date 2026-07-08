use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "file-icons/**/*.svg"]
struct FileIconAssets;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        if let Some(file) = FileIconAssets::get(path) {
            return Ok(Some(file.data));
        }

        let component_assets = gpui_component_assets::Assets;
        component_assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let component_assets = gpui_component_assets::Assets;
        let mut assets = component_assets.list(path)?;
        assets.extend(
            FileIconAssets::iter()
                .filter_map(|entry| entry.starts_with(path).then(|| entry.into())),
        );
        assets.sort();
        assets.dedup();
        Ok(assets)
    }
}
