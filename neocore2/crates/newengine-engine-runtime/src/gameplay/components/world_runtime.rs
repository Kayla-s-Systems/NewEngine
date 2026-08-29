#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_primitives::PrimitiveMesh;

/// Generic progress resource for incremental authored-world assembly.
/// World/profile providers update it; launch/readiness code consumes it without
/// knowing which game, map format or streaming implementation owns the work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldAssemblyProgress {
    pub total: u32,
    pub completed: u32,
    pub failed: u32,
    pub pending: u32,
    pub parts: u32,
    pub triangles: u64,
}

impl WorldAssemblyProgress {
    #[inline]
    pub const fn is_ready(&self) -> bool {
        // A terminal decode/admission failure is not a playable world. The launch gate
        // must stay closed instead of treating "nothing left pending" as success.
        self.pending == 0 && self.failed == 0
    }

    #[inline]
    pub const fn total(&self) -> u32 {
        self.total
    }
    #[inline]
    pub const fn completed(&self) -> u32 {
        self.completed
    }
    #[inline]
    pub const fn failed(&self) -> u32 {
        self.failed
    }
    #[inline]
    pub const fn pending(&self) -> u32 {
        self.pending
    }

    #[inline]
    pub const fn ready(total: u32) -> Self {
        Self {
            total,
            completed: total,
            failed: 0,
            pending: 0,
            parts: 0,
            triangles: 0,
        }
    }
}

/// CPU-prepared mesh packet awaiting bounded render-provider residency/upload.
/// This is deliberately source-agnostic: terrain, voxels, generated geometry and
/// streamed world providers may all publish the same render-prep component.
#[derive(Clone, Debug)]
pub struct PreparedRenderMesh {
    pub mesh: Arc<PrimitiveMesh>,
}

impl PreparedRenderMesh {
    #[inline]
    pub fn new(mesh: Arc<PrimitiveMesh>) -> Self {
        Self { mesh }
    }
}

/// Authored/runtime scene component identifying an imported model actor.
///
/// The component contains only stable model identity. CPU mesh/material bundles and
/// GPU residency stay in the render runtime; ECS does not own renderer-native state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModelRenderComponent {
    pub logical_path: String,
}

impl ModelRenderComponent {
    #[inline]
    pub fn new(logical_path: impl Into<String>) -> Self {
        Self {
            logical_path: logical_path.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_assembly_failures_are_not_ready() {
        assert!(WorldAssemblyProgress::ready(4).is_ready());
        assert!(!WorldAssemblyProgress {
            total: 4,
            completed: 3,
            failed: 1,
            pending: 0,
            parts: 0,
            triangles: 0,
        }
        .is_ready());
    }
}
