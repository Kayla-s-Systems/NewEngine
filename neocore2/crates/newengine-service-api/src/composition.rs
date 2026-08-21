/// Declarative capability slot required or optionally exposed by a runtime
/// composition. The host owns the slot; providers occupy it through normal
/// gateway routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCapabilitySlotSpec {
    pub gateway_id: &'static str,
    pub service_kind: &'static str,
    pub required: bool,
}

impl EngineCapabilitySlotSpec {
    #[inline]
    pub const fn new(
        gateway_id: &'static str,
        service_kind: &'static str,
        required: bool,
    ) -> Self {
        Self {
            gateway_id,
            service_kind,
            required,
        }
    }

    #[inline]
    pub const fn required(gateway_id: &'static str, service_kind: &'static str) -> Self {
        Self::new(gateway_id, service_kind, true)
    }

    #[inline]
    pub const fn optional(gateway_id: &'static str, service_kind: &'static str) -> Self {
        Self::new(gateway_id, service_kind, false)
    }
}

/// Pure data description of the engine shape requested from the host.
///
/// A composition does not instantiate implementations. It declares capability
/// slots; loaded plugins/runtime providers fill those slots by registering
/// compatible routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCompositionSpec {
    pub id: &'static str,
    pub slots: &'static [EngineCapabilitySlotSpec],
}

impl EngineCompositionSpec {
    #[inline]
    pub const fn new(id: &'static str, slots: &'static [EngineCapabilitySlotSpec]) -> Self {
        Self { id, slots }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOTS: &[EngineCapabilitySlotSpec] = &[
        EngineCapabilitySlotSpec::required("engine.render", "render"),
        EngineCapabilitySlotSpec::optional("engine.audio", "audio"),
    ];

    #[test]
    fn composition_is_only_slot_data() {
        let spec = EngineCompositionSpec::new("test.composition", SLOTS);
        assert_eq!(spec.id, "test.composition");
        assert!(spec.slots[0].required);
        assert!(!spec.slots[1].required);
    }
}
