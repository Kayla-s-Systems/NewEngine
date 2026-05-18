# newengine-camera-contracts

Thin contract crate that re-exports stable camera DTOs from `newengine-camera-api`.

Runtime/render orchestration should depend on this contract surface when it only needs frame snapshots, not concrete camera navigation/runtime types.
