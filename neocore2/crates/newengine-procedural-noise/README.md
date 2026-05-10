# newengine-procedural-noise

Deterministic procedural generation foundation for NewEngine.

This crate owns:

- parameterized 2D value/fractal noise;
- heightfield generation;
- CPU terrain mesh baking;
- coarse collision tile generation for current AABB gameplay physics;
- ECS-friendly procedural terrain component.

It intentionally has no renderer, plugin-host, filesystem, or random global state dependencies.
