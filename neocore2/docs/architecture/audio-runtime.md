# Audio Runtime Foundation

`engine.audio` is the stable engine gateway. `newengine-audio-api` owns DTOs and
method names; concrete implementations occupy `audio.backend` routes.

## Runtime route

`newengine-audio-runtime` registers `newengine.audio.native` / `engine.audio.native`
with priority 100 when an OS output device is available. The existing
`engine.audio.echo` semantic queue remains a priority-0 fallback, so server,
headless, CI, and machines without audio hardware do not fail startup.

SoundCue authoring/semantic decode is deliberately separate from physical output:

AssetManager supports descriptor-driven runtime codecs: when no extension-owned codec DLL is registered internally, `asset.decode_v1` resolves the file-type descriptor through `engine.assets.types`, validates its `handler_service` / `read_method` / advertised output, packs the already-resolved VFS bytes into `NECD v1`, and invokes that provider service. Internal codec DLL failures remain authoritative and do not silently fall through.


- `newengine.audio.soundcue-codec` is device-independent and implements `asset.decode_v1` for `.soundcue` -> `audio.sound_cue.v1`;
- AssetManager resolves the VFS source bytes exactly once and supplies them to the codec over `newengine.codec.decode-wire.v1` (`NECD`);
- the `.soundcue` descriptor names the codec service as `handler_service` and `engine.audio` only as its semantic gateway;
- `newengine.audio.soundcue-codec` performs only byte-semantic JSON validation/normalization and never opens files, VFS mounts, or an audio device;
- `engine.audio.native` asks AssetManager for `audio.sound_cue.v1`, consumes that semantic DTO, and never parses authored cue JSON itself;
- headless/editor tooling can therefore inspect/decode SoundCue assets without an output device.

The native provider currently owns:

- OS output stream and mixer lifetime;
- short/ordinary compressed clip bytes through `AssetServiceClient::raw_bytes_v1`; long-form stream bytes through seekable `AssetServiceClient::raw_range_v1`; SoundCue semantics through `AssetServiceClient::decode_v1`;
- clip byte cache (default 128 MiB, `NEWENGINE_AUDIO_CACHE_MB` override);
- WAV/MP3/Ogg Vorbis/FLAC decode;
- 2D and dynamic spatial voices;
- listener pose and stereo ear separation;
- Master/Music/SFX/UI/Dialogue/Ambience gain controls;
- per-voice stop, pause/resume, gain, speed, and spatial position;
- a hard physical mixer budget with logical voice virtualization;
- resumable virtual loop/one-shot timelines using decoder seek and source-time tracking;
- deterministic arbitration by authored priority, current audibility, distance, and stable voice id;
- authored distance attenuation (`linear`, `smoothstep`, `inverse`, `exponential`, `custom`);
- semantic UI tones so existing UI feedback events remain asset-independent;
- SoundCue weighted selection, gain/pitch variation, concurrency, priority, spatial policy,
  and attenuation policy;
- diagnostics through `diagnostics_json_v1`, including logical/physical/virtual voice counts.

`NEWENGINE_AUDIO_DISABLED=1` disables the native route explicitly.
`NEWENGINE_AUDIO_MAX_PHYSICAL_VOICES` sets the physical voice budget (default 64,
clamped to 1..512). Logical voices beyond that budget remain virtual when their
source can be resumed safely.

## Voice budget and virtualization

A voice id now represents a **logical voice**, not a permanent Rodio player:

```text
AudioEmitter / play request
        в”‚
        в–ј
Logical Voice
  source timeline
  priority
  bus/gain
  spatial position
  attenuation
        в”‚
        в–ј
Voice Arbiter  в”Ђв”Ђ max_physical_voices
        в”‚
        в”њв”Ђв”Ђ Physical Voice в†’ Rodio Player / SpatialPlayer
        в”‚
        в””в”Ђв”Ђ Virtual Voice  в†’ source-time advances without mixer ownership
```

The arbiter ranks audible, unpaused logical voices by:

1. authored `priority` (descending),
2. effective audibility after bus gain + attenuation + acoustic transmission (descending),
3. listener distance (ascending),
4. already-physical state for exact ties,
5. stable voice id.

Camera listener updates are the normal once-per-presentation-frame rebalance point.
Emitter position updates still refresh current physical gain immediately; a moving
virtual emitter is promoted on the next listener arbitration pass.

