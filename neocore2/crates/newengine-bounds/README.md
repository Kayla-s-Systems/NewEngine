# newengine-bounds

Bounds/volumes: AABB/сферы и утилиты для derived состояния сцены.

## Роль в архитектуре

- **Слой:** `crates/newengine-bounds`
- **Назначение:** Bounds/volumes: AABB/сферы и утилиты для derived состояния сцены.

## Инварианты

- Bounds должны быть cheap и детерминированны (без NaN propagation).

## Публичный API

- Смотри `src/lib.rs` и модульные реэкспорты.
- Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

## Тестирование и профилирование

- Unit-тесты: `cargo test -p newengine-bounds`
- (Рекомендуется) bench/criterion для hot path.

## Ссылки

- Архитектура workspace: `../../ARCHITECTURE.md`
