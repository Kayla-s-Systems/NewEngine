#[cfg(test)]
mod first_person_stability_tests {
    use super::{equipment_animation_event, secondary_weapon_dynamics_enabled};
    use newengine_engine_runtime::gameplay::WeaponType;

    #[test]
    fn first_person_never_runs_secondary_weapon_inertia() {
        assert!(!secondary_weapon_dynamics_enabled(true, true, false));
        assert!(secondary_weapon_dynamics_enabled(true, false, false));
        assert!(!secondary_weapon_dynamics_enabled(false, false, false));
        assert!(
            !secondary_weapon_dynamics_enabled(true, false, true),
            "an animation-owned weapon root is terminal and must not be detached by visual secondary dynamics"
        );
    }

    #[test]
    fn melee_publishes_ready_only_and_never_aim_or_reload() {
        assert_eq!(
            equipment_animation_event(WeaponType::Melee, false, 0.0),
            "character.equipment.ready"
        );
        assert_eq!(
            equipment_animation_event(WeaponType::Melee, true, 1.0),
            "character.equipment.ready"
        );
    }

    #[test]
    fn firearm_publishes_ready_aim_and_reload_presentation() {
        assert_eq!(
            equipment_animation_event(WeaponType::Firearm, false, 0.0),
            "character.equipment.ready"
        );
        assert_eq!(
            equipment_animation_event(WeaponType::Firearm, false, 1.0),
            "character.equipment.aim"
        );
        assert_eq!(
            equipment_animation_event(WeaponType::Firearm, true, 1.0),
            "character.equipment.reload"
        );
    }
}
