# newengine-assets-ui-runtime

Runtime-hosted semantic compiler for `.neui` UI dictionaries.

Boundary:

```text
engine.assets / VFS          -> bytes
NEF8/ListFile validation     -> envelope/body integrity
engine.assets.ui             -> XMLcentral UI semantics + compile response
engine.ui                    -> live mounted runtime UI
```

Consumers issue request/response calls to `engine.assets.ui`; they do not parse `.neui`, NEF8, deflate, XMLcentral, or VFS details.
