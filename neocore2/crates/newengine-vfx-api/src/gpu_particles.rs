use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

pub const DEFAULT_VFX_GPU_PARTICLE_BRIDGE_CAPACITY: usize = 262_144;
pub const DEFAULT_VFX_GPU_PARTICLE_KILL_CAPACITY: usize = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum VfxGpuParticleKind {
    Smoke = 1,
    Spark = 2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxGpuParticleSpawnV1 {
    pub instance_id: u64,
    pub kind: VfxGpuParticleKind,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub acceleration: [f32; 3],
    pub size: [f32; 2],
    pub growth_per_second: [f32; 2],
    pub color: [f32; 4],
    pub lifetime_seconds: f32,
    pub fade_start_fraction: f32,
}

impl VfxGpuParticleSpawnV1 {
    pub fn normalized(mut self) -> Option<Self> {
        if !self.position.iter().all(|v| v.is_finite())
            || !self.velocity.iter().all(|v| v.is_finite())
            || !self.acceleration.iter().all(|v| v.is_finite())
            || !self.size.iter().all(|v| v.is_finite())
            || !self.growth_per_second.iter().all(|v| v.is_finite())
            || !self.color.iter().all(|v| v.is_finite())
            || !self.lifetime_seconds.is_finite()
            || self.lifetime_seconds <= 0.0
        {
            return None;
        }
        self.size = self.size.map(|v| v.clamp(0.0001, 10_000.0));
        self.growth_per_second = self.growth_per_second.map(|v| v.clamp(-10_000.0, 10_000.0));
        self.lifetime_seconds = self.lifetime_seconds.clamp(0.001, 3_600.0);
        self.fade_start_fraction = if self.fade_start_fraction.is_finite() {
            self.fade_start_fraction.clamp(0.0, 0.999)
        } else {
            0.5
        };
        self.color[3] = self.color[3].clamp(0.0, 1.0);
        Some(self)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VfxGpuParticleBridgeStats {
    pub pending_spawns: u32,
    pub pending_kills: u32,
    pub dropped_spawns: u64,
    pub dropped_kills: u64,
}

#[derive(Debug)]
struct VfxGpuParticleBridgeInner {
    spawn_capacity: usize,
    kill_capacity: usize,
    spawns: VecDeque<VfxGpuParticleSpawnV1>,
    kills: VecDeque<u64>,
    dropped_spawns: u64,
    dropped_kills: u64,
}

impl VfxGpuParticleBridgeInner {
    fn new(spawn_capacity: usize, kill_capacity: usize) -> Self {
        Self {
            spawn_capacity: spawn_capacity.clamp(1, 1_000_000),
            kill_capacity: kill_capacity.clamp(1, 65_536),
            spawns: VecDeque::new(),
            kills: VecDeque::new(),
            dropped_spawns: 0,
            dropped_kills: 0,
        }
    }
}

#[derive(Debug)]
pub struct VfxGpuParticleBridge {
    inner: Mutex<VfxGpuParticleBridgeInner>,
}

impl Default for VfxGpuParticleBridge {
    fn default() -> Self {
        Self::with_capacity(
            DEFAULT_VFX_GPU_PARTICLE_BRIDGE_CAPACITY,
            DEFAULT_VFX_GPU_PARTICLE_KILL_CAPACITY,
        )
    }
}

impl VfxGpuParticleBridge {
    pub fn with_capacity(spawn_capacity: usize, kill_capacity: usize) -> Self {
        Self {
            inner: Mutex::new(VfxGpuParticleBridgeInner::new(
                spawn_capacity,
                kill_capacity,
            )),
        }
    }

    pub fn enqueue_spawn(&self, spawn: VfxGpuParticleSpawnV1) -> bool {
        let Some(spawn) = spawn.normalized() else {
            let mut inner = self.lock();
            inner.dropped_spawns = inner.dropped_spawns.saturating_add(1);
            return false;
        };
        let mut inner = self.lock();
        if inner.spawns.len() >= inner.spawn_capacity {
            inner.dropped_spawns = inner.dropped_spawns.saturating_add(1);
            return false;
        }
        inner.spawns.push_back(spawn);
        true
    }

    pub fn enqueue_kill_instance(&self, instance_id: u64) -> bool {
        if instance_id == 0 {
            return false;
        }
        let mut inner = self.lock();
        inner
            .spawns
            .retain(|spawn| spawn.instance_id != instance_id);
        if inner.kills.len() >= inner.kill_capacity {
            inner.dropped_kills = inner.dropped_kills.saturating_add(1);
            return false;
        }
        inner.kills.push_back(instance_id);
        true
    }

    pub fn drain_spawns(&self, max: usize) -> Vec<VfxGpuParticleSpawnV1> {
        let mut inner = self.lock();
        let count = max.min(inner.spawns.len());
        inner.spawns.drain(..count).collect()
    }

    pub fn drain_kills(&self, max: usize) -> Vec<u64> {
        let mut inner = self.lock();
        let count = max.min(inner.kills.len());
        inner.kills.drain(..count).collect()
    }

    pub fn stats(&self) -> VfxGpuParticleBridgeStats {
        let inner = self.lock();
        VfxGpuParticleBridgeStats {
            pending_spawns: inner.spawns.len().min(u32::MAX as usize) as u32,
            pending_kills: inner.kills.len().min(u32::MAX as usize) as u32,
            dropped_spawns: inner.dropped_spawns,
            dropped_kills: inner.dropped_kills,
        }
    }

    fn lock(&self) -> MutexGuard<'_, VfxGpuParticleBridgeInner> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn(instance_id: u64) -> VfxGpuParticleSpawnV1 {
        VfxGpuParticleSpawnV1 {
            instance_id,
            kind: VfxGpuParticleKind::Smoke,
            position: [0.0; 3],
            velocity: [0.0; 3],
            acceleration: [0.0; 3],
            size: [0.1, 0.2],
            growth_per_second: [0.0; 2],
            color: [1.0; 4],
            lifetime_seconds: 1.0,
            fade_start_fraction: 0.5,
        }
    }

    #[test]
    fn bridge_is_bounded_and_fifo() {
        let bridge = VfxGpuParticleBridge::with_capacity(2, 2);
        assert!(bridge.enqueue_spawn(spawn(1)));
        assert!(bridge.enqueue_spawn(spawn(2)));
        assert!(!bridge.enqueue_spawn(spawn(3)));
        let drained = bridge.drain_spawns(8);
        assert_eq!(
            drained.iter().map(|it| it.instance_id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(bridge.stats().dropped_spawns, 1);
    }

    #[test]
    fn kill_queue_is_bounded() {
        let bridge = VfxGpuParticleBridge::with_capacity(1, 1);
        assert!(bridge.enqueue_spawn(spawn(7)));
        assert!(bridge.enqueue_kill_instance(7));
        assert_eq!(bridge.stats().pending_spawns, 0);
        assert!(!bridge.enqueue_kill_instance(8));
        assert_eq!(bridge.drain_kills(4), vec![7]);
        assert_eq!(bridge.stats().dropped_kills, 1);
    }
}
