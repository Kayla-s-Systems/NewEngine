# gameFps — North Star 3D FPS demo

Standalone 3D FPS launcher for the GameReady runtime profile.

The engine is only the executor. `gameFps` declares what it wants before the
runtime starts:

```text
screen_profile = game
scene_profile  = game_ready_highlands.ymap
boot_options   = startup selector + runtime plugins + platform window + render backend
```

Run from `NewEngine/neocore2`:

```powershell
cargo run -p game-fps
```

Runtime path:

```text
RuntimeHostLauncher -> GameReadyRuntimeProfile -> platform runtime -> render backend -> scene bootstrap
```

Expected behavior:

- the pre-start profile/config window is allowed;
- the app can directly request `engine.runtime.ui.screen_profile = game`;
- editor chrome is not the default game presentation;
- the scene bootstrap is allowed immediately because the UI screen profile is `game`;
- runtime assets are mounted from `gameAssets`;
- runtime plugins are loaded from `pluginsRuntime`.

The game owns rules. The engine owns execution profiles and provider routing.
