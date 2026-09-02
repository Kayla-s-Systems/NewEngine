#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_ytyp_namespace_hydrates_fire_reload_and_world_presentation() {
        let mut item = AuthoredItemDefinition {
            id: "weapon.test".to_owned(),
            definition_ref: "definitions/weapon/test.ytyp@test".to_owned(),
            kind: "weapon".to_owned(),
            ..AuthoredItemDefinition::default()
        };
        let metadata = serde_json::json!({
            "weapon": {
                "class": "pistol",
                "ammo": "ammo.test",
                "fire_mode": "automatic",
                "magazine_capacity": 32,
                "reserve_capacity": 160,
                "fire_interval": 0.075,
                "reload_duration": 1.42,
                "damage": 19.0,
                "range": 87.0,
                "hip_spread_degrees": 2.0,
                "aim_spread_degrees": 0.4,
                "recoil_pitch_degrees": 0.9,
                "recoil_yaw_degrees": 0.3,
                "muzzle_forward_offset": 0.51
            },
            "audio": {
                "fire": "shared/audio/weapon/test/fire.wav",
                "reload_start": "shared/audio/weapon/test/reload.wav",
                "equip": "shared/audio/weapon/test/equip.wav",
                "empty": "shared/audio/weapon/test/empty.wav",
                "shell_eject": "shared/audio/weapon/test/shell.wav"
            },
            "casing": {
                "model_dictionary": "models/weapon/test_shells.ydd",
                "variants": "a,b,c",
                "material_ref": "materials/test_shell.nemat@metal",
                "half_extents": "0.01,0.02,0.03",
                "ejection_delay_seconds": 0.04,
                "origin_local": "0.1,0.2,-0.3",
                "velocity_local": "1.0,2.0,-0.2",
                "velocity_jitter": "0.2,0.1,0.0",
                "axis_local": "0.9,0.1,0.0",
                "angular_velocity": "10,20,30",
                "angular_velocity_jitter": "1,2,3",
                "friction": 0.3,
                "restitution": 0.2,
                "density": 7.5,
                "linear_damping": 0.08,
                "angular_damping": 1.5,
                "contact_min_impulse": 0.002,
                "contact_medium_impulse": 0.011,
                "contact_hard_impulse": 0.031,
                "soft_surface_contains": "dirt,sand,grass",
                "ejection_joint": "shell_eject",
                "inherit_socket_linear_velocity": 0.9,
                "inherit_socket_angular_velocity": 0.25
            },
            "world": {
                "model": "models/weapon/test.ydd@test",
                "material_library": "materials/test.nemat",
                "scale": 1.0,
                "pickup_half_extents": [0.2, 0.4, 0.1]
            },
            "presentation": {
                "enabled": true,
                "handle_from_root": [0.0, 0.014, -0.030],
                "handle_rotation_from_root": [0.0, 0.0, 0.38268343, 0.9238795],
                "muzzle_from_root": [0.108, 0.041, 0.640],
                "left_grip_from_handle": [-0.021, 0.043, 0.306],
                "stock_contact_from_handle": [-0.020, 0.053, -0.341],
                "ready_body_to_root_rotation": [0.036, 0.608, -0.041, 0.792],
                "right_palm_to_handle": [0.019, 0.033, -0.083],
                "authored_socket_to_weapon_handle_basis": [-0.5686547, -0.3117329, -0.3185951, 0.6913404],
                "first_person_hip_handle_offset": [0.20, -0.20, -0.58],
                "first_person_full_body_hip_handle_offset": [0.20, -0.20, -0.08],
                "ads_camera_translation_weight": [1.0, 0.0, 0.75],
                "aim_response_hz": 14.0,
                "secondary_hip_max_angle_radians": 0.08,
                "secondary_ads_max_angle_radians": 0.03,
                "secondary_angular_inertia_gain": 0.31,
                "secondary_movement_inertia_gain": 0.77,
                "secondary_natural_hz_hip": 4.8,
                "secondary_natural_hz_ads": 8.2,
                "secondary_obstruction_hz_boost": 5.7
            },
            "vfx": {
                "shot": "effects/weapons/test.fxd@shot",
                "impact_default": "effects/weapons/test.fxd@impact.default",
                "impact_by_surface": "surface.metal=effects/weapons/test.fxd@impact.metal;surface.wood=effects/weapons/test.fxd@impact.wood"
            }
        });
        apply_weapon_ytyp_namespace(&mut item, &metadata).expect("hydrate");
        let weapon = item.weapon.expect("weapon");
        assert_eq!(weapon.class, "pistol");
        assert_eq!(weapon.fire_mode().expect("mode"), WeaponFireMode::Automatic);
        assert_eq!(weapon.magazine_capacity, 32);
        assert!((weapon.reload_duration - 1.42).abs() < 1.0e-6);
        assert!((weapon.damage - 19.0).abs() < 1.0e-6);
        let audio = item.weapon_audio.expect("weapon audio");
        assert_eq!(audio.fire, "shared/audio/weapon/test/fire.wav");
        assert_eq!(audio.reload_start, "shared/audio/weapon/test/reload.wav");
        assert_eq!(audio.shell_eject, "shared/audio/weapon/test/shell.wav");
        let casing = item.weapon_casing.expect("weapon casing");
        assert_eq!(casing.model_dictionary, "models/weapon/test_shells.ydd");
        assert_eq!(casing.variants, vec!["a", "b", "c"]);
        assert_eq!(casing.half_extents, [0.01, 0.02, 0.03]);
        assert!((casing.ejection_delay_seconds - 0.04).abs() < 1.0e-6);
        assert!((casing.linear_damping - 0.08).abs() < 1.0e-6);
        assert!((casing.angular_damping - 1.5).abs() < 1.0e-6);
        assert_eq!(casing.axis_local, [0.9, 0.1, 0.0]);
        assert_eq!(casing.ejection_joint, "shell_eject");
        assert!((casing.contact_min_impulse - 0.002).abs() < 1.0e-6);
        assert!((casing.contact_medium_impulse - 0.011).abs() < 1.0e-6);
        assert!((casing.contact_hard_impulse - 0.031).abs() < 1.0e-6);
        assert_eq!(casing.soft_surface_contains, vec!["dirt", "sand", "grass"]);
        assert!((casing.inherit_socket_linear_velocity - 0.9).abs() < 1.0e-6);
        assert!((casing.inherit_socket_angular_velocity - 0.25).abs() < 1.0e-6);
        let vfx = item.weapon_vfx.expect("weapon vfx");
        assert_eq!(vfx.shot, "effects/weapons/test.fxd@shot");
        assert_eq!(
            vfx.impact_by_surface
                .get("surface.metal")
                .map(String::as_str),
            Some("effects/weapons/test.fxd@impact.metal")
        );
        let world = item.world.expect("world");
        assert_eq!(world.model, "models/weapon/test.ydd@test");
        assert_eq!(world.material_library, "materials/test.nemat");
        assert_eq!(world.pickup_half_extents, [0.2, 0.4, 0.1]);
        let presentation = item.weapon_presentation.expect("weapon presentation");
        assert!(presentation.enabled);
        assert_eq!(presentation.handle_from_root, [0.0, 0.014, -0.030]);
        assert_eq!(
            presentation.handle_rotation_from_root,
            [0.0, 0.0, 0.38268343, 0.9238795]
        );
        assert_eq!(presentation.left_grip_from_handle, [-0.021, 0.043, 0.306]);
        assert_eq!(
            presentation.ready_body_to_root_rotation,
            [0.036, 0.608, -0.041, 0.792]
        );
        assert_eq!(presentation.right_palm_to_handle, [0.019, 0.033, -0.083]);
        assert_eq!(
            presentation.authored_socket_to_weapon_handle_basis,
            [-0.5686547, -0.3117329, -0.3185951, 0.6913404]
        );
        assert_eq!(
            presentation.first_person_hip_handle_offset,
            [0.20, -0.20, -0.58]
        );
        assert_eq!(
            presentation.first_person_full_body_hip_handle_offset,
            Some([0.20, -0.20, -0.08])
        );
        let runtime_presentation = presentation.compile();
        assert_eq!(
            runtime_presentation.first_person_full_body_hip_handle_offset,
            [0.20, -0.20, -0.08]
        );
        assert_eq!(
            runtime_presentation.ads_camera_translation_weight,
            [1.0, 0.0, 0.75]
        );
        assert!((presentation.aim_response_hz - 14.0).abs() < 1.0e-6);
        assert!((presentation.secondary_angular_inertia_gain - 0.31).abs() < 1.0e-6);
        assert!((presentation.secondary_movement_inertia_gain - 0.77).abs() < 1.0e-6);
        assert!((presentation.secondary_natural_hz_ads - 8.2).abs() < 1.0e-6);
    }

    #[test]
    fn legacy_weapon_presentation_inherits_authored_hip_offset_for_full_body_fpp() {
        let authored = AuthoredWeaponPresentationDefinition {
            enabled: true,
            first_person_hip_handle_offset: [0.18, -0.21, -0.31],
            first_person_full_body_hip_handle_offset: None,
            ..AuthoredWeaponPresentationDefinition::default()
        };
        let runtime = authored.compile();
        assert_eq!(
            runtime.first_person_full_body_hip_handle_offset,
            authored.first_person_hip_handle_offset
        );
    }
    #[test]
    fn pistol_magazine_ytyp_hydrates_component_graph_and_default_install() {
        let mut item = AuthoredItemDefinition {
            id: "weapon.pistol.military".to_owned(),
            definition_ref: "shared/definitions/weapon/pistol_military.ytyp@pistol_military"
                .to_owned(),
            kind: "weapon".to_owned(),
            ..AuthoredItemDefinition::default()
        };
        let metadata = serde_json::json!({
            "weapon": {
                "type": "firearm",
                "ammo": "ammo.sidearm.standard",
                "fire_mode": "semi_auto",
                "firing_pattern_kind": "semi",
                "magazine_capacity": 15,
                "fire_interval": 0.22
            },
            "component_points": {
                "points": "magazine|magazine|pistol_magazine,pistol_magazine_upgrade;mag_mod|mag_mod|pistol_mag_extend"
            },
            "components": {
                "installed": "magazine=pistol_magazine",
                "definitions": "pistol_magazine|magazine|shared/models/weapon/pistol/pistol_magazine.ydd@pistol_magazine||||1|1|1|1|1|1|1|0|0|0;pistol_magazine_upgrade|magazine|shared/models/weapon/pistol/pistol_magazine_upgrade.ydd@pistol_magazine_upgrade||||0.96|0.96|1|1|1|1|1|0|0|0;pistol_mag_extend|mag_mod|shared/models/weapon/pistol/pistol_mag_extend.ydd@pistol_mag_extend||||1|1|1|1|1|1|1|0|0|0"
            }
        });

        apply_weapon_ytyp_namespace(&mut item, &metadata).expect("hydrate pistol magazine graph");
        let authored_graph = item.weapon_components.expect("component graph");
        let runtime_graph = authored_graph.compile().expect("compile component graph");

        assert_eq!(runtime_graph.points.len(), 2);
        assert_eq!(
            runtime_graph
                .default_installed
                .get("magazine")
                .map(String::as_str),
            Some("pistol_magazine")
        );
        let magazine = runtime_graph
            .components
            .get("pistol_magazine")
            .expect("standard magazine");
        assert_eq!(magazine.slot, "magazine");
        assert_eq!(
            magazine.model_ref.as_deref(),
            Some("shared/models/weapon/pistol/pistol_magazine.ydd@pistol_magazine")
        );
        let upgrade = runtime_graph
            .components
            .get("pistol_magazine_upgrade")
            .expect("magazine upgrade");
        assert!((upgrade.modifiers.accuracy_multiplier - 0.96).abs() < 1.0e-6);
        assert!((upgrade.modifiers.recoil_multiplier - 0.96).abs() < 1.0e-6);
        assert_eq!(
            runtime_graph
                .points
                .iter()
                .find(|point| point.id == "mag_mod")
                .expect("mag mod slot")
                .allowed_components,
            vec!["pistol_mag_extend".to_owned()]
        );
    }
}
