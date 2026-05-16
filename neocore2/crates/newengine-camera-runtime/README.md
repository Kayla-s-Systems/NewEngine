# newengine-camera-runtime

Runtime-обвязка, которая подключает `newengine-camera` к ECS/симуляции и host-вводу.

## Ответственность

- `CameraManagerResource`: единый runtime-owner camera policy (director/mode/transition/input-context/gate state).
- Перевод viewport input snapshot в `CameraControlInput`.
- Управление runtime/gameplay camera navigation state (`Orbit`/`Fly`) через ECS.
- Выбор `CameraChannelState` для runtime/runtime режима.
- Возврат `CameraNavResult { frame, controller, cursor }`, где `frame` — готовый `CameraFrame` для render/runtime слоёв.
- Синхронизация `CameraRigComp` и `Transform` без renderer coupling.

## Не ответственность

- Не содержит чистую математику камеры — это `newengine-camera`.
- Не реализует UI и не знает про конкретный backend.
- Не возвращает loose `rig + projection` как публичный результат навигации: renderer-facing контракт — только `CameraFrame`.


## Runtime manager

`CameraManagerResource` хранит:

- active director (`Runtime`, `Gameplay`, ...);
- active mode (`RuntimeOrbit`, `GameplayFirstPerson`, ...);
- transition FSM;
- pending `CameraDirectorRequest` (`PossessPlayer` / `ReleasePlayer`);
- gate-aware input policy.


## Camera frame blending

`CameraFrameBlendPlan` / `CameraFrameBlendState` provide the first real frame-level transition contract:

- `Cut` for immediate recovery/debug switches;
- `Blend` for director/mode transitions;
- `Linear`, `SmoothStep`, `EaseInOut` curves;
- deterministic `from_frame -> target_frame` sampling without progressive self-blending.

The manager now resolves the final camera output through `CameraManagerResource::resolve_camera_frame(...)`.

## Gameplay runners

Gameplay camera behavior is now declared through runners instead of ad-hoc render-controller side effects:

- `GameplayFirstPersonRunner`
- `GameplayThirdPersonFollowRunner`
- `GameplayThirdPersonAimRunner`

The runners produce stable follow-controller intent, while `CameraRuntimeService` applies possession/release/input side effects to ECS.

## Diagnostics

`CameraRuntimeReport` is recorded into render overlay/debug snapshots so a frame can report:

- active director;
- active mode;
- input context;
- gate state;
- transition phase;
- frame blend state.
