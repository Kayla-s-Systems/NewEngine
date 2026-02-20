#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::derive_macro_reexports::PrefixTypeTrait;
use abi_stable::sabi_trait::TD_Opaque;

use newengine_plugin_api::{
    PluginModuleDyn, PluginModuleV2Dyn, PluginModuleV2_TO, PluginModule_TO, PluginRootV1,
    PluginRootV1Ref,
};

use crate::module::LoggingPlugin;

#[no_mangle]
pub extern "C" fn export_plugin_root() -> PluginRootV1Ref {
    PluginRootV1 {
        create: create_module,
        create_v2: create_module_v2,
    }
        .leak_into_prefix()
}

extern "C" fn create_module() -> PluginModuleDyn<'static> {
    PluginModule_TO::from_value(LoggingPlugin::default(), TD_Opaque)
}

extern "C" fn create_module_v2() -> PluginModuleV2Dyn<'static> {
    PluginModuleV2_TO::from_value(LoggingPlugin::default(), TD_Opaque)
}
