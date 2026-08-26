use newengine_ecs::{EntityId, World};
use newengine_gameplay_fps_api::{FpsActionFrame, FpsGameplayPolicySnapshot, WeaponShellCasing};
use newengine_lighting::PointLight;
use newengine_math::{Quat, Vec3};
use newengine_primitives::{builtins as prim_builtins, Primitive};
use newengine_scene::{components::Name, SceneState};
use newengine_sim::{AngularVelocity, CameraRigComp, Velocity};
use newengine_transform::Transform;

use crate::game_data::active_game_data;

use newengine_engine_runtime::gameplay::{
    play_equipped_weapon_audio, CollisionShapeDesc, DisplayVisibility, EquippedWeaponBinding,
    EquippedWeaponMuzzle, GameplayActor, ItemCatalog, ItemId, PhysicsBodyDesc, PhysicsSurface,
    PlayerCommandFrame, PlayerController, PlayerStanceState, WeaponAudioAction,
};

// Projectile facade: weapon-shot presentation and physical sphere launcher are kept separate.
include!("projectiles/types.rs");
include!("projectiles/weapon_fx.rs");
include!("projectiles/sphere_runtime.rs");
include!("projectiles/tests.rs");
