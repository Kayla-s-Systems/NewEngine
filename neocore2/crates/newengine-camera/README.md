# newengine-camera

Камера как чистый доменный контракт: channel → pose/rig → projection/lens → viewport → matrices/frustum → GPU uniforms.

## Ответственность

- `CameraChannel` / `CameraChannelState`: явный владелец кадра камеры, чтобы gameplay/runtime/debug/cinematic/scripted/replay не конкурировали через скрытое глобальное состояние.
- `CameraRig`: world-space pose камеры с единой конвенцией `forward = -Z`.
- `Projection`: перспективная/ортографическая lens-модель с Vulkan-ready clip space `Z = 0..1` и baked Y-flip.
- `CameraViewport`: physical-pixel viewport, aspect и viewport uniform `(w, h, 1/w, 1/h)`.
- `CameraFrame`: единственный renderer-facing пакет: rig, projection, viewport, matrices, frustum, jitter и diagnostics.
- `RuntimeNavController`, `FreeFlyController`, `OrbitController`: deterministic controller state без renderer/UI зависимости.
- `CameraStack`: gameplay/runtime modifier pipeline, который возвращает `CameraFrame`, а не loose matrix tuple.

## Не ответственность

- Не зависит от platform/windowing, UI и render backend.
- Не хранит ECS-компоненты и не читает host input напрямую.
- Не держит legacy compatibility wrappers: старый `CameraState`/`CameraController` контракт удалён.

## Инварианты

- Детерминированность при одинаковом input/dt.
- Никакого скрытого IO и глобального singleton состояния.
- Render layer получает уже собранный `CameraFrame` и не пересобирает view/projection/frustum самостоятельно.
- Input layer передаёт `CameraControlInput`; viewport-runtime отвечает за перевод UI/host input в этот контракт.
