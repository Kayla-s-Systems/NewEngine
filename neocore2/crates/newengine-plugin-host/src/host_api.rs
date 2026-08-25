use crate::host_context::{ctx, ServiceCallLease, ServiceEntry, ServiceLifecycle};
use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{
    Blob, CapabilityId, EventSinkV1Dyn, HostApiV1, MethodName, ServiceV1Dyn,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Weak};
use std::time::Instant;

#[derive(Clone)]
struct CachedServiceDispatch {
    routed_id: String,
    service: Weak<ServiceV1Dyn<'static>>,
    lifecycle: Weak<ServiceLifecycle>,
    owner_plugin_id: Option<String>,
}

struct ResolvedServiceDispatch {
    routed_id: String,
    service: Arc<ServiceV1Dyn<'static>>,
    owner_plugin_id: Option<String>,
    _lease: ServiceCallLease,
}

impl CachedServiceDispatch {
    #[inline]
    fn acquire(&self) -> Option<ResolvedServiceDispatch> {
        let service = self.service.upgrade()?;
        let lifecycle = self.lifecycle.upgrade()?;
        let lease = lifecycle.try_acquire()?;
        Some(ResolvedServiceDispatch {
            routed_id: self.routed_id.clone(),
            service,
            owner_plugin_id: self.owner_plugin_id.clone(),
            _lease: lease,
        })
    }
}

#[derive(Default)]
struct ServiceDispatchCache {
    context_identity: usize,
    generation: u64,
    entries: HashMap<String, CachedServiceDispatch>,
}

thread_local! {
    static IN_HOST_API_LOG: Cell<bool> = const { Cell::new(false) };
    static SERVICE_DISPATCH_CACHE: RefCell<ServiceDispatchCache> = RefCell::new(ServiceDispatchCache::default());
}

#[inline]
fn debug_no_recurse(args: std::fmt::Arguments<'_>) {
    if !newengine_ulog_api::ulog::debug_enabled() {
        return;
    }
    IN_HOST_API_LOG.with(|f| {
        if f.get() {
            return;
        }
        f.set(true);
        newengine_ulog_api::ulog::debug!("{}", args);
        f.set(false);
    });
}

#[inline]
fn service_error_is_expected_rejection(error: &str) -> bool {
    let error = error.trim().to_ascii_lowercase();
    [
        "unavailable",
        "not supported",
        "unsupported",
        "disabled by configuration",
        "disabled by config",
        "capability is disabled",
    ]
    .into_iter()
    .any(|marker| error.contains(marker))
}

extern "C" fn host_log_info(s: RString) {
    newengine_ulog_api::ulog::info!("{}", s);
}

extern "C" fn host_log_warn(s: RString) {
    newengine_ulog_api::ulog::warn!("{}", s);
}

extern "C" fn host_log_error(s: RString) {
    newengine_ulog_api::ulog::error!("{}", s);
}

