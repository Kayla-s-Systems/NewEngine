#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{apply_loadout, PlayerInventory};

    fn sample_package() -> AuthoredItemPackage {
        AuthoredItemPackage {
            items: vec![
                AuthoredItemDefinition {
                    id: "ammo.test".to_owned(),
                    display_name: "Test Ammo".to_owned(),
                    kind: "ammo".to_owned(),
                    ammo_profile: Some(AuthoredAmmoDefinition {
                        caliber: "5.56x45mm".to_owned(),
                        projectile_mass_kg: 0.004,
                        muzzle_velocity_mps: 850.0,
                        penetration_energy_j: 1450.0,
                        max_penetration_m: 0.45,
                        falloff_start_m: 35.0,
                        falloff_end_m: 120.0,
                        falloff_min_multiplier: 0.62,
                        ..AuthoredAmmoDefinition::default()
                    }),
                    max_stack: 100,
                    ..AuthoredItemDefinition::default()
                },
                AuthoredItemDefinition {
                    id: "weapon.test".to_owned(),
                    display_name: "Test Rifle".to_owned(),
                    kind: "weapon".to_owned(),
                    equipment_slot: "primary".to_owned(),
                    weapon: Some(AuthoredWeaponDefinition {
                        ammo: "ammo.test".to_owned(),
                        ..AuthoredWeaponDefinition::default()
                    }),
                    ..AuthoredItemDefinition::default()
                },
            ],
            loadouts: vec![AuthoredLoadoutDefinition {
                id: "loadout.test".to_owned(),
                entries: vec![AuthoredLoadoutEntry {
                    item: "weapon.test".to_owned(),
                    quantity: 1,
                    equip: true,
                }],
                ..AuthoredLoadoutDefinition::default()
            }],
            ..AuthoredItemPackage::default()
        }
    }

    #[test]
    fn authored_package_compiles_and_round_trips_through_nef8() {
        let package = sample_package();
        let compiled = compile_authored_item_package(&package).expect("compile package");
        assert_eq!(compiled.catalog.len(), 2);
        assert!(compiled.catalog.find("weapon.test").is_some());

        let encoded = encode_authored_item_package_nef8(&package, "items/test.neitems")
            .expect("encode NEITEMS");
        assert!(encoded.starts_with(&LIST_FILE_MAGIC_NEF8));
        let header = parse_list_file_header(&encoded).expect("parse V2 header");
        assert_eq!(header.version, newengine_assets_api::LIST_FILE_VERSION);
        assert!(matches!(header.header_len, 32 | 64));
        let decoded = decode_authored_item_package_nef8(&encoded).expect("decode NEITEMS");
        assert_eq!(decoded, package);
    }

    #[test]
    fn test_fps_package_installs_multi_weapon_primary_loadout() {
        let package =
            compile_authored_item_package(&test_fps_item_package()).expect("test package");
        assert!(package.catalog.find("weapon.rifle.standard").is_some());
        assert!(package.catalog.find("weapon.pistol.standard").is_some());
        let mut world = World::new();
        install_compiled_item_package(&mut world, package);
        let owner = world.spawn();
        apply_loadout(
            &mut world,
            owner,
            ItemId::from_name("loadout.fps.default").expect("valid test loadout id"),
        ).expect("default loadout");
        let inventory = world.get::<PlayerInventory>(owner).expect("inventory");
        assert_eq!(inventory.active_slot, Some(EquipmentSlot::Primary));
        assert!(inventory
            .equipped_instance(EquipmentSlot::Sidearm)
            .is_some());
    }

    #[test]
    fn authored_package_rejects_weapon_with_missing_ammo_definition() {
        let mut package = sample_package();
        package.items.remove(0);
        let error = compile_authored_item_package(&package).expect_err("missing ammo must fail");
        assert!(error.contains("missing ammo"));
    }
}
