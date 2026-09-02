fn compact_indexed_vertex_streams(
    positions: &[[f32; 4]],
    uv0: &[[f32; 4]],
    source_indices: &[u32],
    mesh_name: &str,
) -> Result<(Vec<[f32; 4]>, Vec<[f32; 4]>, Vec<u32>, Vec<usize>), String> {
    if positions.len() != uv0.len() {
        return Err(format!(
            "vertex stream length mismatch mesh='{mesh_name}' positions={} uv0={}",
            positions.len(),
            uv0.len()
        ));
    }
    let mut referenced = vec![false; positions.len()];
    for &index in source_indices {
        let source = usize::try_from(index).map_err(|_| {
            format!("source index conversion failed mesh='{mesh_name}' index={index}")
        })?;
        let Some(flag) = referenced.get_mut(source) else {
            return Err(format!(
                "source index outside vertex stream mesh='{mesh_name}' index={source} vertices={}",
                positions.len()
            ));
        };
        *flag = true;
    }
    let source_vertex_indices = referenced
        .iter()
        .enumerate()
        .filter_map(|(index, used)| used.then_some(index))
        .collect::<Vec<_>>();
    if source_vertex_indices.is_empty() {
        return Err(format!(
            "indexed mesh references no vertices mesh='{mesh_name}'"
        ));
    }
    let mut remap = vec![u32::MAX; positions.len()];
    let mut compact_positions = Vec::with_capacity(source_vertex_indices.len());
    let mut compact_uv0 = Vec::with_capacity(source_vertex_indices.len());
    for (dense, &source) in source_vertex_indices.iter().enumerate() {
        remap[source] = u32::try_from(dense)
            .map_err(|_| format!("dense vertex index overflow mesh='{mesh_name}'"))?;
        compact_positions.push(positions[source]);
        compact_uv0.push(uv0[source]);
    }
    let indices = source_indices
        .iter()
        .map(|&source| {
            let source = usize::try_from(source)
                .map_err(|_| format!("source index conversion failed mesh='{mesh_name}'"))?;
            remap
                .get(source)
                .copied()
                .filter(|value| *value != u32::MAX)
                .ok_or_else(|| {
                    format!("source index was not remapped mesh='{mesh_name}' index={source}")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        compact_positions,
        compact_uv0,
        indices,
        source_vertex_indices,
    ))
}
