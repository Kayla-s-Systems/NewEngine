# newengine-transform

    Трансформы и иерархия: Parent/Children, Local/World матрицы, детерминированная пропагация.

    ## Роль в архитектуре

    - **Слой:** `crates/newengine-transform`
    - **Назначение:** Трансформы и иерархия: Parent/Children, Local/World матрицы, детерминированная пропагация.

    ## Инварианты

    - Иерархия должна оставаться консистентной (Parent↔Children).

- Пропагация world-transform должна быть детерминированной независимо от HashMap порядка.

  ## Публичный API

    - Смотри `src/lib.rs` и модульные реэкспорты.
    - Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

  ## Тестирование и профилирование

    - Unit-тесты: `cargo test -p newengine-transform`
    - (Рекомендуется) bench/criterion для hot path.

  ## Ссылки

    - Архитектура workspace: `../../ARCHITECTURE.md`
