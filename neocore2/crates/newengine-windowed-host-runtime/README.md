# newengine-windowed-host-runtime

Concrete window/input/platform/UI frontend composition above `newengine-runtime-host`.

The Void Engine host owns process bootstrap, immutable PreInit capabilities, project/runtime composition and headless control-plane lifecycle. This crate owns native platform DLL discovery, window handles, input-to-UI projection, bootstrap overlays, screen-profile policy and the windowed event loop.

Dependency direction is one-way:

```text
newengine-host-kernel
        ^
newengine-runtime-host
        ^
newengine-windowed-host-runtime
        ^
product composition
```
