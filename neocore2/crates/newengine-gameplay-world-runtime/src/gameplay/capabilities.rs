use std::collections::BTreeMap;
use std::sync::Arc;

use newengine_ecs::World;

/// Stable engine-provided capability ids. These describe functionality only; projects own all
/// event ids, asset references and wiring that decide when/how the functionality is used.
pub const GAMEPLAY_CAPABILITY_AUDIO_PLAY_V1: &str = "engine.audio.play.v1";
pub const GAMEPLAY_CAPABILITY_AUDIO_CUE_PLAY_V1: &str = "engine.audio.cue.play.v1";
pub const GAMEPLAY_CAPABILITY_AUDIO_PRELOAD_V1: &str = "engine.audio.preload.v1";

#[derive(Clone, Debug, PartialEq)]
pub struct GameplayCapabilityRequest {
    pub capability: String,
    pub source: Option<u64>,
    pub target: Option<u64>,
    pub payload: serde_json::Value,
}

impl GameplayCapabilityRequest {
    pub fn new(capability: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            capability: capability.into(),
            source: None,
            target: None,
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let capability = self.capability.trim();
        if capability.is_empty() || capability.len() > 256 {
            return Err("gameplay capability id must contain 1..=256 bytes".to_owned());
        }
        if capability.chars().any(char::is_control) {
            return Err(format!(
                "gameplay capability id contains control characters: '{capability}'"
            ));
        }
        let bytes = serde_json::to_vec(&self.payload).map_err(|error| {
            format!("serialize gameplay capability '{capability}' payload: {error}")
        })?;
        if bytes.len() > 64 * 1024 {
            return Err(format!(
                "gameplay capability '{capability}' payload exceeds 65536 bytes: {}",
                bytes.len()
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct GameplayCapabilityBus {
    requests: Vec<GameplayCapabilityRequest>,
    dropped_requests: u64,
}

impl GameplayCapabilityBus {
    pub const MAX_RETAINED_REQUESTS: usize = 1024;

    pub fn push(&mut self, request: GameplayCapabilityRequest) -> Result<(), String> {
        request.validate()?;
        if self.requests.len() >= Self::MAX_RETAINED_REQUESTS {
            let overflow = self.requests.len() + 1 - Self::MAX_RETAINED_REQUESTS;
            self.requests.drain(0..overflow);
            self.dropped_requests = self.dropped_requests.saturating_add(overflow as u64);
        }
        self.requests.push(request);
        Ok(())
    }

    #[inline]
    pub fn drain(&mut self) -> Vec<GameplayCapabilityRequest> {
        std::mem::take(&mut self.requests)
    }

    #[inline]
    pub fn pending(&self) -> &[GameplayCapabilityRequest] {
        &self.requests
    }

    #[inline]
    pub fn dropped_requests(&self) -> u64 {
        self.dropped_requests
    }
}

pub trait GameplayCapabilityProvider: Send + Sync {
    fn capability_id(&self) -> &'static str;
    fn invoke(&self, world: &mut World, request: &GameplayCapabilityRequest) -> Result<(), String>;
}

#[derive(Clone, Default)]
pub struct GameplayCapabilityRegistry {
    providers: BTreeMap<String, Arc<dyn GameplayCapabilityProvider>>,
}

impl core::fmt::Debug for GameplayCapabilityRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GameplayCapabilityRegistry")
            .field("capabilities", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl GameplayCapabilityRegistry {
    pub fn register(
        &mut self,
        provider: Arc<dyn GameplayCapabilityProvider>,
    ) -> Result<(), String> {
        let id = provider.capability_id().trim();
        if id.is_empty() {
            return Err("gameplay capability provider id must not be empty".to_owned());
        }
        let key = id.to_ascii_lowercase();
        if self.providers.contains_key(&key) {
            return Err(format!("duplicate gameplay capability provider '{id}'"));
        }
        self.providers.insert(key, provider);
        Ok(())
    }

    pub fn register_if_absent(&mut self, provider: Arc<dyn GameplayCapabilityProvider>) {
        let key = provider.capability_id().trim().to_ascii_lowercase();
        if !key.is_empty() {
            self.providers.entry(key).or_insert(provider);
        }
    }

    #[inline]
    pub fn contains(&self, capability: &str) -> bool {
        let capability = capability.trim();
        if self.providers.contains_key(capability) {
            return true;
        }
        capability.bytes().any(|byte| byte.is_ascii_uppercase())
            && self
                .providers
                .contains_key(capability.to_ascii_lowercase().as_str())
    }

    #[inline]
    fn provider(&self, capability: &str) -> Option<Arc<dyn GameplayCapabilityProvider>> {
        let capability = capability.trim();
        if let Some(provider) = self.providers.get(capability) {
            return Some(Arc::clone(provider));
        }
        if capability.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return self
                .providers
                .get(capability.to_ascii_lowercase().as_str())
                .cloned();
        }
        None
    }

    pub fn capabilities(&self) -> impl Iterator<Item = &str> {
        self.providers.keys().map(String::as_str)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameplayCapabilityDispatchReport {
    pub requested: usize,
    pub executed: usize,
    pub missing: Vec<String>,
    pub failed: Vec<String>,
}

pub fn request_gameplay_capability(
    world: &mut World,
    request: GameplayCapabilityRequest,
) -> Result<(), String> {
    world
        .resource_mut_or_insert_default::<GameplayCapabilityBus>()
        .push(request)
}

pub fn drain_gameplay_capability_requests(world: &mut World) -> Vec<GameplayCapabilityRequest> {
    world
        .resource_mut::<GameplayCapabilityBus>()
        .map(GameplayCapabilityBus::drain)
        .unwrap_or_default()
}

pub fn dispatch_gameplay_capabilities(world: &mut World) -> GameplayCapabilityDispatchReport {
    let requests = drain_gameplay_capability_requests(world);
    let registry = world
        .resource::<GameplayCapabilityRegistry>()
        .cloned()
        .unwrap_or_default();
    let mut report = GameplayCapabilityDispatchReport {
        requested: requests.len(),
        ..GameplayCapabilityDispatchReport::default()
    };

    for request in requests {
        let Some(provider) = registry.provider(&request.capability) else {
            report.missing.push(request.capability);
            continue;
        };
        match provider.invoke(world, &request) {
            Ok(()) => report.executed += 1,
            Err(error) => report
                .failed
                .push(format!("{}: {error}", request.capability)),
        }
    }
    report
}

struct AudioPlayCapability;

impl GameplayCapabilityProvider for AudioPlayCapability {
    fn capability_id(&self) -> &'static str {
        GAMEPLAY_CAPABILITY_AUDIO_PLAY_V1
    }

    fn invoke(
        &self,
        _world: &mut World,
        request: &GameplayCapabilityRequest,
    ) -> Result<(), String> {
        let payload = request
            .payload
            .as_object()
            .ok_or("engine.audio.play.v1 payload must be an object")?;
        if payload
            .get("cue")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|cue| !cue.trim().is_empty())
        {
            return Err(
                "engine.audio.play.v1 no longer accepts payload.cue/YSNCD; migrate to native XVAG and use payload.clip"
                    .to_owned(),
            );
        }
        let clip = payload
            .get("clip")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|clip| !clip.is_empty())
            .ok_or("engine.audio.play.v1 requires non-empty payload.clip (native XVAG)")?;
        if !clip.to_ascii_lowercase().ends_with(".xvag") {
            return Err(format!(
                "engine.audio.play.v1 first-party runtime source must be native .xvag: '{clip}'"
            ));
        }

        let route = payload
            .get("route")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|route| !route.is_empty())
            .ok_or("engine.audio.play.v1 requires non-empty project-authored payload.route")?;
        let position = payload
            .get("position")
            .map(|position| {
                serde_json::from_value::<[f32; 3]>(position.clone())
                    .map_err(|error| format!("invalid audio position: {error}"))
            })
            .transpose()?;
        let gain = payload
            .get("gain")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1.0) as f32;
        let pitch = payload
            .get("pitch")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1.0) as f32;

        let mut play = newengine_audio_api::AudioPlayRequest::new(clip.to_owned());
        play.route = newengine_audio_api::AudioRouteId::new(route.to_owned());
        play.route.validate()?;
        play.gain = gain;
        play.speed = pitch;
        play.looping = payload
            .get("looping")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        play.spatial =
            position.map(|position| newengine_audio_api::AudioSpatialParams { position });
        if let Some(attenuation) = payload.get("attenuation") {
            play.attenuation = Some(
                serde_json::from_value::<newengine_audio_api::AudioAttenuationSettings>(
                    attenuation.clone(),
                )
                .map_err(|error| format!("invalid audio attenuation: {error}"))?
                .sanitized(),
            );
        }
        let play = play.sanitized();
        return match newengine_audio_client::play_audio_clip(&play).map_err(|error| {
            format!("audio gateway failed clip='{clip}' route='{route}': {error}")
        })? {
            Some(ack) if ack.accepted => {
                newengine_ulog_api::ulog::trace!(
                        "gameplay audio XVAG accepted clip='{}' route='{}' provider='{}' voice_id={:?} virtualized={}",
                        clip,
                        route,
                        ack.provider,
                        ack.voice_id,
                        ack.virtualized,
                    );
                Ok(())
            }
            Some(ack) => Err(format!(
                "audio play rejected clip='{clip}' route='{route}' provider='{}' message='{}'",
                ack.provider, ack.message
            )),
            None => Err(format!(
                "audio play capability unavailable clip='{clip}' route='{route}'"
            )),
        };
    }
}

struct AudioCuePlayCapability;

impl GameplayCapabilityProvider for AudioCuePlayCapability {
    fn capability_id(&self) -> &'static str {
        GAMEPLAY_CAPABILITY_AUDIO_CUE_PLAY_V1
    }

