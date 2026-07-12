#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::events::EventSub;
use newengine_core::render::{
    BindGroupId, BufferId, RenderApi, RenderBackendEvent, RenderBackendEventKind, RenderTargetId,
};

#[derive(Clone, Copy, Debug)]
enum RetiredGpuResource {
    RenderTarget(RenderTargetId),
    BindGroup(BindGroupId),
    Buffer(BufferId),
}

#[derive(Clone, Copy, Debug)]
struct RetiredGpuResourceEntry {
    resource: RetiredGpuResource,
    after_frame: u64,
}

#[derive(Default)]
pub(super) struct RenderGpuLifetimeQueue {
    retired: Vec<RetiredGpuResourceEntry>,
    latest_completed_frame: u64,
    event_sub: Option<EventSub<RenderBackendEvent>>,
}

impl RenderGpuLifetimeQueue {
    #[inline]
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn subscribe(&mut self, events: &newengine_core::EventHub) {
        if self.event_sub.is_none() {
            self.event_sub = Some(events.subscribe_filtered::<RenderBackendEvent, _>(|event| {
                matches!(event.kind, RenderBackendEventKind::FrameCompleted)
            }));
        }
    }

    #[inline]
    pub(super) fn retire_render_target_after_frame(&mut self, id: RenderTargetId, frame: u64) {
        self.retired.push(RetiredGpuResourceEntry {
            resource: RetiredGpuResource::RenderTarget(id),
            after_frame: frame,
        });
    }

    #[inline]
    pub(super) fn retire_bind_group_after_frame(&mut self, id: BindGroupId, frame: u64) {
        self.retired.push(RetiredGpuResourceEntry {
            resource: RetiredGpuResource::BindGroup(id),
            after_frame: frame,
        });
    }

    #[inline]
    pub(super) fn retire_buffer_after_frame(&mut self, id: BufferId, frame: u64) {
        self.retired.push(RetiredGpuResourceEntry {
            resource: RetiredGpuResource::Buffer(id),
            after_frame: frame,
        });
    }

    pub(super) fn drain_events(&mut self) {
        let Some(sub) = self.event_sub.as_ref() else {
            return;
        };
        let mut latest_completed_frame = self.latest_completed_frame;
        sub.drain(|event| {
            if matches!(event.kind, RenderBackendEventKind::FrameCompleted) {
                latest_completed_frame = latest_completed_frame.max(event.frame_index);
            }
        });
        self.latest_completed_frame = latest_completed_frame;
    }

    pub(super) fn collect(&mut self, r: &mut dyn RenderApi) {
        self.drain_events();
        if self.latest_completed_frame == 0 {
            return;
        }

        let mut i = 0usize;
        while i < self.retired.len() {
            if self.retired[i].after_frame <= self.latest_completed_frame {
                let entry = self.retired.swap_remove(i);
                entry.destroy(r);
            } else {
                i += 1;
            }
        }
    }
}

impl RetiredGpuResourceEntry {
    #[inline]
    fn destroy(self, r: &mut dyn RenderApi) {
        match self.resource {
            RetiredGpuResource::RenderTarget(id) => r.destroy_render_target(id),
            RetiredGpuResource::BindGroup(id) => r.destroy_bind_group(id),
            RetiredGpuResource::Buffer(id) => r.destroy_buffer(id),
        }
    }
}
