#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::library::RootModule;
use abi_stable::sabi_types::VersionStrings;
use abi_stable::StableAbi;

use crate::module::{PluginModuleDyn, PluginModuleV2Dyn, PluginModuleV3Dyn};
use crate::ui::PluginUiAssetsV1;

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = PluginRootV1Ref)))]
pub struct PluginRootV1 {
    #[sabi(last_prefix_field)]
    pub create: extern "C" fn() -> PluginModuleDyn<'static>,

    pub create_v2: extern "C" fn() -> PluginModuleV2Dyn<'static>,

    pub create_v3: extern "C" fn() -> PluginModuleV3Dyn<'static>,

    pub ui_assets_v1: extern "C" fn() -> PluginUiAssetsV1,
}

impl RootModule for PluginRootV1Ref {
    abi_stable::declare_root_module_statics! { PluginRootV1Ref }

    const BASE_NAME: &'static str = "export_plugin_root";
    const NAME: &'static str = "export_plugin_root";
    const VERSION_STRINGS: VersionStrings = abi_stable::package_version_strings!();
}

#[inline]
pub extern "C" fn empty_plugin_ui_assets_v1() -> PluginUiAssetsV1 {
    PluginUiAssetsV1::empty()
}

/// Defines the exported root module symbol.
///
/// Usage in a plugin crate:
/// ```ignore
/// use newengine_plugin_api::prelude::*;
///
/// extern "C" fn create_v1() -> PluginModuleDyn<'static> { /* ... */ }
/// extern "C" fn create_v2() -> PluginModuleV2Dyn<'static> { /* ... */ }
/// extern "C" fn create_v3() -> PluginModuleV3Dyn<'static> { /* ... */ }
/// extern "C" fn ui_assets_v1() -> PluginUiAssetsV1 { PluginUiAssetsV1::empty() }
///
/// export_plugin_root!(create_v1, create_v2, create_v3);
/// export_plugin_root!(create_v1, create_v2, create_v3, ui_assets_v1);
/// ```
#[macro_export]
macro_rules! export_plugin_root {
    ($create_v1:path, $create_v2:path, $create_v3:path) => {
        $crate::export_plugin_root!(
            $create_v1,
            $create_v2,
            $create_v3,
            $crate::empty_plugin_ui_assets_v1
        );
    };
    ($create_v1:path, $create_v2:path, $create_v3:path, $ui_assets_v1:path) => {
        #[abi_stable::export_root_module]
        pub fn export_plugin_root() -> $crate::PluginRootV1Ref {
            $crate::PluginRootV1 {
                create: $create_v1,
                create_v2: $create_v2,
                create_v3: $create_v3,
                ui_assets_v1: $ui_assets_v1,
            }
            .leak_into_prefix()
        }
    };
}
