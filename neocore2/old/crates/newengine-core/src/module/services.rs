/// Engine-wide immutable services.
///
/// This is intentionally small and stable.
/// Extend via Resources if you need typed APIs.
pub trait Services: Send + Sync {
    fn logger(&self) -> &dyn log::Log;
}