Demotion captures the source-time position from the physical player, releases the
mixer object, and advances source-time virtually. Promotion reconstructs the player
from cached VFS bytes and seeks to the corresponding output position. Playback speed
changes preserve source-time explicitly, so later demotion/promotion does not jump.
Completed virtual one-shots are pruned just like physical voices.

Non-virtualizable sources are never allowed to violate the hard physical cap. The
current production decoders (`wav`, `mp3`, `vorbis`, `flac`) are Rodio/Symphonia
paths and support the resume contract; generated UI feedback tones are treated as
short, high-priority physical-only voices.

## Authored attenuation

Attenuation is optional and backward compatible. When absent, a spatial voice keeps
its previous full-gain distance behavior. When authored, the policy is evaluated both
for physical gain and for voice arbitration, so inaudible distant voices naturally
become virtual.

```json
"attenuation": {
  "min_distance": 2.0,
  "max_distance": 140.0,
  "curve": "inverse",
  "rolloff": 0.75,
  "curve_points": []
}
```

Built-in curves are `linear`, `smoothstep`, `inverse`, and `exponential`. `custom`
uses normalized `[distance_fraction, gain]` points and linearly interpolates between
them; authoring sanitation sorts/clamps points and supplies `[0,1]` / `[1,0]`
endpoints when omitted.

The rifle SoundCue corpus now carries concrete authored attenuation policies in both
`Shared/Source/audio/weapon/rifle` and `Shared/Content/audio/weapon/rifle`.

## VFS ownership

The backend does not open authored files directly. `AudioClipRef` and
`SoundCueRef` contain VFS logical paths only. Absolute paths, parent traversal,
and `@entry` selectors are rejected at the audio boundary.

Canonical cross-project content lives at physical `Shared/Content`, but it is
mounted as the `shared/` VFS namespace. Consequently the runtime contract is:

```text
physical: NorthStar/Shared/Content/audio/weapon/rifle/fire.wav
VFS:      shared/audio/weapon/rifle/fire.wav
```

The physical root remains an AssetManager concern; gameplay and audio providers
never discover `Shared/Content` themselves.

## SoundCue asset

`.soundcue` is registered as `audio.sound_cue` with
`newengine.audio.soundcue.json.v1`. It is a VFS-backed plain-text authored asset
whose semantic owner is `engine.audio` and whose byte owner is `engine.assets`.

Example:

```json
{
  "version": 1,
  "clips": [
    {
      "clip": { "uri": "shared/audio/weapon/rifle/fire.wav" },
      "weight": 1.0,
      "gain": 1.0,
      "pitch": 1.0
    }
  ],
  "gain_range": [0.96, 1.04],
  "pitch_range": [0.98, 1.02],
  "bus": "sfx",
  "looping": false,
  "concurrency_group": "weapon.rifle.fire",
  "priority": 100,
  "spatial_policy": "spatial",
  "attenuation": {
    "min_distance": 2.0,
    "max_distance": 140.0,
    "curve": "inverse",
    "rolloff": 0.75,
    "curve_points": []
  }
}
```

The first corpus is under `Shared/{Source,Content}/audio/weapon/rifle` and wraps
existing `fire`, `reload`, `equip`, `unequip`, `empty`, and `shell_eject` WAVs.

## ECS audio

`AudioEmitter` is durable authored ECS state:

```text
Transform + AudioEmitter
        в”‚
        в–ј
AudioSceneRuntimeModule
        в”‚
        в–ј
     engine.audio
```

`AudioEmitter` contains cue, enabled/autoplay, gain, whether the entity supplies a
spatial position, and the authored `AudioOcclusionSettings` probe/smoothing policy.
Provider-local voice handles are mirrored only in
`AudioEmitterRuntime`; they are not authored or restored as durable PIE state.
The stable semantic component id is `audio.emitter`. `engine.ecs`
`SetComponentJson` / `RemoveComponentJson` can author/remove this component from
editor/tooling without exposing native ECS storage or `World` across the service
boundary.
The runtime module snapshots transforms before calling services, so scene locks
are never held across VFS/decode/audio-provider work.

The module also handles:

- entity/cue lifetime and provider-route replacement;
- completed one-shot voices without accidental frame-by-frame retriggering;
- transform-driven spatial voice updates;
- stale entity cleanup;
- bounded retry for temporarily rejected/ unavailable cues;
- concurrency/priority rejection without log spam.

