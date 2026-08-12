use std::sync::Arc;

use super::*;

pub struct FpsInventoryHudProvider;

impl FpsInventoryHudProvider {
    #[inline]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl GameplayUiProvider for FpsInventoryHudProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "newengine.gameplay.fps.inventory-hud"
    }

    #[inline]
    fn dispatch_actions(&self, world: &mut GameplayWorld, frame: &UiEventDispatchFrame) -> bool {
        apply_inventory_ui_actions(world, frame)
    }

    #[inline]
    fn publish_frame(&self, world: &mut GameplayWorld, frame_index: u64) -> GameplayUiFrameOutput {
        publish_inventory_hud_state(world, frame_index)
    }

    #[inline]
    fn input_capture(&self, world: &GameplayWorld) -> GameplayInputCapture {
        if inventory_hud_is_open(world) {
            GameplayInputCapture::modal()
        } else {
            GameplayInputCapture::none()
        }
    }

    fn reset_transient_state(&self, world: &mut GameplayWorld) {
        if world.resource::<InventoryHudState>().is_some() {
            world.insert_resource(InventoryHudState::default());
        }
    }
}
