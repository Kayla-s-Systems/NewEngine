use super::*;
use crate::content::ensure_fps_player_loadouts;
use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::*;
use newengine_math::Vec3;
use newengine_primitives::{builtins as primitive_builtins, Primitive};
use newengine_sim::{AngularVelocity, Velocity};

fn install_fps_content(world: &mut World) {
    GameplayContentProvider::install(&FpsContentProvider, world).expect("install FPS content");
}

fn spawn_fps_player(
    world: &mut World,
    root: Option<EntityId>,
    name: impl Into<String>,
    position: Vec3,
) -> EntityId {
    let player =
        newengine_engine_runtime::gameplay::spawn_default_player(world, root, name, position);
    ensure_fps_player_loadouts(world);
    player
}

#[test]
fn inventory_stacks_items_and_respects_capacity_and_weight() {
    let mut world = World::new();
    install_fps_content(&mut world);
    let owner = world.spawn();
    let mut inventory = PlayerInventory::default();
    inventory.slot_capacity = 2;
    inventory.weight_capacity = 1.0;
    let _ = world.insert(owner, inventory);
    let ammo = default_rifle_ammo_id();
    let mutation = give_item(&mut world, owner, ammo, 200).expect("give ammo");
    assert_eq!(mutation.accepted, 83);
    assert_eq!(mutation.rejected, 117);
    assert_eq!(inventory_quantity(&world, owner, ammo), 83);
    assert_eq!(
        world
            .get::<PlayerInventory>(owner)
            .expect("inventory")
            .used_slots(),
        1
    );
}

#[test]
fn equipping_weapon_preserves_per_instance_magazine_state() {
    let mut world = World::new();
    install_fps_content(&mut world);
    let owner = spawn_fps_player(&mut world, None, "inventory-player", Vec3::ZERO);
    let rifle = default_rifle_item_id();
    let second = give_item(&mut world, owner, rifle, 1)
        .expect("give second rifle")
        .touched_instances[0];
    let first = world
        .get::<EquippedWeaponBinding>(owner)
        .expect("default binding")
        .instance_id;
    world
        .get_mut::<PlayerWeaponState>(owner)
        .expect("weapon state")
        .ammo_in_magazine = 7;
    equip_item_instance(&mut world, owner, second).expect("equip second rifle");
    assert_eq!(
        world
            .get::<PlayerWeaponState>(owner)
            .expect("second weapon state")
            .ammo_in_magazine,
        HitscanWeaponTuning::default().magazine_capacity
    );
    world
        .get_mut::<PlayerWeaponState>(owner)
        .expect("second weapon state")
        .ammo_in_magazine = 19;
    equip_item_instance(&mut world, owner, first).expect("restore first rifle");
    assert_eq!(
        world
            .get::<PlayerWeaponState>(owner)
            .expect("restored weapon state")
            .ammo_in_magazine,
        7
    );
}

#[test]
fn medkit_heals_and_consumes_one_stack_unit() {
    let mut world = World::new();
    install_fps_content(&mut world);
    let owner = spawn_fps_player(&mut world, None, "medkit-player", Vec3::ZERO);
    let medkit = default_medkit_item_id();
    world.get_mut::<Health>(owner).expect("health").current = 20.0;
    let before = inventory_quantity(&world, owner, medkit);
    use_item(&mut world, owner, medkit).expect("use medkit");
    assert_eq!(world.get::<Health>(owner).expect("health").current, 65.0);
    assert_eq!(inventory_quantity(&world, owner, medkit), before - 1);
}

#[test]
fn pickup_collection_transfers_item_and_dormants_source_entity() {
    let mut world = World::new();
    install_fps_content(&mut world);
    let owner = spawn_fps_player(&mut world, None, "pickup-player", Vec3::ZERO);
    let ammo = default_rifle_ammo_id();
    let before = inventory_quantity(&world, owner, ammo);
    let pickup = spawn_item_pickup(&mut world, None, ammo, 12, Vec3::ZERO).expect("spawn pickup");
    assert!(try_collect_item_pickup(&mut world, owner, pickup));
    assert_eq!(inventory_quantity(&world, owner, ammo), before + 12);
    assert!(world.exists(pickup));
    assert!(!world.get::<ItemPickup>(pickup).expect("pickup").enabled);
    assert_eq!(
        world
            .get::<DisplayVisibility>(pickup)
            .expect("visibility")
            .mode,
        DisplayMode::RuntimeHidden
    );
    assert!(world.get::<PhysicsBodyDesc>(pickup).is_none());
}