## Physics occlusion / obstruction

Spatial ECS emitters can opt into provider-neutral acoustic visibility probes. The
physics implementation remains an implementation detail; audio contributes ordinary
`PhysicsQueryDto::Ray` entries to the existing fixed-step `engine.physics` batch through
`GameplayPhysicsQueryProvider`.

```text
resolved CameraFrameSnapshot
        │
        ▼
AudioListenerRuntimeState
        │
        ├───────────────┐
        │               │
        ▼               ▼
AudioEmitter         Transform
+ occlusion policy      │
        └──────┬────────┘
               ▼
AudioOcclusionPhysicsQueryProvider
  nearest-emitter budget
  center + offset rays (1..5)
               │
               ▼
PhysicsFrameInput.queries[]
               │
               ▼
          engine.physics
               │
               ▼
PhysicsQueryHitDto[]
               │
               ▼
AudioOcclusionObservation  (raw fixed-step state)
               │
               ▼
attack/release temporal smoothing
               │
               ▼
AudioAcousticState
  obstruction
  occlusion
  transmission_gain
  high_frequency_gain
  low_pass_hz
               │
               ▼
engine.audio logical voice
               │
               ▼
gain × bus × attenuation × broadband transmission
         + dynamic spectral filter
               │
               ▼
physical voice arbitration / virtualization
```

The default policy emits three rays: center plus two lateral probes. Authoring may
select one to five rays and a probe radius. The provider sorts eligible emitters by
listener distance and processes at most
`NEWENGINE_AUDIO_OCCLUSION_MAX_EMITTERS_PER_TICK` emitters per fixed tick (default 32,
maximum 256), keeping the physics query load bounded. Emitters outside the authored
occlusion `max_distance` are not probed.

A partial blocked-ray fraction is **obstruction**. Full blockage of every ray including
the center ray becomes **occlusion**. The raw observation is intentionally not sent
directly to the mixer. `AudioSceneRuntimeModule` applies authored asymmetric temporal
smoothing (default attack `0.06 s`, release `0.22 s`) and releases stale observations
back toward a clear acoustic state after eight fixed ticks. This suppresses contact-edge
flicker and fixed-step/query jitter.

The default acoustic gains are `0.65` at full obstruction and `0.22` at full
occlusion. The resulting transmission participates in the same audibility calculation
used by the physical voice budget, so a heavily occluded low-priority source can become
virtual while its logical timeline continues normally.

Physics queries now carry additive `ignore_entity: Option<PhysicsEntityKey>` metadata.
This separates query correlation (`seq`) from self-exclusion, which is required for
multi-ray sources. Jolt and Bullet consume the explicit field and retain the legacy
`ignore_entity.unwrap_or(seq)` fallback, so existing query producers and serialized
v1 packets remain compatible. The listener player's body is explicitly ignored by
audio probes; a hit on the emitter entity itself is treated as reaching the endpoint,
not as an acoustic blocker.

`AudioEmitterRuntime` exposes the smoothed obstruction, occlusion, transmission gain,
high-frequency gain, low-pass cutoff, dominant acoustic material, and last raw acoustic
fixed tick for editor/profiler inspection. `AudioDiagnostics` separately reports
obstructed, occluded, and spectrally filtered logical voice counts.

## Acoustic materials / spectral transmission

Physics remains material-agnostic. A blocker hit returns only its stable entity key; the
engine resolves that entity's existing `PhysicsSurface.id` and maps the surface into an
audio-domain `AcousticMaterialProfile`. Thus footsteps, impacts, and acoustics share one
stable surface classification without leaking Jolt/Bullet material types into audio.

`audio.acoustic_surface` is the stable semantic ECS component for per-entity overrides.
It carries `material_id + AcousticMaterialProfile` and is writable through `engine.ecs`
`SetComponentJson` / `RemoveComponentJson`. The component is durable PIE state, is cloned by
editor actor duplication, and takes precedence over `PhysicsSurface.id` fallback mapping.
This allows variants such as thin/thick glass or hollow/solid metal without multiplying
physics surface categories.

First-party fallback presets are:

