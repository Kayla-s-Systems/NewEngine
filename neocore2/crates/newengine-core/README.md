# newengine-core

`newengine-core` — единственный host/kernel слой NewEngine. Он владеет lifecycle, shutdown, фазовым engine-thread scheduler и CPU job system. Runtime-host, editor, gameplay runtime и плагины не создают собственную модель жизненного цикла и не зеркалят состояние ядра: они публикуют команды, work items и module callbacks в core.

## Ответственность ядра

- `EngineFsm`: единый источник правды для состояния приложения.
- `ShutdownToken`: единый cooperative shutdown канал.
- `Scheduler`: engine-thread фазовые задачи с бюджетами `BeginFrame / FixedUpdate / Update / Render / EndFrame`.
- `JobSystem`: единый CPU worker pool с lane/priority контрактом.
- `ModuleCtx`: безопасный доступ модуля к services/resources/events/scheduler/job-system/shutdown.
- `EventHub` и lifecycle/readiness events.
- `ServiceRegistry` и host service boundary.
- Plugin boot/shutdown ordering.
- Startup config: подсистемы конфигурируются через `plugins.*`, без remapping старых форматов.

## FSM

Допустимый путь исполнения:

```text
Created
  -> InitSystem
  -> InitGame
  -> Running
  -> ShutdownGame
  -> ShutdownSystem
  -> Stopped
```

Аварийный путь из любого активного boot/run/shutdown состояния:

```text
* -> Faulted
```

Shutdown request из активного состояния переводит FSM в `ShutdownGame`; внешний `ShutdownToken` синхронизируется обратно в FSM перед module callbacks и перед кадром.

## Thread ownership

`newengine-core` создаёт и делегирует CPU work через `JobSystem`.

Runtime/editor/gameplay слои должны:

- отправлять CPU-heavy работу в `JobSystemHandle`;
- отправлять deterministic engine-thread commits в `Scheduler`;
- запрашивать завершение через `ShutdownToken` / `ModuleCtx::request_exit()`;
- читать lifecycle через `Engine::run_state()`.

Runtime/editor/gameplay слои не должны:

- хранить собственные lifecycle-флаги, зеркалирующие core FSM;
- создавать ad-hoc worker pools для ассетов, стриминга, render-prep или gameplay jobs;
- вызывать plugin/backend implementation напрямую в обход service/module contracts;
- держать fallback-конфиги старого формата внутри core.

## Job lanes

```text
Simulation   - CPU gameplay/simulation preparation
RenderPrep   - CPU preparation for render packets/draw lists
Streaming    - world/visibility/residency preparation
AssetIo      - importer/cooker/file IO work
Plugin       - plugin-owned asynchronous work
Background   - telemetry, maintenance, non-interactive work
```

## Core invariant

Один FSM + один shutdown канал + один job provider. Любой второй lifecycle flag или local worker pool в runtime/editor/gameplay считается architectural regression.
