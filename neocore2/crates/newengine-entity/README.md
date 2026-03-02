# newengine-entity

Фундаментальный крейт, который определяет **EntityId** для всего NewEngine.

## Ответственность

- `EntityId` — runtime идентичность сущности (generational key), детерминированно сортируемая.
- Базовые мета-типы для tooling: `EntityKind`, `EntityName`, `EntityMeta`.

## Не ответственность

- Не хранит компоненты и не содержит ECS.
- Не содержит сериализации/сцен, persistent id и т.д.

## Инварианты

- `EntityId` не используется как persistent id (для этого есть `newengine-scene::EntityGuid`).
- Любая детерминированная сортировка по сущностям должна опираться на `EntityId::stable_u64()`.

## Ссылки

- `../../ARCHITECTURE.md`
