# newengine-editor-viewport-runtime

Focused editor viewport controller and policy owner.

Owns:

- editor viewport state and activation;
- transform drag/history/undo/redo policy;
- orthographic editor camera projection policy;
- gizmo handle types and gizmo geometry planning;
- neutral transform side-effect hook through `EditorTransformEffects`.

It intentionally does not know `newengine-engine-runtime`, `SceneBridge`, gameplay implementations, physics implementation types, material admission internals, or plugin host state.

Engine composition may adapt this policy to concrete Scene/render/physics state, but dependency direction is one-way: engine/composition -> editor viewport runtime.
