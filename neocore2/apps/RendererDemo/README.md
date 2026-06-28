# RendererDemo

Renderer-focused runtime smoke target for NewEngine.

Run from `EngineRepo/NewEngine/neocore2`:

```bash
cargo run -p renderer-demo
```

Expected result:

- opens a platform window titled `RendererDemo: Shaded Lighting Scene`
- loads the `game_ready_highlands.ymap` scene profile
- uses the GameReady material feature pack and light extraction providers
- renders a shaded/lit demo scene through the configured render backend

This app is intentionally a thin launcher over `GameReadyRuntimeProfile`; renderer ownership stays in `Plugins/VulkanRenderer` and the GameReady render feature pack.
