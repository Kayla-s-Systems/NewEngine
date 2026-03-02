# newengine-bounds

Bounds/volumes домен: `Aabb`, `Sphere`, компонент `Bounds` и утилиты derived состояния.

## Ответственность

- Геометрические примитивы: `Aabb`, `Sphere`.
- Компонент `Bounds` (local + derived world-space данные).
- Конвертеры: `aabb_to_sphere`, `sphere_to_aabb`.
- Системы обновления bounds из `Transform` или `Mat4` (без аллокаций в hot path).

## Не ответственность

- Не содержит логики сцены/рендера/физики.

## Инварианты

- Детерминированность: стабильный порядок и отсутствие зависимости от hash-итерации.
- Hot path без heap churn: scratch хранится в `World` как resource.

## Ссылки

- `../../ARCHITECTURE.md`
