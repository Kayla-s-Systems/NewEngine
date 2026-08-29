use super::*;

fn default_pbr_material() -> RawMaterialSpec {
    RawMaterialSpec {
        roughness: default_material_roughness(),
        normal_scale: default_material_normal_scale(),
        occlusion_strength: default_material_occlusion_strength(),
        ..RawMaterialSpec::default()
    }
}

pub(in super::super) fn default_terrain_material() -> RawMaterialSpec {
    default_pbr_material()
}

pub(in super::super) fn default_sky_material() -> RawMaterialSpec {
    default_pbr_material()
}

pub(in super::super) fn default_sun_material() -> RawMaterialSpec {
    default_pbr_material()
}

pub(in super::super) fn default_moon_material() -> RawMaterialSpec {
    default_pbr_material()
}

pub(in super::super) fn default_tree_bark_material() -> RawMaterialSpec {
    default_pbr_material()
}

pub(in super::super) fn default_tree_leaf_material() -> RawMaterialSpec {
    default_pbr_material()
}

pub(in super::super) fn default_tree_branch_material() -> RawMaterialSpec {
    default_pbr_material()
}
