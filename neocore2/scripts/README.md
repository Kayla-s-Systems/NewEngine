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
