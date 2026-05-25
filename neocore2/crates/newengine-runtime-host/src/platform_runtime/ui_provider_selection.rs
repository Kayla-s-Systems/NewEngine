#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ui::UiProviderKind;

/// Runtime-side UI provider selection policy.
///
/// UI is discovered, not configured. If a UI-provider service is registered,
/// the runtime binds the first deterministic provider. If no provider exists,
/// `none` is a valid active mode. Startup config must not select toolkit/backend aliases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiProviderSelection {
    active: UiProviderKind,
}

impl UiProviderSelection {
    #[inline]
    pub(crate) fn new(_ignored_startup_request: UiProviderKind) -> Self {
        Self {
            active: discover_available_ui_provider(),
        }
    }

    #[inline]
    pub(crate) fn active(&self) -> &UiProviderKind {
        &self.active
    }

    #[inline]
    pub(crate) fn binding(&self) -> newengine_ui::UiProviderBinding {
        self.active.binding()
    }

    /// Re-evaluate availability after plugins/services have changed.
    /// Returns `Some(new_active_kind)` when the concrete provider object must be rebuilt.
    pub(crate) fn refresh(&mut self, origin: &'static str) -> Option<UiProviderKind> {
        let next = discover_available_ui_provider();
        if next == self.active {
            return None;
        }

        log_ui_provider_selection(origin, &next);
        self.active = next.clone();
        Some(next)
    }
}

pub(crate) fn discover_available_ui_provider() -> UiProviderKind {
    if newengine_core::has_engine_gateway_route(newengine_ui_api::ENGINE_UI_SERVICE_ID) {
        UiProviderKind::Plugin {
            service_id: newengine_ui_api::ENGINE_UI_SERVICE_ID.to_owned(),
        }
    } else {
        UiProviderKind::Null
    }
}

pub(crate) fn log_ui_provider_selection(origin: &str, active: &UiProviderKind) {
    match active {
        UiProviderKind::Null => {
            log::info!("ui provider: origin='{origin}' discovered=none active=none");
        }
        UiProviderKind::Plugin { service_id } => {
            log::info!(
                "ui provider: origin='{origin}' discovered gateway='{}' active gateway-backed",
                service_id
            );
        }
    }
}
