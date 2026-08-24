use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use newengine_core::{EngineResult, EventHub};
use newengine_game_events_api::{
    validate_envelope_against_descriptor, GameMessageDescriptor, GameMessageDrainResponse,
    GameMessageEnvelope, GameMessageRegistrySnapshot, GAME_MESSAGE_DESCRIPTOR_CONTRACT,
};
use parking_lot::RwLock;

#[derive(Clone)]
pub struct GameMessageRegistry {
    inner: Arc<RwLock<RegistryState>>,
}

#[derive(Default)]
struct RegistryState {
    generation: u64,
    descriptors: BTreeMap<String, GameMessageDescriptor>,
}

impl Default for GameMessageRegistry {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryState::default())),
        }
    }
}

impl GameMessageRegistry {
    pub fn register(&self, mut descriptor: GameMessageDescriptor) -> Result<(), String> {
        descriptor.id = descriptor.id.trim().to_owned();
        descriptor.owner = descriptor.owner.trim().to_owned();
        descriptor.tags = descriptor
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_owned())
            .filter(|tag| !tag.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        descriptor.validate()?;
        let mut state = self.inner.write();
        if state.descriptors.contains_key(&descriptor.id) {
            return Err(format!(
                "game message already registered: {}",
                descriptor.id
            ));
        }
        state.descriptors.insert(descriptor.id.clone(), descriptor);
        state.generation = state.generation.wrapping_add(1);
        Ok(())
    }

    pub fn unregister(&self, id: &str) -> bool {
        let mut state = self.inner.write();
        let removed = state.descriptors.remove(id.trim()).is_some();
        if removed {
            state.generation = state.generation.wrapping_add(1);
        }
        removed
    }

    pub fn descriptor(&self, id: &str) -> Option<GameMessageDescriptor> {
        self.inner.read().descriptors.get(id.trim()).cloned()
    }

    pub fn validate(&self, envelope: &GameMessageEnvelope) -> Result<(), String> {
        let descriptor = self
            .descriptor(&envelope.id)
            .ok_or_else(|| format!("unregistered game message: {}", envelope.id))?;
        validate_envelope_against_descriptor(envelope, &descriptor)
    }

    pub fn snapshot(&self) -> GameMessageRegistrySnapshot {
        let state = self.inner.read();
        GameMessageRegistrySnapshot {
            contract: GAME_MESSAGE_DESCRIPTOR_CONTRACT.to_owned(),
            generation: state.generation,
            descriptors: state.descriptors.values().cloned().collect(),
        }
    }
}

#[derive(Clone)]
pub struct GameMessageQueue {
    inner: Arc<RwLock<QueueState>>,
    capacity: usize,
}

#[derive(Default)]
struct QueueState {
    next_sequence: u64,
    messages: VecDeque<GameMessageEnvelope>,
    dropped: u64,
}

impl Default for GameMessageQueue {
    fn default() -> Self {
        Self::new(4096)
    }
}

impl GameMessageQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(QueueState::default())),
            capacity: capacity.clamp(1, 1_000_000),
        }
    }

    pub fn publish(
        &self,
        registry: &GameMessageRegistry,
        mut envelope: GameMessageEnvelope,
    ) -> Result<GameMessageEnvelope, String> {
        registry.validate(&envelope)?;
        let mut state = self.inner.write();
        state.next_sequence = state.next_sequence.wrapping_add(1).max(1);
        envelope.sequence = state.next_sequence;
        if state.messages.len() >= self.capacity {
            state.messages.pop_front();
            state.dropped = state.dropped.saturating_add(1);
        }
        state.messages.push_back(envelope.clone());
        Ok(envelope)
    }

    pub fn drain(&self, max_messages: usize) -> GameMessageDrainResponse {
        let mut state = self.inner.write();
        let max_messages = max_messages.clamp(1, self.capacity);
        let mut messages = Vec::with_capacity(max_messages.min(state.messages.len()));
        for _ in 0..max_messages {
            let Some(message) = state.messages.pop_front() else {
                break;
            };
            messages.push(message);
        }
        GameMessageDrainResponse {
            messages,
            remaining: state.messages.len(),
            dropped: state.dropped,
        }
    }
}

