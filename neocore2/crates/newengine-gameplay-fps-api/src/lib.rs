#![forbid(unsafe_op_in_unsafe_fn)]

//! Data-only FPS gameplay contracts. Engine input/runtime crates do not depend on this crate;
//! FPS gameplay and FPS input profiles share it to interpret generic semantic action transport.

mod runtime;
pub use runtime::{
    FpsDemoGoal, FpsDemoHazard, FpsDemoPickup, FpsDemoRules, FpsDemoState, FpsDemoTarget,
    FpsPlayerTuning,
};

use newengine_input_actions_api::ActionCommandFrame;

pub mod action {
    pub const PLAYER_JUMP: &str = "player.jump";
    pub const PLAYER_CROUCH: &str = "player.crouch";
    pub const PLAYER_FIRE_PRIMARY: &str = "player.fire.primary";
    pub const PLAYER_LAUNCH_PROJECTILE: &str = "player.projectile.launch";
    pub const PLAYER_AIM: &str = "player.aim";
    pub const PLAYER_RELOAD: &str = "player.reload";
    pub const PLAYER_INTERACT: &str = "player.interact";
    pub const INVENTORY_TOGGLE: &str = "player.inventory.toggle";
    pub const HUD_VISIBILITY_TOGGLE: &str = "game.hud.visibility.toggle";
    pub const EQUIP_PRIMARY: &str = "player.equipment.primary";
    pub const EQUIP_SECONDARY: &str = "player.equipment.secondary";
    pub const EQUIP_SIDEARM: &str = "player.equipment.sidearm";
    pub const EQUIP_MELEE: &str = "player.equipment.melee";
    pub const EQUIP_THROWABLE: &str = "player.equipment.throwable";
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FpsActionFrame {
    pub jump_pressed: bool,
    pub crouch_held: bool,
    pub fire_primary_held: bool,
    pub launch_projectile_pressed: bool,
    pub aim_held: bool,
    pub reload_pressed: bool,
    pub interact_pressed: bool,
    pub inventory_toggle_pressed: bool,
    pub hud_visibility_toggle_pressed: bool,
    pub equipment_slot_pressed: Option<u8>,
}

impl FpsActionFrame {
    #[inline]
    pub fn from_commands(commands: &ActionCommandFrame) -> Self {
        Self {
            jump_pressed: commands.is_pressed(action::PLAYER_JUMP),
            crouch_held: commands.is_held(action::PLAYER_CROUCH),
            fire_primary_held: commands.is_held(action::PLAYER_FIRE_PRIMARY),
            launch_projectile_pressed: commands.is_pressed(action::PLAYER_LAUNCH_PROJECTILE),
            aim_held: commands.is_held(action::PLAYER_AIM),
            reload_pressed: commands.is_pressed(action::PLAYER_RELOAD),
            interact_pressed: commands.is_pressed(action::PLAYER_INTERACT),
            inventory_toggle_pressed: commands.is_pressed(action::INVENTORY_TOGGLE),
            hud_visibility_toggle_pressed: commands.is_pressed(action::HUD_VISIBILITY_TOGGLE),
            equipment_slot_pressed: [
                action::EQUIP_PRIMARY,
                action::EQUIP_SECONDARY,
                action::EQUIP_SIDEARM,
                action::EQUIP_MELEE,
                action::EQUIP_THROWABLE,
            ]
            .into_iter()
            .position(|action| commands.is_pressed(action))
            .map(|index| index as u8 + 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_view_is_derived_from_generic_commands() {
        let commands = ActionCommandFrame {
            held: vec![
                action::PLAYER_CROUCH.into(),
                action::PLAYER_FIRE_PRIMARY.into(),
            ],
            pressed: vec![action::PLAYER_JUMP.into(), action::EQUIP_SIDEARM.into()],
            released: Vec::new(),
        };
        let fps = FpsActionFrame::from_commands(&commands);
        assert!(fps.crouch_held);
        assert!(fps.fire_primary_held);
        assert!(fps.jump_pressed);
        assert_eq!(fps.equipment_slot_pressed, Some(3));
    }
}
