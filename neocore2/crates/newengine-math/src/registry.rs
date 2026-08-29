// Copyright (c) 2026 NewEngine | Take Some(). All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use crate::collections::prelude::NeHashMap;
use newengine_ulog_api::ulog::debug;
use once_cell::sync::Lazy;
use parking_lot::RwLock;

use crate::{MathError, MathResult, MathValue, MathValueType, Signature};

/// Human-readable identifier of a math function.
///
/// Convention: reverse-dns + version + path, e.g. `"k-sys.math.vec3.dot.v1"`.
pub type MathFnId = Arc<str>;

/// Identifier of a provider (engine module / plugin).
pub type ProviderId = Arc<str>;

/// A dynamically invokable math function.
pub trait DynMathFn: Send + Sync + 'static {
    fn id(&self) -> &str;
    fn signature(&self) -> &Signature;
    fn invoke(&self, args: &[MathValue]) -> MathResult<MathValue>;
}

#[inline]
fn push_unique<T: PartialEq>(v: &mut Vec<T>, item: T) {
    if !v.contains(&item) {
        v.push(item);
    }
}

/// Helper trait for concise registration.
pub trait RegisterMathFn {
    fn register_dyn(&self, provider: ProviderId, f: Arc<dyn DynMathFn>) -> MathResult<()>;
}

impl RegisterMathFn for MathRegistry {
    #[inline]
    fn register_dyn(&self, provider: ProviderId, f: Arc<dyn DynMathFn>) -> MathResult<()> {
        self.register(provider, f)
    }
}

#[derive(Default)]
struct RegistryState {
    /// `id -> providers` in priority order (latest wins unless priority is introduced).
    providers: NeHashMap<MathFnId, Vec<ProviderEntry>>,
    /// `provider -> ids` for fast bulk removal.
    by_provider: NeHashMap<ProviderId, Vec<MathFnId>>,
}

#[derive(Clone)]
struct ProviderEntry {
    provider: ProviderId,
    fun: Arc<dyn DynMathFn>,
}

/// Engine-wide math registry.
///
/// This is intentionally type-erased to allow dynamic extension from plugins.
#[derive(Default)]
pub struct MathRegistry {
    st: RwLock<RegistryState>,
}

pub type MathRegistryRef = &'static MathRegistry;

static GLOBAL: Lazy<MathRegistry> = Lazy::new(MathRegistry::default);

impl MathRegistry {
    /// Global registry instance.
    #[inline]
    pub fn global() -> MathRegistryRef {
        &GLOBAL
    }

    /// Registers multiple functions for the same provider.
    #[inline]
    pub fn register_many<I>(&self, provider: ProviderId, funs: I) -> MathResult<()>
    where
        I: IntoIterator<Item = Arc<dyn DynMathFn>>,
    {
        for f in funs {
            self.register(provider.clone(), f)?;
        }
        Ok(())
    }

    /// Registers a function implementation under `id`.
    ///
    /// If another implementation exists, this one becomes the new "active" one.
    pub fn register(&self, provider: ProviderId, fun: Arc<dyn DynMathFn>) -> MathResult<()> {
        let id: MathFnId = Arc::<str>::from(fun.id());
        let sig = fun.signature().clone();

        let mut st = self.st.write();

        // Contract enforcement: a given id must keep a single, stable signature.
        // Allow replacing implementations, but reject attempts to "overload" the same id
        // with a different signature.
        if let Some(expected) = st
            .providers
            .get(&id)
            .and_then(|list| list.first().map(|e| e.fun.signature().clone()))
        {
            if expected != sig {
                return Err(MathError::SignatureConflict {
                    id: id.to_string(),
                    expected,
                    got: sig,
                    provider: provider.to_string(),
                });
            }
        }

        // Track ids per provider without duplicates.
        push_unique(
            st.by_provider.entry(provider.clone()).or_default(),
            id.clone(),
        );

        let list = st.providers.entry(id.clone()).or_default();
        let prev_active_provider = list.last().map(|e| e.provider.clone());

        // If the same provider registers the same id again, replace its implementation
        // and move it to the end so it becomes active.
        let replaced_same_provider =
            if let Some(pos) = list.iter().position(|e| e.provider == provider) {
                let _old = list.remove(pos);
                list.push(ProviderEntry {
                    provider: provider.clone(),
                    fun,
                });
                true
            } else {
                list.push(ProviderEntry {
                    provider: provider.clone(),
                    fun,
                });
                false
            };

        drop(st);

        match (replaced_same_provider, prev_active_provider) {
            (true, Some(prev)) if prev.as_ref() != provider.as_ref() => {
                debug!(
                    target: "newengine_math::registry",
                    "math.register id='{}' provider='{}' replaced=true prev_active='{}' sig={:?}",
                    id,
                    provider,
                    prev,
                    sig
                );
            }
            (true, _) => {
                debug!(
                    target: "newengine_math::registry",
                    "math.register id='{}' provider='{}' replaced=true sig={:?}",
                    id,
                    provider,
                    sig
                );
            }
            (false, Some(prev)) if prev.as_ref() != provider.as_ref() => {
                debug!(
                    target: "newengine_math::registry",
                    "math.register id='{}' provider='{}' override=true prev_active='{}' sig={:?}",
                    id,
                    provider,
                    prev,
                    sig
                );
            }
            _ => {
                debug!(
                    target: "newengine_math::registry",
                    "math.register id='{}' provider='{}' sig={:?}",
                    id,
                    provider,
                    sig
                );
            }
        }

        Ok(())
    }

