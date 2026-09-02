pub(crate) fn with_current_plugin_id<R>(plugin_id: &str, f: impl FnOnce() -> R) -> R {
    CURRENT_PLUGIN_ID.with(|c| {
        let prev = c.replace(Some(plugin_id.to_owned()));
        struct Restore<'a> {
            cell: &'a RefCell<Option<String>>,
            prev: Option<String>,
        }
        impl<'a> Drop for Restore<'a> {
            fn drop(&mut self) {
                let _ = self.cell.replace(self.prev.take());
            }
        }
        let _restore = Restore { cell: c, prev };
        f()
    })
}

pub(crate) fn current_plugin_id() -> Option<String> {
    CURRENT_PLUGIN_ID.with(|c| (*c.borrow()).clone())
}

/// Runs a host module callback in a topology-read-only scope. `Module::init()` is
/// intentionally not wrapped: it owns a ProviderRegistrationTransaction instead.
pub fn with_host_module_callback<R>(owner_id: &str, f: impl FnOnce() -> R) -> R {
    CURRENT_HOST_CALLBACK_OWNER.with(|cell| {
        let previous = cell.replace(Some(owner_id.to_owned()));
        struct Restore<'a> {
            cell: &'a RefCell<Option<String>>,
            previous: Option<String>,
        }
        impl<'a> Drop for Restore<'a> {
            fn drop(&mut self) {
                let _ = self.cell.replace(self.previous.take());
            }
        }
        let _restore = Restore { cell, previous };
        f()
    })
}

#[inline]
pub(crate) fn reject_topology_mutation_from_host_callback(operation: &str) -> Result<(), String> {
    CURRENT_HOST_CALLBACK_OWNER.with(|cell| {
        if let Some(owner) = cell.borrow().as_deref() {
            Err(format!(
                "topology mutation is forbidden during host module callback: owner='{}' operation='{}'; publish providers only through Module::init() transaction or host control plane",
                owner, operation
            ))
        } else {
            Ok(())
        }
    })
}
