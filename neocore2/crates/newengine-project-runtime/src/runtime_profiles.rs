use std::{collections::BTreeMap, path::Path};

use newengine_project_api::PROJECT_RUNTIME_PROFILE_ABI_V1;

pub type RuntimeProfileLaunchFn = fn(&Path) -> Result<(), String>;

#[derive(Clone, Copy)]
pub struct RuntimeProfileRegistration {
    pub id: &'static str,
    pub display_name: &'static str,
    pub abi: &'static str,
    pub launch: RuntimeProfileLaunchFn,
}

impl RuntimeProfileRegistration {
    #[inline]
    pub const fn new(
        id: &'static str,
        display_name: &'static str,
        launch: RuntimeProfileLaunchFn,
    ) -> Self {
        Self {
            id,
            display_name,
            abi: PROJECT_RUNTIME_PROFILE_ABI_V1,
            launch,
        }
    }
}

#[derive(Default)]
pub struct RuntimeProfileRegistry {
    registrations: BTreeMap<&'static str, RuntimeProfileRegistration>,
}

impl RuntimeProfileRegistry {
    pub fn register(&mut self, registration: RuntimeProfileRegistration) -> Result<(), String> {
        if registration.id.trim().is_empty() {
            return Err("runtime profile id must not be empty".to_owned());
        }
        if registration.abi != PROJECT_RUNTIME_PROFILE_ABI_V1 {
            return Err(format!(
                "runtime profile '{}' ABI '{}' is incompatible with host ABI '{}'",
                registration.id, registration.abi, PROJECT_RUNTIME_PROFILE_ABI_V1
            ));
        }
        if self.registrations.contains_key(registration.id) {
            return Err(format!(
                "runtime profile '{}' is already registered",
                registration.id
            ));
        }
        self.registrations.insert(registration.id, registration);
        Ok(())
    }

    #[inline]
    pub fn get(&self, id: &str) -> Option<&RuntimeProfileRegistration> {
        self.registrations.get(id.trim())
    }

    pub fn launch(&self, id: &str, manifest_path: &Path) -> Result<(), String> {
        let id = id.trim();
        let registration = self.get(id).ok_or_else(|| {
            let available = self
                .registrations
                .keys()
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "runtime profile '{id}' is not registered in this NewEngine build; available=[{available}]"
            )
        })?;
        (registration.launch)(manifest_path)
    }

    pub fn descriptors(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        self.registrations
            .values()
            .map(|registration| (registration.id, registration.display_name, registration.abi))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_op(_: &Path) -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn registry_rejects_duplicate_runtime_profile_ids() {
        let mut registry = RuntimeProfileRegistry::default();
        let registration = RuntimeProfileRegistration::new("runtime.test", "Test", no_op);
        registry.register(registration).unwrap();
        assert!(registry.register(registration).is_err());
    }
}
