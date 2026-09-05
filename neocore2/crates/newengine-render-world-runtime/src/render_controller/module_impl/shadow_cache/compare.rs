//! Sample-space equality for cached shadow frames.
//!
//! Admission compares what the GPU will actually sample. The epsilons below are the
//! whole policy: too loose and the atlas holds for several frames and then jumps
//! (visible stepping/flicker), too tight and a static scene can never reuse its atlas.

use super::super::shadows::ShadowFrame;

// Directional sun motion is render-cadence input. A loose matrix epsilon makes the
// atlas hold several frames and then jump, which is visible as shadow stepping/flicker.
// Keep only a machine-noise guard here; static texel-snapped projections remain bit-stable.
pub(super) const SHADOW_DIRECTIONAL_MATRIX_EPSILON: f32 = 1.0e-6;
// Local-light atlases retain a looser threshold because small point/spot transform noise
// would otherwise fan out into six perspective redraws per light.
pub(super) const SHADOW_LOCAL_MATRIX_EPSILON: f32 = 2.0e-4;
const SHADOW_PARAM_EPSILON: f32 = 1.0e-4;
const SHADOW_SPLIT_EPSILON: f32 = 1.0e-3;

#[inline]
fn slices_nearly_equal(a: &[f32], b: &[f32], epsilon: f32) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(left, right)| (*left - *right).abs() <= epsilon)
}

#[inline]
pub(super) fn shadow_matrices_match(
    a: newengine_math::Mat4,
    b: newengine_math::Mat4,
    epsilon: f32,
) -> bool {
    let a_cols = a.to_cols_array();
    let b_cols = b.to_cols_array();
    slices_nearly_equal(&a_cols, &b_cols, epsilon)
}

#[inline]
fn shadow_viewport_matches(
    a: newengine_core::render::Viewport,
    b: newengine_core::render::Viewport,
) -> bool {
    a.x.to_bits() == b.x.to_bits()
        && a.y.to_bits() == b.y.to_bits()
        && a.w.to_bits() == b.w.to_bits()
        && a.h.to_bits() == b.h.to_bits()
        && a.min_depth.to_bits() == b.min_depth.to_bits()
        && a.max_depth.to_bits() == b.max_depth.to_bits()
}

#[inline]
fn shadow_scissor_matches(
    a: newengine_core::render::RectI32,
    b: newengine_core::render::RectI32,
) -> bool {
    a.x == b.x && a.y == b.y && a.w == b.w && a.h == b.h
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ShadowFrameMismatch {
    pub(super) texture: bool,
    pub(super) matrix: bool,
    pub(super) split: bool,
    pub(super) params: bool,
    pub(super) extra: bool,
}

impl ShadowFrameMismatch {
    #[inline]
    pub(super) fn any(self) -> bool {
        self.texture || self.matrix || self.split || self.params || self.extra
    }
}

pub(super) fn shadow_frame_mismatch(a: ShadowFrame, b: ShadowFrame) -> ShadowFrameMismatch {
    let mut mismatch = ShadowFrameMismatch {
        texture: a.texture != b.texture || a.cascade_count != b.cascade_count,
        ..ShadowFrameMismatch::default()
    };
    let count = a.cascade_count.min(b.cascade_count).clamp(
        1,
        super::super::shadows::MAX_DIRECTIONAL_SHADOW_CASCADES as u32,
    ) as usize;
    for i in 0..count {
        mismatch.matrix |= !shadow_matrices_match(
            a.cascade_light_mvp[i],
            b.cascade_light_mvp[i],
            SHADOW_DIRECTIONAL_MATRIX_EPSILON,
        );
        mismatch.split |= (a.cascade_splits[i] - b.cascade_splits[i]).abs() > SHADOW_SPLIT_EPSILON;
    }
    mismatch.params = !slices_nearly_equal(&a.params, &b.params, SHADOW_PARAM_EPSILON);
    mismatch.extra = !slices_nearly_equal(&a.extra, &b.extra, SHADOW_PARAM_EPSILON);
    mismatch
}

#[inline]
pub(super) fn local_shadow_frames_match_sample_space(
    a: newengine_render_feature_api::LocalShadowFrame,
    b: newengine_render_feature_api::LocalShadowFrame,
) -> bool {
    if a.texture != b.texture
        || a.atlas_extent != b.atlas_extent
        || a.light_count != b.light_count
        || a.view_count != b.view_count
    {
        return false;
    }
    let light_count =
        a.light_count
            .min(newengine_render_feature_api::MAX_LOCAL_SHADOW_LIGHTS as u32) as usize;
    for i in 0..light_count {
        let left = a.lights[i];
        let right = b.lights[i];
        if left.stable_id != right.stable_id
            || left.light_kind != right.light_kind
            || left.packed_light_index != right.packed_light_index
            || left.first_view != right.first_view
            || left.view_count != right.view_count
            || left.resolution != right.resolution
            || (left.range - right.range).abs() > SHADOW_PARAM_EPSILON
            || (left.bias - right.bias).abs() > SHADOW_PARAM_EPSILON
            || (left.normal_bias - right.normal_bias).abs() > SHADOW_PARAM_EPSILON
            || (left.strength - right.strength).abs() > SHADOW_PARAM_EPSILON
        {
            return false;
        }
    }
    let view_count =
        a.view_count
            .min(newengine_render_feature_api::MAX_LOCAL_SHADOW_VIEWS as u32) as usize;
    for i in 0..view_count {
        let left = a.views[i];
        let right = b.views[i];
        if !shadow_matrices_match(left.light_mvp, right.light_mvp, SHADOW_LOCAL_MATRIX_EPSILON)
            || !shadow_viewport_matches(left.viewport, right.viewport)
            || !shadow_scissor_matches(left.scissor, right.scissor)
            || left.light_slot != right.light_slot
            || left.face_index != right.face_index
            || left.resolution != right.resolution
        {
            return false;
        }
    }
    true
}
