use super::*;

pub(super) fn report_from_dto(
    report: PhysicsStepReportDto,
    events: Vec<newengine_physics_api::PhysicsEventDto>,
    key_to_entity: &BTreeMap<u64, EntityId>,
) -> PhysicsStepReport {
    let mut converted_events = Vec::new();
    for event in events {
        match event {
            newengine_physics_api::PhysicsEventDto::ContactBegin(contact) => {
                if let Some(contact) = contact_from_dto(contact, key_to_entity) {
                    converted_events.push(PhysicsEvent::ContactBegin(contact));
                }
            }
            newengine_physics_api::PhysicsEventDto::ContactPersist(contact) => {
                if let Some(contact) = contact_from_dto(contact, key_to_entity) {
                    converted_events.push(PhysicsEvent::ContactPersist(contact));
                }
            }
            newengine_physics_api::PhysicsEventDto::ContactEnd { a, b } => {
                if let (Some(a), Some(b)) = (
                    key_to_entity.get(&a).copied(),
                    key_to_entity.get(&b).copied(),
                ) {
                    converted_events.push(PhysicsEvent::ContactEnd {
                        a: a.into(),
                        b: b.into(),
                    });
                }
            }
            newengine_physics_api::PhysicsEventDto::BodyCreated { entity } => {
                if let Some(entity) = key_to_entity.get(&entity).copied() {
                    converted_events.push(PhysicsEvent::BodyCreated {
                        entity: entity.into(),
                    });
                }
            }
            newengine_physics_api::PhysicsEventDto::BodyDestroyed { entity } => {
                if let Some(entity) = key_to_entity.get(&entity).copied() {
                    converted_events.push(PhysicsEvent::BodyDestroyed {
                        entity: entity.into(),
                    });
                }
            }
        }
    }

    PhysicsStepReport {
        fixed_tick: report.fixed_tick,
        dt: report.dt,
        substeps: report.substeps,
        active_bodies: report.active_bodies,
        static_bodies: report.static_bodies,
        dynamic_bodies: report.dynamic_bodies,
        contacts: report.contacts,
        commands_applied: report.commands_applied,
        events: converted_events,
    }
}
