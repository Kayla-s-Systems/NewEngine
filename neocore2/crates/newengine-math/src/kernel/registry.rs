#![forbid(unsafe_op_in_unsafe_fn)]

use core::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use abi_stable::std_types::{RBox, RResult, RSlice, RString, RVec};
use log::{debug, warn};
use parking_lot::RwLock;

use crate::api::{MathError, MathFn_TO, MathRegistry, MathRegistry_TO, MathResult};
use crate::desc::MathFnDesc;
use crate::value::MathValue;

struct Provider {
    plugin_id: RString,
    desc: MathFnDesc,
    fun: MathFn_TO<'static, RBox<()>>,
    ordinal: u64,
}

impl Provider {
    fn cmp_effective(a: &Provider, b: &Provider) -> Ordering {
        // Higher version wins.
        match a.desc.version.cmp(&b.desc.version) {
            Ordering::Equal => {}
            other => return other,
        }

        // Then plugin_id lexical to make selection deterministic.
        // Smaller plugin_id should win => reverse so "max" selects it.
        match a.plugin_id.as_str().cmp(b.plugin_id.as_str()) {
            Ordering::Equal => {}
            other => return other.reverse(),
        }

        // Then ordinal (stable tie-breaker).
        a.ordinal.cmp(&b.ordinal).reverse()
    }
}

#[derive(Default)]
struct State {
    providers: BTreeMap<RString, Vec<Provider>>,
    by_plugin: BTreeMap<RString, BTreeSet<RString>>,
    ordinal: u64,
}

#[derive(Default)]
pub struct KernelMathRegistry {
    s: RwLock<State>,
}

impl KernelMathRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert registry into ABI trait object.
    ///
    /// Uses TD_Opaque to satisfy abi_stable bounds across toolchain versions.
    #[inline]
    pub fn into_abi(self) -> MathRegistry_TO<'static, RBox<()>> {
        MathRegistry_TO::from_value(self, abi_stable::type_level::downcasting::TD_Opaque)
    }

    #[inline]
    fn pick_effective<'a>(providers: &'a [Provider]) -> Option<&'a Provider> {
        providers
            .iter()
            .max_by(|a, b| Provider::cmp_effective(*a, *b))
    }

    fn validate_signature(desc: &MathFnDesc, inputs: &[MathValue]) -> Result<(), MathError> {
        let sig = &desc.signature;

        if sig.inputs.len() != inputs.len() {
            return Err(MathError::InvalidArgs(RString::from(format!(
                "arity mismatch: expected {}, got {}",
                sig.inputs.len(),
                inputs.len()
            ))));
        }

        for (i, (expected, got)) in sig.inputs.iter().zip(inputs.iter()).enumerate() {
            let got_t = got.math_type();
            if *expected != got_t {
                return Err(MathError::InvalidArgs(RString::from(format!(
                    "type mismatch at {}: expected {:?}, got {:?}",
                    i, expected, got_t
                ))));
            }
        }

        Ok(())
    }
}

impl MathRegistry for KernelMathRegistry {
    fn register_fn(&self, plugin_id: RString, fun: MathFn_TO<'static, RBox<()>>) -> MathResult<()> {
        let desc = fun.desc();

        if desc.id.is_empty() {
            warn!("math: register_fn rejected: empty id (plugin={})", plugin_id);
            return RResult::RErr(MathError::InvalidArgs(RString::from("desc.id is empty")));
        }

        debug!(
            "math: register_fn plugin={} id={} v={} inputs={} outputs={} determinism={:?} call_kind={:?}",
            plugin_id,
            desc.id,
            desc.version,
            desc.signature.inputs.len(),
            desc.signature.outputs.len(),
            desc.determinism,
            desc.call_kind
        );

        // Prepare locals BEFORE taking mutable borrows from the state.
        let id = desc.id.clone();
        let version = desc.version;

        let (replaced, providers_now, ordinal_for_provider) = {
            let mut st = self.s.write();
            st.ordinal = st.ordinal.wrapping_add(1);
            let ordinal = st.ordinal;

            // Track ownership deterministically.
            st.by_plugin
                .entry(plugin_id.clone())
                .or_default()
                .insert(id.clone());

            let list = st.providers.entry(id.clone()).or_default();

            // Replace duplicate registration from the same plugin for the same id+version.
            let mut replaced_local = false;
            list.retain(|p| {
                let keep = !(p.plugin_id == plugin_id && p.desc.version == version);
                if !keep {
                    replaced_local = true;
                }
                keep
            });

            list.push(Provider {
                plugin_id: plugin_id.clone(),
                desc,
                fun,
                ordinal,
            });

            (replaced_local, list.len(), ordinal)
        };

        if replaced {
            debug!(
                "math: register_fn replaced existing provider plugin={} id={} v={}",
                plugin_id, id, version
            );
        }

        debug!(
            "math: register_fn done id={} v={} ordinal={} providers_now={}",
            id, version, ordinal_for_provider, providers_now
        );

        RResult::ROk(())
    }

    fn unregister_plugin(&self, plugin_id: RString) -> MathResult<()> {
        let (ids_count, removed_providers) = {
            let mut st = self.s.write();

            let ids = match st.by_plugin.remove(&plugin_id) {
                Some(ids) => ids,
                None => {
                    // no-op
                    return {
                        debug!("math: unregister_plugin no-op (plugin={})", plugin_id);
                        RResult::ROk(())
                    };
                }
            };

            let ids_count = ids.len();
            let mut removed_providers: usize = 0;

            // Avoid borrowing `plugin_id` in closure directly (cleaner with clones).
            let pid = plugin_id.clone();

            for id in ids {
                if let Some(list) = st.providers.get_mut(&id) {
                    let before = list.len();
                    list.retain(|p| p.plugin_id != pid);
                    removed_providers += before.saturating_sub(list.len());

                    if list.is_empty() {
                        st.providers.remove(&id);
                    }
                }
            }

            (ids_count, removed_providers)
        };

        debug!(
        "math: unregister_plugin plugin={} ids={} removed_providers={}",
        plugin_id, ids_count, removed_providers
    );

        RResult::ROk(())
    }


    fn list(&self) -> MathResult<RVec<MathFnDesc>> {
        let st = self.s.read();
        let mut out = RVec::new();

        for (_id, providers) in st.providers.iter() {
            if let Some(p) = Self::pick_effective(providers) {
                out.push(p.desc.clone());
            }
        }

        debug!("math: list functions={}", out.len());
        RResult::ROk(out)
    }

    fn call_by_id(&self, id: RString, inputs: RSlice<'_, MathValue>) -> MathResult<RVec<MathValue>> {
        // Note: We keep the read-lock during the call to avoid cloning `MathFn_TO`,
        // which may not implement `Clone` in the current abi_stable version.
        // This is acceptable for the kernel/in-process registry. Providers should not
        // re-enter the registry from within `call` to avoid lock inversion.

        let st = self.s.read();

        let providers = match st.providers.get(&id) {
            Some(v) => v,
            None => return RResult::RErr(MathError::NotFound),
        };

        let Some(p) = Self::pick_effective(providers) else {
            return RResult::RErr(MathError::NotFound);
        };

        debug!(
        "math: call_by_id id={} -> plugin={} v={}",
        id, p.plugin_id, p.desc.version
    );

        if let Err(e) = Self::validate_signature(&p.desc, inputs.as_slice()) {
            return RResult::RErr(e);
        }

        p.fun.call(inputs)
    }
}