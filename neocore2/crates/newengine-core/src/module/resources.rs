use crate::error::{EngineError, EngineResult};

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Type-safe storage for engine-local resources and module APIs.
///
/// This container is **engine-thread local** by design.
/// It intentionally allows storing !Send / !Sync values (e.g. native window handles),
/// because many platform handles are thread-affine.
///
/// If you later introduce multi-threaded systems, do not share `Resources` across threads.
/// Use explicit thread-safe APIs (Arc/Mutex/etc.) for cross-thread communication.
#[derive(Default)]
pub struct Resources {
    typed: HashMap<TypeId, Box<dyn Any>>,
    apis: HashMap<&'static str, Box<dyn Any>>,
}

impl Resources {
    /* ============================
       Typed storage (TypeId)
       ============================ */

    #[inline]
    pub fn insert<T>(&mut self, value: T)
    where
        T: Any + 'static,
    {
        self.typed.insert(TypeId::of::<T>(), Box::new(value));
    }

    #[inline]
    pub fn insert_once<T>(&mut self, value: T) -> EngineResult<()>
    where
        T: Any + 'static,
    {
        let k = TypeId::of::<T>();
        if self.typed.contains_key(&k) {
            return Err(EngineError::Other("resource already exists".to_string()));
        }
        self.typed.insert(k, Box::new(value));
        Ok(())
    }

    #[inline]
    pub fn get<T>(&self) -> Option<&T>
    where
        T: Any + 'static,
    {
        self.typed
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
    }

    #[inline]
    pub fn get_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Any + 'static,
    {
        self.typed
            .get_mut(&TypeId::of::<T>())
            .and_then(|v| v.downcast_mut::<T>())
    }

    #[inline]
    pub fn get_required<T>(&self, name: &'static str) -> EngineResult<&T>
    where
        T: Any + 'static,
    {
        self.get::<T>()
            .ok_or_else(|| EngineError::Other(format!("required resource missing: {name}")))
    }

    #[inline]
    pub fn remove<T>(&mut self) -> Option<T>
    where
        T: Any + 'static,
    {
        self.typed
            .remove(&TypeId::of::<T>())
            .and_then(|v| v.downcast::<T>().ok())
            .map(|b| *b)
    }

    #[inline]
    pub fn take_required<T>(&mut self, name: &'static str) -> EngineResult<T>
    where
        T: Any + 'static,
    {
        self.remove::<T>()
            .ok_or_else(|| EngineError::Other(format!("required resource missing: {name}")))
    }

    /* ============================
       Named APIs (string id)
       ============================ */

    #[inline]
    pub fn register_api<T>(&mut self, id: &'static str, api: T) -> EngineResult<()>
    where
        T: Any + 'static,
    {
        if self.apis.contains_key(id) {
            return Err(EngineError::Other(format!("api already registered: {id}")));
        }
        self.apis.insert(id, Box::new(api));
        Ok(())
    }

    #[inline]
    pub fn api<T>(&self, id: &'static str) -> Option<&T>
    where
        T: Any + 'static,
    {
        self.apis.get(id).and_then(|v| v.downcast_ref::<T>())
    }

    #[inline]
    pub fn api_mut<T>(&mut self, id: &'static str) -> Option<&mut T>
    where
        T: Any + 'static,
    {
        self.apis.get_mut(id).and_then(|v| v.downcast_mut::<T>())
    }

    #[inline]
    pub fn api_required<T>(&self, id: &'static str) -> EngineResult<&T>
    where
        T: Any + 'static,
    {
        self.api::<T>(id)
            .ok_or_else(|| EngineError::Other(format!("required api missing: {id}")))
    }

    #[inline]
    pub fn has_api(&self, id: &'static str) -> bool {
        self.apis.contains_key(id)
    }

    #[inline]
    pub fn unregister_api<T>(&mut self, id: &'static str) -> Option<T>
    where
        T: Any + 'static,
    {
        self.apis
            .remove(id)
            .and_then(|v| v.downcast::<T>().ok())
            .map(|b| *b)
    }
}