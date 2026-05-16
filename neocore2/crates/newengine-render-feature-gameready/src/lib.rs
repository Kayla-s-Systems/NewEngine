#![forbid(unsafe_op_in_unsafe_fn)]

//! GameReady profile-owned render feature pack.
//!
//! This crate is not a renderer backend. It owns GameReady scene extraction and
//! light/shadow planning policy, then registers those providers into the reusable
//! render controller. Backend-native rendering remains behind `render.api`.

use std::sync::Arc;

use newengine_core::render::RenderDrawListKind;
use newengine_core::EngineResult;
use newengine_engine_runtime::render_controller::feature_api::{
    draw_primitives, draw_primitives_shadow, draw_procedural_terrain,
    draw_procedural_terrain_shadow, primary_directional_light, primary_point_light,
    retire_shadow_rt, shadow_and_opaque_list, try_build_directional_shadow_plan, ui_list,
    warn_unsupported_point_shadow_once, warn_unsupported_spot_shadow_once, DrawListBuildCtx,
    LightExtractionCtx, LightExtractionProvider, LightExtractionProviderMetadata,
    LightShadowPlan, RenderDrawListProvider, RenderDrawListProviderMetadata,
    SceneExtractionCtx, ShadowLightKind,
};
use newengine_engine_runtime::RuntimeRenderController;
use newengine_lighting::ShadowMethod;
use newengine_material_domain_gameready::{
    GameReadyLitMaterialDomainProvider, GAME_READY_LIT_PIPELINE_KEY,
};

pub const GAME_READY_TERRAIN_PROVIDER_ID: &str = "gameready.terrain";
pub const GAME_READY_PRIMITIVE_MESH_PROVIDER_ID: &str = "gameready.primitive_mesh";
pub const GAME_READY_UI_PROVIDER_ID: &str = "gameready.ui";
pub const GAME_READY_DIRECTIONAL_SHADOW_PROVIDER_ID: &str = "gameready.directional_shadow";
pub const GAME_READY_POINT_CUBE_SHADOW_PROVIDER_ID: &str = "gameready.point_cube_shadow";
pub const GAME_READY_SPOT_SHADOW_PROVIDER_ID: &str = "gameready.spot_shadow";
pub const GAME_READY_AMBIENT_OCCLUSION_PROVIDER_ID: &str = "gameready.ambient_occlusion";

#[derive(Default)]
pub struct GameReadyRenderFeaturePack;

impl GameReadyRenderFeaturePack {
    #[inline]
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn install(self, controller: RuntimeRenderController) -> RuntimeRenderController {
        controller
            .with_material_pipeline_provider(Box::new(GameReadyLitMaterialDomainProvider::new()))
            .with_primary_lit_material_domain(GAME_READY_LIT_PIPELINE_KEY)
            .with_draw_list_provider(Arc::new(GameReadyTerrainProvider))
            .with_draw_list_provider(Arc::new(GameReadyPrimitiveMeshProvider))
            .with_draw_list_provider(Arc::new(GameReadyUiProvider))
            .with_light_extraction_provider(Arc::new(GameReadyDirectionalShadowProvider))
            .with_light_extraction_provider(Arc::new(GameReadyPointCubeShadowProvider))
            .with_light_extraction_provider(Arc::new(GameReadySpotShadowProvider))
            .with_light_extraction_provider(Arc::new(GameReadyAmbientOcclusionProvider))
    }
}

struct GameReadyTerrainProvider;

impl RenderDrawListProvider for GameReadyTerrainProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_TERRAIN_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::feature(self.id(), "GameReady terrain draw extraction")
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        shadow_and_opaque_list(ctx.render_shadow_map)
    }

    fn extract(&self, ctx: &SceneExtractionCtx<'_>, out: &mut DrawListBuildCtx<'_>) -> EngineResult<()> {
        if ctx.render_shadow_map {
            let _ = out.record(RenderDrawListKind::ShadowCasters, |this, r| {
                draw_procedural_terrain_shadow(
                    this,
                    r,
                    ctx.scene,
                    ctx.lit,
                    ctx.shadow_frame.light_mvp,
                    &ctx.lights,
                    ctx.runtime,
                )
            })?;
        }

        let _ = out.record(RenderDrawListKind::OpaqueForward, |this, r| {
            draw_procedural_terrain(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.viewproj,
                &ctx.lights,
                ctx.shadow_frame.texture,
                ctx.runtime,
            )
        })?;

        Ok(())
    }
}

struct GameReadyPrimitiveMeshProvider;

