# Game Ready FPS vertical slice

`game-ready-fps` is a standalone runtime profile. It does not depend on the editor scene state for the playable demo.

## Declarative scene profile

Default profile:

```text
neocore2/apps/game-ready-fps/assets/game_ready_highlands.scene.json
```

Override with:

```bat
set NEWENGINE_GAME_READY_PROFILE=C:\path\to\scene.json
```

The profile owns the basic vertical-slice contract:

- player start, yaw, movement speed, look sensitivity;
- player collision/body tuning, camera eye height, sprint multiplier;
- gravity/contact skin;
- deterministic terrain generator seed and shape parameters;
- skydome and collision ceiling;
- arena boundaries;
- obstacle and foliage procedural placement;
- prefabs/material-training anchors;
- pickups, hazards, extraction markers;
- HUD/status/progress text.

## Runtime path expectations

The launcher enables `NEWENGINE_GAME_READY_DEMO=1` and resolves the profile from the app assets folder when `NEWENGINE_GAME_READY_PROFILE` is not set.

Platform runtime lookup accepts both singular and plural plugin directory environment variables:

```bat
set NEWENGINE_PLUGIN_DIR=NewEngine\neocore2\plugins
set NEWENGINE_PLUGINS_DIR=NewEngine\neocore2\plugins
set NEWENGINE_PLATFORM_RUNTIME_DIR=NewEngine\neocore2\plugins
```

## Goal of the slice

The baseline playable loop is:

1. spawn into a deterministic procedural 3D arena;
2. walk/sprint with terrain collision;
3. collect all blue cores;
4. avoid purple hazard triggers;
5. reach red extraction beacon;
6. display stable HUD progress/status.
