# newengine-system-runtime

Bridge/presenter helpers for the NewEngine system layer.

This crate owns no renderer, no gameplay, no platform window and no UI provider. It maps subsystem
state into stable contracts such as `ScreenOverlayStatus` and provides ABI-safe conversion for the
platform shell.
