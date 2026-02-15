#![forbid(unsafe_op_in_unsafe_fn)]

/// Marker trait for ECS components.
///
/// The engine assumes worlds may be shared across threads via scene bridges,
/// therefore components must be `Send + Sync`.
pub trait Component: 'static + Send + Sync {}

impl<T: 'static + Send + Sync> Component for T {}