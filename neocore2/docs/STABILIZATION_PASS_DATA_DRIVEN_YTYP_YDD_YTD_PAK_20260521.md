# Stabilization pass — data-driven YTYP/YDD/YTD/PAK chain

## Target

Move the engine toward a data-driven construction model:

```text
.ytyp Definition Entries -> .ydd drawable dictionary -> .ytd texture dictionary
.pak package -> VFS delivery for the chain
```

Scene/runtime code should no longer grow branches like "create material here, put
it on model there". `.ytyp` declares dependencies, AssetManager/codecs expose
format manifests, and `engine.model` can return a declarative construction plan.

## Decisions

- `.ydd` is the only native 3D drawable dictionary container.
- `.ytd` is the public texture dictionary contract.
- `.neytd` is legacy/cache-only and is no longer part of the authored chain.
- `.pak` is the public and canonical package role; NEPAK remains only the internal wire magic name
  container extension and compatibility alias.

## Runtime contract

Added `DataDrivenConstructionPlan` in `newengine-model-domain-api` and exposed it
through:

```text
engine.model:model.construction_plan_json_v1
```

The plan contains object construction rows with links for definition, drawable,
texture dictionary, physics, bounds, LOD and material binding policy. This is the
future bridge between authored data and runtime model/material assembly.

## Tooling

`tools/neassetchain` now has:

- CLI doctor / JSON / role inspection;
- construction-plan template output;
- egui premium inspector UI matching the current NewEngine tool style;
- workspace entry point `asset-chain-ui`.
