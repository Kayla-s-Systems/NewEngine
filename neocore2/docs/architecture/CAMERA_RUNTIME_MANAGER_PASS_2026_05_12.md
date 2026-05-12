# Camera runtime manager pass — 2026-05-12

## Что сделано

Этот pass поднимает камеру на уровень выше простого `Orbit/Fly` helper и закладывает архитектурный фундамент в духе reference `camera.zip`:

- добавлен `CameraManagerResource`;
- введены явные `CameraDirectorKind`;
- введены явные `CameraRuntimeMode`;
- введён transition FSM (`CameraTransitionState`);
- введён `CameraDirectorRequest` для player possession/release;
- camera policy вынесена из прямой play-mode логики render controller.

## Новые runtime-контракты

```text
newengine-camera-runtime/
  src/manager/
    mod.rs
    types.rs
    resource.rs
```

### CameraDirectorKind

- `Editor`
- `Gameplay`
- `Cinematic`
- `Scripted`
- `Replay`
- `Debug`

### CameraRuntimeMode

- `EditorOrbit`
- `EditorFly`
- `GameplayPreview`
- `GameplayFirstPerson`
- `GameplayThirdPersonFollow`
- `GameplayThirdPersonAim`
- `CinematicPreview`
- `DebugFree`

## Как теперь работает policy

### До release launch gate

Если play/runtime уже запрошен, но scene gate ещё заблокирован:

- директор = `Gameplay`
- mode = `GameplayPreview`
- input context = `TransitionLocked`
- player possession не выполняется

Это устраняет прежнюю проблему, когда camera/play policy была жёстко привязана к одному boolean play-mode.

### После release gate и public activation

При реальном direct-control runtime:

- директор = `Gameplay`
- mode = `GameplayFirstPerson`
- `CameraDirectorRequest::PossessPlayer`

### В editor

- директор = `Editor`
- mode = `EditorOrbit` или `EditorFly`
- input context = `EditorNav`

## Что это даёт

- один источник правды для camera runtime policy;
- явная FSM-модель переходов вместо ad-hoc play/camera условий;
- render controller больше не решает напрямую, кто владеет камерой;
- камера готова к следующему шагу: blends, cinematic/scripted/replay directors, third-person contexts, debug camera.

## Следующий правильный шаг

1. Добавить `CameraFrameBlendPlan` и реальный blended output между director'ами.
2. Вынести gameplay `FirstPerson / ThirdPersonFollow / ThirdPersonAim` в отдельные mode runners.
3. Добавить `CameraRuntimeReport` в overlay diagnostics.
4. Убрать оставшиеся direct possession side effects из render controller в отдельный camera-runtime service.
