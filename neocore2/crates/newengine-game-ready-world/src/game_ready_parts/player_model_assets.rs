use super::*;

use newengine_model_domain_api::ModelAssetRequest;
use newengine_model_runtime::ModelGatewayClient;
use newengine_model_skeleton_api::ModelSkeletonMetadata;

pub(super) fn ensure_player_runtime_model_parts(
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
) -> Result<
    (
        String,
        Vec<PlayerRuntimeModelPart>,
        Option<ModelSkeletonMetadata>,
    ),
    String,
> {
    let mut request = ModelAssetRequest::new(assignment.source.clone())
        .with_human_scale(assignment.target_height, assignment.eye_height_ratio);
    if let Some(properties_ref) = assignment.properties_ref.as_deref() {
        request = request.with_properties_ref(properties_ref);
    }
    if let Some(dictionary) = assignment.texture_dictionary.as_deref() {
        request = request.with_texture_dictionary(dictionary);
    }
    if let Some(skeleton) = assignment.skeleton_source.as_deref() {
        request = request.with_skeleton(skeleton);
    }

    let constructor = ModelGatewayClient::new(newengine_plugin_host::default_host_api());
    let bundle = constructor.assemble_bundle(&request)?;

    if let Some(metadata) = bundle.skeleton.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: player skeleton metadata bound source='{}' skeleton='{}' format='{}' bytes={} joints={} status='{}'",
            bundle.source,
            metadata.source,
            metadata.source_format,
            metadata.byte_len,
            metadata.joints.len(),
            metadata.decode_status
        );
    }

    let mut out = Vec::with_capacity(bundle.parts.len());
    let mut registered_parts = 0usize;
    let mut registered_vertices = 0usize;
    let mut registered_indices = 0usize;
    for (part_index, part) in bundle.parts.into_iter().enumerate() {
        let skin = part.skin.clone();
        // A character may legitimately have multiple geometries using the same material slot
        // (a character may legitimately have multiple geometries for one material slot). Part index is therefore part of the stable mesh id.
        let primitive_id = PrimitiveId(fnv1a_64(&format!(
            "player-model:{}:revision={}:{}:{}",
            bundle.source, assignment.revision, part_index, part.material_slot
        )));
        let material_name = part
            .material
            .material_ref
            .clone()
            .unwrap_or_else(|| format!("Player/Avatar/{}", part.material_slot));
        let material_id = mats.upsert_named_with_textures(
            &material_name,
            part.material.descriptor,
            part.material.textures.clone().sanitized(),
        );
        if !prims.is_registered(primitive_id) {
            let vertex_count = part.mesh.vertices.len();
            let index_count = part.mesh.indices.len();
            prims.register_mesh(
                primitive_id,
                format!(
                    "PlayerModel/Part{}:{} ({})",
                    part_index, part.material_slot, bundle.source
                ),
                part.mesh,
            );
            registered_parts += 1;
            registered_vertices += vertex_count;
            registered_indices += index_count;
            newengine_ulog_api::ulog::debug!(
                "game-ready: player model part registered source='{}' part={} slot='{}' vertices={} indices={} material='{}' policy='ydd->nemat->ytd'",
                bundle.source,
                part_index,
                part.material_slot,
                vertex_count,
                index_count,
                material_name
            );
        }

        out.push(PlayerRuntimeModelPart {
            primitive_id,
            material_id,
            material_slot: part.material_slot,
            color: part.material.fallback_color,
            skin,
        });
    }

    if registered_parts > 0 {
        newengine_ulog_api::ulog::info!(
            "game-ready: player model registered source='{}' parts={} vertices={} indices={} materials={}",
            bundle.source,
            registered_parts,
            registered_vertices,
            registered_indices,
            out.len(),
        );
    }

    if let Some(dictionary) = bundle.texture_dictionary.as_deref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: player model texture dictionary bound source='{}' dictionary='{}' materials={}",
            bundle.source,
            dictionary,
            out.len()
        );
    }

    if let Some(properties_ref) = bundle.properties_ref.as_deref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: player model properties descriptor bound source='{}' properties_ref='{}' policy='.ydd slots -> .ytyp material bindings -> .nemat/.ytd'",
            bundle.source,
            properties_ref
        );
    }

    if !bundle.collisions.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: player model collision bindings derived source='{}' collisions={}",
            bundle.source,
            bundle.collisions.len()
        );
    }

    Ok((bundle.source, out, bundle.skeleton))
}
