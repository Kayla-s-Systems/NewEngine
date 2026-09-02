#![forbid(unsafe_op_in_unsafe_fn)]

//! Reusable FPS character input, locomotion, grounding and foot-contact mechanics.
//! No project content, mission policy, UI or runtime-profile composition is owned here.

use newengine_assets::AssetServiceClient;
use newengine_ecs::EntityId;
use newengine_engine_runtime::world_authoring::{apply_exact_material, primitive_bounds};
use newengine_materials::{MaterialFlags, MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_plugin_host::default_host_api;
use newengine_primitives::{
    fnv1a_64, Primitive, PrimitiveId, PrimitiveMesh, PrimitiveRegistry, PrimitiveVertex,
};
use newengine_scene::spawn_named;
use newengine_transform::{set_parent, Transform};

mod animation_events;
mod animation_semantic;
mod authored_presentation;
mod character_control;
mod character_physics;
mod equipment_visual;
mod impact_debris;
mod noclip;
mod player_hair;
mod player_model;
mod vfx_decal_materials;
mod weapon_animation;
mod weapon_casing;
mod weapon_grip;

mod foliage {
    pub(crate) use newengine_model_runtime::ydd_runtime::{
        decode_runtime_ydd_prefab, DecodedRuntimeYddMeshPart as DecodedPrefabMeshPart,
    };
}

mod material_source {
    pub(crate) use newengine_material_runtime::authored_registration::{
        register_required_material, register_required_material_ref,
    };
}

mod materials_terrain {
    pub(crate) use newengine_engine_runtime::world_authoring::{
        spawn_primitive as spawn_game_primitive, PrimitiveSpawnSpec,
    };
}

use self::material_source::*;
use self::materials_terrain::*;

mod presentation_policy;
mod presentation_runtime;

pub use authored_presentation::AuthoredPlayerModelSpec;
pub use character_control::apply_fps_character_commands;
pub use character_physics::{
    collect_character_queries, resolve_character_query_hits, step_character_locomotion,
    sync_physics_world_settings,
};
pub use noclip::{
    fps_noclip_enabled, set_fps_noclip, step_fps_noclip_motion, toggle_fps_noclip,
    toggle_fps_noclip_once_for_source_frame,
};

pub use player_hair::{
    bind_compiled_player_groom_v1, bind_player_nehair_v1, install_nehair_groom_v1,
    load_nehair_groom_v1,
};
pub use player_model::{spawn_authored_player_model, tick_player_model_assignments};

pub use presentation_policy::{
    reconcile_existing_player_assignments_with_policy, PlayableCharacterSelection,
};

pub use presentation_runtime::{
    install_fps_character_presentation_runtime, install_fps_character_presentation_runtime_adapter,
    FpsCharacterPresentationRuntimeAdapter, FpsCharacterPresentationRuntimeBinding,
    FpsCharacterPresentationWorldRuntimeProvider,
};

pub use animation_events::*;
pub use animation_semantic::*;
