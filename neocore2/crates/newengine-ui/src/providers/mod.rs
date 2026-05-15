use crate::provider::{UiProvider, UiProviderKind, UiProviderOptions};

mod null;
mod plugin;

pub fn create_provider(opts: UiProviderOptions) -> Box<dyn UiProvider> {
    match opts.kind {
        UiProviderKind::Null => Box::new(null::NullUiProvider::new()),
        UiProviderKind::Plugin { service_id } => Box::new(plugin::PluginUiProvider::new(service_id)),
    }
}
