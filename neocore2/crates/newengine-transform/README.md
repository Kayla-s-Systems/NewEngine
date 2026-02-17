# newengine-transform

Трансформы и иерархия: `Parent/Children`, `Transform` (local), `GlobalTransform`/`WorldPose` (derived),
детерминированная пропагация без аллокаций в hot path.

## Роль в архитектуре

- **Слой:** `crates/newengine-transform`
- **Назначение:** единый источник истины для трансформов, пригодный для редактора и рантайма.

## Инварианты

- Иерархия должна оставаться консистентной (`Parent` ↔ `Children`).
- Пропагация должна быть детерминированной независимо от порядка в hash-коллекциях.
- Математические типы берём только из `newengine-math` (не тянем `glam` напрямую).

## Публичный API

- Смотри `src/lib.rs` и модульные реэкспорты.

## Тестирование и профилирование

- Unit-тесты: `cargo test -p newengine-transform`
- (Рекомендуется) bench/criterion для hot path.

## Ссылки

- Архитектура workspace: `../../ARCHITECTURE.md`

## Allocation-free propagation

`propagate_transforms()` stores reusable scratch buffers in the ECS World as a resource: `TransformPropagationScratch`.
This avoids per-frame heap churn and keeps propagation deterministic.
