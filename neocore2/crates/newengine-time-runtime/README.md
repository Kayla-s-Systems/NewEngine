# newengine-time-runtime

Engine-owned baseline provider for `engine.time`.

It is deliberately small and replaceable. Future providers may implement
replay-deterministic, editor scrub, or network-synchronised clock behaviour under
the same `newengine.time.runtime.v1` contract.