    fn invoke(
        &self,
        _world: &mut World,
        request: &GameplayCapabilityRequest,
    ) -> Result<(), String> {
        let payload = request
            .payload
            .as_object()
            .ok_or("engine.audio.cue.play.v1 payload must be an object")?;
        let cue = payload
            .get("cue")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|cue| !cue.is_empty())
            .ok_or("engine.audio.cue.play.v1 requires non-empty payload.cue (YSNCD entry)")?;
        let parsed = newengine_assets_api::parse_asset_reference(cue)
            .map_err(|error| format!("invalid YSNCD cue reference '{cue}': {error}"))?;
        if !parsed.has_extension("ysncd")
            || parsed
                .entry
                .as_deref()
                .is_none_or(|entry| entry.trim().is_empty())
        {
            return Err(format!(
                "engine.audio.cue.play.v1 requires a YSNCD entry reference '<path>.ysncd@<cue>': '{cue}'"
            ));
        }

        let route = payload
            .get("route")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|route| !route.is_empty())
            .ok_or("engine.audio.cue.play.v1 requires non-empty project-authored payload.route")?;
        let position = payload
            .get("position")
            .map(|position| {
                serde_json::from_value::<[f32; 3]>(position.clone())
                    .map_err(|error| format!("invalid audio cue position: {error}"))
            })
            .transpose()?;
        let gain = payload
            .get("gain")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1.0) as f32;
        let pitch = payload
            .get("pitch")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1.0) as f32;
        let seed = payload.get("seed").and_then(serde_json::Value::as_u64);
        let scope_id = payload
            .get("scope_id")
            .and_then(serde_json::Value::as_u64)
            .or(request.source)
            .filter(|id| *id != 0);

        let mut play = newengine_audio_api::AudioCuePlayRequest::new(cue.to_owned());
        play.route = newengine_audio_api::AudioRouteId::new(route.to_owned());
        play.route.validate()?;
        play.position = position;
        play.gain = gain;
        play.pitch = pitch;
        play.seed = seed;
        play.scope_id = scope_id;
        let play = play.sanitized();

        match newengine_audio_client::play_audio_cue(&play)
            .map_err(|error| format!("audio gateway failed cue='{cue}' route='{route}': {error}"))?
        {
            Some(ack) if ack.accepted => {
                newengine_ulog_api::ulog::trace!(
                    "gameplay audio YSNCD accepted cue='{}' route='{}' provider='{}' voice_id={:?} virtualized={}",
                    cue,
                    route,
                    ack.provider,
                    ack.voice_id,
                    ack.virtualized,
                );
                Ok(())
            }
            Some(ack) => Err(format!(
                "audio cue play rejected cue='{cue}' route='{route}' provider='{}' message='{}'",
                ack.provider, ack.message
            )),
            None => Err(format!(
                "audio cue play capability unavailable cue='{cue}' route='{route}'"
            )),
        }
    }
}

