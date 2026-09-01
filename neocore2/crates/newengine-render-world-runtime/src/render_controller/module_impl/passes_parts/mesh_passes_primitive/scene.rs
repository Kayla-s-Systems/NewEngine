use super::plan::*;

use super::*;

#[path = "scene_editor_overlay.rs"]
mod editor_overlay;
#[path = "scene_pass.rs"]
mod pass;
#[path = "scene_wireframe.rs"]
mod wireframe;

use editor_overlay::draw_editor_viewport_overlays;
use pass::{draw_primitives_for_pass, PrimitivePassSlice};
use wireframe::draw_primitives_wireframe;

pub fn draw_primitives(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    shadow_texture: TextureId,
    local_shadow_texture: TextureId,
    runtime: bool,
    camera_position: Vec3,
    camera_forward: Vec3,
    deferred: bool,
) -> newengine_core::EngineResult<()> {
    if this.editor_viewport.is_active()
        && this.editor_viewport.shading() == newengine_ui_api::UiEditorViewportShading::Wireframe
    {
        draw_primitives_wireframe(this, r, scene, viewproj, runtime)?;
        super::draw_model_components_wireframe(this, r, scene, viewproj, runtime)?;
        return draw_editor_viewport_overlays(this, r, scene, viewproj);
    }
    draw_primitives_for_pass(
        this,
        r,
        scene,
        lit,
        SceneMeshPass::Forward,
        viewproj,
        lights,
        shadow_texture,
        local_shadow_texture,
        runtime,
        camera_position,
        camera_forward,
        deferred,
        PrimitivePassSlice::NonDecal,
    )?;
    super::draw_skinned_player_primitives(
        this,
        r,
        scene,
        lit,
        SceneMeshPass::Forward,
        viewproj,
        lights,
        shadow_texture,
        local_shadow_texture,
        runtime,
        camera_position,
        camera_forward,
    )?;
    super::draw_model_components(
        this,
        r,
        scene,
        lit,
        SceneMeshPass::Forward,
        viewproj,
        lights,
        shadow_texture,
        local_shadow_texture,
        runtime,
        camera_position,
        camera_forward,
        deferred,
    )?;
    // Decals are forward overlays. Replaying them before model/YDD world geometry lets later opaque
    // batches overwrite their color even though the decal pipeline itself is depth-read-only.
    // Keep the overlay partition physically after all opaque/skinned/model world draws.
    draw_primitives_for_pass(
        this,
        r,
        scene,
        lit,
        SceneMeshPass::Forward,
        viewproj,
        lights,
        shadow_texture,
        local_shadow_texture,
        runtime,
        camera_position,
        camera_forward,
        deferred,
        PrimitivePassSlice::DecalOnly,
    )?;
    draw_editor_viewport_overlays(this, r, scene, viewproj)
}

pub fn draw_primitives_gbuffer(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
    camera_position: Vec3,
    camera_forward: Vec3,
    deferred: bool,
) -> newengine_core::EngineResult<()> {
    if this.editor_viewport.is_active()
        && this.editor_viewport.shading() == newengine_ui_api::UiEditorViewportShading::Wireframe
    {
        return Ok(());
    }
    draw_primitives_for_pass(
        this,
        r,
        scene,
        lit,
        SceneMeshPass::GBuffer,
        viewproj,
        lights,
        lit.white_texture,
        lit.white_texture,
        runtime,
        camera_position,
        camera_forward,
        deferred,
        PrimitivePassSlice::All,
    )?;
    super::draw_skinned_player_primitives(
        this,
        r,
        scene,
        lit,
        SceneMeshPass::GBuffer,
        viewproj,
        lights,
        lit.white_texture,
        lit.white_texture,
        runtime,
        camera_position,
        camera_forward,
    )?;
    super::draw_model_components(
        this,
        r,
        scene,
        lit,
        SceneMeshPass::GBuffer,
        viewproj,
        lights,
        lit.white_texture,
        lit.white_texture,
        runtime,
        camera_position,
        camera_forward,
        deferred,
    )
}
