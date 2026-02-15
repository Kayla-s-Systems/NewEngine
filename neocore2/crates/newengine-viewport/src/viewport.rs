#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::Extent2D;
use newengine_ecs::EntityId;

use crate::{ViewportDescriptor, ViewportRenderResources, ViewportRuntime};

/// High-level viewport abstraction.
///
/// Combines logical descriptor,
/// runtime parameters and render backing.
#[derive(Clone, Debug)]
pub struct Viewport {
    descriptor: ViewportDescriptor,
    runtime: ViewportRuntime,
    render: ViewportRenderResources,
}

impl Viewport {
    #[inline]
    pub fn new(camera: Option<EntityId>) -> Self {
        Self {
            descriptor: ViewportDescriptor::new(camera),
            runtime: ViewportRuntime::new(),
            render: ViewportRenderResources::default(),
        }
    }

    // --- Descriptor ---

    #[inline]
    pub fn camera(&self) -> Option<EntityId> {
        self.descriptor.camera
    }

    #[inline]
    pub fn set_camera(&mut self, camera: Option<EntityId>) {
        self.descriptor.camera = camera;
    }

    // --- Runtime ---

    #[inline]
    pub fn extent(&self) -> Extent2D {
        self.runtime.extent()
    }

    #[inline]
    pub fn set_extent(&mut self, width: u32, height: u32) {
        self.runtime.set_extent(width, height);
    }

    /// Returns whether viewport was resized since last call and clears the flag.
    #[inline]
    pub fn take_resize_flag(&mut self) -> bool {
        let resized = self.runtime.is_resized();
        self.runtime.clear_resize_flag();
        resized
    }

    // --- Render Resources ---

    #[inline]
    pub fn render_resources(&self) -> &ViewportRenderResources {
        &self.render
    }

    #[inline]
    pub fn render_resources_mut(&mut self) -> &mut ViewportRenderResources {
        &mut self.render
    }
}