# 06 — Зазоры и технический долг: roadmap

## P0 — release blockers / strict cleanliness

### 1. Provider conformance tests

Add domain conformance suites:

```text
render.api provider tests
physics.api provider tests
asset.manager provider tests
```

Physics tests should cover deterministic replay packets, collider insertion/removal, commands, queries, null provider behavior, and native provider parity expectations.

### 2. Zero-warning CI

Current run still shows visibility warnings in render draw/light provider internals and one unused stride helper. These are not architecture violations but should be fixed before release CI.

### 3. Runtime service contract source of truth

Startup contract validation still has manual service lists. Target: profile/startup declares required capabilities/services; validation walks resolver output.

## P1 — scalability

### 4. Input API formalization

Input is service-backed but should become a first-class `newengine-input-api` crate with typed envelopes and host adapter.

### 5. UI provider API formalization

UI provider selection should follow the same strict pattern as render/physics: explicit service API crate, provider capability, null provider, resolver validation.

### 6. Physics hardening

- contact manifolds/events;
- collision filters/layers;
- streamed collider diffing;
- binary frame packet option;
- debug draw packet protocol;
- replay fixtures.

## P2 — tooling and content

### 7. Codec tools

- `netexturetool`: stronger `.neytd` validation, batch compression profiles, mip audit.
- `NePak`: canonical `.nepak` output, archive signing option, package diff/patch workflows.

### 8. Shutdown/reload

DLL unload remains process-lifetime in normal runs. Add recreate/reload tests for providers and document unload policy.

## Updated sprint order

```text
Sprint A — Provider conformance and zero-warning cleanup
Sprint B — Physics contact/replay hardening
Sprint C — Input/UI API formalization
Sprint D — Asset codec validation tooling
Sprint E — Reload/shutdown test matrix
```
