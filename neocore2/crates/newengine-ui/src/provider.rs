#![forbid(unsafe_op_in_unsafe_fn)]

use crate::draw::UiDrawList;
use crate::input::UiInputFrame;
use crate::schema::UiProviderCatalog;
use crate::surface::{UiProviderBinding, UiProviderManifest};
use std::any::Any;

/// Frame descriptor (extended).
#[derive(Debug, Clone)]
pub struct UiFrameDesc {
    pub dt_sec: f32,

    /// Input snapshot provided by the host (must originate from INPUT plugin).
    pub input: Option<UiInputFrame>,

    /// Physical surface size in pixels.
    pub surface_size_px: [u32; 2],

    /// Native pixels per logical point.
    pub pixels_per_point: f32,
}

impl UiFrameDesc {
    #[inline]
    pub fn new(dt_sec: f32) -> Self {
        Self {
            dt_sec,
            input: None,
            surface_size_px: [0, 0],
            pixels_per_point: 1.0,
        }
    }

    #[inline]
    pub fn with_input(mut self, input: UiInputFrame) -> Self {
        self.input = Some(input);
        self
    }

    #[inline]
    pub fn with_surface(mut self, width: u32, height: u32, pixels_per_point: f32) -> Self {
        self.surface_size_px = [width, height];
        self.pixels_per_point = pixels_per_point.max(0.0001);
        self
    }
}

/// Output of a UI frame.
#[derive(Debug, Clone)]
pub struct UiFrameOutput {
    pub draw_list: UiDrawList,
}

impl UiFrameOutput {
    #[inline]
    pub fn empty() -> Self {
        Self {
            draw_list: UiDrawList::new(),
        }
    }
}

/// Object-safe UI build callback.
/// Providers may expose a typed context via `ctx_any`; callers can downcast.
pub trait UiBuildFn {
    fn begin_frame(&mut self, _frame: &UiFrameDesc) {}

    fn build(&mut self, ctx_any: &mut dyn Any);
}

/// Provider kind selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiProviderKind {
    /// Built-in no-UI provider. This is the only provider compiled into engine crates.
    ///
    /// This is a valid, explicit UI mode: the engine keeps running and UI
    /// surfaces project to provider=`none`. Native startup/error fallback may
    /// still be used before provider handoff to keep fatal diagnostics visible.
    Null,

    /// Runtime UI provider requested by plugin service/capability id.
    ///
    /// The concrete implementation must be supplied by a plugin. If that plugin
    /// is absent or not yet bound, the runtime degrades to `Null`.
    Plugin { service_id: String },
}


impl UiProviderKind {
    /// Build a provider kind from a startup-config plugin id.
    ///
    /// Empty ids are treated as `Null` so launchers do not duplicate this policy.
    #[inline]
    pub fn from_plugin_id(plugin_id: Option<&str>) -> Self {
        match plugin_id.map(str::trim).filter(|id| !id.is_empty()) {
            Some(service_id) => Self::Plugin {
                service_id: service_id.to_owned(),
            },
            None => Self::Null,
        }
    }

    #[inline]
    pub fn binding(&self) -> UiProviderBinding {
        match self {
            Self::Null => UiProviderBinding::None,
            Self::Plugin { service_id } => UiProviderBinding::Plugin {
                service_id: service_id.clone(),
            },
        }
    }

    #[inline]
    pub fn service_id(&self) -> Option<&str> {
        match self {
            Self::Plugin { service_id } => Some(service_id.as_str()),
            Self::Null => None,
        }
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// Provider creation options.
#[derive(Debug, Clone)]
pub struct UiProviderOptions {
    pub kind: UiProviderKind,
}

impl Default for UiProviderOptions {
    #[inline]
    fn default() -> Self {
        Self {
            kind: UiProviderKind::Null,
        }
    }
}

/// Replaceable UI provider interface.
///
/// The trait is platform-agnostic by design:
/// - window and events are passed as `dyn Any`
/// - concrete provider decides what it supports
pub trait UiProvider: Send {
    fn kind(&self) -> UiProviderKind;

    #[inline]
    fn binding(&self) -> UiProviderBinding {
        self.kind().binding()
    }

    fn manifest(&self) -> UiProviderManifest {
        UiProviderManifest {
            provider: self.binding(),
            version: 1,
            surfaces: Vec::new(),
            features: Vec::new(),
        }
    }

    /// Full provider-owned catalog. This is the canonical declaration of what
    /// UI exists, which layout document owns it, what state it consumes and
    /// which actions it can emit. Engines must not hardcode runtime UI outside
    /// this catalog unless running the native startup/fatal fallback.
    fn catalog(&self) -> UiProviderCatalog {
        UiProviderCatalog::from_manifest(self.manifest())
    }

    /// Optional concrete layout document for a surface. Plugin providers may
    /// source this from AssetManager, hot-reloadable JSON, or a service call.
    fn layout_document(&self, _surface_id: &str) -> Option<String> {
        None
    }

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Feed platform event (optional).
    /// IMPORTANT: UI must not consume platform input directly; input must come from INPUT plugin.
    fn on_platform_event(&mut self, _window: &dyn Any, _event: &dyn Any) {}

    /// Run one UI frame.
    fn run_frame(
        &mut self,
        window: &dyn Any,
        frame: UiFrameDesc,
        build: &mut dyn UiBuildFn,
    ) -> UiFrameOutput;
}
