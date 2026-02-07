use ash::vk;

pub(super) const FRAMES_IN_FLIGHT: usize = 2;

#[derive(Clone, Copy)]

pub(super) struct FrameSync {
    #[warn(private_interfaces)]
    pub(super) image_available: vk::Semaphore,
    pub(super) render_finished: vk::Semaphore,
    pub(super) in_flight: vk::Fence,
}
