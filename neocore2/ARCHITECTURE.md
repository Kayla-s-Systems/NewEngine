# NewEngine Architecture (Workspace)

## Цели

- **Foundation-first:** сначала ядро и контракты, затем реализации.
- **Determinism-first:** воспроизводимость, явные стадии, отсутствие скрытого async.
- **Plugin-first:** модульность и ABI-контракты (внешние плагины и внутренние модули).

## Слои

1. **Platform host** (`newengine-platform-winit`): окно/события/ввод/тайминг.
2. **Core** (`newengine-core`): модули, плагины, сервисы/события, рендер-абстракции.
3. **Contracts/ABI** (`newengine-plugin-api`, `*-api`): стабильные интерфейсы.
4. **Simulation** (`newengine-ecs`, `newengine-sim`, `newengine-scene`, `newengine-transform`): мир, стадии, derived.
5. **Backends** (например `newengine-modules-render-vulkan-ash`, `newengine-physics-jolt`): реализации интерфейсов.
6. **Apps** (`apps/editor`): редактор и инструменты.

## Правила зависимостей

- `newengine-plugin-api` и `*-api` не зависят от реализаций.
- `newengine-core` не зависит от конкретных backend’ов.
- Backends зависят от `newengine-core` и контрактов, но не наоборот.
- Apps зависят от нужных крейтов, но не размывают границы (editor ≠ core).

## AAA Roadmap (high-level)

- ECS 2.0: ComponentId registry, (опционально) archetypes/chunks, доступ-дескрипторы систем.
- Task graph: параллельный планировщик по конфликтам данных.
- Scene/Content: GUID, serialization, prefab instances, streaming/subscenes.
- Tooling: профайлер, инспекторы, hot-reload, DDC/asset cooking.
