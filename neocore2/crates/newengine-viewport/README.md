# newengine-viewport

    Viewport: описание, runtime-состояние, render-resources для привязки UI↔render target.

    ## Роль в архитектуре

    - **Слой:** `crates/newengine-viewport`
    - **Назначение:** Viewport: описание, runtime-состояние, render-resources для привязки UI↔render target.

    ## Инварианты

    - Viewport API должен быть стабильным для Editor и Runtime.

- Никаких зависимостей на конкретный рендер-бэкенд.

  ## Публичный API

    - Смотри `src/lib.rs` и модульные реэкспорты.
    - Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

  ## Тестирование и профилирование

    - Unit-тесты: `cargo test -p newengine-viewport`
    - (Рекомендуется) bench/criterion для hot path.

  ## Ссылки

    - Архитектура workspace: `../../ARCHITECTURE.md`
