use std::time::Instant;

use newengine_material_domain_api::{MaterialDomainResult, MaterialRenderDevice};
use newengine_render_api::*;

use crate::manifest::StandardShaderAssetRef;

pub(super) fn create_manifest_shader(
    r: &mut dyn MaterialRenderDevice,
    stage: ShaderStage,
    shader: &StandardShaderAssetRef,
    label: &str,
) -> MaterialDomainResult<ShaderId> {
    let source_kind = shader.source_kind()?;
    let started_at = Instant::now();
    let asset = ShaderAssetDesc::new(shader.logical_path.clone(), source_kind)
        .with_entry(shader.entry.clone())
        .with_variant(shader.variant_id.clone());
    let result = r.create_shader(
        ShaderDesc::from_asset(
            stage,
            shader.entry.clone(),
            shader.logical_path.clone(),
            source_kind,
        )
        .with_asset(asset)
        .with_label(label),
    );
    let elapsed_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    match &result {
        Ok(id) if elapsed_ms >= 8.0 => newengine_ulog_api::ulog::warn!(
            "standard material domain: shader stage exceeded warmup budget label='{}' path='{}' stage='{:?}' shader_id={:?} elapsed_ms={:.2}",
            label,
            shader.logical_path,
            stage,
            id,
            elapsed_ms,
        ),
        Err(error) => newengine_ulog_api::ulog::error!(
            "standard material domain: shader build failed label='{}' path='{}' stage='{:?}' err='{}' elapsed_ms={:.2}",
            label,
            shader.logical_path,
            stage,
            error,
            elapsed_ms,
        ),
        _ => {}
    }
    result
}
