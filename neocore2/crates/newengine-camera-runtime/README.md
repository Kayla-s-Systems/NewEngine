# newengine-camera-runtime

Runtime-обвязка, которая подключает `newengine-camera` к ECS/симуляции и host-вводу.

## Ответственность

- Перевод viewport input snapshot в `CameraControlInput`.
- Управление editor/gameplay camera navigation state (`Orbit`/`Fly`) через ECS.
- Выбор `CameraChannelState` для runtime/editor режима.
- Возврат `CameraNavResult { frame, controller, cursor }`, где `frame` — готовый `CameraFrame` для render/runtime слоёв.
- Синхронизация `CameraRigComp` и `Transform` без renderer coupling.

## Не ответственность

- Не содержит чистую математику камеры — это `newengine-camera`.
- Не реализует UI и не знает про конкретный backend.
- Не возвращает loose `rig + projection` как публичный результат навигации: renderer-facing контракт — только `CameraFrame`.
