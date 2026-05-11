#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    BeginRenderTargetDesc, Extent2D, RectI32, RenderApi, RenderTargetDesc, RenderTargetId,
    TextureFormat, TextureId, Viewport,
};
use newengine_core::EngineResult;
use newengine_lighting::{ShadowMethod, ShadowSettings};
use newengine_math::{Mat4, Vec3};

use super::lights;
use super::passes;
use super::scene::BoundsSnap;
use super::EditorRenderController;

#[derive(Clone, Copy, Debug)]
pub(super) struct ShadowFrame {
    pub texture: TextureId,
    pub light_mvp: Mat4,
    pub params: [f32; 4],
}

impl ShadowFrame {
    #[inline]
    pub(super) fn disabled(fallback: TextureId) -> Self {
        Self {
            texture: fallback,
            light_mvp: Mat4::IDENTITY,
            params: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

#[inline]
pub(super) fn prepare_shadow_frame(
    this: &mut EditorRenderController,
    r: &mut dyn RenderApi,
    scene: &newengine_scene::Scene,
    bounds: BoundsSnap,
    lit: super::super::gpu::LitPipeline,
    runtime: bool,
) -> EngineResult<ShadowFrame> {
    let world = scene.world();
    let settings = world
        .resource::<ShadowSettings>()
        .copied()
        .unwrap_or_default()
        .sanitized();

    if !settings.enabled || matches!(settings.method, ShadowMethod::None) {
        retire_shadow_rt(this);
        return Ok(ShadowFrame::disabled(lit.white_texture));
    }

    let Some(dir_light) = lights::primary_directional_light(world) else {
        return Ok(ShadowFrame::disabled(lit.white_texture));
    };

    let Some((rt, shadow_texture)) = ensure_shadow_rt(this, r, settings.resolution)? else {
        return Ok(ShadowFrame::disabled(lit.white_texture));
    };

    let dir = Vec3::new(
        dir_light.direction_ws[0],
        dir_light.direction_ws[1],
        dir_light.direction_ws[2],
    )
    .normalize_or_zero();
    if dir.length_squared() <= 1.0e-8 {
        return Ok(ShadowFrame::disabled(lit.white_texture));
    }

    let radius = bounds.radius.max(4.0).min(settings.max_distance.max(4.0));
    let center = bounds.center;
    let eye = center - dir * (radius * 1.75);
    let up = if dir.dot(Vec3::Y).abs() > 0.92 { Vec3::Z } else { Vec3::Y };
    let view = Mat4::look_at_rh(eye, center, up);
    let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.1, radius * 4.0);
    let light_mvp = proj * view;
    let params = [
        1.0,
        settings.bias,
        settings.contact_strength,
        settings.normal_bias,
    ];

    r.begin_render_target(
        BeginRenderTargetDesc::new(rt)
            .with_clear_color([1.0, 1.0, 1.0, 1.0])
            .with_clear_depth(1.0),
    )?;
    let extent = Extent2D::new(settings.resolution, settings.resolution);
    r.set_viewport(Viewport::full(extent))?;
    r.set_scissor(RectI32::new(
        0,
        0,
        settings.resolution as i32,
        settings.resolution as i32,
    ))?;

    let shadow_lights = lights::collect_lights(world).with_shadow(light_mvp, params);
    let draw_result = (|| -> EngineResult<()> {
        passes::draw_procedural_terrain_shadow(this, r, scene, lit, light_mvp, &shadow_lights, runtime)?;
        passes::draw_primitives_shadow(this, r, scene, lit, light_mvp, &shadow_lights, runtime)?;
        Ok(())
    })();
    let end_result = r.end_render_target();
    draw_result?;
    end_result?;

    Ok(ShadowFrame {
        texture: shadow_texture,
        light_mvp,
        params,
    })
}

#[inline]
fn ensure_shadow_rt(
    this: &mut EditorRenderController,
    r: &mut dyn RenderApi,
    requested_resolution: u32,
) -> EngineResult<Option<(RenderTargetId, TextureId)>> {
    let resolution = requested_resolution.clamp(256, 8192);
    let recreate = this.shadow_rt.is_none() || this.shadow_rt_resolution != resolution;

    if recreate {
        if let Some(old) = this.shadow_rt.take() {
            this.retire_render_target(old);
        }
        this.shadow_rt_resolution = 0;
        let rt = r.create_render_target(
            RenderTargetDesc::new(Extent2D::new(resolution, resolution), TextureFormat::Bgra8Unorm)
                .with_depth(TextureFormat::Depth32Float)
                .with_label(format!("editor_shadow_map_{resolution}")),
        )?;
        this.shadow_rt = Some(rt);
        this.shadow_rt_resolution = resolution;
    }

    let Some(rt) = this.shadow_rt else {
        return Ok(None);
    };
    let tex = r.render_target_color_texture_id(rt)?;
    Ok(Some((rt, tex)))
}

#[inline]
fn retire_shadow_rt(this: &mut EditorRenderController) {
    if let Some(old) = this.shadow_rt.take() {
        this.retire_render_target(old);
    }
    this.shadow_rt_resolution = 0;
}
