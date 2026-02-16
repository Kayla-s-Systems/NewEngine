#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RBox, RResult, RSlice, RString, RVec};
use abi_stable::{StableAbi, sabi_trait};

use crate::desc::MathFnDesc;
use crate::value::MathValue;

#[repr(u8)]
#[derive(StableAbi, Clone, Debug)]
pub enum MathError {
    NotFound,
    Conflict(RString),
    InvalidArgs(RString),
    ProviderFailed(RString),
    NotSupported(RString),
}

pub type MathResult<T> = RResult<T, MathError>;

/// ABI-stable math function object.
///
/// Plugins should implement this and register it in a MathRegistry.
/// The function must be deterministic according to `desc().determinism`.
#[sabi_trait]
pub trait MathFn: Send + Sync {
    /// Function metadata.
    fn desc(&self) -> MathFnDesc;

    /// Call the function.
    ///
    /// Inputs and outputs must match the declared signature.
    fn call(&self, inputs: RSlice<'_, MathValue>) -> MathResult<RVec<MathValue>>;
}

/// ABI-stable registry service.
///
/// Host owns a registry instance and exposes it to plugins/modules.
/// Plugins register functions by (plugin_id, MathFn).
#[sabi_trait]
pub trait MathRegistry: Send + Sync {
    /// Register a math function provider.
    ///
    /// Conflict policy is implementation-defined, but must be deterministic.
    fn register_fn(&self, plugin_id: RString, fun: MathFn_TO<'static, RBox<()>>) -> MathResult<()>;

    /// Unregister everything that belongs to `plugin_id`.
    fn unregister_plugin(&self, plugin_id: RString) -> MathResult<()>;

    /// List all registered functions (effective set).
    fn list(&self) -> MathResult<RVec<MathFnDesc>>;

    /// Call by `id` (exact id match).
    fn call_by_id(&self, id: RString, inputs: RSlice<'_, MathValue>) -> MathResult<RVec<MathValue>>;
}