# PERFORMANCE INCIDENT 2026-05-25 — Async Tasks / Render Hot Path

> [!INFO] INFO BLOCK — текущее положение дел
> **У нас сейчас:** FPS остаётся около 15 после удаления лог-флуда и перевода modal UI на binary path. Новый profiler report показывает, что `aurelia.ui.api::draw_frame_v1` больше не является главным offender, но `winit` всё ещё видит `host.step` в районе 80–95 ms. Главный новый симптом — `render controller: material texture pump yielded ... elapsed_ms=95..668+`, то есть render frame синхронно ждёт `.ytd@entry` texture runtime decode / packet preparation.
>
> **Technical details (EN):** Source snapshot: `NorthStar-Engine-source-20260525-230138.zip`; profiler report: `profiler_report_20260525_200112_242Z.zip`. Hot path: `RuntimeRenderController::pump_material_texture_requests` -> `AssetServiceClient::textures_entry_runtime_ref_v1_typed` -> `engine.assets.textures / assets.textures.entry_runtime_v1` -> `TextureRuntimeState::ensure_runtime_dictionary_cache` -> `engine.assets / asset.decode_v1`.

> [!WARN] WARN BLOCK — synchronous provider call in frame
> **У нас сейчас:** `call_service_v1` остаётся синхронным ABI-вызовом. Это нормально для small control methods, но опасно для heavy frame work. Когда render-controller вызывает texture/material/model decode через service call, caller thread ждёт provider до конца.
>
> **Почему это важно:** render thread не должен выполнять IO, NEF8 decode, texture dictionary parse, material graph expansion или asset graph resolve. Эти задачи должны стать job/task work; render frame только enqueues, polls and applies completed packets.
>
> **Technical details (EN):** `newengine-plugin-host::call_service_v1` gets service from registry and calls `svc.call(method, payload)` directly on the caller thread. Profiler diagnostics wrap the call but do not make it asynchronous.

> [!NOTE] NOTE BLOCK — текущий pass
> **Было бы здорово:** первый безопасный шаг — не менять ABI `HostApiV1` сразу, а убрать blocking `.ytd` texture decode из render frame. Render-controller теперь запускает bounded CPU decode jobs, poll-ит завершённые jobs и только потом enqueue-ит GPU upload. Это bridge до полноценного `engine.jobs`/`engine.assets.jobs` contract.
>
> **Technical details (EN):** `RenderMaterialGpuState::texture_decode_jobs: FxHashMap<String, JoinHandle<Result<RuntimeTextureAsset, AssetError>>>` is now used by `resource_cache.rs`. `pump_material_texture_requests` no longer calls `assets.textures.entry_runtime_v1` synchronously and no longer calls `asset.pump_v1` from render frame.

## Почему не “просто сделать call_service_v1 async”

`call_service_v1` сейчас возвращает `RResult<Blob, RString>`. Если сделать его magically async, сломаются методы, которым ответ нужен прямо сейчас:

```text
info_json
state snapshot
input state
small route diagnostics
binary UI draw output
render command batch submit result
```

Правильная модель:

```text
small control/query method:
  call_service_v1 -> immediate result

heavy method:
  submit/enqueue -> JobTicket / AssetId / NotReady
  worker executes IO/decode/build
  frame polls status/result
  apply stage commits completed output
```

## Новое правило hot path

```text
Frame hot path must not perform provider-owned heavy work.
Frame hot path may only:
  - enqueue work;
  - poll bounded completions;
  - upload already prepared payloads through GPU queues;
  - use fallback/default resources while work is pending.
```

## Следующий архитектурный pass

1. Add `engine.jobs` / `engine.assets.jobs` provider-visible capability.
2. Add service methods:

```text
engine.assets.jobs / assets.job_submit_v1
engine.assets.jobs / assets.job_status_v1
engine.assets.jobs / assets.job_result_bin_v1
```

3. Convert these methods from blocking to ticket-based:

```text
assets.textures.entry_runtime_v1
assets.materials.resolve_graph_v1
assets.graph.resolve_v1
model.assemble_json_v1 / model.assemble_bin_v1
```

4. Keep JSON only for diagnostics/control:

```text
*_json_v1 = debug/devtools/control
*_bin_v1  = frame hot path
```

## NEF8 family cleanup note

The `newengine-asset-format-*` crates are mostly descriptor wrappers around the same NEF8/ListFile envelope. The target is:

```text
newengine-asset-format-nef8
  registry of declared ListFile content kinds
  extension -> content_kind -> gateway -> outputs
```

Then old per-format crates can be removed with root-level `DELETE_FILES.txt` only after Cargo dependencies are rewired. Do not drop those directories before the workspace member list and downstream dependencies are moved, or the build will break.