pub fn host_register_service_impl(svc: ServiceV1Dyn<'static>) -> RResult<(), RString> {
    let service_id = svc.id().to_string();
    let describe_json = svc.describe().to_string();
    let owner = crate::host_context::current_plugin_id();
    let owner_for_log = owner.as_deref().unwrap_or("<none>").to_string();

    if let Some(pid) = owner.as_deref() {
        match crate::host_context::plugin_declares_provided_service(pid, &service_id) {
            Some(true) => {}
            Some(false) => {
                return RResult::RErr(RString::from(format!(
                    "service not declared in descriptor: plugin='{}' service='{}'",
                    pid, service_id
                )));
            }
            None => {
                newengine_ulog_api::ulog::warn_event!(
                    "engine.services.descriptor_missing",
                    "Plugin has no descriptor; skipping capability validation",
                    {
                        "plugin_id": pid,
                        "service_id": service_id.as_str()
                    }
                );
            }
        }
    }

    let entry = ServiceEntry::new(owner.clone(), svc);
    match crate::host_context::stage_service_registration(service_id.clone(), entry.clone()) {
        Ok(true) => {
            debug_no_recurse(format_args!(
                "services: staged id='{}' owner='{}' describe_len={}",
                service_id,
                owner_for_log,
                describe_json.len()
            ));
            return RResult::ROk(());
        }
        Ok(false) => {
            if let Some(plugin_id) = owner.as_deref() {
                return RResult::RErr(RString::from(format!(
                    "plugin-owned service publication requires provider transaction: plugin='{}' service='{}'",
                    plugin_id, service_id
                )));
            }
            if let Err(error) = crate::host_context::reject_topology_mutation_from_host_callback(
                "register_service_v1",
            ) {
                return RResult::RErr(RString::from(error));
            }
        }
        Err(error) => return RResult::RErr(RString::from(error)),
    }

    let c = ctx();
    {
        let mut g = match c.services.lock() {
            Ok(v) => v,
            Err(_) => return RResult::RErr(RString::from("services mutex poisoned")),
        };
        if let Some(existing) = g.get(&service_id) {
            return RResult::RErr(RString::from(format!(
                "service already registered: {} owner='{}' contender='{}'",
                service_id,
                existing.owner_plugin_id.as_deref().unwrap_or("<none>"),
                owner_for_log,
            )));
        }
        g.insert(service_id.clone(), entry);
    }
    crate::host_context::bump_services_generation();

    debug_no_recurse(format_args!(
        "services: registered id='{}' owner='{}' describe_len={}",
        service_id,
        owner_for_log,
        describe_json.len()
    ));
    RResult::ROk(())
}

extern "C" fn host_register_service_v1_plain(svc: ServiceV1Dyn<'static>) -> RResult<(), RString> {
    host_register_service_impl(svc)
}

fn resolve_service_dispatch(requested_id: &str) -> Result<ResolvedServiceDispatch, RString> {
    loop {
        let context_identity = crate::host_context::current_host_context_identity();
        let generation = crate::host_context::services_generation();
        if generation & 1 != 0 {
            std::thread::yield_now();
            continue;
        }

        if let Some(cached) = SERVICE_DISPATCH_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.context_identity != context_identity || cache.generation != generation {
                cache.context_identity = context_identity;
                cache.generation = generation;
                cache.entries.clear();
            }
            cache.entries.get(requested_id).cloned()
        }) {
            if crate::host_context::current_host_context_identity() == context_identity
                && crate::host_context::services_generation() == generation
            {
                if let Some(resolved) = cached.acquire() {
                    return Ok(resolved);
                }
            }
            SERVICE_DISPATCH_CACHE.with(|cache| {
                cache.borrow_mut().entries.remove(requested_id);
            });
            continue;
        }

        let routed_id = crate::host_context::resolve_service_for_engine_gateway(requested_id)
            .unwrap_or_else(|| requested_id.to_owned());
        let c = ctx();
        let entry = {
            let services = c
                .services
                .lock()
                .map_err(|_| RString::from("services mutex poisoned"))?;
            services.get(&routed_id).cloned()
        };

        if crate::host_context::current_host_context_identity() != context_identity
            || crate::host_context::services_generation() != generation
        {
            continue;
        }

        let Some(entry) = entry else {
            let message = if routed_id != requested_id {
                format!("service not found: requested={requested_id} routed={routed_id}")
            } else {
                format!("service not found: {routed_id}")
            };
            return Err(RString::from(message));
        };

        let cached = CachedServiceDispatch {
            routed_id,
            service: Arc::downgrade(&entry.service),
            lifecycle: Arc::downgrade(&entry.lifecycle),
            owner_plugin_id: entry.owner_plugin_id,
        };
        SERVICE_DISPATCH_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.context_identity == context_identity
                && cache.generation == generation
                && crate::host_context::current_host_context_identity() == context_identity
                && crate::host_context::services_generation() == generation
            {
                cache
                    .entries
                    .insert(requested_id.to_owned(), cached.clone());
            }
        });
        if crate::host_context::current_host_context_identity() == context_identity
            && crate::host_context::services_generation() == generation
        {
            if let Some(resolved) = cached.acquire() {
                return Ok(resolved);
            }
        }
    }
}

