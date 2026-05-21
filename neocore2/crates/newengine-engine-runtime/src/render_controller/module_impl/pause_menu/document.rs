#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets::AssetServiceClient;
use newengine_ui_menu_runtime::MenuRuntime;
use newengine_ui_navigation_api::{MenuDocument, ENGINE_PAUSE_MENU_ASSET_PATH};

pub(super) fn try_load_pause_menu_document() -> Result<MenuRuntime, String> {
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let bytes = assets.text_v1(ENGINE_PAUSE_MENU_ASSET_PATH).map_err(|err| {
        format!(
            "asset.text_v1 failed path='{}' err='{}'",
            ENGINE_PAUSE_MENU_ASSET_PATH, err
        )
    })?;
    let text = String::from_utf8(bytes).map_err(|_| {
        format!(
            "asset.text_v1 returned non-utf8 MenuDocument path='{}'",
            ENGINE_PAUSE_MENU_ASSET_PATH
        )
    })?;
    let document = MenuDocument::from_json_str(&text).map_err(|err| {
        format!(
            "invalid MenuDocument JSON path='{}' err='{}'",
            ENGINE_PAUSE_MENU_ASSET_PATH, err
        )
    })?;
    MenuRuntime::new(document).map_err(|err| {
        format!(
            "invalid MenuDocument contract path='{}' err='{}'",
            ENGINE_PAUSE_MENU_ASSET_PATH, err
        )
    })
}
