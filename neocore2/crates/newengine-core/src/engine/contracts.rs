use super::Engine;

use crate::error::{EngineError, EngineResult};
use crate::module::ApiVersion;

use newengine_math::collections_prelude::NeHashMap as HashMap;

impl<E: Send + 'static> Engine<E> {
    pub(super) fn validate_api_contracts_strict(&self) -> EngineResult<()> {
        let mut provided: HashMap<&'static str, ApiVersion> = HashMap::default();
        let mut provider: HashMap<&'static str, &'static str> = HashMap::default();

        for s in self.modules.iter() {
            let m = s.module.as_ref();
            for p in m.provides().iter() {
                match provided.get(p.id) {
                    Some(v) if *v >= p.version => {}
                    _ => {
                        provided.insert(p.id, p.version);
                        provider.insert(p.id, m.id());
                    }
                }
            }
        }

        for s in self.modules.iter() {
            let m = s.module.as_ref();
            for r in m.requires().iter() {
                let Some(have) = provided.get(r.id) else {
                    return Err(EngineError::Other(format!(
                        "module '{}' requires API '{}' >= {}.{}.{} but it is not provided",
                        m.id(),
                        r.id,
                        r.min_version.major,
                        r.min_version.minor,
                        r.min_version.patch,
                    )));
                };

                if *have < r.min_version {
                    let prov = provider.get(r.id).copied().unwrap_or("<unknown>");
                    return Err(EngineError::Other(format!(
                        "module '{}' requires API '{}' >= {}.{}.{} but provider '{}' offers {}.{}.{}",
                        m.id(),
                        r.id,
                        r.min_version.major,
                        r.min_version.minor,
                        r.min_version.patch,
                        prov,
                        have.major,
                        have.minor,
                        have.patch,
                    )));
                }
            }
        }

        Ok(())
    }
}
