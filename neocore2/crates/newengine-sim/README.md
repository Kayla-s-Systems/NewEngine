# newengine-sim

Симуляция поверх ECS: стадии `Input/Controllers/Physics/Derived`, минимальный детерминированный scheduler и базовые
системы (интеграция скоростей, простые контроллеры).

## Роль в архитектуре

- **Слой:** `crates/newengine-sim`
- **Назначение:** thin-layer между ECS и геймплейными модулями/плагинами.

## Инварианты

- Переменный и фиксированный шаг разделены; `fixed_tick` — источник детерминизма.
- Системы — `fn` pointers (без динамического диспетча в hot loop).
- Математические типы берём только из `newengine-math` (не тянем `glam` напрямую).

## Parallel execution

- Feature: `parallel`
- Модель: детерминированные batch'и по `AccessMask` + commit через `CommandBuffer` в стабильном порядке.

```bash
cargo test -p newengine-sim --features parallel
```

  ## Публичный API

    - Смотри `src/lib.rs` и модульные реэкспорты.
    - Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

  ## Тестирование и профилирование

    - Unit-тесты: `cargo test -p newengine-sim`
    - (Рекомендуется) bench/criterion для hot path.

  ## Ссылки

    - Архитектура workspace: `../../ARCHITECTURE.md`
