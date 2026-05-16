use newengine_physics_contracts::{PhysicsEvent, PhysicsStepReport};

#[derive(Clone, Debug, Default)]
pub struct PhysicsEventBus {
    events: Vec<PhysicsEvent>,
}

impl PhysicsEventBus {
    #[inline]
    pub fn publish_report(&mut self, report: &PhysicsStepReport) {
        self.events.extend(report.events.iter().copied());
    }

    #[inline]
    pub fn drain(&mut self) -> impl Iterator<Item = PhysicsEvent> + '_ {
        self.events.drain(..)
    }
}
