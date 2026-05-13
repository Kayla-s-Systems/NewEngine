fn material_for_slot(slot: &str, materials: DemoMaterials, palette: &GameReadyPaletteSpec) -> (MaterialId, [f32; 4]) {
    let s = slot.to_ascii_lowercase();
    if s.contains("leaf") {
        (materials.tree_leaf, palette.tree_leaf)
    } else if s.contains("branch") {
        (materials.tree_branch, palette.tree_branch)
    } else {
        (materials.tree_bark, palette.tree_bark)
    }
}
