# Technical Debt and Gaps Roadmap

## P0: release blockers

### Provider and gateway conformance tests

Add conformance suites for:

```text
engine.render
engine.physics
engine.assets
engine.input
```

Tests should cover metadata validation, unknown service kinds, missing gateways, multiple providers, null providers, degradation mode, and strict mode.

### Zero-warning CI

CI should run `cargo fmt`, Clippy with `-D warnings`, build, tests, doc tests, Miri, and coverage.

### Gateway routing conformance

Required tests:

- descriptor-only provider selection;
- no filename identity;
- deterministic tie-breakers;
- service id rewrite through `engine.*` gateways;
- missing provider degradation;
- explicit strict fatal mode.

## P1: scalability

- Formalize `newengine-input-api`.
- Add a formal UI provider API.
- Harden physics contact events, replay fixtures, streamed collider diffs, and optional binary packets.

## P2: tooling and content

- Strengthen `.neytd` validation and mip audits.
- Add `.nepak` signing, package diffing, and patch workflows.
- Expand reload/shutdown tests.
