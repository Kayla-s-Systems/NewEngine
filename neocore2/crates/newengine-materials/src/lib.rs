#![forbid(unsafe_op_in_unsafe_fn)]

use parking_lot::RwLock;
use std::sync::Arc;

/// Stable, deterministic material identifier.
///
/// We use a 64-bit id to keep the component compact and renderer-friendly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialId(pub u64);

impl MaterialId {
    #[inline]
    pub const fn invalid() -> Self {
        Self(0)
    }
}

/// Minimal forward-compatible material description.
///
/// This intentionally starts small and editor-friendly:
/// - base color factor
/// - metallic/roughness scalars (for future PBR pipeline)
///
/// Texture binding is not part of this initial step to keep contracts clean
/// until the asset pipeline exposes a stable GPU-texture handle.
#[derive(Clone, Copy, Debug)]
pub struct MaterialDesc {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
}

impl Default for MaterialDesc {
    #[inline]
    fn default() -> Self {
        Self {
            base_color: [0.85, 0.85, 0.90, 1.0],
            metallic: 0.0,
            roughness: 0.75,
        }
    }
}

/// Material reference component (entity -> registry id).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialRef {
    pub id: MaterialId,
}

impl Default for MaterialRef {
    #[inline]
    fn default() -> Self {
        Self { id: MaterialId::invalid() }
    }
}

#[derive(Clone, Debug)]
struct Entry {
    id: MaterialId,
    name: String,
    desc: MaterialDesc,
}

/// Deterministic material registry.
///
/// - No randomized hashing.
/// - Stable ids derived from names (FNV-1a 64) with collision resolution.
/// - Thread-safe via `Arc<RwLock<_>>` (editor UI reads often, updates rarely).
#[derive(Clone, Default)]
pub struct MaterialRegistry {
    inner: Arc<RwLock<Vec<Entry>>>,
}

impl MaterialRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry pre-populated with editor-friendly built-ins.
    #[inline]
    pub fn with_builtins() -> Self {
        let reg = Self::new();
        // Deterministic names.
        let _ = reg.register_named("Default", MaterialDesc::default());
        let _ = reg.register_named(
            "NeutralGrey",
            MaterialDesc {
                base_color: [0.55, 0.55, 0.58, 1.0],
                metallic: 0.0,
                roughness: 0.85,
            },
        );
        let _ = reg.register_named(
            "Red",
            MaterialDesc {
                base_color: [0.95, 0.25, 0.25, 1.0],
                metallic: 0.0,
                roughness: 0.75,
            },
        );
        reg
    }

    /// Returns ids in insertion order.
    #[inline]
    pub fn ids(&self) -> Vec<MaterialId> {
        self.inner.read().iter().map(|e| e.id).collect()
    }

    #[inline]
    pub fn name(&self, id: MaterialId) -> Option<String> {
        self.inner
            .read()
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.name.clone())
    }

    #[inline]
    pub fn get(&self, id: MaterialId) -> Option<MaterialDesc> {
        self.inner
            .read()
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.desc)
    }

    #[inline]
    pub fn set_desc(&self, id: MaterialId, desc: MaterialDesc) -> bool {
        let mut v = self.inner.write();
        if let Some(e) = v.iter_mut().find(|e| e.id == id) {
            e.desc = desc;
            true
        } else {
            false
        }
    }

    /// Registers (or returns existing) material by name.
    pub fn register_named(&self, name: &str, desc: MaterialDesc) -> MaterialId {
        // If already present by name, return it.
        {
            let v = self.inner.read();
            if let Some(e) = v.iter().find(|e| e.name == name) {
                return e.id;
            }
        }

        let mut id = MaterialId(fnv1a64(name.as_bytes()));
        if id.0 == 0 {
            id.0 = 1;
        }

        let mut v = self.inner.write();
        // Resolve collisions deterministically: linear probing on the id.
        while v.iter().any(|e| e.id == id) {
            id.0 = id.0.wrapping_add(1);
            if id.0 == 0 {
                id.0 = 1;
            }
        }

        v.push(Entry {
            id,
            name: name.to_string(),
            desc,
        });
        id
    }
}

#[inline]
fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;

    let mut h = OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}
