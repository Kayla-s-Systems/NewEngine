# Camera Runtime Blend Runners Pass

Camera blend runners own transitional camera motion between runtime camera states.

Design goals:

- deterministic transitions;
- explicit blend lifetime;
- no hidden writes to transform storage;
- renderer receives final snapshots, not controller internals.
