# KΛYLΛ FPS: Extraction Yard

Minimal playable first-person vertical slice built on the `newengine-game-ready-profile` runtime profile.

Run from the workspace root:

```bash
cargo run -p game-ready-fps
```

Controls:

- `WASD` — move
- Mouse — look
- `Shift` — sprint
- `ESC` — open the pause menu / exit through the declarative menu action

Goal:

Collect the 3 blue cores, avoid purple hazard zones, then reach the red extraction beacon.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/apps/game-ready-fps`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 3 direct files.

**Direct file examples:** `build.rs`, `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
