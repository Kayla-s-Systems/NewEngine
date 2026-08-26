# Composition V2

**Status:** normative architecture contract
**Scope:** discovery, provider selection, runtime-unit composition, contract validation, Editor parity, freeze semantics, and composition observability.

Composition V2 defines the authority boundaries for assembling a NorthStar Engine instance. The rules in this document are invariants, not implementation suggestions.

## 1. Constitutional laws

1. **Discovery is inventory, not authority.** Discovery may enumerate artifacts, descriptors, capabilities, contracts, tags, formats, origins, and runtime units. It MUST NOT decide which semantic provider wins.
2. **`CompositionSolver` is the only provider-selection authority.** Any choice among alternative providers or runtime units MUST be represented as `CompositionCandidate` + `CompositionRequirement`/`CapabilityMatrix` input to the shared solver.
3. **Runtime-unit dependency closure uses the same authority.** A selected runtime unit's transitive `requires` entries become new composition requirements. The host re-solves until a fixed point; dependency closure MUST NOT contain a local ranker such as `max_by`, version-first selection, provider-name preference, or tag-specific selector.
4. **`CompositionPlan` is immutable after freeze.** Once the authoritative plan is frozen for an Engine instance, bootstrap/load stages may materialize and verify it but MUST NOT silently recompute a different provider choice.
5. **No semantic provider filtering before the authoritative plan.** Inventory normalization and structurally invalid descriptor rejection are allowed. Policy such as versions, contracts, preferred/required/forbidden tags, fallback, priority, and provider competition belongs to composition input and solver evaluation.
6. **The loader verifies; it does not select.** A live provider descriptor loaded from an artifact MUST match the frozen discovery identity/metadata expected for that candidate. A mismatch is a load/conformance error, not permission to choose another provider locally.
7. **`ActiveGatewayRegistry` materializes `CompositionPlan`.** The registry is a route/read model over the plan. It MUST NOT implement a second ranking algorithm.
8. **Runtime units and gateways share one capability vocabulary.** Runtime-unit `provides/requires` and engine gateway requirements are expressed through the same composition requirement semantics and system tags.
9. **Editor uses the same solver and contract catalog.** Editor format/provider composition MUST NOT introduce its own ranking, contract registry, or compatibility algorithm.
10. **Contract identity is normative, not string coincidence.** Provider and requirement contract references resolve through `RuntimeContractCatalog` to a canonical registry identity and version policy before composition.
11. **Process environment ends at the bootstrap boundary.** Launch/bootstrap code snapshots process environment once into the Engine-owned `HostContext`. Runtime policy consumes that instance snapshot and MUST NOT use process-global environment as a hidden composition input.
12. **Observability is a stable read-model boundary.** Runtime/Editor/Profiler/Console/crash tooling consume versioned composition DTOs (`composition.snapshot_v1`, `composition.diff_v1`), not serialized internal solver structures.

## 2. Authority pipeline

```text
Artifact / static-unit discovery
        |
        v
Inventory
  descriptors
  capabilities + versions
  contract references
  tags / priority / origin
  runtime-unit provides/requires
        |
        v
Contract canonicalization / validation
        |
        v
CapabilityMatrix + CompositionCandidate[]
        |
        v
CompositionSolver
        |
        +--> CompositionPlan
        |
        +--> CompositionExplanationGraph
        |
        v
freeze (when the launch mode requires authority freeze)
        |
        v
Loader verification + ActiveGatewayRegistry materialization
        |
        v
Runtime consumers
```

No arrow in this pipeline grants a lower layer permission to select a replacement provider independently.

## 3. Discovery contract

Discovery answers **what exists**. It may read static runtime-unit inventory, plugin discovery sidecars, provider manifests and first-party metadata without loading provider code. Its output is evidence for composition.

Discovery may reject malformed or unverifiable inventory entries. It may not turn `priority`, tags, versions, provider names, platform preferences, or fallback policy into an imperative load list outside `CompositionSolver`.

A frozen discovery record is also a security/conformance statement: when the loader later maps a provider, the live descriptor must agree with the identity and metadata used during composition.

## 4. Provider and requirement model

A provider candidate may expose:

- gateway/capability identity;
- capability version;
- canonicalized contract/version;
- system and per-capability tags;
- backend priority and origin bias;
- explicit fallback state;
- domain preflight eligibility facts when a domain adapter must report facts the generic solver cannot derive (for example Editor format mismatch).

A requirement may express:

- capability identity and cardinality;
- required/optional/preferred strength;
- capability version range;
- contract identity and version range;
- required, preferred and forbidden tags;
- fallback providers where the product policy explicitly declares them.

The solver owns compatibility, ranking, deterministic tie-breaking, selected/shadowed/rejected state and explanation.

## 5. Runtime-unit fixed-point closure

Runtime-unit dependencies are declarative requirements, not imperative lookups.

