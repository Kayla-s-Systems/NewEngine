#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FoliageMaterialRole {
    Bark,
    Branch,
    Leaf,
}

const FOLIAGE_SLOT_RULES: [(&str, FoliageMaterialRole); 2] = [
    ("leaf", FoliageMaterialRole::Leaf),
    ("branch", FoliageMaterialRole::Branch),
];

#[inline]
fn foliage_role_for_slot(slot: &str) -> FoliageMaterialRole {
    let slot = slot.to_ascii_lowercase();
    FOLIAGE_SLOT_RULES
        .iter()
        .find_map(|(needle, role)| slot.contains(needle).then_some(*role))
        .unwrap_or(FoliageMaterialRole::Bark)
}

fn material_for_slot(slot: &str, materials: DemoMaterials, palette: &GameReadyPaletteSpec) -> (MaterialId, [f32; 4]) {
    match foliage_role_for_slot(slot) {
        FoliageMaterialRole::Bark => (materials.tree_bark, palette.tree_bark),
        FoliageMaterialRole::Branch => (materials.tree_branch, palette.tree_branch),
        FoliageMaterialRole::Leaf => (materials.tree_leaf, palette.tree_leaf),
    }
}