struct AudioPreloadCapability;

impl GameplayCapabilityProvider for AudioPreloadCapability {
    fn capability_id(&self) -> &'static str {
        GAMEPLAY_CAPABILITY_AUDIO_PRELOAD_V1
    }

    fn invoke(
        &self,
        _world: &mut World,
        request: &GameplayCapabilityRequest,
    ) -> Result<(), String> {
        if request
            .payload
            .get("cue")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|cue| !cue.trim().is_empty())
        {
            return Err(
                "engine.audio.preload.v1 no longer accepts payload.cue/YSNCD; use payload.clip with native XVAG"
                    .to_owned(),
            );
        }
        let clip = request
            .payload
            .get("clip")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|clip| !clip.is_empty())
            .ok_or("engine.audio.preload.v1 requires non-empty payload.clip (native XVAG)")?;
        if !clip.to_ascii_lowercase().ends_with(".xvag") {
            return Err(format!(
                "engine.audio.preload.v1 first-party runtime source must be native .xvag: '{clip}'"
            ));
        }
        let preload = newengine_audio_api::AudioPreloadRequest {
            clip: newengine_audio_api::AudioClipRef::new(clip.to_owned()),
        };
        match newengine_audio_client::preload_audio_clip(&preload)
            .map_err(|error| format!("audio preload gateway failed clip='{clip}': {error}"))?
        {
            Some(ack) if ack.accepted => Ok(()),
            Some(ack) => Err(format!(
                "audio preload rejected clip='{clip}' provider='{}' diagnostics='{}'",
                ack.provider,
                ack.diagnostics.join(" | ")
            )),
            None => Err(format!(
                "audio preload capability unavailable for clip='{clip}'"
            )),
        }
    }
}

