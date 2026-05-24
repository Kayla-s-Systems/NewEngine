# newengine-scripting-runtime

Runtime shell for the `engine.scripting` gateway.

This crate intentionally does not embed or name any scripting implementation. It
registers an engine-owned baseline provider that accepts opaque `.ysc@entry`
module bytes and opaque scripting request bytes, then returns an empty response
until a real provider overrides the `engine.scripting` route.

The primary path is:

```text
scripting.load_module_bytes_v1
scripting.invoke_bytes_v1
scripting.frame_bytes_v1
```

Deprecated JSON frame/module methods are kept as compatibility adapters so
existing engine code keeps working during migration. They do not interpret script
payloads and do not declare a language/VM whitelist.

Providers own interpretation. ECS/entity/scene/UI/audio mutation must happen via
validated engine-facing outputs and authoritative apply stages, never through
direct provider access to engine internals.
