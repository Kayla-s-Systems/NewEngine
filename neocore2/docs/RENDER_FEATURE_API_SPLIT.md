# Render Feature API Split

## Purpose

`newengine-render-feature-api` is the in-process contract between reusable engine
runtime and profile-owned render feature packs.

The split removes the old coupling where a feature pack had to depend on
`newengine-engine-runtime` just to implement draw-list and light extraction
traits. Feature packs now implement stable provider contracts and the active
runtime consumes those providers as trait objects.

## Crate ownership

```text
newengine-render-feature-api
  owns provider traits, extraction DTO refs, provider metadata,
  packed light/shadow DTOs and feature commands

newengine-engine-runtime
  owns RuntimeRenderController, feature registries, command lowering,
  GPU resource caches, shadow render-target lifetime and backend submission

newengine-render-feature-gameready
  owns GameReady terrain/mesh/UI/light policy providers
  depends on newengine-render-feature-api
  does not depend on newengine-engine-runtime

newengine-game-ready-profile
  composes RuntimeRenderController + GameReadyRenderFeaturePack providers
```

## Draw-list provider flow

1. The profile creates `RuntimeRenderController`.
2. The profile obtains providers from `GameReadyRenderFeaturePack`.
3. The controller stores `Arc<dyn RenderDrawListProvider>`.
4. During extraction, providers emit feature-level draw commands through
   `DrawListBuildCtx`.
5. `newengine-engine-runtime` lowers those commands through its internal render
   passes and submits them to `render.api`.

Providers never receive `RuntimeRenderController` directly.

## Light extraction flow

Light providers return `LightExtractionCommand` values such as:

- `DirectionalShadow`
- `Unsupported(ShadowLightKind::Point)`
- `Unsupported(ShadowLightKind::Spot)`
- `Disabled`

The runtime lowers those commands into `LightShadowPlan` because render-target
allocation, shadow cache invalidation and backend submission are runtime-owned
responsibilities.

## Architectural invariant

Feature packs are profile-owned provider bundles, not appendages of a concrete
runtime controller. Runtime owns lowering. Backend owns rendering. Profile owns
policy.
