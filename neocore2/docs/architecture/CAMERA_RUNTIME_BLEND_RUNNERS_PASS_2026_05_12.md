# Camera runtime blend/runners pass — 2026-05-12

## Цель

Следующий уровень после `CameraManagerResource`: камера должна не только выбирать director/mode, но и реально управлять переходом между кадрами, gameplay-runners и diagnostics.

## Что добавлено

```text
newengine-camera-runtime/
  src/blend.rs
  src/modes/mod.rs
  src/service.rs
```

## CameraFrameBlendPlan

Добавлен реальный frame-level blend:

```rust
CameraFrameBlendPlan {
    policy,
    curve,
    duration_sec,
    lock_input,
    preserve_previous_frame,
}
```

Поддерживаются:

- `Cut`
- `Blend`
- `Linear`
- `SmoothStep`
- `EaseInOut`

`CameraFrameBlendState` хранит:

- immutable `from_frame`;
- previous presented frame;
- elapsed time;
- active flag.

Это важно: blend не является progressive self-blend. Новый target-frame каждый tick смешивается с исходным `from_frame`, что сохраняет детерминированность и предсказуемость.

## Gameplay runners

Добавлены отдельные runners:

- `GameplayFirstPersonRunner`
- `GameplayThirdPersonFollowRunner`
- `GameplayThirdPersonAimRunner`

Они не трогают renderer. Они описывают camera-follow intent для ECS.

## CameraRuntimeService

Side-effects вынесены из render controller:

- ensure manager resource;
- drain/apply `CameraDirectorRequest`;
- possess/release player camera;
- apply/clear player motor input.

Render controller больше не вызывает `attach_active_camera_to_player` / `detach_active_camera_from_player` напрямую.

## Diagnostics

`CameraRuntimeReport` теперь записывается в:

- runtime overlay text;
- `RenderFrameDebugSnapshot.notes`.

Диагностика показывает:

```text
CAM Gameplay/GameplayFirstPerson GameplayLook gate=false blend=on 42%
CAM transition Blending 0.08s target=Some(...)
```

## Что осталось следующим pass-ом

1. Перенести `PlaySessionSnapshot` полностью из render controller в camera-runtime session service.
2. Добавить runtime-переключатель `FirstPerson / ThirdPersonFollow / ThirdPersonAim` из scene/profile config.
3. Добавить настоящие collision constraints для third-person spring arm.
4. Добавить cinematic/scripted director runners.
