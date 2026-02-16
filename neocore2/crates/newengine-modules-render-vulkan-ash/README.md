# newengine-modules-render-vulkan-ash

    Vulkan рендер-модуль: реализация RenderApi на ash, swapchain, пайплайны, ресурсы.

    ## Роль в архитектуре

    - **Слой:** `crates/newengine-modules-render-vulkan-ash`
    - **Назначение:** Vulkan рендер-модуль: реализация RenderApi на ash, swapchain, пайплайны, ресурсы.

    ## Инварианты

    - Только реализация backend’а; интерфейс — в newengine-core::render.

- Должен уметь пересоздавать swapchain/RT без утечек и nondeterministic поведения.

  ## Публичный API

    - Смотри `src/lib.rs` и модульные реэкспорты.
    - Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

  ## Тестирование и профилирование

    - Unit-тесты: `cargo test -p newengine-modules-render-vulkan-ash`
    - (Рекомендуется) bench/criterion для hot path.

  ## Ссылки

    - Архитектура workspace: `../../ARCHITECTURE.md`
