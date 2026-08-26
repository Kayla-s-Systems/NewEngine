/// Runtime tuning for the simple physics sphere launcher used by the GameReady FPS profile.
///
/// It is a normal ECS resource rather than a renderer/debug shortcut, so authored profiles or
/// future gameplay code can replace these values without changing the physics backend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileSphereTuning {
    pub radius: f32,
    pub speed: f32,
    pub lifetime_seconds: f32,
    pub spawn_clearance: f32,
    pub restitution: f32,
    pub friction: f32,
    pub density: f32,
}

impl Default for ProjectileSphereTuning {
    fn default() -> Self {
        // Mechanics-safe values for explicit tests/tools only. Production launcher tuning is
        // materialized from the active project's `GameDataSnapshot`.
        Self {
            radius: 0.12,
            speed: 24.0,
            lifetime_seconds: 5.0,
            spawn_clearance: 0.35,
            restitution: 0.2,
            friction: 0.4,
            density: 1.0,
        }
    }
}

impl ProjectileSphereTuning {
    #[inline]
    pub fn from_data(data: &newengine_game_data::ProjectileData) -> Self {
        Self {
            radius: data.radius,
            speed: data.speed,
            lifetime_seconds: data.lifetime_seconds,
            spawn_clearance: data.spawn_clearance,
            restitution: data.restitution,
            friction: data.friction,
            density: data.density,
        }
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        let fallback = Self::default();
        Self {
            radius: finite_or(self.radius, fallback.radius).clamp(0.03, 2.0),
            speed: finite_or(self.speed, fallback.speed).clamp(0.1, 250.0),
            lifetime_seconds: finite_or(self.lifetime_seconds, fallback.lifetime_seconds)
                .clamp(0.25, 120.0),
            spawn_clearance: finite_or(self.spawn_clearance, fallback.spawn_clearance)
                .clamp(0.05, 8.0),
            restitution: finite_or(self.restitution, fallback.restitution).clamp(0.0, 1.0),
            friction: finite_or(self.friction, fallback.friction).clamp(0.0, 2.0),
            density: finite_or(self.density, fallback.density).clamp(0.01, 1000.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileSphereRuntime {
    pub owner: EntityId,
    pub source_frame: u64,
    pub remaining_seconds: f32,
}

const WEAPON_TRACER_SPEED_MPS: f32 = 320.0;
const WEAPON_TRACER_HALF_LENGTH_M: f32 = 0.12;
const MUZZLE_FLASH_LIFETIME_SECONDS: f32 = 0.042;
const MUZZLE_CORE_LIFETIME_SECONDS: f32 = 0.030;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WeaponShotFxKind {
    MuzzleFlash,
    MuzzleCore,
    Tracer,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WeaponShotFxRuntime {
    owner: EntityId,
    shot_sequence: u64,
    kind: WeaponShotFxKind,
    origin: Vec3,
    velocity: Vec3,
    traveled: f32,
    max_distance: f32,
    remaining_seconds: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingWeaponShellEjection {
    owner: EntityId,
    shot_sequence: u64,
    weapon_item_id: u64,
    shot_origin: Vec3,
    shot_direction: Vec3,
    remaining_seconds: f32,
}
