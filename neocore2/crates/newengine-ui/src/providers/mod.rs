use crate::provider::{UiProvider, UiProviderKind, UiProviderOptions};

mod null;

#[cfg(feature = "provider-egui")]
mod egui;

pub fn create_provider(opts: UiProviderOptions) -> Box<dyn UiProvider> {
    match opts.kind {
        UiProviderKind::Null => Box::new(null::NullUiProvider::new()),

        UiProviderKind::Egui => {
            #[cfg(feature = "provider-egui")]
            {
                Box::new(egui::EguiUiProvider::new())
            }
            #[cfg(not(feature = "provider-egui"))]
            {
                Box::new(null::NullUiProvider::new())
            }
        }
    }
}
