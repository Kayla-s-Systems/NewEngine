use crate::provider::{UiProvider, UiProviderKind, UiProviderOptions};

mod null;

pub fn create_provider(opts: UiProviderOptions) -> Box<dyn UiProvider> {
    match opts.kind {
        UiProviderKind::Null => Box::new(null::NullUiProvider::new()),
        UiProviderKind::Plugin { .. } => Box::new(null::NullUiProvider::new()),
    }
}
