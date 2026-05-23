# newengine-scripting-runtime

Runtime shell for the `engine.scripting` gateway.

This crate intentionally does **not** embed Lua, visual scripting, WASM or any other VM. It provides a baseline engine-owned scripting gateway service that speaks the neutral scripting DTO contract and returns empty frame output. Real scripting backends can replace it by declaring the same `engine.scripting` gateway and the appropriate backend capability.

The runtime owns orchestration, validation-friendly DTOs and diagnostics. Script providers own language execution. ECS/entity/scene mutations must happen later through authoritative apply stages, never directly inside providers.
