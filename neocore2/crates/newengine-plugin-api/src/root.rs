#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::library::RootModule;
use abi_stable::sabi_types::VersionStrings;
use abi_stable::StableAbi;

use crate::module::{PluginModuleDyn, PluginModuleV2Dyn};

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = PluginRootV1Ref)))]
pub struct PluginRootV1 {
    #[sabi(last_prefix_field)]
    pub create: extern "C" fn() -> PluginModuleDyn<'static>,

    pub create_v2: extern "C" fn() -> PluginModuleV2Dyn<'static>,
}

impl RootModule for PluginRootV1Ref {
    abi_stable::declare_root_module_statics! { PluginRootV1Ref }

    const BASE_NAME: &'static str = "export_plugin_root";
    const NAME: &'static str = "export_plugin_root";
    const VERSION_STRINGS: VersionStrings = abi_stable::package_version_strings!();
}

/// Defines the exported root module symbol.
///
/// Usage in a plugin crate:
/// ```ignore
/// use newengine_plugin_api::prelude::*;
///
/// extern "C" fn create_v1() -> PluginModuleDyn<'static> { /* ... */ }
/// extern "C" fn create_v2() -> PluginModuleV2Dyn<'static> { /* ... */ }
///
/// export_plugin_root!(create_v1, create_v2);
/// ```
#[macro_export]
macro_rules! export_plugin_root {
    ($create_v1:path, $create_v2:path) => {
        #[abi_stable::export_root_module]
        pub fn export_plugin_root() -> $crate::PluginRootV1Ref {
            $crate::PluginRootV1 {
                create: $create_v1,
                create_v2: $create_v2,
            }
            .leak_into_prefix()
        }
    };
}