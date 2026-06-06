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
    slot: &str,
    material_ref: Option<&str>,
    materials: DemoMaterials,
    material_specs: &GameReadyMaterialSetSpec,
    palette: &GameReadyPaletteSpec,
) -> (MaterialId, [f32; 4]) {
    let role = material_ref
        .and_then(|reference| foliage_role_for_material_ref(reference, material_specs))
        .unwrap_or_else(|| {
            newengine_ulog_api::ulog::warn!(
                "game-ready foliage: .ydd mesh part material slot='{}' ref={:?} did not resolve through authored .nemat material refs; falling back to slot-name classifier",
                slot,
                material_ref
            );
            foliage_role_for_slot(slot)
        });

    match role {
        FoliageMaterialRole::Bark => (materials.tree_bark, palette.tree_bark),
        FoliageMaterialRole::Branch => (materials.tree_branch, palette.tree_branch),
        FoliageMaterialRole::Leaf => (materials.tree_leaf, palette.tree_leaf),
    }
}
