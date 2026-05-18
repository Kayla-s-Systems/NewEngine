# newengine-input-bindings

Engine-owned third-level input domain for semantic bindings and resolved gameplay actions.

- `engine.input.bindings` owns data/configuration.
- `engine.input.actions` owns semantic interpretation output.

Gameplay and camera systems consume actions such as `player.move.forward` and `camera.view.next`; they do not hard-code physical keys.
