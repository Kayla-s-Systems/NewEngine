#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::{Arc, Mutex};

use newengine_platform_winit::egui;
use newengine_ui::{UiContributor, UiDynFrame, UiLayer, UiOrder};

use crate::plugin_manager::PluginManagerUi;

/// Plugin Manager window contribution.
///
/// This demonstrates the unified draw mechanism:
/// editor code does not "manually" call `PluginManagerUi::show` from multiple places.
/// Instead, it is registered once into `UiHub` and executed every UI frame.
pub struct PluginManagerContributor {
    pm: Arc<Mutex<PluginManagerUi>>,
}

impl PluginManagerContributor {
    #[inline]
    pub fn new(pm: Arc<Mutex<PluginManagerUi>>) -> Self {
        Self { pm }
    }
}

impl UiContributor for PluginManagerContributor {
    fn id(&self) -> &'static str {
        "editor.plugin_manager"
    }

    fn layer(&self) -> UiLayer {
        UiLayer::Main
    }

    fn order(&self) -> UiOrder {
        50
    }

    fn draw(&mut self, frame: &mut UiDynFrame<'_>) {
        // Take egui Context immutably to avoid holding a &mut borrow of `frame`.
        let Some(ctx) = (&*frame.ctx_any).downcast_ref::<egui::Context>() else {
            return;
        };

        if let Ok(mut pm) = self.pm.lock() {
            pm.show(ctx);
        }
    }
}
