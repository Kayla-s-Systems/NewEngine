# North Star Engine — Engine Jobs Hot-path Contract Fix

> [!INFO] INFO BLOCK — текущее положение дел
> **У нас сейчас:** FPS не вырос после UI binary-pass, потому что следующий bottleneck находится в asset/material texture path: render-controller инициирует тяжёлый `.ytd@entry` decode из кадра. `call_service_v1` остаётся синхронным по ABI, поэтому тяжёлый provider call не должен выполняться на render/present thread.
>
> **Technical details (EN):** `RuntimeRenderController::pump_material_texture_requests` now submits material texture decode work to `JobSystemHandle` on `JobLane::AssetIo` with `JobPriority::Interactive`; render polls completed tickets and only enqueues GPU upload on completion.

> [!WARN] WARN BLOCK — риск / болячка / техдолг
> **У нас сейчас:** попытка сделать temporary async bridge через `std::thread::spawn` нарушает целевую модель: работа становится невидимой для `engine.jobs`, profiler/job diagnostics не видят lifecycle, а поток создаётся ad-hoc вместо engine-owned worker pool.
>
> **Почему это важно:** `jobs/tasks async by default` должно быть контрактом задачи, а не скрытым side effect. Иначе невозможно управлять cancel/pause, lane budget, pending/running counters и shutdown ordering.
>
> **Technical details (EN):** no ad-hoc thread in render texture decode path; state stores `MaterialTextureDecodeJob { ticket, result }`, where `ticket` is an engine-owned `JobTicket`.

> [!NOTE] NOTE BLOCK — желаемое направление
> **Было бы здорово:** следующий pass должен поднять это из render-local bridge в официальный `engine.assets.jobs` / `engine.jobs` protocol: `submit -> ticket`, `poll -> ready/not_ready`, `result -> binary DTO`, `cancel -> cooperative checkpoint`.
>
> **Technical details (EN):** keep `call_service_v1` sync for immediate methods (`info_json`, `state`, `draw_frame`, `submit_frame`). Heavy methods must expose explicit async methods or return `not_ready` and a ticket/id.

## Contract rule

```text
Small query/control:
  sync call_service_v1 OK

Heavy hot-path work:
  submit job -> ticket/id
  worker executes provider call
  render/gameplay polls status/result
  apply completed output on owner thread
```

## Fixed in this pass

```text
render_controller/resource_cache.rs
  before: std::thread::spawn per texture decode job
  after: engine.jobs JobSystemHandle submit_request(... JobLane::AssetIo ...)

render_controller/state.rs
  adds MaterialTextureDecodeJob and texture_decode_jobs state

render_controller/material_bindings.rs
  adds CpuDecoding residency state

render_controller/module_impl/render_entry.rs
render_controller/module_impl/prelaunch_gate.rs
  pass ctx.job_system() into material texture pump

runtime-host/platform_runtime/ui_gateway_frame.rs
  removes stale unused pause_menu publisher left after modal UI ownership moved to render-controller
```

## Next destructive cleanup

Do **not** add `DELETE_FILES.txt` for NEF8 family crates until workspace/Cargo rewiring is ready.

Target destructive migration:

```text
newengine-asset-format-ytd
newengine-asset-format-ydd
newengine-asset-format-ytyp
newengine-asset-format-nemat
newengine-asset-format-ymap
...
  -> newengine-asset-format-nef8
     + content-kind descriptor registry
     + extension/content_kind/gateway map
     + per-domain body schema modules where real semantics differ
```

Rule:

```text
NEF8/ListFile frame is one implementation.
Content kinds are declarations.
Only real semantic body schemas deserve separate modules.
```
