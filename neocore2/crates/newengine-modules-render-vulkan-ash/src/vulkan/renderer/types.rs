use ash::vk;

pub(super) const FRAMES_IN_FLIGHT: usize = 2;

#[derive(Clone, Copy)]
pub(super) struct FrameSync {
    pub(super) image_available: vk::Semaphore,
    pub(super) in_flight: vk::Fence,
}