    /// Removes all implementations registered by `provider`.
    ///
    /// Returns `(ids_count, removed_impls)`.
    pub fn unregister_provider(&self, provider: ProviderId) -> (usize, usize) {
        let mut st = self.st.write();

        let Some(ids) = st.by_provider.remove(&provider) else {
            return (0, 0);
        };

        let ids_count = ids.len();
        let mut removed_impls = 0usize;

        for id in ids {
            if let Some(list) = st.providers.get_mut(&id) {
                let before = list.len();
                list.retain(|e| e.provider.as_ref() != provider.as_ref());
                removed_impls += before.saturating_sub(list.len());

                if list.is_empty() {
                    st.providers.remove(&id);
                }
            }
        }

        debug!(
            target: "newengine_math::registry",
            "math.unregister_provider provider='{}' ids={} removed_impls={}",
            provider,
            ids_count,
            removed_impls
        );

        (ids_count, removed_impls)
    }

    /// Returns the active function implementation for `id`.
    #[inline]
    pub fn get(&self, id: &str) -> Option<Arc<dyn DynMathFn>> {
        let st = self.st.read();
        let key: MathFnId = Arc::<str>::from(id);
        st.providers
            .get(&key)
            .and_then(|v| v.last())
            .map(|e| e.fun.clone())
    }

    /// Invokes a registered function by `id`.
    pub fn call(&self, id: &str, args: &[MathValue]) -> MathResult<MathValue> {
        let f = self
            .get(id)
            .ok_or_else(|| MathError::NotFound { id: id.to_string() })?;

        // Validate signature.
        let sig = f.signature();
        if sig.inputs.len() != args.len() {
            return Err(MathError::InvalidArgs {
                expected: sig.clone(),
                got: arg_types(args),
                arg_index: None,
            });
        }

        for (i, (exp, got)) in sig
            .inputs
            .iter()
            .zip(args.iter().map(MathValue::ty))
            .enumerate()
        {
            if *exp != got {
                return Err(MathError::InvalidArgs {
                    expected: sig.clone(),
                    got: arg_types(args),
                    arg_index: Some(i),
                });
            }
        }

        let out = f.invoke(args)?;
        if out.ty() != sig.output {
            return Err(MathError::ProviderError {
                id: id.to_string(),
                message: format!(
                    "signature mismatch: expected {:?}, got {:?}",
                    sig.output,
                    out.ty()
                ),
            });
        }

        Ok(out)
    }

    /// Snapshot of all registered ids and their active signatures.
    pub fn snapshot(&self) -> Vec<(String, Signature, ProviderId)> {
        let st = self.st.read();
        let mut out = Vec::with_capacity(st.providers.len());

        for (id, entries) in st.providers.iter() {
            if let Some(active) = entries.last() {
                out.push((
                    id.to_string(),
                    active.fun.signature().clone(),
                    active.provider.clone(),
                ));
            }
        }

        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

/// Small helper to build typed wrappers around dynamic functions.
pub fn arg_types(args: &[MathValue]) -> Vec<MathValueType> {
    args.iter().map(MathValue::ty).collect()
}
