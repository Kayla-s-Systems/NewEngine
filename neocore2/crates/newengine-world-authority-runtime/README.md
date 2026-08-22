# newengine-world-authority-runtime

Domain adapter for inspecting and driving the selected `engine.ecs` / `engine.entity` provider authority. It intentionally lives outside `newengine-runtime-host` so engine/world runtimes never depend upward on the process host merely to inspect provider topology.
