#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{RenderApi, RenderGraphResourceLifetime, RenderTargetId};

#[derive(Clone, Copy, Debug)]
struct RetiredRenderTarget {
    id: RenderTargetId,
    retired_frame: u64,
    lifetime: RenderGraphResourceLifetime,
}

#[derive(Default)]
pub(super) struct RenderTargetLifetimeQueue {
    retired: Vec<RetiredRenderTarget>,
}

impl RenderTargetLifetimeQueue {
    #[inline]
    pub(super) fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub(super) fn retire_after_frames(&mut self, id: RenderTargetId, current_frame: u64, frames: u32) {
        self.retired.push(RetiredRenderTarget {
            id,
            retired_frame: current_frame,
            lifetime: RenderGraphResourceLifetime::Frames(frames.max(1)),
        });
    }

    pub(super) fn collect(&mut self, r: &mut dyn RenderApi, current_frame: u64) {
        let mut i = 0;
        while i < self.retired.len() {
            let retire = self.retired[i];
            if retire.is_expired(current_frame) {
                r.destroy_render_target(retire.id);
                self.retired.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
}

impl RetiredRenderTarget {
    #[inline]
    fn is_expired(self, current_frame: u64) -> bool {
        match self.lifetime {
            RenderGraphResourceLifetime::Persistent | RenderGraphResourceLifetime::External => false,
            RenderGraphResourceLifetime::TransientFrame => current_frame > self.retired_frame,
            RenderGraphResourceLifetime::Frames(frames) => {
                current_frame.saturating_sub(self.retired_frame) > frames as u64
            }
        }
    }
}
