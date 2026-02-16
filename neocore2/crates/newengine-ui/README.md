# newengine-ui

    UI слой: markup-документы, state/vars/events, провайдеры (egui), рендер текста/текстур, asset service client.

    ## Роль в архитектуре

    - **Слой:** `crates/newengine-ui`
    - **Назначение:** UI слой: markup-документы, state/vars/events, провайдеры (egui), рендер текста/текстур, asset service client.

    ## Инварианты

    - UI не должна напрямую мутировать мир: только команды/события.

- Markup — внешняя спецификация; state — сериализуемый и детерминированный.

  ## Публичный API

    - Смотри `src/lib.rs` и модульные реэкспорты.
    - Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

  ## Тестирование и профилирование

    - Unit-тесты: `cargo test -p newengine-ui`
    - (Рекомендуется) bench/criterion для hot path.

  ## Ссылки

    - Архитектура workspace: `../../ARCHITECTURE.md`
