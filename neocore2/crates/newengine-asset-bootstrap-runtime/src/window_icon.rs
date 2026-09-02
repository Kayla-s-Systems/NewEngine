use newengine_assets::AssetServiceClient;
use newengine_platform_api::PlatformAppIconV1;
use std::path::PathBuf;

#[cfg(feature = "window-icon")]
use abi_stable::std_types::RVec;
#[cfg(feature = "window-icon")]
use newengine_assets::{wait_ready, AssetAccess};
#[cfg(feature = "window-icon")]
use std::time::Duration;

#[cfg(feature = "window-icon")]
pub fn try_load_window_icon_best_effort(
    icon_path: Option<&str>,
    assets: Option<&AssetServiceClient>,
    roots: &[PathBuf],
) -> Option<PlatformAppIconV1> {
    let _ = roots;
    let path = icon_path?;

    let Some(assets) = assets else {
        newengine_ulog_api::ulog::info!(
            "window icon: AssetManager unavailable; skipping icon path='{}' because runtime assets must not be read directly from filesystem",
            path
        );
        return None;
    };

    let id_hex32 = match assets.import_v1(path) {
        Ok(value) => value,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "window icon: asset.import_v1 failed path='{path}' err='{error}'"
            );
            return None;
        }
    };

    if let Err(error) = wait_ready(assets, &id_hex32, Duration::from_millis(500)) {
        newengine_ulog_api::ulog::warn!(
            "window icon: wait_ready failed path='{path}' id='{id_hex32}' err='{error:?}'"
        );
        return None;
    }

    let texture = match assets.texture_rgba8_v1(&id_hex32) {
        Ok(value) => value,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "window icon: texture_rgba8_v1 failed path='{path}' id='{id_hex32}' err='{error}'"
            );
            return None;
        }
    };

    Some(PlatformAppIconV1 {
        rgba: RVec::from(texture.rgba),
        width: texture.width,
        height: texture.height,
    })
}

#[cfg(not(feature = "window-icon"))]
pub fn try_load_window_icon_best_effort(
    icon_path: Option<&str>,
    assets: Option<&AssetServiceClient>,
    roots: &[PathBuf],
) -> Option<PlatformAppIconV1> {
    let _ = icon_path;
    let _ = assets;
    let _ = roots;
    None
}
