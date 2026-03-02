# newengine-camera-runtime

Runtime-обвязка, которая подключает `newengine-camera` к ECS/симуляции и host-вводу.

## Ответственность

- ECS-системы и helpers для навигации камеры (Orbit/Fly) в контексте viewport.
- Интеграция с `newengine-sim` (stages/commands) без привязки к конкретному render backend.

## Не ответственность

- Не содержит чистую математику камеры (это `newengine-camera`).
- Не реализует UI (это `newengine-ui`/editor).

## Ссылки

- `../../ARCHITECTURE.md`
