use super::*;

#[inline]
pub fn gameplay_default_listeners() -> Vec<InputActionListenerRegistration> {
    vec![
        InputActionListenerRegistration::new("newengine-ui", "ui-navigation")
            .with_actions([
                action::UI_NAVIGATION_TOGGLE,
                action::UI_NAVIGATION_ACCEPT,
                action::UI_NAVIGATION_BACK,
                action::UI_NAVIGATION_UP,
                action::UI_NAVIGATION_DOWN,
                action::UI_NAVIGATION_LEFT,
                action::UI_NAVIGATION_RIGHT,
            ])
            .with_priority(100)
            .consuming(),
        InputActionListenerRegistration::new("app.asset_browser", "asset-browser-ui")
            .with_actions([action::ASSET_CATALOG_UI_TOGGLE])
            .with_priority(110)
            .consuming(),
        InputActionListenerRegistration::new("app.asset_browser", "assets-browser-navigation")
            .with_actions([
                action::UI_NAVIGATION_ACCEPT,
                action::UI_NAVIGATION_BACK,
                action::UI_NAVIGATION_UP,
                action::UI_NAVIGATION_DOWN,
                action::UI_NAVIGATION_LEFT,
                action::UI_NAVIGATION_RIGHT,
            ])
            .with_priority(110),
        InputActionListenerRegistration::new("newengine-camera-runtime", "camera-view")
            .with_actions([
                action::CAMERA_VIEW_NEXT,
                action::CAMERA_VIEW_PREVIOUS,
                action::CAMERA_VIEW_FIRST_PERSON,
                action::CAMERA_VIEW_THIRD_PERSON_FOLLOW,
                action::CAMERA_VIEW_THIRD_PERSON_AIM,
            ])
            .with_priority(50),
        InputActionListenerRegistration::new("newengine-gameplay", "player-controller")
            .with_actions([
                action::PLAYER_MOVE_FORWARD,
                action::PLAYER_MOVE_BACK,
                action::PLAYER_MOVE_LEFT,
                action::PLAYER_MOVE_RIGHT,
                action::PLAYER_MOVE_UP,
                action::PLAYER_MOVE_DOWN,
                action::PLAYER_SPRINT,
                action::PLAYER_JUMP,
                action::PLAYER_CROUCH,
                action::PLAYER_FIRE_PRIMARY,
                action::PLAYER_LAUNCH_PROJECTILE,
                action::PLAYER_AIM,
                action::PLAYER_RELOAD,
                action::PLAYER_INTERACT,
            ])
            .with_priority(10),
        InputActionListenerRegistration::new("newengine-inventory", "inventory-controller")
            .with_actions([
                action::INVENTORY_TOGGLE,
                action::CHARACTER_SELECT_TOGGLE,
                action::EQUIP_PRIMARY,
                action::EQUIP_SECONDARY,
                action::EQUIP_SIDEARM,
                action::EQUIP_MELEE,
                action::EQUIP_THROWABLE,
            ])
            .with_priority(120),
    ]
}
