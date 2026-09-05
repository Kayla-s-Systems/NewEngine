#![forbid(unsafe_op_in_unsafe_fn)]

//! Temporal shadow-atlas cache admission.
//!
//! The cache answers one question per atlas per frame: may this frame reuse the atlas
//! that is already resident, or must it be re-rendered? Admission deliberately compares
//! what the GPU will sample - projections, caster membership, caster pose, skin
//! publication revisions - instead of trusting ECS change ticks, because transform
//! propagation marks many static components changed every frame and would defeat reuse
//! entirely.
//!
//! * [`compare`] - sample-space equality and the epsilon policy that decides reuse.
//! * [`casters`] - who casts a shadow, plus the per-tick caster revision both atlases share.
//! * [`directional`] - sun/cascade atlas admission.
//! * [`local`] - point/spot atlas admission.

use super::super::controller::RuntimeRenderController;

mod casters;
mod compare;
mod directional;
mod local;

#[cfg(test)]
mod tests;

impl RuntimeRenderController {
    /// Caster culling is resolved per frame and shared by whichever atlas renders it,
    /// so it is owned by the cache root rather than by one of the two admission paths.
    #[inline]
    pub(super) fn set_shadow_caster_cull(
        &mut self,
        cull: Option<super::shadows::ShadowCasterCull>,
    ) {
        self.shadows.current_caster_cull = cull;
    }

    #[inline]
    pub(super) fn shadows_current_cull(&self) -> Option<super::shadows::ShadowCasterCull> {
        self.shadows.current_caster_cull
    }
}
