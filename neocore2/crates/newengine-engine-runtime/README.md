# newengine-engine-runtime

Reusable runtime composition layer for standalone GameFirst runtime apps.

This crate owns systems that are engine runtime responsibilities, not application responsibilities:

- scene bridge and runtime scene commands;
- gameplay components/schedules used by runtime profiles;
- viewport bridge;
- render controller that talks only to `newengine-render-api`;
- material/texture residency orchestration above the render backend.

Standalone games depend on this crate through product profiles such as `newengine-game-ready-profile`. They must not call Vulkan, create pipelines, upload textures, build shadow passes or assemble postFX directly.
