# newengine-ui-notify-runtime

Runtime-owned baseline provider for the `engine.ui.notify` gateway.

The crate subscribes to the typed `GameMessageEnvelope` pipeline and converts only
the `engine.ui.notify[.*]` message family into a bounded `UiToastStack`. It also
exposes direct push, dismiss, clear and snapshot methods through the replaceable
gateway provider route `engine.ui.notify.runtime`.

The runtime owns queue policy, lifetime, deduplication and severity mapping.
`engine.ui` owns retained UI presentation, and the renderer remains unaware of
notification semantics. Missing UI presentation is non-fatal: messages remain
bounded and queryable through the gateway snapshot.
