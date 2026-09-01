# newengine-gameplay-world-runtime

Engine-generic ECS gameplay world runtime. It owns reusable gameplay components, scheduling,
physics frame projection, player/controller semantics, inventory, interaction and generic runtime
state. Product FPS behavior remains in product/profile crates.

The crate consumes provider-neutral boundaries and does not depend on `newengine-engine-runtime`.
