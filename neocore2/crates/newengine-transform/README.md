# newengine-transform

Рантайм-алгоритмы трансформа: иерархия и детерминированная пропагация `Transform` → `GlobalTransform/WorldPose`.

## Ответственность

- Алгоритмы и системные helpers (feature `ecs`):
    - `set_parent` (делегирует в `newengine-transform-api`)
    - `ensure_transform_outputs`
    - `propagate_transforms` (allocation-free через scratch resource)
    - world-space helpers

## Не ответственность

- Не определяет компоненты/контракты (это `newengine-transform-api`).
- Не тянет `newengine-core` по умолчанию.

## Features

- `ecs` — включает ECS-интеграцию (иерархия/propagate/world-space).
- `service` — **опциональная** регистрация transform runtime в `newengine-core::ServiceRegistry`.

## Инварианты

- Детерминизм: сортировка по `EntityId::stable_u64()` и отсутствие зависимости от порядка hash-итерации.
- Hot path без аллокаций: scratch хранится в `World` через `resource_mut_or_insert_default`.

## Ссылки

- `../../ARCHITECTURE.md`
