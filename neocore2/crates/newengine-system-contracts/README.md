# newengine-system-contracts

Small value contracts for the NewEngine system layer.

This crate intentionally contains no platform, renderer, asset-manager or gameplay implementation.
It describes system state in a uniform format so platform shells, runtime hosts and tooling can
communicate boot/apply/sync/recovery/profiling status without parsing logs or renderer debug text.