```text
solve root runtime-unit requirements
        |
        v
discover requires of selected units
        |
        v
promote unseen requires to CompositionRequirement
        |
        v
solve the complete requirement set again
        |
        +--> new dependency discovered --> repeat
        |
        +--> selected set + requirement set stable --> fixed point
        v
topological activation order
```

The topological pass orders an already selected set; it does not choose providers. Cycles and missing required capabilities are composition/activation errors.

## 6. Gateway-only consumption

Consumers address stable `engine.*` gateways. A concrete provider service ID is implementation metadata and MUST NOT become a compatibility escape hatch.

For example:

```text
engine.assets route absent
        =
assets unavailable
```

The host must not reinterpret this as “try `asset_manager.api` directly”. Compatibility aliases that must remain temporarily MUST NOT be inferred from `has_service(concrete_provider_id)`. A compatibility alias owned by one native provider may be installed only as part of that provider's own registration. If another provider already owns the canonical gateway, it must explicitly advertise any compatibility gateway it supports; RuntimeHost must not fabricate one on its behalf.

## 7. Contract catalog parity

`RuntimeContractCatalog` is the instance-scoped authority for contract existence and compatibility. A contract reference may be a stable registry key or an advertised boundary token; both are canonicalized before solver matching.

```text
provider reference ----\
                       > RuntimeContractCatalog -> canonical key -> solver
requirement reference -/
```

The Editor receives the same catalog (Engine default for standalone use, instance catalog for embedded/PIE use). Unknown contracts and impossible version ranges are configuration errors, not “no provider found”.

## 8. Freeze and load semantics

Before freeze, a composition may be live and evolve as inventory/policy is assembled. Freeze establishes the authoritative plan for the Engine instance.

After freeze:

- loader/discovery code MUST NOT select a shadowed provider because the selected artifact failed;
- a selected artifact whose live descriptor disagrees with frozen metadata is rejected;
- `ActiveGatewayRegistry` exposes the selected routes described by the plan;
- diagnostics may expose shadowed/rejected alternatives but cannot promote them without a new explicit composition epoch permitted by lifecycle policy.

## 9. Explainability

`CompositionExplanationGraph` is produced by the shared composition stack. It distinguishes:

- `Selected` — compatible winner;
- `Shadowed` — compatible but outranked;
- `Rejected` — incompatible, with typed reason codes.

The graph owns requirement evaluations, score decomposition, rank/`outranked_by`, rejection reasons, and contract provenance. Runtime and Editor presentation code may render these records but MUST NOT reconstruct selection logic.

## 10. Observability protocol

Internal Rust structures are not wire contracts. The stable read-models are:

- `composition.snapshot_v1` — one coherent Engine composition epoch;
- `composition.diff_v1` — semantic difference between two snapshots.

A runtime snapshot carries opaque Engine `instance_id`, stable even `topology_generation`, `composition_epoch`, live/frozen provenance, gateways, candidates, requirements, explanations, contract provenance and unsatisfied requirements.

V1 defines:

```text
composition_epoch = topology_generation / 2
```

Snapshots are captured only across a stable even topology generation. An odd generation denotes a transaction in progress and is not publishable as a coherent snapshot.

## 11. Environment boundary

```text
Process environment
        |
        v
Launcher / bootstrap
        |
        v
snapshot once
        |
        v
HostContext / RuntimeLaunchContext
        |
        v
runtime policy
```

Environment reads required for executable discovery, launcher behavior, hardware/pre-init probing or early bootstrap logging belong before the runtime boundary. Instance runtime policy reads the Engine-owned snapshot.

## 12. Governance gates

The architecture is protected by executable gates rather than comments alone. The current set includes:

- `kernel_dependency_gate.py` — kernel floor and PreInit ordering;
- `dependency_direction_gate.py` — dependency direction and typed provider boundaries;
- `environment_isolation_gate.py` — process-environment boundary;
- `p0_invariant_scan.py` — broad provider/service and boundary invariants;
- `no_legacy_scan.py` — retired compatibility APIs and repository backup/cache hygiene;
- `p0_composition_selection_authority_gate.py` — runtime-unit fixed-point solver authority and gateway-only compatibility paths;
- `p1_editor_composition_unification_gate.py`;
- `p1_editor_metadata_parity_gate.py`;
- `p1_editor_requirement_metadata_gate.py`;
- `p1_editor_contract_registry_parity_gate.py`;
- `p1_composition_explainability_parity_gate.py`;
- `p1_composition_observability_surface_gate.py`.

A gate must follow semantic ownership rather than historical spellings. Typed constants, official V2 APIs, provider-source roots and generated descriptor metadata are not legacy merely because an older scanner did not know them.

## 13. Explicit non-goals

Composition V2 does not require physical DLL reclamation after escaped raw callbacks; generation-aware callback leases remain a lifecycle maturity concern. It also does not require every legacy plugin root ABI to disappear before the composition authority is correct. Those are separate lifecycle/ABI hygiene milestones and must not reintroduce selection authority while being migrated.
