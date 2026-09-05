use super::casters::{dynamic_skin_shadow_refresh_due, shadow_skin_pose_hash};
use super::compare::{
    shadow_matrices_match, SHADOW_DIRECTIONAL_MATRIX_EPSILON, SHADOW_LOCAL_MATRIX_EPSILON,
};

#[test]
fn directional_matrix_cache_tracks_real_sub_old_epsilon_motion() {
    let stable = newengine_math::Mat4::IDENTITY;
    let machine_noise = newengine_math::Mat4::from_translation(newengine_math::Vec3::new(
        SHADOW_DIRECTIONAL_MATRIX_EPSILON * 0.25,
        0.0,
        0.0,
    ));
    let real_motion =
        newengine_math::Mat4::from_translation(newengine_math::Vec3::new(1.0e-5, 0.0, 0.0));
    assert!(shadow_matrices_match(
        stable,
        machine_noise,
        SHADOW_DIRECTIONAL_MATRIX_EPSILON,
    ));
    assert!(!shadow_matrices_match(
        stable,
        real_motion,
        SHADOW_DIRECTIONAL_MATRIX_EPSILON,
    ));
    assert!(shadow_matrices_match(
        stable,
        real_motion,
        SHADOW_LOCAL_MATRIX_EPSILON,
    ));
}

#[test]
fn skin_shadow_hash_tracks_publication_revision_without_rehashing_palette_content() {
    let mut world = newengine_ecs::World::new();
    let owner = world.spawn();
    let visual = world.spawn();
    let _ = world.insert(
        owner,
        newengine_gameplay_world_runtime::gameplay::PlayerSkinPose {
            palette: vec![newengine_math::Mat4::IDENTITY],
            revision: 1,
        },
    );
    let _ = world.insert(
        visual,
        newengine_primitives::Primitive {
            id: newengine_primitives::builtins::ID_CUBE,
            color: [1.0; 4],
        },
    );
    let _ = world.insert(
        visual,
        newengine_gameplay_world_runtime::gameplay::PlayerSkinBinding {
            owner,
            vertices: Vec::new(),
            source_to_model: newengine_math::Mat4::IDENTITY.to_cols_array(),
        },
    );

    let casters = vec![visual];
    let baseline = shadow_skin_pose_hash(&world, &casters);
    world
        .get_mut::<newengine_gameplay_world_runtime::gameplay::PlayerSkinPose>(owner)
        .expect("skin pose")
        .revision = 2;
    let published = shadow_skin_pose_hash(&world, &casters);
    assert_ne!(
        baseline, published,
        "published skin revision must invalidate shadow geometry"
    );
    world
        .get_mut::<newengine_gameplay_world_runtime::gameplay::PlayerSkinPose>(owner)
        .expect("skin pose")
        .palette[0] =
        newengine_math::Mat4::from_translation(newengine_math::Vec3::new(0.02, 0.0, 0.0));
    let uncommitted = shadow_skin_pose_hash(&world, &casters);
    assert_eq!(
        published, uncommitted,
        "palette mutation without publication revision is outside the component contract"
    );
}

#[test]
fn skin_only_shadow_refresh_is_bounded_without_losing_pending_change() {
    assert!(dynamic_skin_shadow_refresh_due(1, true));
    assert!(!dynamic_skin_shadow_refresh_due(1, false));
    assert!(dynamic_skin_shadow_refresh_due(2, false));
    assert!(!dynamic_skin_shadow_refresh_due(3, false));
    assert!(dynamic_skin_shadow_refresh_due(4, false));
}
