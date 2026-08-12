use super::*;

#[inline]
pub fn gameplay_default_actions() -> Vec<InputActionDefinition> {
    vec![
        InputActionDefinition::new(action::PLAYER_MOVE_FORWARD)
            .with_label("Move forward")
            .with_effect(InputActionEffect::MoveMask {
                mask: move_mask::FORWARD,
            }),
        InputActionDefinition::new(action::PLAYER_MOVE_BACK)
            .with_label("Move back")
            .with_effect(InputActionEffect::MoveMask {
                mask: move_mask::BACK,
            }),
        InputActionDefinition::new(action::PLAYER_MOVE_LEFT)
            .with_label("Move left")
            .with_effect(InputActionEffect::MoveMask {
                mask: move_mask::LEFT,
            }),
        InputActionDefinition::new(action::PLAYER_MOVE_RIGHT)
            .with_label("Move right")
            .with_effect(InputActionEffect::MoveMask {
                mask: move_mask::RIGHT,
            }),
        InputActionDefinition::new(action::PLAYER_MOVE_UP)
            .with_label("Move up")
            .with_effect(InputActionEffect::MoveMask {
                mask: move_mask::UP,
            }),
        InputActionDefinition::new(action::PLAYER_MOVE_DOWN)
            .with_label("Move down")
            .with_effect(InputActionEffect::MoveMask {
                mask: move_mask::DOWN,
            }),
        InputActionDefinition::new(action::PLAYER_SPRINT)
            .with_label("Sprint")
            .with_effect(InputActionEffect::MoveMask {
                mask: move_mask::SPRINT,
            })
            .with_effect(InputActionEffect::Sprint { enabled: true }),
        InputActionDefinition::new(action::PLAYER_JUMP).with_label("Jump"),
        InputActionDefinition::new(action::PLAYER_CROUCH).with_label("Crouch"),
        InputActionDefinition::new(action::PLAYER_FIRE_PRIMARY).with_label("Primary fire"),
        InputActionDefinition::new(action::PLAYER_LAUNCH_PROJECTILE)
            .with_label("Launch physics sphere"),
        InputActionDefinition::new(action::PLAYER_AIM).with_label("Aim"),
        InputActionDefinition::new(action::PLAYER_RELOAD).with_label("Reload"),
        InputActionDefinition::new(action::PLAYER_INTERACT).with_label("Interact"),
        InputActionDefinition::new(action::INVENTORY_TOGGLE).with_label("Toggle inventory"),
        InputActionDefinition::new(action::EQUIP_PRIMARY).with_label("Equip primary weapon"),
        InputActionDefinition::new(action::EQUIP_SECONDARY).with_label("Equip secondary weapon"),
        InputActionDefinition::new(action::EQUIP_SIDEARM).with_label("Equip sidearm"),
        InputActionDefinition::new(action::EQUIP_MELEE).with_label("Equip melee weapon"),
        InputActionDefinition::new(action::EQUIP_THROWABLE).with_label("Equip throwable"),
        InputActionDefinition::new(action::CAMERA_VIEW_NEXT)
            .with_label("Next camera view")
            .with_effect(InputActionEffect::CameraView {
                request: CameraViewRequest::Next,
            }),
        InputActionDefinition::new(action::CAMERA_VIEW_PREVIOUS)
            .with_label("Previous camera view")
            .with_effect(InputActionEffect::CameraView {
                request: CameraViewRequest::Previous,
            }),
        InputActionDefinition::new(action::CAMERA_VIEW_FIRST_PERSON)
            .with_label("First-person camera")
            .with_effect(InputActionEffect::CameraView {
                request: CameraViewRequest::Set(newengine_camera_api::CameraViewMode::FirstPerson),
            }),
        InputActionDefinition::new(action::CAMERA_VIEW_THIRD_PERSON_FOLLOW)
            .with_label("Third-person follow camera")
            .with_effect(InputActionEffect::CameraView {
                request: CameraViewRequest::Set(
                    newengine_camera_api::CameraViewMode::ThirdPersonFollow,
                ),
            }),
        InputActionDefinition::new(action::CAMERA_VIEW_THIRD_PERSON_AIM)
            .with_label("Third-person aim camera")
            .with_effect(InputActionEffect::CameraView {
                request: CameraViewRequest::Set(
                    newengine_camera_api::CameraViewMode::ThirdPersonAim,
                ),
            }),
        InputActionDefinition::new(action::UI_NAVIGATION_TOGGLE)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Toggle UI")
            .with_effect(InputActionEffect::UiToggle),
        InputActionDefinition::new(action::UI_NAVIGATION_ACCEPT)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Accept")
            .with_effect(InputActionEffect::UiAccept),
        InputActionDefinition::new(action::UI_NAVIGATION_BACK)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Back")
            .with_effect(InputActionEffect::UiBack),
        InputActionDefinition::new(action::UI_NAVIGATION_UP)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("UI up")
            .with_effect(InputActionEffect::UiNav { x: 0, y: -1 }),
        InputActionDefinition::new(action::UI_NAVIGATION_DOWN)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("UI down")
            .with_effect(InputActionEffect::UiNav { x: 0, y: 1 }),
        InputActionDefinition::new(action::UI_NAVIGATION_LEFT)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("UI left")
            .with_effect(InputActionEffect::UiNav { x: -1, y: 0 }),
        InputActionDefinition::new(action::UI_NAVIGATION_RIGHT)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("UI right")
            .with_effect(InputActionEffect::UiNav { x: 1, y: 0 }),
        InputActionDefinition::new(action::ASSET_CATALOG_UI_TOGGLE)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Toggle assets catalog UI"),
    ]
}