pub extern "C" fn call_service_v1(
    cap_id: CapabilityId,
    method: MethodName,
    payload: Blob,
) -> RResult<Blob, RString> {
    let requested_id = cap_id.as_str();
    let dispatch = match resolve_service_dispatch(requested_id) {
        Ok(dispatch) => dispatch,
        Err(error) => return RResult::RErr(error),
    };
    let ResolvedServiceDispatch {
        routed_id: id,
        service: svc,
        owner_plugin_id: owner,
        _lease: lease,
    } = dispatch;
    let method_name = method.as_str();
    let payload_len = payload.len();
    let started = Instant::now();
    let do_call = || svc.call(method.clone(), payload);

    let res = match owner.as_deref() {
        Some(pid) => catch_unwind(AssertUnwindSafe(|| {
            crate::host_context::with_current_plugin_id(pid, do_call)
        }))
        .unwrap_or_else(|_| {
            newengine_ulog_api::ulog::error_event!(
                "engine.services.call_panicked",
                "Service call panicked; auto-unregistering owner",
                {
                    "service_id": id.as_str(),
                    "requested_id": requested_id,
                    "method": method_name,
                    "owner": pid,
                    "auto_unregister": true
                }
            );
            crate::host_context::unregister_by_owner(pid);
            RResult::RErr(RString::from("service panicked"))
        }),
        None => catch_unwind(AssertUnwindSafe(do_call)).unwrap_or_else(|_| {
            newengine_ulog_api::ulog::error_event!(
                "engine.services.call_panicked",
                "Host-owned service call panicked",
                {
                    "service_id": id.as_str(),
                    "requested_id": requested_id,
                    "method": method_name,
                    "owner": "<host>",
                    "auto_unregister": false
                }
            );
            RResult::RErr(RString::from("service panicked"))
        }),
    };

    drop(lease);
    let elapsed_ms = crate::diagnostics::elapsed_ms(started);
    match &res {
        RResult::ROk(b) => {
            if elapsed_ms >= 8.0 {
                newengine_ulog_api::ulog::debug_event!(
                    "engine.services.call_slow",
                    "Slow service call",
                    {
                        "service_id": id.as_str(),
                        "requested_id": requested_id,
                        "method": method_name,
                        "owner": owner.as_deref().unwrap_or("<host>"),
                        "payload_bytes": payload_len,
                        "output_bytes": b.len(),
                        "elapsed_ms": elapsed_ms
                    }
                );
            }
        }
        RResult::RErr(e) => {
            let error = e.to_string();
            if service_error_is_expected_rejection(&error) {
                newengine_ulog_api::ulog::warn_event!(
                    "engine.services.call_rejected",
                    "Service call rejected by provider",
                    {
                        "service_id": id.as_str(),
                        "requested_id": requested_id,
                        "method": method_name,
                        "owner": owner.as_deref().unwrap_or("<host>"),
                        "payload_bytes": payload_len,
                        "elapsed_ms": elapsed_ms,
                        "error": error.as_str(),
                        "expected_fallback": true
                    }
                );
            } else {
                newengine_ulog_api::ulog::error_event!(
                    "engine.services.call_failed",
                    "Service call returned error",
                    {
                        "service_id": id.as_str(),
                        "requested_id": requested_id,
                        "method": method_name,
                        "owner": owner.as_deref().unwrap_or("<host>"),
                        "payload_bytes": payload_len,
                        "elapsed_ms": elapsed_ms,
                        "error": error.as_str()
                    }
                );
            }
        }
    }
    res
}

