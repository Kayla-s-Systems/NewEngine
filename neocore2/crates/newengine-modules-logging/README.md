# newengine-modules-logging

Модуль логирования: sinks/форматирование/маршрутизация логов.

## Роль в архитектуре

- **Слой:** `crates/newengine-modules-logging`
- **Назначение:** Модуль логирования: sinks/форматирование/маршрутизация логов.

## Инварианты

- Логи — сервис ядра; модуль не должен тянуть платформу/рендер.

## Публичный API

- Смотри `src/lib.rs` и модульные реэкспорты.
- Для контрактов/ABI: фиксируйте изменения через версионирование (semver) и миграции.

## Конфигурация через env

Базовые:

- `NEWENGINE_LOG` — строка фильтров `env_logger` (например: `info,newengine_core=debug`).
- `NEWENGINE_LOG_LEVEL` — базовый уровень (если `NEWENGINE_LOG` не задан).
- `NEWENGINE_LOG_TARGET=stdout|stderr` — консольный sink.
- `NEWENGINE_LOG_STYLE=auto|always|never` — стиль вывода.
- `NEWENGINE_LOG_COLORS=0|false` — отключить цвета.
- `NEWENGINE_LOG_TIMESTAMP=none|seconds|milliseconds|microseconds|nanoseconds`
- `NEWENGINE_LOG_INDENT=none|<usize>`

Файл:

- `NEWENGINE_LOG_FILE=<path>` — включить запись в файл (append).
- `NEWENGINE_LOG_TEE=1|true` — писать одновременно в консоль и в файл (по умолчанию включено, если задан
  `NEWENGINE_LOG_FILE`).

Rolling:

- `NEWENGINE_LOG_ROLL_MAX_BYTES=<u64>` — ротация при превышении размера.
- `NEWENGINE_LOG_ROLL_MAX_FILES=<usize>` — количество бэкапов для size-rolling (`.1..N`).
- `NEWENGINE_LOG_ROLL_KEEP_DAYS=<usize>` — суточная ротация (UTC epoch-day) и хранение последних N дневных файлов.

## Тестирование и профилирование

- Unit-тесты: `cargo test -p newengine-modules-logging`
- (Рекомендуется) bench/criterion для hot path.

## Ссылки

- Архитектура workspace: `../../ARCHITECTURE.md`
