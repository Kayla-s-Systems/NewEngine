use newengine_render_api::{RenderGraphPassId, RenderGraphPassKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StandardRenderPhase {
    BeginFrame,
    ShadowMap,
    DepthPrepass,
    ViewportGBuffer,
    DeferredLighting,
    ViewportForward,
    Transparent,
    Water,
    PostFx,
    UiComposite,
    DebugOverlay,
    EndFrame,
}

impl StandardRenderPhase {
    #[inline]
    pub const fn pass_kind(self) -> Option<RenderGraphPassKind> {
        match self {
            Self::BeginFrame | Self::EndFrame => None,
            Self::ShadowMap => Some(RenderGraphPassKind::ShadowMap),
            Self::DepthPrepass => Some(RenderGraphPassKind::DepthPrepass),
            Self::ViewportGBuffer => Some(RenderGraphPassKind::GBuffer),
            Self::DeferredLighting => Some(RenderGraphPassKind::DeferredLighting),
            Self::ViewportForward => Some(RenderGraphPassKind::ForwardOpaque),
            Self::Transparent => Some(RenderGraphPassKind::Transparent),
            Self::Water => Some(RenderGraphPassKind::Water),
            Self::PostFx => Some(RenderGraphPassKind::PostFx),
            Self::UiComposite => Some(RenderGraphPassKind::UiComposite),
            Self::DebugOverlay => Some(RenderGraphPassKind::DebugOverlay),
        }
    }

    #[inline]
    pub const fn stable_pass_id(self) -> Option<RenderGraphPassId> {
        let id = match self {
            Self::BeginFrame | Self::EndFrame => return None,
            Self::ShadowMap => 100,
            Self::DepthPrepass => 200,
            Self::ViewportGBuffer => 300,
            Self::DeferredLighting => 400,
            Self::ViewportForward => 500,
            Self::Transparent => 600,
            Self::Water => 700,
            Self::PostFx => 800,
            Self::UiComposite => 900,
            Self::DebugOverlay => 1_000,
        };
        Some(RenderGraphPassId(id))
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BeginFrame => "begin_frame",
            Self::ShadowMap => "shadow_map",
            Self::DepthPrepass => "depth_prepass",
            Self::ViewportGBuffer => "viewport_gbuffer",
            Self::DeferredLighting => "deferred_lighting",
            Self::ViewportForward => "viewport_forward",
            Self::Transparent => "transparent",
            Self::Water => "water",
            Self::PostFx => "postfx",
            Self::UiComposite => "ui_composite",
            Self::DebugOverlay => "debug_overlay",
            Self::EndFrame => "end_frame",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPhaseDesc {
    pub phase: StandardRenderPhase,
    pub pass_id: Option<RenderGraphPassId>,
    pub label: String,
}

impl RenderPhaseDesc {
    #[inline]
    pub fn standard(phase: StandardRenderPhase) -> Self {
        Self {
            phase,
            pass_id: phase.stable_pass_id(),
            label: phase.label().to_string(),
        }
    }
}