/// Validate and publish a stable game message into the native typed EventHub.
/// Subscribers use `EventSub<GameMessageEnvelope>` while scripts/plugins keep the
/// provider-neutral string id + version contract.
pub fn publish_to_event_hub(
    events: &EventHub,
    registry: &GameMessageRegistry,
    envelope: GameMessageEnvelope,
) -> EngineResult<()> {
    registry
        .validate(&envelope)
        .map_err(newengine_core::EngineError::Other)?;
    events.publish(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> GameMessageRegistry {
        let registry = GameMessageRegistry::default();
        registry
            .register(GameMessageDescriptor {
                id: "game.player.spawned".into(),
                owner: "game.test".into(),
                ..Default::default()
            })
            .unwrap();
        registry
    }

    #[test]
    fn queue_is_bounded_and_assigns_sequences() {
        let registry = registry();
        let queue = GameMessageQueue::new(2);
        for frame in 1..=3 {
            queue
                .publish(
                    &registry,
                    GameMessageEnvelope {
                        id: "game.player.spawned".into(),
                        frame_index: frame,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let drained = queue.drain(8);
        assert_eq!(drained.messages.len(), 2);
        assert_eq!(drained.messages[0].sequence, 2);
        assert_eq!(drained.messages[1].sequence, 3);
        assert_eq!(drained.dropped, 1);
    }

    #[test]
    fn event_hub_bridge_uses_one_stable_envelope_type() {
        let registry = registry();
        let events = EventHub::new();
        let sub = events.subscribe::<GameMessageEnvelope>();
        publish_to_event_hub(
            &events,
            &registry,
            GameMessageEnvelope {
                id: "game.player.spawned".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(sub.try_recv().unwrap().id, "game.player.spawned");
    }
}

struct GameEventsService {
    registry: GameMessageRegistry,
    queue: GameMessageQueue,
    events: Option<EventHub>,
}

impl newengine_plugin_api::ServiceV1 for GameEventsService {
    fn id(&self) -> newengine_plugin_api::CapabilityId {
        abi_stable::std_types::RString::from(
            newengine_game_events_api::ENGINE_GAME_EVENTS_SERVICE_ID,
        )
    }

    fn describe(&self) -> abi_stable::std_types::RString {
        use newengine_game_events_api::game_events_method as method;
        abi_stable::std_types::RString::from(
            serde_json::json!({
                "id": newengine_game_events_api::ENGINE_GAME_EVENTS_SERVICE_ID,
                "version": 1,
                "protocol": "newengine.game-events/v1",
                "methods": [
                    method::INFO_JSON_V1,
                    method::REGISTER_JSON_V1,
                    method::UNREGISTER_JSON_V1,
                    method::DESCRIBE_JSON_V1,
                    method::PUBLISH_JSON_V1,
                    method::DRAIN_JSON_V1
                ],
                "features": [
                    "versioned-message-descriptors",
                    "bounded-message-queue",
                    "script-plugin-safe-envelope",
                    "native-eventhub-bridge"
                ]
            })
            .to_string(),
        )
    }

    fn call(
        &self,
        method: newengine_plugin_api::MethodName,
        payload: newengine_plugin_api::Blob,
    ) -> abi_stable::std_types::RResult<newengine_plugin_api::Blob, abi_stable::std_types::RString>
    {
        use newengine_game_events_api::{
            game_events_method as m, GameMessageDescriptor, GameMessageDrainRequest,
            GameMessageEnvelope, GameMessageIdRequest, GameMessageMutationResponse,
        };
        fn ok_json<T: serde::Serialize>(
            value: &T,
        ) -> abi_stable::std_types::RResult<
            newengine_plugin_api::Blob,
            abi_stable::std_types::RString,
        > {
            match serde_json::to_vec(value) {
                Ok(bytes) => {
                    abi_stable::std_types::RResult::ROk(newengine_plugin_api::Blob::from(bytes))
                }
                Err(error) => abi_stable::std_types::RResult::RErr(
                    abi_stable::std_types::RString::from(error.to_string()),
                ),
            }
        }
        fn decode<T: serde::de::DeserializeOwned>(
            payload: &newengine_plugin_api::Blob,
        ) -> Result<T, abi_stable::std_types::RString> {
            serde_json::from_slice(payload.as_slice())
                .map_err(|error| abi_stable::std_types::RString::from(error.to_string()))
        }

        let method = method.to_string();
        match method.as_str() {
            m::INFO_JSON_V1 => ok_json(&serde_json::json!({
                "service": newengine_game_events_api::ENGINE_GAME_EVENTS_SERVICE_ID,
                "contract": newengine_game_events_api::GAME_MESSAGE_CONTRACT,
                "registry_generation": self.registry.snapshot().generation
            })),
            m::REGISTER_JSON_V1 => {
                let descriptor = match decode::<GameMessageDescriptor>(&payload) {
                    Ok(value) => value,
                    Err(error) => return abi_stable::std_types::RResult::RErr(error),
                };
                let id = descriptor.id.trim().to_owned();
                match self.registry.register(descriptor) {
                    Ok(()) => ok_json(&GameMessageMutationResponse {
                        ok: true,
                        id,
                        message: "game message registered".to_owned(),
                    }),
                    Err(error) => abi_stable::std_types::RResult::RErr(
                        abi_stable::std_types::RString::from(error),
                    ),
                }
            }
            m::UNREGISTER_JSON_V1 => {
                let request = match decode::<GameMessageIdRequest>(&payload) {
                    Ok(value) => value,
                    Err(error) => return abi_stable::std_types::RResult::RErr(error),
                };
                let id = request.id.trim().to_owned();
                let removed = self.registry.unregister(&id);
                ok_json(&GameMessageMutationResponse {
                    ok: removed,
                    id,
                    message: if removed {
                        "game message unregistered"
                    } else {
                        "game message not found"
                    }
                    .to_owned(),
                })
            }
            m::DESCRIBE_JSON_V1 => ok_json(&self.registry.snapshot()),
            m::PUBLISH_JSON_V1 => {
                let envelope = match decode::<GameMessageEnvelope>(&payload) {
                    Ok(value) => value,
                    Err(error) => return abi_stable::std_types::RResult::RErr(error),
                };
                match self.queue.publish(&self.registry, envelope) {
                    Ok(published) => {
                        if let Some(events) = self.events.as_ref() {
                            if let Err(error) = events.publish(published.clone()) {
                                newengine_ulog_api::ulog::warn!(
                                    "game events: native EventHub bridge publish failed id='{}' err='{}'",
                                    published.id,
                                    error
                                );
                            }
                        }
                        ok_json(&published)
                    }
                    Err(error) => abi_stable::std_types::RResult::RErr(
                        abi_stable::std_types::RString::from(error),
                    ),
                }
            }
            m::DRAIN_JSON_V1 => {
                let request = if payload.is_empty() {
                    GameMessageDrainRequest { max_messages: 256 }
                } else {
                    match decode::<GameMessageDrainRequest>(&payload) {
                        Ok(value) => value,
                        Err(error) => return abi_stable::std_types::RResult::RErr(error),
                    }
                };
                ok_json(&self.queue.drain(request.max_messages.max(1)))
            }
            _ => abi_stable::std_types::RResult::RErr(abi_stable::std_types::RString::from(
                format!("unknown game-events method '{method}'"),
            )),
        }
    }
}

pub fn init_game_events_service(registry: GameMessageRegistry, queue: GameMessageQueue) {
    init_game_events_service_inner(registry, queue, None);
}

pub fn init_game_events_service_with_event_hub(
    registry: GameMessageRegistry,
    queue: GameMessageQueue,
    events: EventHub,
) {
    init_game_events_service_inner(registry, queue, Some(events));
}

fn init_game_events_service_inner(
    registry: GameMessageRegistry,
    queue: GameMessageQueue,
    events: Option<EventHub>,
) {
    let service = newengine_plugin_api::ServiceV1Dyn::from_value(
        GameEventsService {
            registry,
            queue,
            events,
        },
        abi_stable::sabi_trait::TD_Opaque,
    );
    let _ = newengine_plugin_host::host_register_service_impl(service);
}

#[cfg(test)]
mod service_tests {
    use super::*;
    use newengine_plugin_api::ServiceV1;

    #[test]
    fn service_registers_and_publishes_versioned_message() {
        let events = EventHub::new();
        let subscription = events.subscribe::<GameMessageEnvelope>();
        let service = GameEventsService {
            registry: GameMessageRegistry::default(),
            queue: GameMessageQueue::new(8),
            events: Some(events),
        };
        let descriptor = newengine_game_events_api::GameMessageDescriptor {
            id: "game.test.message".to_owned(),
            owner: "test".to_owned(),
            ..Default::default()
        };
        let result = service.call(
            abi_stable::std_types::RString::from(
                newengine_game_events_api::game_events_method::REGISTER_JSON_V1,
            ),
            newengine_plugin_api::Blob::from(serde_json::to_vec(&descriptor).unwrap()),
        );
        assert!(result.is_ok());
        let envelope = newengine_game_events_api::GameMessageEnvelope {
            id: descriptor.id,
            source: "test".to_owned(),
            ..Default::default()
        };
        let result = service.call(
            abi_stable::std_types::RString::from(
                newengine_game_events_api::game_events_method::PUBLISH_JSON_V1,
            ),
            newengine_plugin_api::Blob::from(serde_json::to_vec(&envelope).unwrap()),
        );
        assert!(result.is_ok());
        assert_eq!(service.queue.drain(8).messages.len(), 1);
        let bridged = subscription.try_recv().expect("bridged game message");
        assert_eq!(bridged.id, "game.test.message");
        assert_eq!(bridged.sequence, 1);
    }
}