| Surface family | Broadband transmission | HF absorption | Full-block low-pass |
| --- | ---: | ---: | ---: |
| concrete / stone / brick | 0.16 | 0.92 | 1100 Hz |
| glass | 0.58 | 0.42 | 6500 Hz |
| wood / timber | 0.36 | 0.72 | 2800 Hz |
| metal / steel | 0.12 | 0.84 | 1700 Hz |
| dirt / soil / earth | 0.24 | 0.86 | 1900 Hz |

For multi-ray obstruction the coefficients of the actually blocked rays are averaged,
while the most frequent `PhysicsSurface.id` is retained as the dominant material for
diagnostics. Clear rays remain acoustically transparent. Geometric obstruction/occlusion
and material transmission are then combined into broadband `transmission_gain`,
`high_frequency_gain`, and `low_pass_hz` before the existing attack/release smoothing.

Physical clip voices use a runtime-adjustable `DynamicSpectralSource`. It maintains a
one-pole low-pass state per channel and blends the removed high-frequency residual back
according to `high_frequency_gain`, approximating a material-dependent high shelf. The
cutoff and HF gain are shared atomic controls, so `set_voice_json_v1` changes spectral
transmission in place: no player reconstruction, decoder restart, or source-time jump is
required. Filter state is reset only when an actual seek occurs. Generated UI tones keep
the unfiltered path.

The Rodio built-in low-pass adapter is intentionally not used; the current Rodio 0.22.2
source documents that adapter as potentially buggy. The first-party filter therefore owns
its small deterministic DSP implementation until the provider ABI grows a dedicated DSP
backend contract.

## Environment / reverb zones / portal sends

The environment layer describes how sound lives inside spaces after direct-path attenuation and
material transmission have already been evaluated. It is intentionally not a global
`reverb_amount`; authored rooms and portals form a deterministic acoustic environment graph.

```text
AudioEnvironmentZone + Transform
        │
        ├── room / corridor / concrete hall / metal hangar / outdoor response
        ├── oriented-box membership + boundary blend
        └── priority / transition policy
                        │
resolved listener ─────┼───── emitter world position
                        ▼
                AudioEnvironmentFrame
                        │
                strongest portal route
                        │
                        ▼
              AudioEnvironmentState
                source_send
                listener_send
                portal_gain
                        │
                 temporal smoothing
                        │
                        ▼
              engine.audio logical voice
                        │
                        ▼
             DynamicEnvironmentSource
              source-room reverb tank
              listener-room reverb tank
```

`audio.environment_zone` is a durable semantic ECS component. The zone is an oriented box: its
center and orientation come from the normal entity `Transform`, while authored `half_extents`
are local-space dimensions scaled by the propagated world scale. It does not require a physics
body. Overlapping zones are selected deterministically by higher `priority`, then lower
normalized distance to the zone center, then stable entity key. `blend_distance` fades the wet
send near a room boundary, while `transition_seconds` smooths the final provider-neutral state.

The first-party preset library exposes bounded room-shape parameters rather than backend effect
objects:

| Preset | Early reflections | Pre-delay | Decay | Damping | Diffusion |
| --- | ---: | ---: | ---: | ---: | ---: |
| room | 0.22 | 11 ms | 0.85 s | 0.58 | 0.68 |
| corridor | 0.28 | 18 ms | 1.45 s | 0.46 | 0.74 |
| concrete hall | 0.34 | 24 ms | 2.8 s | 0.32 | 0.82 |
| metal hangar | 0.40 | 31 ms | 4.2 s | 0.18 | 0.90 |
| outdoor | 0.05 | 7 ms | 0.28 s | 0.82 | 0.22 |

These are defaults only; every zone stores the full `AudioReverbPreset`, so projects can author
variants without extending an enum.

`audio.portal` is the stable semantic ECS connection between two `zone_id` values. A portal is
bidirectional and contributes:

```text
route_gain = openness × transmission_gain × send_gain
```

clamped to `[0, 1]`. The listener zone is the root of a max-product graph traversal. Multi-hop
paths therefore work (`room -> corridor -> hall`) and the strongest acoustic route wins. Closed
or disabled portals disappear from the graph. For different source/listener rooms the route gain
scales both source-room and listener-room wet sends. When both ends occupy the same room only the
listener-room send is emitted, avoiding duplicated same-room reverb.

Portal gain deliberately does **not** multiply the dry signal in V1. Direct-path energy is already
owned by distance attenuation plus physics occlusion/material transmission. Keeping these domains
separate prevents double attenuation. A later direct-sound acoustic-portal policy can be added as
an explicit contract rather than being hidden inside the reverb graph.

