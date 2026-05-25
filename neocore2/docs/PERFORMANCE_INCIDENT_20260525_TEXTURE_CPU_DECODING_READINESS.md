# PERFORMANCE INCIDENT — texture CPU decoding readiness state

> [!INFO] INFO BLOCK — текущее положение дел
> **У нас сейчас:** async material texture decode добавил новое состояние `MaterialTextureGpuResidency::CpuDecoding`, но launch/readiness gate всё ещё был exhaustive match только по старым состояниям.
>
> **Technical details (EN):** `newengine-engine-runtime/src/render_controller/module_impl/readiness.rs` matched `Ready`, `Failed`, `Requested`, `AssetLoading`, `GpuLoading`; after introducing `CpuDecoding`, Rust correctly rejected the non-exhaustive match with `E0004`.

## Fix

`CpuDecoding` is a non-fatal pending state. It means:

```text
texture request accepted
CPU decode job is running on engine.jobs
GPU upload not queued yet
render must keep using fallback texture
launch gate must treat it as Waiting, not Failed
```

The readiness gate now handles:

```rust
MaterialTextureGpuResidency::Requested
| MaterialTextureGpuResidency::AssetLoading { .. }
| MaterialTextureGpuResidency::CpuDecoding { .. } => TextureReadyState::Waiting,
```

## Rule

Every new residency/state-machine variant must be classified in all readiness, fallback and diagnostics match sites.

```text
Ready       -> can launch/use
Waiting     -> keep rendering fallback / keep launch gate blocked
Failed      -> failure diagnostics / optional skip policy
```

## Next migration

The next destructive migration must consolidate copied NEF8 family crates only after the unified `newengine-asset-format-nef8` registry is wired into Cargo and all consumers.

Target shape:

```text
newengine-asset-format-nef8
  descriptors:
    extension
    content_kind
    semantic_gateway
    owner_domain
    body_schema
```

Only after that should root-level `DELETE_FILES.txt` remove old duplicate crates.
