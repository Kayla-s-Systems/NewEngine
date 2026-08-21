# North Star Engine Profiler Report

> [!INFO] INFO BLOCK — как читать отчёт
> **У нас сейчас:** отчёт показывает instrumented wall-clock time по job/service/plugin событиям. Главная строка для поиска виновника — `total_elapsed_ms` и `total_share_percent`; главная строка для бюджетов кадра — `load`, где `1.0` значит ровно бюджет, а `>1.0` значит перерасход.
>
> **Technical details (EN):** `load = elapsed_ms / budget_ms`; CSV files are emitted next to JSON/MD and duplicated in the timestamped ZIP archive when archive output is enabled.

- reason: `service.shutdown_v1`
- uptime_ms: `50.442`
- events_seen: `69`
- malformed_events: `0`

## Quick answer — кто жрёт время

**Worst offender:** `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/starVault-assetManager-3.5.2-release.dll` — 8.220 ms total, 19.7% of captured time, 1 calls, max load 0.49x, slow/over-budget 0, failed 0.

```text
captured time share  [██████░░░░░░░░░░░░░░░░░░░░░░░░░░]  19.7%
max budget load      [████░░░░░░░░░░░░░░░░░░░░░░░░░░░░]  0.49x
```

## Executive summary

| Metric | Value | Meaning |
|---|---:|---|
| `active_jobs` | `0` | работа ещё не завершилась; если висит долго — смотреть `Active jobs` |
| `completed_jobs_kept` | `14` | сколько завершённых записей осталось в ring buffer |
| `failed_jobs` | `2` | ошибки, которые надо читать вместе с diagnostics |
| `slow_or_over_budget_jobs` | `0` | slow threshold или `load >= 1.0` |
| `total_elapsed_ms` | `41.679` | сумма captured wall-clock времени по завершённым jobs |
| `average_elapsed_ms` | `2.977` | среднее время одной завершённой job |
| `max_elapsed_ms` | `8.220` | самая дорогая одиночная job |

## Profiler-first telemetry view

| Question | Count | Share | Meaning |
|---|---:|---:|---|
| `what was scheduled` | `11` | `-` | jobs that entered the visible scheduling path |
| `what was blocked` | `0` | `0.0%` | jobs that reported blocked/waiting/dependency/residency/barrier state |
| `what was polling` | `0` | `-` | jobs/status events that stayed in a poll/ticket loop |
| `what waited on GPU` | `0` | `0.0%` | jobs with gpu_wait_ms or GPU/fence/present/upload wait reason |
| `what exceeded frame budget` | `0` | `-` | jobs where elapsed_ms exceeded explicit frame_budget_ms |
| `what stayed async` | `0` | `0.0%` | jobs tagged as async/ticket/engine.threading/render-prep/streaming work |

> [!NOTE] REQUEST NOTE — profiler-first culture
> **У нас сейчас:** report теперь отделяет timing от scheduling/waiting/async facts, чтобы не путать `ms` с причиной тормоза.
> **Было бы здорово:** every heavy lane should emit `lane`, `priority`, `dependency_group`, `frame_id`, `frame_budget_ms`, `gpu_wait_ms`, `wait_reason`, and `async_mode` when known.
> **Technical details (EN):** StarProfiler report schema is `newengine.profiler.report.v3`; CSV consumers can read `profiler_first_latest.csv`, `profiler_lanes_latest.csv`, and `profiler_frame_budget_latest.csv`.

## Flush and scheduling policy

| Setting | Value |
|---|---|
| `service_flush_mode` | `engine_jobs` |
| `shutdown_flush_mode` | `sync_final` |
| `prefer_engine_threading` | `true` |
| `require_engine_threading` | `true` |
| `lock_policy` | `snapshot_then_build_and_write_outside_lock` |

> [!NOTE] REQUEST NOTE — profiler safety
> **У нас сейчас:** heavy report build/write is outside the runtime state lock; async flush is routed through `engine.threading` by default.
> **Было бы здорово:** keep every future heavy profiler export as a visible job/task, never as an invisible background load.
> **Technical details (EN):** `profiler.flush_report_v1` uses configured service flush mode; `profiler.flush_report_sync_v1` is the explicit synchronous worker entrypoint for `engine.threading` and shutdown-final flush.

## Percentiles — latency and budget load

| Metric | p50 | p90 | p95 | p99 |
|---|---:|---:|---:|---:|
| `elapsed_ms` | 3.203 | 4.612 | 4.612 | 8.220 |
| `load` | 0.19x | 0.28x | 0.28x | 0.49x |

