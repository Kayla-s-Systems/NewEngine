#![forbid(unsafe_op_in_unsafe_fn)]

mod module;
mod service;

use abi_stable::library::RootModule;
use abi_stable::sabi_types::VersionStrings;
use abi_stable::StableAbi;

use newengine_plugin_api::{PluginModuleDyn, PluginRootV1, PluginRootV1Ref};

use crate::module::AudioNullModule;

#[export_name = "export_plugin_root"]
pub extern "C" fn export_plugin_root() -> PluginRootV1Ref {
    PluginRootV1 {
        create: create_plugin,
    }
    .leak_into_prefix()
}

extern "C" fn create_plugin() -> PluginModuleDyn<'static> {
    PluginModuleDyn::from_value(AudioNullModule::new(), abi_stable::std_types::RBox::new(()))
}

impl RootModule for PluginRootV1Ref {
    abi_stable::declare_root_module_statics! { PluginRootV1Ref }

    const BASE_NAME: &'static str = "export_plugin_root";
    const NAME: &'static str = "export_plugin_root";
    const VERSION_STRINGS: VersionStrings = abi_stable::package_version_strings!();
}