#[test]
fn loadout_reset_and_active_weapon_drop_leave_no_orphan_weapon_state() {
    let mut world = World::new();
    install_fps_content(&mut world);
    let owner = spawn_fps_player(&mut world, None, "lifecycle-player", Vec3::ZERO);
    apply_loadout(&mut world, owner, default_fps_loadout_id()).expect("reapply default loadout");
    {
        let inventory = world.get::<PlayerInventory>(owner).expect("inventory");
        assert!(inventory
            .weapon_states
            .keys()
            .all(|instance| { inventory.entry(*instance).is_some() }));
    }
    let dropped_instance = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| {
            inventory
                .entries
                .iter()
                .find(|entry| entry.item == default_rifle_item_id())
        })
        .map(|entry| entry.instance_id)
        .expect("active rifle instance");
    drop_item(&mut world, owner, default_rifle_item_id(), 1).expect("drop active rifle");
    let inventory = world.get::<PlayerInventory>(owner).expect("inventory");
    assert!(inventory.active_slot.is_none());
    assert!(!inventory.weapon_states.contains_key(&dropped_instance));
    assert!(inventory
        .weapon_states
        .keys()
        .all(|instance| inventory.entry(*instance).is_some()));
    assert!(world.get::<EquippedWeaponBinding>(owner).is_none());
    assert!(world.get::<PlayerWeaponState>(owner).is_none());
}

#[test]
fn world_pickup_uses_authored_visual_metadata_and_renderable_fallback() {
    let mut world = World::new();
    install_fps_content(&mut world);
    let pickup = spawn_item_pickup(
        &mut world,
        None,
        default_rifle_item_id(),
        1,
        Vec3::new(2.0, 1.0, -3.0),
    )
    .expect("spawn world pickup");
    let presentation = world
        .get::<WorldItemPresentation>(pickup)
        .expect("world presentation");
    assert_eq!(
        presentation.model_ref.as_deref(),
        Some("models/weapons/service_rifle.ydd@service_rifle")
    );
    assert_eq!(presentation.fallback_primitive, primitive_builtins::ID_CUBE);
    assert!(world.get::<Primitive>(presentation.visual_entity).is_some());
    assert_eq!(
        world
            .get::<WorldItemVisualPart>(presentation.visual_entity)
            .expect("visual marker")
            .owner,
        pickup
    );
    assert!(world
        .get::<PhysicsBodyDesc>(pickup)
        .expect("pickup physics")
        .is_trigger());
}

#[test]
fn dropped_item_gets_dynamic_physics_and_pickup_cooldown() {
    let mut world = World::new();
    install_fps_content(&mut world);
    let owner = spawn_fps_player(&mut world, None, "drop-player", Vec3::ZERO);
    let dropped = drop_item(&mut world, owner, default_rifle_item_id(), 1).expect("drop rifle");
    assert!(world
        .get::<PhysicsBodyDesc>(dropped)
        .expect("dropped physics")
        .dynamic());
    assert!(world.get::<Velocity>(dropped).is_some());
    assert!(world.get::<AngularVelocity>(dropped).is_some());
    assert!(
        world
            .get::<WorldItemRuntime>(dropped)
            .expect("world runtime")
            .dropped
    );
    assert!(!world.get::<ItemPickup>(dropped).expect("pickup").enabled);
    step_world_items(&mut world, 0.3);
    assert!(world.get::<ItemPickup>(dropped).expect("pickup").enabled);
    assert!(
        world
            .get::<Interactable>(dropped)
            .expect("interactable")
            .enabled
    );
}

#[test]
fn persistent_world_pickup_hides_and_respawns_without_changing_entity_id() {
    let mut world = World::new();
    install_fps_content(&mut world);
    let owner = spawn_fps_player(&mut world, None, "pickup-owner", Vec3::ZERO);
    let entity = spawn_persistent_item_pickup(
        &mut world,
        None,
        default_rifle_ammo_id(),
        7,
        Vec3::new(4.0, 1.0, 2.0),
        "test.pickup.rifle_ammo.01",
        0.02,
    )
    .expect("persistent pickup");
    assert!(try_collect_item_pickup(&mut world, owner, entity));
    assert!(world.exists(entity));
    assert_eq!(
        world
            .get::<DisplayVisibility>(entity)
            .expect("visibility")
            .mode,
        DisplayMode::RuntimeHidden
    );
    assert_eq!(world.get::<ItemPickup>(entity).expect("pickup").quantity, 0);
    step_world_items(&mut world, 0.03);
    assert!(world.exists(entity));
    assert_eq!(world.get::<ItemPickup>(entity).expect("pickup").quantity, 7);
    assert!(world.get::<ItemPickup>(entity).expect("pickup").enabled);
    assert_eq!(
        world
            .get::<DisplayVisibility>(entity)
            .expect("visibility")
            .mode,
        DisplayMode::Both
    );
    assert!(world.get::<PhysicsBodyDesc>(entity).is_some());
}

#[test]
fn default_loadout_equips_rifle_and_provisions_ammo() {
    let mut world = World::new();
    install_fps_content(&mut world);
    let owner = world.spawn();
    apply_loadout(&mut world, owner, default_fps_loadout_id()).expect("default loadout");
    assert_eq!(
        inventory_quantity(&world, owner, default_rifle_ammo_id()),
        90
    );
    assert!(world.get::<EquippedWeaponBinding>(owner).is_some());
    assert_eq!(
        world
            .get::<PlayerWeaponState>(owner)
            .expect("weapon state")
            .reserve_ammo,
        90
    );
}
