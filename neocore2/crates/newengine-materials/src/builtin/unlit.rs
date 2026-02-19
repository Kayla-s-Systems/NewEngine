use crate::api::{MaterialDescriptor, MaterialDomain, MaterialFlags, ShadingModel};
use crate::core::MaterialRegistry;

pub fn register(reg: &MaterialRegistry) {
    let _ = reg.register_named("Default", MaterialDescriptor::default());

    let _ = reg.register_named(
        "NeutralGrey",
        MaterialDescriptor {
            domain: MaterialDomain::Surface,
            shading_model: ShadingModel::Unlit,
            base_color: [0.55, 0.55, 0.58, 1.0],
            emissive: [0.0, 0.0, 0.0],
            metallic: 0.0,
            roughness: 0.85,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            alpha_cutoff: 0.5,
            flags: MaterialFlags::NONE,
            reserved: [0; 2],
        },
    );

    let _ = reg.register_named(
        "Red",
        MaterialDescriptor {
            domain: MaterialDomain::Surface,
            shading_model: ShadingModel::Unlit,
            base_color: [0.95, 0.25, 0.25, 1.0],
            emissive: [0.0, 0.0, 0.0],
            metallic: 0.0,
            roughness: 0.75,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            alpha_cutoff: 0.5,
            flags: MaterialFlags::NONE,
            reserved: [0; 2],
        },
    );
}
