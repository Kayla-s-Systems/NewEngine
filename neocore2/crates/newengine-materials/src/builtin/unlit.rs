use crate::api::{MaterialDescriptor, MaterialFlags};
use crate::core::MaterialRegistry;

pub fn register(reg: &MaterialRegistry) {
    let _ = reg.register_named("Default", MaterialDescriptor::default());

    let _ = reg.register_named(
        "NeutralGrey",
        MaterialDescriptor {
            base_color: [0.55, 0.55, 0.58, 1.0],
            metallic: 0.0,
            roughness: 0.85,
            flags: MaterialFlags::NONE,
            reserved: [0; 4],
        },
    );

    let _ = reg.register_named(
        "Red",
        MaterialDescriptor {
            base_color: [0.95, 0.25, 0.25, 1.0],
            metallic: 0.0,
            roughness: 0.75,
            flags: MaterialFlags::NONE,
            reserved: [0; 4],
        },
    );
}
