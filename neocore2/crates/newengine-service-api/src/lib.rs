#![forbid(unsafe_op_in_unsafe_fn)]

use core::fmt;

/// Generic service lifecycle method names shared by host and providers.
///
/// Domain API crates may re-export these names, but the literal is owned here
/// so teardown hooks do not drift between plugin host and services.
pub mod lifecycle_method {
    /// Optional explicit shutdown hook called before service unregister/drop.
    pub const SHUTDOWN_V1: &str = "shutdown_v1";
}

pub const SERVICE_METHOD_SHUTDOWN_V1: &str = lifecycle_method::SHUTDOWN_V1;

/// Stable identifier of a service provided through `newengine-core`'s service registry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ServiceKey(pub u128);

impl ServiceKey {
    #[inline]
    pub const fn new(v: u128) -> Self {
        Self(v)
    }
}

impl fmt::Debug for ServiceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServiceKey(0x{:032x})", self.0)
    }
}

/// Stable identifier of an interface (vtable contract) exposed by a service.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct InterfaceId(pub u128);

impl InterfaceId {
    #[inline]
    pub const fn new(v: u128) -> Self {
        Self(v)
    }
}

impl fmt::Debug for InterfaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InterfaceId(0x{:032x})", self.0)
    }
}

/// Deterministic, compile-time friendly 128-bit hash for stable IDs.
///
/// Implementation: two independent FNV-1a 64-bit hashes (different offsets), concatenated into `u128`.
/// This is *not* a crypto hash; it is used only for stable identifiers.
#[inline]
pub const fn hash_u128(s: &str) -> u128 {
    const FNV_PRIME: u64 = 1099511628211;
    const OFFSET_1: u64 = 14695981039346656037;
    const OFFSET_2: u64 = 7809847782465536322;

    let bytes = s.as_bytes();
    let mut h1 = OFFSET_1;
    let mut h2 = OFFSET_2;

    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i] as u64;
        h1 ^= b;
        h1 = h1.wrapping_mul(FNV_PRIME);

        // cheap decorrelation: fold index in second stream
        h2 ^= b.wrapping_add((i as u64).wrapping_mul(0x9e3779b97f4a7c15));
        h2 = h2.wrapping_mul(FNV_PRIME);

        i += 1;
    }

    ((h1 as u128) << 64) | (h2 as u128)
}

/// Trait implemented by typed interface wrappers in `*-api` crates.
pub trait ServiceInterface: Sized {
    type VTable;
    const INTERFACE_ID: InterfaceId;

    /// # Safety
    /// `instance` must be a valid instance pointer for the service, and `vtable` must match `Self::VTable`.
    unsafe fn from_raw(instance: *mut (), vtable: *const Self::VTable) -> Self;
}
