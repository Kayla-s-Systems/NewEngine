# newengine-sim

    Симуляция поверх ECS: стадии Input/Controllers/Physics/Derived, минимальный детерминированный scheduler и базовые системы.

    ## Роль в архитектуре

    - **Слой:** `crates/newengine-sim`
    - **Назначение:** Симуляция поверх ECS: стадии Input/Controllers/Physics/Derived, минимальный детерминированный scheduler и базовые системы.

    ## Инварианты

    - Переменный и фиксированный шаг разделены; fixed_tick — источник детерминизма.

- Системы — fn pointers (без динамического диспетча в hot loop).

  ## Публичный API

    - Смотри `src/lib.rs` и модульные реэкспорты.
    - Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

  ## Тестирование и профилирование

    - Unit-тесты: `cargo test -p newengine-sim`
    - (Рекомендуется) bench/criterion для hot path.

  ## Ссылки

    - Архитектура workspace: `../../ARCHITECTURE.md`
