#![forbid(unsafe_op_in_unsafe_fn)]

use core::fmt;
use core::ptr::NonNull;

use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use newengine_service_api::{InterfaceId, ServiceInterface, ServiceKey};

/// Missing-service behavior policy.
///
/// Defaults should always favor degradation over death.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingServicePolicy {
    /// Log `warn!` (rate-limited) and continue.
    Optional,
    /// Log `error!` (rate-limited) and disable the dependent feature.
    RequiredSoft,
    /// Truly fatal (avoid unless kernel cannot proceed).
    RequiredHard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MissingKey {
    service: ServiceKey,
    iface: InterfaceId,
}

#[derive(Debug)]
struct MissingState {
    last_logged_at: Instant,
    count: u64,
}

type DropFn = unsafe fn(*mut ());
type QueryFn = unsafe fn(*mut (), InterfaceId) -> *const ();

/// Type-erased service instance stored by the host.
///
/// # Safety contract
/// - `instance` must remain valid until `drop_fn` is called.
/// - `query_fn` must return a valid vtable pointer for the given interface id, or null.
#[repr(C)]
pub struct ErasedService {
    instance: NonNull<()>,
    drop_fn: DropFn,
    query_fn: QueryFn,
}

impl fmt::Debug for ErasedService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErasedService")
            .field("instance", &self.instance)
            .finish_non_exhaustive()
    }
}

unsafe impl Send for ErasedService {}
unsafe impl Sync for ErasedService {}

impl ErasedService {
    #[inline]
    pub fn new(instance: *mut (), drop_fn: DropFn, query_fn: QueryFn) -> Self {
        let instance = NonNull::new(instance).expect("ErasedService instance must not be null");
        Self {
            instance,
            drop_fn,
            query_fn,
        }
    }

    #[inline]
    pub fn instance_ptr(&self) -> *mut () {
        self.instance.as_ptr()
    }

    #[inline]
    pub fn query_interface_vtable(&self, interface_id: InterfaceId) -> *const () {
        unsafe { (self.query_fn)(self.instance.as_ptr(), interface_id) }
    }
}

impl Drop for ErasedService {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.instance.as_ptr()) };
    }
}

#[derive(Debug)]
pub struct ServiceRegistry {
    services: RwLock<HashMap<ServiceKey, ErasedService>>,
    missing: Mutex<HashMap<MissingKey, MissingState>>,
    missing_log_cooldown: Duration,
}

impl Default for ServiceRegistry {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceRegistry {
    #[inline]
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
            missing: Mutex::new(HashMap::new()),
            missing_log_cooldown: Duration::from_secs(2),
        }
    }

    /// Registers (or replaces) a service singleton.
    ///
    /// Replacing a service is allowed and will drop the previous instance.
    #[inline]
    pub fn register(&self, key: ServiceKey, service: ErasedService) {
        self.services.write().insert(key, service);
    }

    #[inline]
    pub fn contains(&self, key: ServiceKey) -> bool {
        self.services.read().contains_key(&key)
    }

    /// Queries a typed interface from a service.
    ///
    /// This function never returns references into the registry and never leaks locks.
    /// It either returns a lightweight typed wrapper or `None` with rate-limited logging.
    #[inline]
    pub fn require_interface<T: ServiceInterface>(
        &self,
        service: ServiceKey,
        policy: MissingServicePolicy,
    ) -> Option<T> {
        let map = self.services.read();

        let svc = match map.get(&service) {
            Some(s) => s,
            None => {
                drop(map);
                self.log_missing(service, T::INTERFACE_ID, policy);
                return None;
            }
        };

        let vtbl = svc.query_interface_vtable(T::INTERFACE_ID);
        if vtbl.is_null() {
            drop(map);
            self.log_missing(service, T::INTERFACE_ID, policy);
            return None;
        }

        let vtbl_t = vtbl as *const T::VTable;
        Some(unsafe { T::from_raw(svc.instance_ptr(), vtbl_t) })
    }

    #[inline]
    fn log_missing(&self, service: ServiceKey, iface: InterfaceId, policy: MissingServicePolicy) {
        if policy == MissingServicePolicy::RequiredHard {
            log::error!("fatal: missing service={:?} interface={:?}", service, iface);
            return;
        }

        let key = MissingKey { service, iface };
        let mut miss = self.missing.lock();
        let now = Instant::now();

        match miss.get_mut(&key) {
            Some(state) => {
                state.count = state.count.saturating_add(1);
                if now.duration_since(state.last_logged_at) < self.missing_log_cooldown {
                    return;
                }
                state.last_logged_at = now;
            }
            None => {
                miss.insert(
                    key,
                    MissingState {
                        last_logged_at: now,
                        count: 1,
                    },
                );
            }
        }

        match policy {
            MissingServicePolicy::Optional => {
                log::warn!(
                    "missing optional service={:?} interface={:?}",
                    service,
                    iface
                )
            }
            MissingServicePolicy::RequiredSoft => log::error!(
                "missing required service={:?} interface={:?} (soft)",
                service,
                iface
            ),
            MissingServicePolicy::RequiredHard => {}
        }
    }

    #[inline]
    pub fn missing_count(&self, service: ServiceKey, iface: InterfaceId) -> u64 {
        self.missing
            .lock()
            .get(&MissingKey { service, iface })
            .map(|s| s.count)
            .unwrap_or(0)
    }

    /// Optional tuning for log rate-limiting.
    #[inline]
    pub fn set_missing_log_cooldown(&mut self, cooldown: Duration) {
        self.missing_log_cooldown = cooldown;
    }
}