extern "C" fn host_emit_event_v1(topic: RString, payload: Blob) -> RResult<(), RString> {
    match crate::host_context::emit_plugin_event(topic, payload) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

extern "C" fn host_subscribe_events_v1(sink: EventSinkV1Dyn<'static>) -> RResult<(), RString> {
    match crate::host_context::subscribe_event_sink(sink) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

pub fn default_host_api() -> HostApiV1 {
    crate::ulog_event::install_structured_ulog_sink_once();
    HostApiV1 {
        log_info: host_log_info,
        log_warn: host_log_warn,
        log_error: host_log_error,
        register_service_v1: host_register_service_v1_plain,
        call_service_v1,
        emit_event_v1: host_emit_event_v1,
        subscribe_events_v1: host_subscribe_events_v1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::sabi_trait::TD_Opaque;
    use abi_stable::std_types::{RResult, RString};
    use newengine_plugin_api::{CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};

    struct TransactionTestService(&'static str);

    impl ServiceV1 for TransactionTestService {
        fn id(&self) -> CapabilityId {
            CapabilityId::from(self.0)
        }

        fn describe(&self) -> RString {
            RString::from("{\"test\":true}")
        }

        fn call(&self, _method: MethodName, payload: Blob) -> RResult<Blob, RString> {
            RResult::ROk(payload)
        }
    }

    fn transaction_test_service(id: &'static str) -> ServiceV1Dyn<'static> {
        ServiceV1Dyn::from_value(TransactionTestService(id), TD_Opaque)
    }

    #[test]
    fn staged_host_service_is_invisible_until_atomic_commit() {
        let _context = crate::host_context::create_host_context();
        let service_id = "test.transaction.atomic-visibility";
        let generation_before = crate::host_context::services_generation();
        assert_eq!(generation_before & 1, 0);

        let transaction = crate::host_context::ProviderRegistrationTransaction::begin_host(
            "module:test.transaction.atomic-visibility",
        )
        .expect("begin host provider transaction");
        assert!(matches!(
            host_register_service_impl(transaction_test_service(service_id)),
            RResult::ROk(())
        ));

        assert!(!crate::host_context::has_service(service_id));
        assert_eq!(
            crate::host_context::services_generation(),
            generation_before,
            "staging must not publish a topology epoch"
        );

        transaction.validate().expect("validate staged provider");
        assert_eq!(transaction.commit().expect("commit staged provider"), 1);
        assert!(crate::host_context::has_service(service_id));
        assert_eq!(
            crate::host_context::services_generation(),
            generation_before + 2,
            "commit must publish exactly one stable topology epoch"
        );
    }

    #[test]
    fn provider_transaction_rollback_leaves_topology_unchanged() {
        let _context = crate::host_context::create_host_context();
        let service_id = "test.transaction.rollback";
        let generation_before = crate::host_context::services_generation();

        let transaction = crate::host_context::ProviderRegistrationTransaction::begin_host(
            "module:test.transaction.rollback",
        )
        .expect("begin host provider transaction");
        assert!(matches!(
            host_register_service_impl(transaction_test_service(service_id)),
            RResult::ROk(())
        ));
        transaction.rollback();

        assert!(!crate::host_context::has_service(service_id));
        assert_eq!(
            crate::host_context::services_generation(),
            generation_before
        );
    }

    #[test]
    fn host_module_callback_cannot_publish_topology() {
        let _context = crate::host_context::create_host_context();
        let service_id = "test.transaction.callback-forbidden";
        let generation_before = crate::host_context::services_generation();

        let result = crate::host_context::with_host_module_callback(
            "module:test.transaction.callback-forbidden",
            || host_register_service_impl(transaction_test_service(service_id)),
        );
        let error = match result {
            RResult::RErr(error) => error.to_string(),
            RResult::ROk(()) => panic!("callback topology mutation unexpectedly succeeded"),
        };
        assert!(error.contains("topology mutation is forbidden"), "{error}");
        assert!(!crate::host_context::has_service(service_id));
        assert_eq!(
            crate::host_context::services_generation(),
            generation_before
        );
    }

    #[test]
    fn unavailable_optional_backend_feature_is_expected_rejection() {
        assert!(service_error_is_expected_rejection(
            "independent multi-adapter mesh workers are unavailable"
        ));
        assert!(service_error_is_expected_rejection(
            "feature not supported by the selected adapter"
        ));
        assert!(service_error_is_expected_rejection(
            "capability is disabled"
        ));
    }

    #[test]
    fn malformed_requests_and_panics_remain_failures() {
        assert!(!service_error_is_expected_rejection("invalid payload"));
        assert!(!service_error_is_expected_rejection("service panicked"));
        assert!(!service_error_is_expected_rejection("unknown method"));
        assert!(!service_error_is_expected_rejection("unknown service"));
    }
}
