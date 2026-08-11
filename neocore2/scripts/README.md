# scripts

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/scripts`

**Role:** Maintenance/source-audit scripts for the engine workspace.

**Local contents:** 0 direct subdirectories, 20 direct files.

**Direct file examples:** `asset_domain_boundary_scan.py`, `asset_format_boilerplate_scan.py`, `assetmanager_codec_sync_scan.py`, `dataset_maturity_scan.py`, `gateway_contract_scan.py`, `no_hidden_thread_scan.py`, `no_legacy_scan.py`, `no_product_ui_provider_branches_scan.py`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->

## Game Ready runtime gates

Run the complete Windows shipping-path regression suite:

```powershell
python scripts/game_ready_smoke_suite.py
```

The suite drives the real Winit window and covers authored frontend/pause/settings
navigation, responsive paint-hit-test parity, save/restart/load, virtual device
hot-plug, controller-only navigation, world-partition churn with a process-memory
plateau, post-warmup render CPU budgets, Chronicle ULOG policy, and an isolated
package outside the source tree. Results are written to
`target/smoke/game-ready-suite-report.json`.

Use a physical-copy package instead of hardlinks:

```powershell
python scripts/game_ready_smoke_suite.py --package-copy
```

Run the shipping endurance profile for four wall-clock hours:

```powershell
python scripts/game_ready_smoke_suite.py --long-soak-hours 4
```

The endurance gate keeps frame-budget sampling active, samples process Working Set,
emits periodic heartbeats, enforces a stderr growth guard, and relies on Chronicle
per-run rotation for bounded structured logs.

Individual gates:

```powershell
python scripts/game_ready_pause_flow_smoke.py
python scripts/game_ready_resolution_smoke.py
python scripts/game_ready_save_load_smoke.py
python scripts/game_ready_validation_smoke.py hotplug
python scripts/game_ready_validation_smoke.py controller
python scripts/game_ready_streaming_stress_smoke.py --steps 512
python scripts/game_ready_render_soak_smoke.py --duration 14400
python scripts/game_ready_package_smoke.py --copy
```
