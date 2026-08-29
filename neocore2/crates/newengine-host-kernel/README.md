# newengine-host-kernel

Minimal host assembly for `neocore2`.

This crate owns **no gameplay or engine-domain implementation**. It constructs the
runtime kernel (lifecycle, scheduler, service registry, event bus, shutdown and
plugin host). Render, physics, assets, UI, input, networking, world and gameplay
arrive through capability/provider composition above this crate.