## Load chart — категории по суммарному времени

```text
plugin_lifecycle                   [██████████████████████████░░]  39.322 ms  94.3%
renderer                           [██░░░░░░░░░░░░░░░░░░░░░░░░░░]   2.356 ms   5.7%
event                              [░░░░░░░░░░░░░░░░░░░░░░░░░░░░]   0.000 ms   0.0%
```

## Load chart — top offenders

```text
newengine-plugin-host:plugin_life… [██████░░░░░░░░░░░░░░░░░░░░░░]   8.220 ms  19.7%
newengine-plugin-host:plugin_life… [███░░░░░░░░░░░░░░░░░░░░░░░░░]   4.612 ms  11.1%
newengine-plugin-host:plugin_life… [███░░░░░░░░░░░░░░░░░░░░░░░░░]   4.266 ms  10.2%
newengine-plugin-host:plugin_life… [███░░░░░░░░░░░░░░░░░░░░░░░░░]   4.066 ms   9.8%
newengine-plugin-host:plugin_life… [██░░░░░░░░░░░░░░░░░░░░░░░░░░]   3.422 ms   8.2%
newengine-plugin-host:plugin_life… [██░░░░░░░░░░░░░░░░░░░░░░░░░░]   3.392 ms   8.1%
newengine-plugin-host:plugin_life… [██░░░░░░░░░░░░░░░░░░░░░░░░░░]   3.203 ms   7.7%
newengine-plugin-host:plugin_life… [██░░░░░░░░░░░░░░░░░░░░░░░░░░]   3.115 ms   7.5%
newengine-plugin-host:plugin_life… [██░░░░░░░░░░░░░░░░░░░░░░░░░░]   2.656 ms   6.4%
newengine-plugin-host:plugin_life… [██░░░░░░░░░░░░░░░░░░░░░░░░░░]   2.371 ms   5.7%
```

## Load chart — lanes

```text
plugin                             [██████████████████████████░░]  39.322 ms  94.3%
render-init                        [██░░░░░░░░░░░░░░░░░░░░░░░░░░]   2.356 ms   5.7%
unspecified                        [░░░░░░░░░░░░░░░░░░░░░░░░░░░░]   0.000 ms   0.0%
```

## Top offenders by total elapsed time

| Rank | Offender | Source | Category | Calls | Total ms | Share | Avg ms | Max ms | Max load | Slow | Failed |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/starVault-assetManager-3.5.2-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 8.220 | 19.7% | 8.220 | 8.220 | 0.49x | 0 | 0 |
| 2 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/vulkan-renderer-0.31.2-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 4.612 | 11.1% | 4.612 | 4.612 | 0.28x | 0 | 1 |
| 3 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/fps-game-0.1.0-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 4.266 | 10.2% | 4.266 | 4.266 | 0.26x | 0 | 0 |
| 4 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/gravitas-physics-0.3.0-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 4.066 | 9.8% | 4.066 | 4.066 | 0.24x | 0 | 0 |
| 5 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/projectBrowser-egui-0.1.0-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 3.422 | 8.2% | 3.422 | 3.422 | 0.21x | 0 | 0 |
| 6 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/constellation-ecs-0.1.5-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 3.392 | 8.1% | 3.392 | 3.392 | 0.20x | 0 | 0 |
| 7 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/gameReady-runtime-0.1.0-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 3.203 | 7.7% | 3.203 | 3.203 | 0.19x | 0 | 0 |
| 8 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/editor-runtime-0.3.0-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 3.115 | 7.5% | 3.115 | 3.115 | 0.19x | 0 | 0 |
| 9 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/aurelia-ui-0.5.12-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 2.656 | 6.4% | 2.656 | 2.656 | 0.16x | 0 | 0 |
| 10 | `newengine-plugin-host:plugin_lifecycle::plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/egui-ui-0.1.0-release.dll` | `newengine-plugin-host` | `plugin_lifecycle` | 1 | 2.371 | 5.7% | 2.371 | 2.371 | 0.14x | 0 | 0 |
| 11 | `engine.render.vulkan:renderer::Vulkan renderer bootstrap` | `engine.render.vulkan` | `renderer` | 1 | 2.356 | 5.7% | 2.356 | 2.356 | 0.14x | 0 | 1 |
| 12 | `engine.render.vulkan:event::engine.ui.loading.status.v1` | `engine.render.vulkan` | `event` | 3 | 0.000 | 0.0% | 0.000 | 0.000 | 0.00x | 0 | 0 |

