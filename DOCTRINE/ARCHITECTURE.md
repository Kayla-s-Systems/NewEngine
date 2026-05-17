# Architecture Doctrine

CoreEngine is a host, not a hardwired backend wrapper.

## Principles

- Engine-owned gateways are the public runtime entrypoints.
- Provider services are private implementation endpoints.
- Descriptor metadata selects implementations.
- Missing optional providers degrade by default.
- Strict requirements are explicit profile/runtime policy.
- Feature packs own gameplay/profile-specific behavior.

## Target shape

```text
consumer -> engine.* gateway -> selected provider service -> typed adapter -> runtime system
```
