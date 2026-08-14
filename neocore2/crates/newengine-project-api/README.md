# newengine-project-api

Project/game manifest, launch-plan and logical content-mount contracts for NewEngine.

`game.toml` is the authoritative product boundary. Engine/runtime code must not contain a concrete map, player, HUD, gameplay package or project path when the same value can be declared here or supplied by a provider.

## Launch presets

A single project may be opened in different runtime modes without using a different executable:

```toml
format_version = 1
id = "my-game"
name = "My Game"
runtime_profile = "my.runtime-profile"
startup_scene = "maps/start.ymap"
default_launch = "editor"

[launch.editor]
profile = "editor"

[launch.game]
profile = "game"
startup_presentation_state = "gameplay"

[[content]]
id = "my-game.content"
namespace = "game"
root = "Content"
mount = "/"
priority = 300
required = true
owner = "project:my-game"
```

The launcher resolves a `ResolvedProjectLaunch` from project defaults plus the selected preset. `--launch <id>` and `NEWENGINE_PROJECT_LAUNCH_PRESET` select a preset explicitly.

## Game module boundary

`game_module` is the project-owned runtime plugin identity. The versioned `newengine-game-module-api` contract defines how a future game module describes its capabilities and provider interfaces through `engine.game.module / game.describe_v1`.

The intended dependency direction is:

```text
NewEngine / runtime-host
        -> engine contracts
        -> project manifest
        -> versioned provider interfaces
        <- game module plugin
        <- game scripts/content
```

not `engine -> concrete FPS/game crate`.
