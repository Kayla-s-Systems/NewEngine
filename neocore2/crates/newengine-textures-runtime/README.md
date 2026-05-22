# newengine-textures-runtime

Engine-owned `engine.textures` runtime service for `.ytd` texture dictionaries.

Boundary rule:

```text
engine.textures owns .ytd semantics and runtime texture packets.
engine.assets owns VFS bytes and codec dispatch.
renderer/UI/materials consume texture packets or validation DTOs, never raw .ytd bytes.
```