`AudioEnvironmentState` is the only room DTO crossing `engine.audio`. It carries two independent
`AudioReverbSend` values plus diagnostic `portal_gain`. Room changes interpolate send gains and all
reverb parameters before `set_voice_json_v1`, so walking through a doorway does not hard-switch the
wet tail.

The native provider realizes those sends with `DynamicEnvironmentSource`. Each physical clip voice
owns two bounded feedback-delay tanks (source room and listener room). The implementation includes
pre-delay/early reflections, RT60-style feedback decay, damping, and diffusion. Parameters are
shared through atomic controls and sampled in small batches; changing zones or portal openness does
not rebuild the player, restart the decoder, or change source time. DSP history is reset only on an
actual seek. UI feedback tones remain on the dry path.

`AudioEmitterRuntime` exposes target source/listener zone ids, portal gain, smoothed send gains, and
source/listener decay values. `AudioEnvironmentRuntimeState` exposes the current listener zone plus
zone/portal counts even when no emitter voice is active. Provider diagnostics separately count
`reverberant_voices`.

Both `audio.environment_zone` and `audio.portal` support `engine.ecs` semantic Set/Remove operations,
are durable across PIE snapshot/restore, and are copied by editor actor duplication. The editor scene
component list exposes them as `Audio Environment Zone` and `Audio Portal`.


## Long-form streaming / bounded PCM ring

Long-form music and ambience use the stable `engine.audio` method `play_stream_json_v1`.
A stream returns the same logical `voice_id` contract as ordinary clips, so `stop_voice_json_v1`
and `set_voice_json_v1` remain the only lifetime/control operations exposed to gameplay.

```text
VFS logical uri
    ↓
RangedAssetReader : Read + Seek
    ↓
engine.assets.raw_range_v1
    ↓ bounded compressed chunks / NARR v1
per-stream compressed LRU cache
    ↓
BufReader<RangedAssetReader>
    ↓
seekable Rodio/Symphonia Decoder
    ↓ decode worker
bounded decoded PCM chunks
    ↓
crossbeam bounded ring
    ↓ non-blocking try_recv
Rodio Source
    ↓
DynamicSpectralSource
    ↓
DynamicEnvironmentSource
    ↓
physical Player / SpatialPlayer
```

`AudioStreamBufferConfig` authors both compressed and decoded budgets. Defaults are `1500 ms` decoded
PCM capacity, `300 ms` startup prefill, `2048` frames per producer chunk, `64 KiB` compressed range
chunks, and a `512 KiB` per-stream compressed LRU cache. Capacity is clamped and rounded to complete
chunks. The producer may block on the bounded queue; the audio consumer never does. If the producer
falls behind, the consumer emits a zero sample and increments an underrun counter once per empty
streak rather than blocking the mixer thread.

Streaming bypasses the ordinary full-payload clip cache. `RangedAssetReader` implements ordinary
`Read + Seek` while every physical byte request stays behind `engine.assets.raw_range_v1`; the audio
provider never opens authored files or package files directly. The range contract returns requested
offset, total asset length, EOF state, and bounded bytes, which gives the decoder a real byte length
without first materializing the whole compressed asset. `BufReader` absorbs parser-sized reads and the
compressed LRU prevents repeated range RPCs around local decoder seeks.

The VFS keeps compiled-first/source-fallback semantics for ranged reads. Filesystem sources implement
real seek + bounded reads. First-party NEPAK advertises provider-owned `container.file_backed_io` and
`container.raw_range`; AssetManager therefore mounts the package without a resident `Arc<Vec<u8>>`
and delegates package-local list/read-range operations only through that explicit codec capability.
The NEPAK codec owns its binary index and opens the backing container path for the bounded physical
span; this delegation is confined to the VFS container source and is never visible to audio/gameplay.
Legacy/third-party container codecs retain the compatibility full-read path until they advertise an
equivalent bounded range capability.

Streams support an authored initial `start_seconds` and live `AudioVoiceUpdateRequest::seek_seconds`.
A seek advances the stream generation, discards queued/current PCM from the previous generation,
performs decoder seek on the worker, and refills the bounded ring before new-generation samples can
reach the mixer. This prevents pre-seek PCM from leaking into the new timeline. Runtime speed changes
remain outside the long-form streaming contract, and streams are still physical-only rather than
virtualizable when preempted; failed admission is explicit. Spectral transmission and environment/
reverb DSP remain active for streaming voices.

