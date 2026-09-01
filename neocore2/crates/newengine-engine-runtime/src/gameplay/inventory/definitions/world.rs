#[derive(Clone, Debug, PartialEq)]
pub struct WorldItemDefinition {
    pub model_ref: Option<String>,
    pub material_library_ref: Option<String>,
    pub fallback_primitive: PrimitiveId,
    pub scale: [f32; 3],
    pub color: [f32; 4],
    pub pickup_half_extents: [f32; 3],
    pub respawn_seconds: f32,
}

impl WorldItemDefinition {
    pub fn for_kind(kind: ItemKind) -> Self {
        match kind {
            ItemKind::Weapon => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_CUBE,
                scale: [0.42, 0.12, 0.10],
                color: [0.22, 0.27, 0.32, 1.0],
                pickup_half_extents: [0.42, 0.12, 0.10],
                respawn_seconds: 0.0,
            },
            ItemKind::Ammo => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_CUBE,
                scale: [0.18, 0.10, 0.14],
                color: [0.72, 0.52, 0.18, 1.0],
                pickup_half_extents: [0.18, 0.10, 0.14],
                respawn_seconds: 0.0,
            },
            ItemKind::Consumable => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_CUBE,
                scale: [0.20, 0.14, 0.22],
                color: [0.74, 0.18, 0.22, 1.0],
                pickup_half_extents: [0.20, 0.14, 0.22],
                respawn_seconds: 0.0,
            },
            ItemKind::Key | ItemKind::Quest => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_TORUS,
                scale: [0.16, 0.16, 0.05],
                color: [0.25, 0.70, 0.92, 1.0],
                pickup_half_extents: [0.16, 0.16, 0.08],
                respawn_seconds: 0.0,
            },
            ItemKind::Generic | ItemKind::Component => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_SPHERE_UV,
                scale: [0.16, 0.16, 0.16],
                color: [0.48, 0.55, 0.65, 1.0],
                pickup_half_extents: [0.16, 0.16, 0.16],
                respawn_seconds: 0.0,
            },
        }
    }

    pub fn sanitized(mut self) -> Self {
        self.scale = sanitize_positive_vec3(self.scale, 0.01, 20.0);
        self.pickup_half_extents = sanitize_positive_vec3(self.pickup_half_extents, 0.01, 10.0);
        self.color = self.color.map(|value| {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                1.0
            }
        });
        self.respawn_seconds = sanitize_non_negative(self.respawn_seconds).min(86_400.0);
        self.model_ref = self
            .model_ref
            .take()
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        self.material_library_ref = self
            .material_library_ref
            .take()
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        self
    }
}

impl Default for WorldItemDefinition {
    fn default() -> Self {
        Self::for_kind(ItemKind::Generic)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldItemPresentation {
    pub visual_entity: EntityId,
    pub model_ref: Option<String>,
    pub fallback_primitive: PrimitiveId,
    pub scale: Vec3,
    pub color: [f32; 4],
    pub pickup_half_extents: Vec3,
    /// True only after the authored model/material hierarchy has been admitted.
    /// Authored items intentionally do not expose the generic fallback primitive while false.
    pub authored_visual_admitted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldItemVisualPart {
    pub owner: EntityId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldItemRuntime {
    pub persistent_id: u64,
    pub spawn_position: Vec3,
    pub original_quantity: u32,
    pub respawn_seconds: f32,
    pub respawn_remaining: f32,
    pub pickup_cooldown_remaining: f32,
    pub dropped: bool,
}

impl WorldItemRuntime {
    #[inline]
    pub fn persistent_source(
        persistent_id: u64,
        spawn_position: Vec3,
        quantity: u32,
        respawn_seconds: f32,
    ) -> Self {
        Self {
            persistent_id,
            spawn_position,
            original_quantity: quantity.max(1),
            respawn_seconds: sanitize_non_negative(respawn_seconds),
            respawn_remaining: 0.0,
            pickup_cooldown_remaining: 0.0,
            dropped: false,
        }
    }

    #[inline]
    pub fn dropped(persistent_id: u64, spawn_position: Vec3, quantity: u32) -> Self {
        Self {
            persistent_id,
            spawn_position,
            original_quantity: quantity.max(1),
            respawn_seconds: 0.0,
            respawn_remaining: 0.0,
            pickup_cooldown_remaining: 0.25,
            dropped: true,
        }
    }
}
