#![forbid(unsafe_op_in_unsafe_fn)]

//! Shared authoring kit for NewEngine plugins.
//!
//! This crate is intentionally tiny: it does not define a second ABI.
//! It reexports the central engine API and provides macros so runtime plugins
//! do not manually construct `PluginRootV1` or copy ABI boilerplate.

pub use abi_stable;
pub use newengine_plugin_api as plugin_api;
pub use newengine_plugin_api::*;

pub mod prelude {
    pub use crate::{export_newengine_plugin, export_newengine_plugin_root};
    pub use crate::plugin_api::*;
}

/// Export a plugin root from explicitly declared create callbacks.
///
/// This is a thin wrapper over the central `newengine-plugin-api` export macro.
/// It always provides empty editor extensions unless a newer overload is added.
#[macro_export]
macro_rules! export_newengine_plugin_root {
    ($create_v1:path, $create_v2:path, $create_v3:path) => {
        $crate::plugin_api::export_plugin_root!($create_v1, $create_v2, $create_v3);
    };
    ($create_v1:path, $create_v2:path, $create_v3:path, $ui_assets_v1:path) => {
        $crate::plugin_api::export_plugin_root!($create_v1, $create_v2, $create_v3, $ui_assets_v1);
    };
    ($create_v1:path, $create_v2:path, $create_v3:path, $ui_assets_v1:path, $editor_extensions_v1:path) => {
        $crate::plugin_api::export_plugin_root!(
            $create_v1,
            $create_v2,
            $create_v3,
            $ui_assets_v1,
            $editor_extensions_v1
        );
    };
}

/// Export the common case where one module type implements V1, V2 and V3.
///
/// Example:
/// ```ignore
/// use newengine_plugin_kit::prelude::*;
/// use crate::module::AssetsPlugin;
/// static PLUGIN_ICON_PNG: &[u8] = include_bytes!("../assets/plugin_icon.png");
/// export_newengine_plugin!(module = AssetsPlugin::default(), icon_png = PLUGIN_ICON_PNG);
/// ```
#[macro_export]
macro_rules! export_newengine_plugin {
    (module = $module:expr $(, icon_png = $icon:path)? $(,)?) => {
        extern "C" fn create_module() -> $crate::plugin_api::PluginModuleDyn<'static> {
            $crate::plugin_api::PluginModule_TO::from_value(
                $module,
                $crate::abi_stable::sabi_trait::TD_Opaque,
            )
        }

        extern "C" fn create_module_v2() -> $crate::plugin_api::PluginModuleV2Dyn<'static> {
            $crate::plugin_api::PluginModuleV2_TO::from_value(
                $module,
                $crate::abi_stable::sabi_trait::TD_Opaque,
            )
        }

        extern "C" fn create_module_v3() -> $crate::plugin_api::PluginModuleV3Dyn<'static> {
            $crate::plugin_api::PluginModuleV3_TO::from_value(
                $module,
                $crate::abi_stable::sabi_trait::TD_Opaque,
            )
        }

        extern "C" fn ui_assets_v1() -> $crate::plugin_api::PluginUiAssetsV1 {
            $crate::export_newengine_plugin!(@ui_assets $($icon)?)
        }

        $crate::plugin_api::export_plugin_root!(
            create_module,
            create_module_v2,
            create_module_v3,
            ui_assets_v1
        );
    };

    (@ui_assets $icon:path) => {
        $crate::plugin_api::PluginUiAssetsV1::icon_png($icon)
    };

    (@ui_assets) => {
        $crate::plugin_api::PluginUiAssetsV1::empty()
    };
}
