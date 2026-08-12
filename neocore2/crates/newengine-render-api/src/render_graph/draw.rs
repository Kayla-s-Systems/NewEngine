use serde::{Deserialize, Serialize};

use super::RenderGraphPassKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RenderDrawListKind {
    /// Geometry that casts into shadow maps.
    ShadowCasters,
    /// Opaque world geometry for the forward viewport path.
    OpaqueForward,
    /// Transparent world geometry that must be drawn after opaque geometry.
    Transparent,
    /// UI draw commands and UI-provider composite work.
    Ui,
    /// Editor/runtime debug primitives and overlays.
    Debug,
}

impl RenderDrawListKind {
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ShadowCasters => "shadow_casters",
            Self::OpaqueForward => "opaque_forward",
            Self::Transparent => "transparent",
            Self::Ui => "ui",
            Self::Debug => "debug",
        }
    }

    #[inline]
    pub const fn default_pass_kind(self) -> RenderGraphPassKind {
        match self {
            Self::ShadowCasters => RenderGraphPassKind::ShadowMap,
            Self::OpaqueForward => RenderGraphPassKind::ForwardOpaque,
            Self::Transparent => RenderGraphPassKind::Transparent,
            Self::Ui => RenderGraphPassKind::UiComposite,
            Self::Debug => RenderGraphPassKind::DebugOverlay,
        }
    }

    #[inline]
    pub const fn is_compatible_with_pass(self, pass: RenderGraphPassKind) -> bool {
        matches!(
            (self, pass),
            (
                Self::ShadowCasters,
                RenderGraphPassKind::ShadowMap
                    | RenderGraphPassKind::ShadowCascadeMap
                    | RenderGraphPassKind::DepthPrepass,
            ) | (
                Self::OpaqueForward,
                RenderGraphPassKind::ForwardOpaque | RenderGraphPassKind::GBuffer,
            ) | (Self::Transparent, RenderGraphPassKind::Transparent)
                | (Self::Ui, RenderGraphPassKind::UiComposite)
                | (Self::Debug, RenderGraphPassKind::DebugOverlay)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RenderMaterialDomain {
    OpaqueLit,
    Terrain,
    Vegetation,
    ShadowCaster,
    Transparent,
    Water,
    Ui,
    PostFx,
    Debug,
    Custom,
}

impl Default for RenderMaterialDomain {
    #[inline]
    fn default() -> Self {
        Self::Custom
    }
}

impl RenderMaterialDomain {
    #[inline]
    pub const fn is_compatible_with_pass(self, pass: RenderGraphPassKind) -> bool {
        matches!(
            (self, pass),
            (
                Self::ShadowCaster,
                RenderGraphPassKind::ShadowMap
                    | RenderGraphPassKind::ShadowCascadeMap
                    | RenderGraphPassKind::DepthPrepass,
            ) | (
                Self::OpaqueLit | Self::Terrain | Self::Vegetation,
                RenderGraphPassKind::ForwardOpaque | RenderGraphPassKind::GBuffer,
            ) | (Self::Transparent, RenderGraphPassKind::Transparent)
                | (Self::Water, RenderGraphPassKind::Water)
                | (Self::Ui, RenderGraphPassKind::UiComposite)
                | (
                    Self::PostFx,
                    RenderGraphPassKind::PostFx
                        | RenderGraphPassKind::BloomExtract
                        | RenderGraphPassKind::BloomBlur
                        | RenderGraphPassKind::TaaResolve
                        | RenderGraphPassKind::MsaaResolve
                        | RenderGraphPassKind::DeferredLighting
                        | RenderGraphPassKind::UiBackdropBlur,
                )
                | (Self::Debug, RenderGraphPassKind::DebugOverlay)
                | (Self::Custom, _)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PipelineKey(pub String);

impl PipelineKey {
    #[inline]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawPacket {
    pub pass_kind: RenderGraphPassKind,
    pub draw_list_kind: RenderDrawListKind,
    pub material_domain: RenderMaterialDomain,
    pub pipeline_key: PipelineKey,
    pub sort_key: u64,
    pub commands: Vec<crate::RenderCommand>,
}

impl DrawPacket {
    #[inline]
    pub fn is_compatible_with_pass(&self, pass: RenderGraphPassKind) -> bool {
        self.pass_kind == pass
            && self.draw_list_kind.is_compatible_with_pass(pass)
            && self.material_domain.is_compatible_with_pass(pass)
    }
}
