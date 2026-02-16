# newengine-scene

    Сцена: World + сценовые компоненты/инварианты (root, active camera), derived state (bounds/кэши).

    ## Роль в архитектуре

    - **Слой:** `crates/newengine-scene`
    - **Назначение:** Сцена: World + сценовые компоненты/инварианты (root, active camera), derived state (bounds/кэши).

    ## Инварианты

    - Сцена валидирует свои фундаментальные роли (root/camera) детерминированно.

- Derived обновляется отдельными системами, сцена сама не выполняет рендер/инпут.

  ## Публичный API

    - Смотри `src/lib.rs` и модульные реэкспорты.
    - Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

  ## Тестирование и профилирование

    - Unit-тесты: `cargo test -p newengine-scene`
    - (Рекомендуется) bench/criterion для hot path.

  ## Ссылки

    - Архитектура workspace: `../../ARCHITECTURE.md`
