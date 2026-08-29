# newengine-audio-world-runtime

Bounded world-audio **implementation owner**. It owns spatial emitter lifecycle, authored ambience/environment resolution, the priority-0 `engine.audio` fallback provider, and the audio-scene/ambience runtime-unit materializers.

Boundary rules:

- stable `engine.audio` calls come from `newengine-audio-client`;
- transient physics/world-audio state shapes come from `newengine-audio-world-api`;
- this crate may know Scene/ECS and provider infrastructure;
- it must not know `newengine-engine-runtime`, `SceneBridge`, gameplay physics contributors, or render-controller implementation;
- the fallback provider may register `engine.audio`, but it must not contain generic client transport calls.
