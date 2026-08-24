use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FoliageMaterialRole {
    Bark,
    Branch,
    Leaf,
}

pub(super) const FOLIAGE_SLOT_RULES: [(&str, FoliageMaterialRole); 2] = [
    ("leaf", FoliageMaterialRole::Leaf),
    ("branch", FoliageMaterialRole::Branch),
];

#[inline]
pub(super) fn foliage_role_for_slot(slot: &str) -> FoliageMaterialRole {
    let slot = slot.to_ascii_lowercase();
    FOLIAGE_SLOT_RULES
        .iter()
        .find_map(|(needle, role)| slot.contains(needle).then_some(*role))
        .unwrap_or(FoliageMaterialRole::Bark)
}

#[inline]
pub(super) fn canonical_material_ref(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

pub(super) fn foliage_role_for_material_ref(
    material_ref: &str,
    material_specs: &GameReadyMaterialSetSpec,
) -> Option<FoliageMaterialRole> {
    let material_ref = canonical_material_ref(material_ref);
    let matches = |candidate: &Option<String>| {
        candidate
            .as_deref()
            .map(canonical_material_ref)
            .map(|candidate| candidate == material_ref)
            .unwrap_or(false)
    };
    if matches(&material_specs.tree_bark.asset) {
        return Some(FoliageMaterialRole::Bark);
    }
    if matches(&material_specs.tree_branch.asset) {
        return Some(FoliageMaterialRole::Branch);
    }
    if matches(&material_specs.tree_leaf.asset) {
        return Some(FoliageMaterialRole::Leaf);
    }
    None
}

pub(super) fn material_for_slot(
    mats: &MaterialRegistry,
    slot: &str,
    material_ref: Option<&str>,
    materials: DemoMaterials,
    material_specs: &GameReadyMaterialSetSpec,
    palette: &GameReadyPaletteSpec,
) -> (MaterialId, [f32; 4]) {
    // YDD material selectors are authoritative. This is required for imported
    // SpeedTree assets: the generated branch/leaf/atlas parts must keep their own
    // NEMAT/YTD chain rather than being silently rebound to the map's generic tree
    // materials.
    if let Some(reference) = material_ref.filter(|value| is_nemat_entry_ref(value)) {
        if let Some(mut response) = load_material_descriptor_asset(reference) {
            response.descriptor.sanitize_in_place();
            let material_name = if response.name.trim().is_empty() {
                format!("foliage:{}", slot)
            } else {
                response.name
            };
            let id = mats.upsert_named_with_textures(
                &material_name,
                response.descriptor,
                response.textures.sanitized(),
            );
            return (id, [1.0, 1.0, 1.0, 1.0]);
        }
        newengine_ulog_api::ulog::warn!(
            "game-ready foliage material exact selector unresolved ref='{}' slot='{}'; using role fallback",
            reference,
            slot,
        );
    }

    let role = material_ref
        .and_then(|reference| foliage_role_for_material_ref(reference, material_specs))
        .unwrap_or_else(|| foliage_role_for_slot(slot));

    match role {
        FoliageMaterialRole::Bark => (materials.tree_bark, palette.tree_bark),
        FoliageMaterialRole::Branch => (materials.tree_branch, palette.tree_branch),
        FoliageMaterialRole::Leaf => (materials.tree_leaf, palette.tree_leaf),
    }
}
