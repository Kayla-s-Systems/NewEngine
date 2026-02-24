#![forbid(unsafe_op_in_unsafe_fn)]

use core::fmt;
use newengine_math::collections::prelude::*;

use crate::{PrimitiveId, PrimitiveMesh};

/// Builder signature for a primitive mesh.
///
/// Keep it pure and deterministic: no RNG, no global time, no IO.
pub type PrimitiveBuildFn = fn(&PrimitiveParams) -> PrimitiveMesh;

/// Common parameters for built-in primitives.
///
/// Not every field is used by every primitive. Builders are expected to treat
/// zero values as "use a sensible default".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrimitiveParams {
    /// Generic angular tessellation (e.g. circle/cylinder).
    pub segments: u32,
    /// Generic rings count (e.g. capsule hemispheres).
    pub rings: u32,
    /// UV-sphere slices.
    pub slices: u32,
    /// UV-sphere stacks.
    pub stacks: u32,
    /// Plane/grid subdivisions per axis.
    pub subdivisions: u32,
    /// Torus major ring segments.
    pub major_segments: u32,
    /// Torus minor ring segments.
    pub minor_segments: u32,
}

/// Primitive metadata for editor/UI.
#[derive(Clone, Debug)]
pub struct PrimitiveDesc {
    pub id: PrimitiveId,
    pub name: &'static str,
    pub defaults: PrimitiveParams,
}

#[derive(Debug, Clone)]
pub struct PrimitiveBuildError {
    pub id: PrimitiveId,
}

impl fmt::Display for PrimitiveBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "primitive mesh builder not registered: {:?}", self.id)
    }
}

impl std::error::Error for PrimitiveBuildError {}

#[derive(Clone, Debug)]
struct Entry {
    name: &'static str,
    defaults: PrimitiveParams,
    build: PrimitiveBuildFn,
}

/// Deterministic primitive registry.
///
/// - No globals (callers keep it in engine state / services)
/// - Plugins can register new primitives at runtime
/// - Built-ins are just registrations
#[derive(Clone, Debug, Default)]
pub struct PrimitiveRegistry {
    entries: NeHashMap<PrimitiveId, Entry>,
}

impl PrimitiveRegistry {
    #[inline]
    pub fn new() -> Self {
        Self {
            entries: NeHashMap::default(),
        }
    }

    /// Convenience: create and register built-ins.
    #[inline]
    pub fn with_builtins() -> Self {
        let mut r = Self::new();
        crate::builtins::register(&mut r);
        r
    }

    /// Register (or override) a primitive builder.
    ///
    /// Overriding is allowed intentionally:
    /// - for editor/dev tools
    /// - for platform-specific mesh variants
    /// If you want to forbid overrides, add a `try_register` method.
    #[inline]
    pub fn register(
        &mut self,
        id: PrimitiveId,
        name: &'static str,
        defaults: PrimitiveParams,
        build: PrimitiveBuildFn,
    ) {
        self.entries.insert(
            id,
            Entry {
                name,
                defaults,
                build,
            },
        );
    }

    #[inline]
    pub fn is_registered(&self, id: PrimitiveId) -> bool {
        self.entries.contains_key(&id)
    }

    #[inline]
    pub fn name(&self, id: PrimitiveId) -> Option<&'static str> {
        self.entries.get(&id).map(|e| e.name)
    }

    #[inline]
    pub fn desc(&self, id: PrimitiveId) -> Option<PrimitiveDesc> {
        self.entries.get(&id).map(|e| PrimitiveDesc {
            id,
            name: e.name,
            defaults: e.defaults,
        })
    }

    /// Build mesh by id.
    #[inline]
    pub fn build_mesh(&self, id: PrimitiveId) -> Result<PrimitiveMesh, PrimitiveBuildError> {
        let e = self.entries.get(&id).ok_or(PrimitiveBuildError { id })?;
        Ok((e.build)(&e.defaults))
    }

    /// Build mesh by id with explicit parameters.
    #[inline]
    pub fn build_mesh_with(
        &self,
        id: PrimitiveId,
        params: &PrimitiveParams,
    ) -> Result<PrimitiveMesh, PrimitiveBuildError> {
        let e = self.entries.get(&id).ok_or(PrimitiveBuildError { id })?;
        Ok((e.build)(params))
    }

    /// Enumerate IDs (stable ordering can be obtained by sorting externally).
    #[inline]
    pub fn ids(&self) -> impl Iterator<Item=PrimitiveId> + '_ {
        self.entries.keys().copied()
    }
}
