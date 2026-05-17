# Camera director runtime completion pass — 2026-05-17

## Goal

Bring the in-engine camera closer to the reference camera architecture while keeping NewEngine's Rust/runtime boundary clean:

```text
camera metadata/settings
  -> director runner state
  -> director mixer
  -> resolved camera frame + post-effect sidecar
  -> viewport cache/fader
  -> renderer-facing CameraFrame + RenderFrameEnvelope postfx
```

The renderer still consumes `CameraFrame` and backend-facing render envelopes. Camera policy, director arbitration, events, transitions and post-effect state stay in `newengine-camera` / `newengine-camera-runtime`.

## Implemented

### Director model

`newengine-camera-runtime` now has a real runtime director layer:

- `CameraRuntimeDirectorOutput`
- `CameraDirectorMixer`
- `CameraRenderedDirector`
- `StaticCameraDirectorRunner`
- `CinematicDirectorRunner`
- `ScriptedDirectorRunner`
- `ReplayDirectorRunner`
- aliases for cutscene/switch/synced-scene/anim-scene/marketing/debug static runners

The mixer accepts multiple rendering directors, sorts by priority and deterministic director order, blends lower-priority outputs into higher-priority outputs, and reports the dominant director.

### Runtime settings

`CameraRuntimeSettings` provides per-director configuration:

- blend-in duration
- blend-out duration
- blend curve
- input lock policy
- default post-effects

Supported director lanes now include runtime, gameplay, cinematic, scripted, replay, cutscene, switch, synced scene, anim scene, marketing and debug.

### Transitions

`CameraManagerResource` no longer only does a plain frame blend. It now owns:

- transition state;
- frame blend state;
- director mixer;
- viewport manager;
- last resolved frame;
- event queue;
- per-director settings.

World policy still chooses runtime/gameplay for current gameplay flow, but higher-level systems can now submit cinematic/scripted/replay/cutscene director outputs through `submit_director_output`.

### Events

`CameraRuntimeEvent` captures camera lifecycle events:

- director requested;
- director activated/deactivated/bypassed;
- transition started/completed;
- dominant director changed;
- effects changed;
- viewport changed.

Events are bounded to 64 queued entries and can be drained with `take_events()`.

### Effects

`CameraResolvedFrame` post-effect sidecars are now propagated into renderer envelopes instead of staying as unused data.

`PostFxFrameParams` now carries:

- DOF planes/blend/high-quality flag;
- motion blur strength/decay;
- shake amplitude;
- exposure bias;
- camera jitter override.

Runtime postfx maps camera exposure bias into the display exposure multiplier immediately. Other fields are now present in the backend envelope for provider-side implementation.

### Viewports

`CameraViewportManagerResource` tracks viewport layers, active layer, last frame, fade state and viewport-change events.

## Kept invariants

- `RuntimeRenderController` still orchestrates only.
- Camera policy does not introduce renderer implementation ownership.
- Renderer providers continue to receive typed `RenderFrameEnvelope` packets through the render service backend boundary.
- New systems are data-driven and do not add provider-specific rendering branches.

## Remaining provider-side work

- Vulkan postfx shader can now consume the new DOF/motion-blur/shake fields from `PostFxFrameParams`; current pass already applies exposure bias because exposure is part of the existing postfx constants.
- Add camera conformance tests once CI toolchain is available in the environment.