impl RenderDrawListProvider for GameReadyPrimitiveMeshProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_PRIMITIVE_MESH_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::feature(self.id(), "GameReady primitive mesh draw extraction")
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        shadow_and_opaque_list(ctx.render_shadow_map)
    }

    fn extract(&self, ctx: &SceneExtractionCtx<'_>, out: &mut DrawListBuildCtx<'_>) -> EngineResult<()> {
        if ctx.render_shadow_map {
            let _ = out.record(RenderDrawListKind::ShadowCasters, |this, r| {
                draw_primitives_shadow(
                    this,
                    r,
                    ctx.scene,
                    ctx.lit,
                    ctx.shadow_frame.light_mvp,
                    &ctx.lights,
                    ctx.runtime,
                    ctx.rig.position,
                )
            })?;
        }

        let _ = out.record(RenderDrawListKind::OpaqueForward, |this, r| {
            draw_primitives(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.viewproj,
                &ctx.lights,
                ctx.shadow_frame.texture,
                ctx.runtime,
                ctx.rig.position,
            )
        })?;

        Ok(())
    }
}

struct GameReadyUiProvider;

impl RenderDrawListProvider for GameReadyUiProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_UI_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::feature(self.id(), "GameReady UI draw extraction")
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        ui_list(ctx.ui.is_some())
    }

    fn extract(&self, ctx: &SceneExtractionCtx<'_>, out: &mut DrawListBuildCtx<'_>) -> EngineResult<()> {
        let Some(ui) = ctx.ui else {
            return Ok(());
        };
        let extent = ctx.surface_extent;
        let _ = out.record(RenderDrawListKind::Ui, |_this, r| {
            r.set_viewport(newengine_core::render::Viewport::full(extent))?;
            r.set_scissor(newengine_core::render::RectI32::new(
                0,
                0,
                extent.width as i32,
                extent.height as i32,
            ))?;
            r.set_ui_draw_list(ui.clone());
            Ok(())
        })?;
        Ok(())
    }
}

struct GameReadyDirectionalShadowProvider;

impl LightExtractionProvider for GameReadyDirectionalShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_DIRECTIONAL_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        LightExtractionProviderMetadata::feature(self.id(), "GameReady directional shadow planning")
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::Auto | ShadowMethod::DirectionalDepthMap)
            && primary_directional_light(ctx.world).is_some()
    }

    #[inline]
    fn extract(&self, ctx: &mut LightExtractionCtx<'_>) -> EngineResult<Option<LightShadowPlan>> {
        try_build_directional_shadow_plan(
            &mut *ctx.controller,
            &mut *ctx.render,
            ctx.world,
            ctx.bounds,
            ctx.lit,
            ctx.settings,
        )
    }
}

struct GameReadyPointCubeShadowProvider;

impl LightExtractionProvider for GameReadyPointCubeShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_POINT_CUBE_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        LightExtractionProviderMetadata::feature(self.id(), "GameReady point shadow planning")
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::Auto | ShadowMethod::PointCubeMap)
            && primary_point_light(ctx.world).is_some()
    }

    #[inline]
    fn extract(&self, ctx: &mut LightExtractionCtx<'_>) -> EngineResult<Option<LightShadowPlan>> {
        warn_unsupported_point_shadow_once(&mut *ctx.controller);
        retire_shadow_rt(&mut *ctx.controller);
        Ok(Some(LightShadowPlan::unsupported(
            ShadowLightKind::Point,
            ctx.lit.white_texture,
            ctx.settings.resolution,
        )))
    }
}

struct GameReadySpotShadowProvider;

impl LightExtractionProvider for GameReadySpotShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_SPOT_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        LightExtractionProviderMetadata::feature(self.id(), "GameReady spot shadow planning")
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::SpotDepthMap)
    }

    #[inline]
    fn extract(&self, ctx: &mut LightExtractionCtx<'_>) -> EngineResult<Option<LightShadowPlan>> {
        warn_unsupported_spot_shadow_once(&mut *ctx.controller);
        retire_shadow_rt(&mut *ctx.controller);
        Ok(Some(LightShadowPlan::unsupported(
            ShadowLightKind::Spot,
            ctx.lit.white_texture,
            ctx.settings.resolution,
        )))
    }
}

struct GameReadyAmbientOcclusionProvider;

impl LightExtractionProvider for GameReadyAmbientOcclusionProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_AMBIENT_OCCLUSION_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        LightExtractionProviderMetadata::feature(self.id(), "GameReady ambient occlusion planning")
    }

    #[inline]
    fn supports(&self, _ctx: &LightExtractionCtx<'_>) -> bool {
        false
    }

    #[inline]
    fn extract(&self, _ctx: &mut LightExtractionCtx<'_>) -> EngineResult<Option<LightShadowPlan>> {
        Ok(None)
    }
}
