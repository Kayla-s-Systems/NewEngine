# newengine-render-ui-bridge

Renderer-neutral bridge from `engine.ui` output to the runtime frame graph.

This crate does not know about Aurelia internals and does not depend on Vulkan.
It only routes an already-built `UiDrawList` into the `RenderDrawListKind::Ui`
path so the active renderer backend can composite it through its own UI pass.