/// Installs engine-provided capability implementations. This only exposes functionality; it does
/// not trigger any action. Projects decide whether/when to request these capabilities.
pub fn ensure_builtin_gameplay_capabilities(world: &mut World) {
    let registry = world.resource_mut_or_insert_default::<GameplayCapabilityRegistry>();
    // This executes on every fixed tick. Avoid allocating/dropping three Arc control blocks
    // once the built-ins are already resident; the common path is now allocation-free.
    if !registry.contains(GAMEPLAY_CAPABILITY_AUDIO_PLAY_V1) {
        registry.register_if_absent(Arc::new(AudioPlayCapability));
    }
    if !registry.contains(GAMEPLAY_CAPABILITY_AUDIO_CUE_PLAY_V1) {
        registry.register_if_absent(Arc::new(AudioCuePlayCapability));
    }
    if !registry.contains(GAMEPLAY_CAPABILITY_AUDIO_PRELOAD_V1) {
        registry.register_if_absent(Arc::new(AudioPreloadCapability));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ProbeCapability;
    impl GameplayCapabilityProvider for ProbeCapability {
        fn capability_id(&self) -> &'static str {
            "test.capability.v1"
        }
        fn invoke(
            &self,
            world: &mut World,
            request: &GameplayCapabilityRequest,
        ) -> Result<(), String> {
            world.insert_resource(request.payload.clone());
            Ok(())
        }
    }

    #[test]
    fn builtin_capability_ensure_is_idempotent_and_keeps_provider_allocations() {
        let mut world = World::new();
        ensure_builtin_gameplay_capabilities(&mut world);
        let first = world
            .resource::<GameplayCapabilityRegistry>()
            .unwrap()
            .provider(GAMEPLAY_CAPABILITY_AUDIO_CUE_PLAY_V1)
            .unwrap();
        ensure_builtin_gameplay_capabilities(&mut world);
        let second = world
            .resource::<GameplayCapabilityRegistry>()
            .unwrap()
            .provider(GAMEPLAY_CAPABILITY_AUDIO_CUE_PLAY_V1)
            .unwrap();
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn lowercase_capability_lookup_uses_exact_registered_key() {
        let mut registry = GameplayCapabilityRegistry::default();
        registry.register(Arc::new(ProbeCapability)).unwrap();
        assert!(registry.contains("test.capability.v1"));
        assert!(registry.contains("TEST.CAPABILITY.V1"));
        assert!(registry.provider("test.capability.v1").is_some());
        assert!(registry.provider("TEST.CAPABILITY.V1").is_some());
    }

    #[test]
    fn arbitrary_capability_provider_is_project_composable() {
        let mut world = World::new();
        let mut registry = GameplayCapabilityRegistry::default();
        registry.register(Arc::new(ProbeCapability)).unwrap();
        world.insert_resource(registry);
        request_gameplay_capability(
            &mut world,
            GameplayCapabilityRequest::new("test.capability.v1", serde_json::json!({"value": 7})),
        )
        .unwrap();
        let report = dispatch_gameplay_capabilities(&mut world);
        assert_eq!(report.requested, 1);
        assert_eq!(report.executed, 1);
        assert_eq!(world.resource::<serde_json::Value>().unwrap()["value"], 7);
    }

    #[test]
    fn project_audio_capability_requires_authored_route_before_gateway_dispatch() {
        let mut world = World::new();
        let request = GameplayCapabilityRequest::new(
            GAMEPLAY_CAPABILITY_AUDIO_PLAY_V1,
            serde_json::json!({"clip": "shared/audio/test.xvag"}),
        );
        let error = AudioPlayCapability
            .invoke(&mut world, &request)
            .expect_err("missing project route must fail before audio gateway");
        assert!(error.contains("project-authored payload.route"));
    }

    #[test]
    fn project_audio_capability_rejects_legacy_cue_even_when_native_clip_is_present() {
        let mut world = World::new();
        let request = GameplayCapabilityRequest::new(
            GAMEPLAY_CAPABILITY_AUDIO_PLAY_V1,
            serde_json::json!({
                "clip": "audio/footsteps/stone/walk_01.xvag",
                "cue": "shared/audio/footsteps/footsteps.ysncd@stone_walk",
                "route": "room.world.foley"
            }),
        );
        let error = AudioPlayCapability
            .invoke(&mut world, &request)
            .expect_err("legacy cue must be rejected at gameplay boundary");
        assert!(error.contains("no longer accepts payload.cue/YSNCD"));
    }

    #[test]
    fn project_audio_cue_capability_rejects_non_ysncd_reference_before_gateway_dispatch() {
        let mut world = World::new();
        let request = GameplayCapabilityRequest::new(
            GAMEPLAY_CAPABILITY_AUDIO_CUE_PLAY_V1,
            serde_json::json!({
                "cue": "audio/footsteps/stone/walk_01.xvag",
                "route": "room.world.foley"
            }),
        );
        let error = AudioCuePlayCapability
            .invoke(&mut world, &request)
            .expect_err("cue capability must require an authored YSNCD entry");
        assert!(error.contains("requires a YSNCD entry reference"));
    }

    #[test]
    fn unknown_capability_is_reported_not_silently_reinterpreted() {
        let mut world = World::new();
        request_gameplay_capability(
            &mut world,
            GameplayCapabilityRequest::new("project.missing.v1", serde_json::Value::Null),
        )
        .unwrap();
        let report = dispatch_gameplay_capabilities(&mut world);
        assert_eq!(report.executed, 0);
        assert_eq!(report.missing, vec!["project.missing.v1"]);
    }
}
