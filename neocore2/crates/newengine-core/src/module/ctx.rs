use crate::frame::Frame;
use crate::module::{Bus, Resources, Services};
use crate::sched::Scheduler;

/// Context passed to modules.
///
/// This prevents modules from taking `&mut Engine` (god object problem).
pub struct ModuleCtx<'a, E: Send + 'static> {
    services: &'a dyn Services,
    resources: &'a mut Resources,
    bus: &'a Bus<E>,
    scheduler: &'a mut Scheduler,
    exit: &'a mut bool,

    /// Frame snapshot for the current stage (stored by value).
    frame: Option<Frame>,
}

impl<'a, E: Send + 'static> ModuleCtx<'a, E> {
    #[inline]
    pub(crate) fn new(
        services: &'a dyn Services,
        resources: &'a mut Resources,
        bus: &'a Bus<E>,
        scheduler: &'a mut Scheduler,
        exit: &'a mut bool,
    ) -> Self {
        Self {
            services,
            resources,
            bus,
            scheduler,
            exit,
            frame: None,
        }
    }

    /// Attaches a frame snapshot to the context.
    #[inline]
    pub fn set_frame(&mut self, frame: &Frame) {
        self.frame = Some(*frame);
    }

    /// Returns the current frame snapshot, if attached.
    #[inline]
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    #[inline]
    pub fn services(&self) -> &dyn Services {
        self.services
    }

    #[inline]
    pub fn resources(&mut self) -> &mut Resources {
        self.resources
    }

    #[inline]
    pub fn bus(&self) -> &Bus<E> {
        self.bus
    }

    #[inline]
    pub fn scheduler(&mut self) -> &mut Scheduler {
        self.scheduler
    }

    #[inline]
    pub fn request_exit(&mut self) {
        *self.exit = true;
    }

    #[inline]
    pub fn is_exit_requested(&self) -> bool {
        *self.exit
    }
}