use newengine_math::Vec3;

/// Integer world-streaming cell coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneCellCoord {
    pub x: i32,
    pub z: i32,
}

impl SceneCellCoord {
    #[inline]
    pub fn from_world_pos(pos: Vec3, cell_size_x: f32, cell_size_z: f32) -> Self {
        let sx = cell_size_x.max(1.0);
        let sz = cell_size_z.max(1.0);
        Self {
            x: (pos.x / sx).floor() as i32,
            z: (pos.z / sz).floor() as i32,
        }
    }

    #[inline]
    pub fn center(self, cell_size_x: f32, cell_size_z: f32) -> Vec3 {
        Vec3::new(
            (self.x as f32 + 0.5) * cell_size_x,
            0.0,
            (self.z as f32 + 0.5) * cell_size_z,
        )
    }

    #[inline]
    pub const fn chebyshev_distance(self, other: Self) -> i32 {
        let dx = (self.x - other.x).abs();
        let dz = (self.z - other.z).abs();
        if dx > dz {
            dx
        } else {
            dz
        }
    }

    #[inline]
    pub const fn manhattan_distance(self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.z - other.z).abs()
    }

    #[inline]
    pub const fn distance_key(self, other: Self) -> (i32, i32, i32, i32) {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        let ax = dx.abs();
        let az = dz.abs();
        let chebyshev = if ax > az { ax } else { az };
        (dx * dx + dz * dz, chebyshev, self.x, self.z)
    }
}

/// Compact radius/budget contract for scene streaming profiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneStreamingBudget {
    pub resident_radius: i32,
    pub unload_radius: i32,
    pub max_commits_per_tick: usize,
}

impl Default for SceneStreamingBudget {
    #[inline]
    fn default() -> Self {
        Self {
            resident_radius: 2,
            unload_radius: 4,
            max_commits_per_tick: 4,
        }
    }
}

impl SceneStreamingBudget {
    pub const MAX_RESIDENT_RADIUS: i32 = 8;
    pub const MAX_UNLOAD_RADIUS: i32 = 12;
    pub const MAX_COMMITS_PER_TICK: usize = 16;

    #[inline]
    pub fn sanitized(self) -> Self {
        let resident_radius = self.resident_radius.clamp(0, Self::MAX_RESIDENT_RADIUS);
        Self {
            resident_radius,
            unload_radius: self.unload_radius.clamp(
                (resident_radius + 1).max(1),
                Self::MAX_UNLOAD_RADIUS.max(resident_radius + 1),
            ),
            max_commits_per_tick: self
                .max_commits_per_tick
                .clamp(1, Self::MAX_COMMITS_PER_TICK),
        }
    }
}

/// Scene residency layer. Render residency answers "what can be drawn now".
/// Simulation residency answers "what must keep ticking even if it is invisible".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SceneResidencyLayer {
    Render,
    Simulation,
}

/// Focus point used by streaming planners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneStreamingObserver {
    pub position: Vec3,
    pub forward: Vec3,
    pub velocity: Vec3,
    pub read_ahead_seconds: f32,
}

impl SceneStreamingObserver {
    #[inline]
    pub fn at(position: Vec3) -> Self {
        Self {
            position,
            forward: Vec3::new(0.0, 0.0, 1.0),
            velocity: Vec3::ZERO,
            read_ahead_seconds: 0.0,
        }
    }

    #[inline]
    pub fn with_motion(mut self, forward: Vec3, velocity: Vec3, read_ahead_seconds: f32) -> Self {
        self.forward = forward;
        self.velocity = velocity;
        self.read_ahead_seconds = read_ahead_seconds.max(0.0);
        self
    }

    #[inline]
    pub fn focus_position(self) -> Vec3 {
        self.position + self.velocity * self.read_ahead_seconds
    }

    #[inline]
    pub fn cell(self, cell_size_x: f32, cell_size_z: f32) -> SceneCellCoord {
        SceneCellCoord::from_world_pos(self.focus_position(), cell_size_x, cell_size_z)
    }
}

/// Dual-layer scene streaming policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneStreamingProfile {
    pub render: SceneStreamingBudget,
    pub simulation: SceneStreamingBudget,
}

impl Default for SceneStreamingProfile {
    #[inline]
    fn default() -> Self {
        Self {
            render: SceneStreamingBudget::default(),
            simulation: SceneStreamingBudget {
                resident_radius: 4,
                unload_radius: 6,
                max_commits_per_tick: 2,
            },
        }
    }
}

impl SceneStreamingProfile {
    #[inline]
    pub fn sanitized(self) -> Self {
        let render = self.render.sanitized();
        let mut simulation = self.simulation.sanitized();
        if simulation.resident_radius < render.resident_radius {
            simulation.resident_radius = render.resident_radius;
        }
        if simulation.unload_radius < render.unload_radius {
            simulation.unload_radius = render.unload_radius;
        }
        Self { render, simulation }
    }
}
