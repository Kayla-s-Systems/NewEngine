use newengine_ui::{create_provider, UiProviderBinding, UiProviderKind, UiProviderOptions};

use super::super::HostPlatformRuntime;

impl HostPlatformRuntime {
    pub(crate) fn ui_provider_binding(&self) -> UiProviderBinding {
        self.ui_selection.binding()
    }

    pub(crate) fn overlay_provider_binding(&self) -> UiProviderBinding {
        match self.ui_selection.active() {
            UiProviderKind::Plugin { .. } => self.ui_provider_binding(),
            UiProviderKind::Null => UiProviderBinding::None,
        }
    }

    pub(crate) fn refresh_ui_provider_binding(&mut self, origin: &'static str) {
        let Some(next) = self.ui_selection.refresh(origin) else {
            return;
        };

        self.ui = create_provider(UiProviderOptions { kind: next });
    }
}