## Top methods by total elapsed time

| Rank | Method | Source | Category | Calls | Total ms | Share | Avg ms | Max ms | Max load | Slow | Failed |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `newengine-plugin-host:plugin_lifecycle::<no-method>` | `newengine-plugin-host` | `plugin_lifecycle` | 10 | 39.322 | 94.3% | 3.932 | 8.220 | 0.49x | 0 | 1 |
| 2 | `engine.render.vulkan:renderer::<no-method>` | `engine.render.vulkan` | `renderer` | 1 | 2.356 | 5.7% | 2.356 | 2.356 | 0.14x | 0 | 1 |
| 3 | `engine.render.vulkan:event::<no-method>` | `engine.render.vulkan` | `event` | 3 | 0.000 | 0.0% | 0.000 | 0.000 | 0.00x | 0 | 0 |

## Budget violations — что пробило кадр/лимит

| Rank | Status | Category | Source | Name | Elapsed ms | Budget ms | Load | Detail |
|---:|---|---|---|---|---:|---:|---:|---|

## Frame budget violations — explicit frame envelope misses

| Rank | Frame | Lane | Category | Source | Name | Elapsed ms | Frame budget ms | Over ms | GPU wait ms | Wait reason | Async | Detail |
|---:|---:|---|---|---|---|---:|---:|---:|---:|---|---|---|

## Top single jobs by elapsed time