`AudioDiagnostics` reports `active_streams`, total buffered/capacity frames, accumulated
`stream_underruns`, `stream_range_requests`, `stream_compressed_bytes_fetched`, and
`stream_seek_operations`.

## Authored ambience beds

`audio.ambience_bed` is durable semantic ECS state for background ambience and long-form ambient
emitters. Beds are writable through `engine.ecs`, captured/restored by PIE, cloned by editor actor
duplication, and surfaced as `Audio Ambience Bed` in editor component inspection. GameReady registers
`AudioAmbienceRuntimeModule` alongside the ordinary `AudioSceneRuntimeModule`.

A bed can be `global`, `indoor`, `outdoor`, or scoped to explicit `zone_id` values.
`AudioEnvironmentZone` now carries explicit `AudioEnvironmentKind::{Indoor, Outdoor}`; older authored
zone payloads remain compatible because the serde default is `Indoor`. A listener with no containing
zone is treated as outdoor. Before a canonical listener snapshot exists, ambience activation is zero
for every scope, including global beds, preventing startup/editor-origin false positives.

```text
AudioAmbienceBed
    ↓
listener environment snapshot
    ├── Global  → 1.0
    ├── Indoor  → listener indoor only
    ├── Outdoor → listener outdoor only
    └── Zones
         ├── exact listener zone → 1.0
         └── strongest portal route × portal_bleed
    ↓
authored exponential fade
    ↓
play_stream_json_v1 / set_voice_json_v1
    ↓
Ambience bus
```

Zone-scoped ambience reuses the existing strongest max-product portal graph. This makes an ambience
bed authored for a courtyard or adjacent room leak naturally through partially open connected portals
without duplicating portal logic. `portal_bleed` controls only ambience activation; ordinary direct
sound and room reverb keep their existing independent contracts.

Non-spatial beds use the listener room response. Spatial ambience beds use their owner `Transform`
and resolve source/listener reverb sends through the same environment graph as other spatial voices.
Gain changes are exponentially smoothed by authored `fade_seconds`, so moving between outdoor,
corridor, room, and connected zones crossfades instead of starting/stopping abruptly.

`AudioAmbienceBedRuntime` exposes voice id, current/target gain, listener zone/outdoor state, portal
gain, stream uri, and active provider for editor/profiler inspection.

## Replaceable native AudioRuntime plugin

The replaceable native provider lives at `PluginsSrc/AudioRuntime`. It remains a thin provider wrapper
around the shared `newengine-audio-runtime` implementation rather than hosting a second mixer. Its
backend descriptor advertises `streaming-playback`, `bounded-pcm-ring`, `long-form-audio`,
`compressed-range-streaming`, `seekable-streaming`, `bounded-compressed-cache`,
`authored-ambience-beds`, `environment-zones`, `portal-sends`, and `dynamic-reverb`. Streaming
metadata records `engine.assets.raw_range_v1` as compressed-byte ownership, with the bounded
compressed cache and PCM ring as provider-owned ephemeral state.

## Camera listener

The listener is synchronized from the canonical resolved `CameraFrameSnapshot`
after gameplay camera smoothing/orbit resolution. Position, forward, and up
therefore match the actual presented view rather than a pre-smoothing ECS camera
transform. Non-finite camera frames are rejected before they reach audio.

## Next production passes

1. Add Music State Machine semantics on top of the seekable ranged transport: a stable transport clock,
   authored transitions, stems, beat/bar quantization, sample-accurate crossfade scheduling, and
   deterministic resume points.
2. Add ambience layering/scatter: random one-shots, density curves, time/weather/game-state modulation,
   and authored layer budgets while retaining zone/portal gating.
3. Add Audio editor tooling: compressed range/cache telemetry, stream/ring occupancy, seek/underrun
   timelines, ambience activation, SoundCue auditioning, bus meters, logical/physical/virtual/occluded
   voice tables, and VFS diagnostics.
4. Promote first-party per-voice reverb tanks to shared room aux buses/convolution in providers that
   support them, without changing the existing provider-neutral environment send contract.
5. Extend package/source transports with native ranged reads wherever a third-party container/source
   currently relies on the compatibility full-read fallback.
