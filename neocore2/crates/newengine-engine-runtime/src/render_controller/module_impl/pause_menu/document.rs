#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ui_menu_runtime::MenuRuntime;
use newengine_ui_navigation_api::ENGINE_PAUSE_MENU_SURFACE_REF;

pub(super) fn try_load_pause_menu_document() -> Result<MenuRuntime, String> {
    Err(format!(
        "legacy runtime MenuDocument JSON loading is retired; compile '{}' through engine.assets.ui and mount it through engine.ui instead",
        ENGINE_PAUSE_MENU_SURFACE_REF
    ))
}