| Rank | Status | Category | Source | Name | Elapsed ms | Budget ms | Load | Detail |
|---:|---|---|---|---|---:|---:|---:|---|
| 1 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/starVault-assetManager-3.5.2-release.dll` | 8.220 | 16.670 | 0.49x | plugin loaded in 7 ms |
| 2 | `failed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/vulkan-renderer-0.31.2-release.dll` | 4.612 | 16.670 | 0.28x | plugin load failed or was skipped after 4.134 ms |
| 3 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/fps-game-0.1.0-release.dll` | 4.266 | 16.670 | 0.26x | plugin loaded in 2 ms |
| 4 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/gravitas-physics-0.3.0-release.dll` | 4.066 | 16.670 | 0.24x | plugin loaded in 2 ms |
| 5 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/projectBrowser-egui-0.1.0-release.dll` | 3.422 | 16.670 | 0.21x | plugin loaded in 1 ms |
| 6 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/constellation-ecs-0.1.5-release.dll` | 3.392 | 16.670 | 0.20x | plugin loaded in 1 ms |
| 7 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/gameReady-runtime-0.1.0-release.dll` | 3.203 | 16.670 | 0.19x | plugin loaded in 0 ms |
| 8 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/editor-runtime-0.3.0-release.dll` | 3.115 | 16.670 | 0.19x | plugin loaded in 1 ms |
| 9 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/aurelia-ui-0.5.12-release.dll` | 2.656 | 16.670 | 0.16x | plugin loaded in 1 ms |
| 10 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/egui-ui-0.1.0-release.dll` | 2.371 | 16.670 | 0.14x | plugin loaded in 0 ms |
| 11 | `failed` | `renderer` | `engine.render.vulkan` | `Vulkan renderer bootstrap` | 2.356 | 16.670 | 0.14x | service not found: engine.platform |
| 12 | `completed` | `event` | `engine.render.vulkan` | `engine.ui.loading.status.v1` | 0.000 | 16.670 | 0.00x | Vulkan provider is parsing config and preparing the native surface handshake. |
| 13 | `completed` | `event` | `engine.render.vulkan` | `engine.ui.loading.status.v1` | 0.000 | 16.670 | 0.00x | Shader-cache policy, clear color and backend feature flags are known. |
| 14 | `completed` | `event` | `engine.render.vulkan` | `engine.ui.loading.status.v1` | 0.000 | 16.670 | 0.00x | service not found: engine.platform |

## Top single jobs by budget load

| Rank | Status | Category | Source | Name | Elapsed ms | Budget ms | Load | Detail |
|---:|---|---|---|---|---:|---:|---:|---|
| 1 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/starVault-assetManager-3.5.2-release.dll` | 8.220 | 16.670 | 0.49x | plugin loaded in 7 ms |
| 2 | `failed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/vulkan-renderer-0.31.2-release.dll` | 4.612 | 16.670 | 0.28x | plugin load failed or was skipped after 4.134 ms |
| 3 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/fps-game-0.1.0-release.dll` | 4.266 | 16.670 | 0.26x | plugin loaded in 2 ms |
| 4 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/gravitas-physics-0.3.0-release.dll` | 4.066 | 16.670 | 0.24x | plugin loaded in 2 ms |
| 5 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/projectBrowser-egui-0.1.0-release.dll` | 3.422 | 16.670 | 0.21x | plugin loaded in 1 ms |
| 6 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/constellation-ecs-0.1.5-release.dll` | 3.392 | 16.670 | 0.20x | plugin loaded in 1 ms |
| 7 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/gameReady-runtime-0.1.0-release.dll` | 3.203 | 16.670 | 0.19x | plugin loaded in 0 ms |
| 8 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/editor-runtime-0.3.0-release.dll` | 3.115 | 16.670 | 0.19x | plugin loaded in 1 ms |
| 9 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/aurelia-ui-0.5.12-release.dll` | 2.656 | 16.670 | 0.16x | plugin loaded in 1 ms |
| 10 | `completed` | `plugin_lifecycle` | `newengine-plugin-host` | `plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/egui-ui-0.1.0-release.dll` | 2.371 | 16.670 | 0.14x | plugin loaded in 0 ms |
| 11 | `failed` | `renderer` | `engine.render.vulkan` | `Vulkan renderer bootstrap` | 2.356 | 16.670 | 0.14x | service not found: engine.platform |
| 12 | `completed` | `event` | `engine.render.vulkan` | `engine.ui.loading.status.v1` | 0.000 | 16.670 | 0.00x | Vulkan provider is parsing config and preparing the native surface handshake. |
| 13 | `completed` | `event` | `engine.render.vulkan` | `engine.ui.loading.status.v1` | 0.000 | 16.670 | 0.00x | Shader-cache policy, clear color and backend feature flags are known. |
| 14 | `completed` | `event` | `engine.render.vulkan` | `engine.ui.loading.status.v1` | 0.000 | 16.670 | 0.00x | service not found: engine.platform |

## By category

| Category | Count | Failed | Slow | Total ms | Share | Avg ms | Max ms |
|---|---:|---:|---:|---:|---:|---:|---:|
| `plugin_lifecycle` | 10 | 1 | 0 | 39.322 | 94.3% | 3.932 | 8.220 |
| `renderer` | 1 | 1 | 0 | 2.356 | 5.7% | 2.356 | 2.356 |
| `event` | 3 | 0 | 0 | 0.000 | 0.0% | 0.000 | 0.000 |

## By source

| Source | Calls | Total ms | Share | Avg ms | Max ms | Max load | Slow | Failed |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `newengine-plugin-host` | 10 | 39.322 | 94.3% | 3.932 | 8.220 | 0.49x | 0 | 1 |
| `engine.render.vulkan` | 4 | 2.356 | 5.7% | 0.589 | 2.356 | 0.14x | 0 | 1 |

## Active jobs

No active jobs at report flush time.

## CSV outputs

When enabled, the profiler writes these machine-readable tables:

| CSV | Purpose |
|---|---|
| `profiler_jobs_latest.csv` | all completed jobs with elapsed/budget/load columns |
| `profiler_top_offenders_latest.csv` | grouped suspects sorted by total captured time |
| `profiler_categories_latest.csv` | category totals and share-of-time |
| `profiler_sources_latest.csv` | source totals and share-of-time |
| `profiler_active_jobs_latest.csv` | jobs still running at flush time with current load |
| `profiler_timeline_latest.csv` | completed jobs with run-relative start/end offsets |
| `profiler_methods_latest.csv` | method/service grouped timing totals |
| `profiler_budget_violations_latest.csv` | jobs where `load >= 1.0` or slow threshold was crossed |
| `profiler_first_latest.csv` | scheduled/blocked/polling/GPU-wait/frame-budget/async counters |
| `profiler_lanes_latest.csv` | lane totals and share-of-time |
| `profiler_frame_budget_latest.csv` | explicit frame-budget misses with lane/frame/wait fields |
| `profiler_diagnostics_latest.csv` | warnings/errors emitted by profiler analysis |

## Diagnostics

- `error` `failed_job`: job 'plugin_load:C:/Users/Aiden/Documents/Repos/NorthStar/pluginsRuntime/vulkan-renderer-0.31.2-release.dll' failed: <no error payload>
- `error` `failed_job`: job 'Vulkan renderer bootstrap' failed: <no error payload>
- `warn` `job_end_without_begin`: job 'host.plugin_load.4' ended without a matching begin event
- `warn` `job_end_without_begin`: job 'host.plugin_load.4' ended without a matching begin event
