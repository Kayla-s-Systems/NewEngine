#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::derive_macro_reexports::PrefixTypeTrait;
use abi_stable::sabi_trait::TD_Opaque;

use newengine_plugin_api::{PluginModuleDyn, PluginModule_TO, PluginRootV1, PluginRootV1Ref};

use crate::module::ImageImporterPlugin;

#[no_mangle]
pub extern "C" fn export_plugin_root() -> PluginRootV1Ref {
    PluginRootV1 {
        create: create_module,
    }
    .leak_into_prefix()
}

extern "C" fn create_module() -> PluginModuleDyn<'static> {
    PluginModule_TO::from_value(ImageImporterPlugin::default(), TD_Opaque)
}
