# newengine-scene-runtime

Runtime-hosted `engine.scene` gateway runtime service.

This crate owns scene load/save DTO handling and route registration. Product/profile crates decide whether to install it; scene gateway implementation no longer lives inside the game-ready profile crate.
