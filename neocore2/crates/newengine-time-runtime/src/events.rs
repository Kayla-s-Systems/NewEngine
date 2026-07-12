use std::collections::BTreeMap;

use newengine_time_api::{TimeCancelEventRequestV1, TimeDueEventsV1, TimeScheduledEventV1};

use crate::state::RuntimeHostedTimeState;

impl RuntimeHostedTimeState {
    pub(crate) fn schedule_event(
        &mut self,
        mut event: TimeScheduledEventV1,
    ) -> TimeScheduledEventV1 {
        if event.id.trim().is_empty() {
            event.id = format!("time.event.{}", self.scheduled_events.len() + 1);
        }
        self.scheduled_events
            .insert(event.id.clone(), event.clone());
        event
    }

    pub(crate) fn cancel_event(&mut self, request: TimeCancelEventRequestV1) -> TimeDueEventsV1 {
        let events = self
            .scheduled_events
            .remove(request.id.trim())
            .into_iter()
            .collect();
        TimeDueEventsV1 { events }
    }

    pub(crate) fn due_events(&mut self) -> TimeDueEventsV1 {
        let now_ns = self.monotonic_ns();
        let scheduled = std::mem::take(&mut self.scheduled_events);
        let mut pending = BTreeMap::new();
        let mut events = Vec::new();

        for (id, event) in scheduled {
            let due_tick = event
                .due_simulation_tick
                .is_some_and(|tick| tick <= self.tick);
            let due_time = event
                .due_monotonic_ns
                .is_some_and(|deadline| deadline <= now_ns);
            if due_tick || due_time {
                events.push(event);
            } else {
                pending.insert(id, event);
            }
        }

        self.scheduled_events = pending;
        TimeDueEventsV1 { events }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_event_extraction_keeps_future_events_without_id_clones() {
        let mut state = RuntimeHostedTimeState {
            tick: 10,
            ..RuntimeHostedTimeState::default()
        };
        state.schedule_event(TimeScheduledEventV1 {
            id: "due".to_owned(),
            due_simulation_tick: Some(10),
            ..Default::default()
        });
        state.schedule_event(TimeScheduledEventV1 {
            id: "future".to_owned(),
            due_simulation_tick: Some(11),
            ..Default::default()
        });

        let due = state.due_events();
        assert_eq!(due.events.len(), 1);
        assert_eq!(due.events[0].id, "due");
        assert!(state.scheduled_events.contains_key("future"));
    }
}
