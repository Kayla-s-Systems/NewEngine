use abi_stable::derive_macro_reexports::PrefixTypeTrait;
use abi_stable::export_root_module;
use abi_stable::sabi_trait::TD_Opaque;

use newengine_plugin_api::{PluginModuleDyn, PluginModule_TO, PluginRootV1, PluginRootV1_Ref};

use crate::module::InputPlugin;

#[export_root_module]
pub fn export_plugin_root() -> PluginRootV1_Ref {
    PluginRootV1 {
        create: create_module,
    }
        .leak_into_prefix()
}

extern "C" fn create_module() -> PluginModuleDyn<'static> {
    PluginModule_TO::from_value(InputPlugin::default(), TD_Opaque)
}