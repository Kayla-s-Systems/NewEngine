# P4 Pose Graph Optimization Design

Date: 2026-08-28
Status: Approved design, pending implementation plan
Scope: `newengine-animation-runtime` generic Animation Graph runtime

## 1. Purpose

P4 introduces a compiled pose dependency graph and frame-local memoization so animation graph complexity can grow without evaluating shared pose subgraphs repeatedly. The design must preserve all semantics already established by P1 Transition Semantics V2, P2 Sync System V2, and P3 Blend2D.

P4 comprises five ordered slices:

1. P4.1 Compiled dependency DAG
2. P4.2 Domain-qualified frame cache
3. P4.3 PoseRef / CachedPose
4. P4.4 PoseSelector
5. P4.5 Cache, event, root-motion, and regression acceptance

The subsystem is generic. No GameReady-specific pose policy belongs in the graph runtime.

## 2. Non-goals

P4 does not implement inertialization, aim offsets, motion matching, per-layer state machines, arbitrary user scripting in pose nodes, or new transition semantics. It also does not migrate GameReady content merely to exercise the feature. Product adoption is a separate content task.

## 3. Current boundary

Today, states and layers directly own a `CompiledAnimationMotion` plus domain-local playback state. This works for Clip, Blend1D, and Blend2D, but there is no stable identity for a shared pose subgraph. Consequently, repeated consumers cannot safely share an evaluated pose, and named references/selectors cannot form a validated dependency graph.

P4 replaces direct tree ownership with a compiled graph of stable node IDs while preserving state and layer clocks as the owners of temporal evaluation domains.

## 4. Core architecture

### 4.1 Flat compiled pose DAG

The compiled graph owns a dense node table:

```text
CompiledAnimationGraph
├── pose_nodes: Vec<CompiledPoseNode>
├── pose_node_index: name -> PoseNodeId
├── pose_topology: Vec<PoseNodeId>
├── states: roots into pose_nodes
├── layers: roots into pose_nodes
└── existing transitions / clips / sync metadata
```

`PoseNodeId` is a dense stable index within one `CompiledAnimationGraph`. Runtime code never uses authored string lookup on the frame hot path.

Each `CompiledPoseNode` contains exactly one operation plus resolved dependency IDs. Dependencies are immutable after compilation.

### 4.2 Node vocabulary

Existing motion definitions remain valid and are compiled into pose nodes. P4 adds authored indirection/selection without removing Clip, Blend1D, or Blend2D:

```text
AnimationMotionDefinition
├── Clip
├── Blend1D
├── Blend2D
├── PoseRef
├── CachedPose
└── PoseSelector
```

Top-level named pose definitions provide stable reference targets. State and layer roots may continue to embed motion definitions directly; embedded definitions are lowered into anonymous compiled pose nodes.

### 4.3 Authored named pose registry

`AnimationGraphDefinition` gains an additive, serde-defaulted named pose collection. Names are canonicalized with the same case-insensitive policy used by graph state/parameter lookup.

Required invariants:

- empty pose names are rejected;
- duplicate canonical names are rejected;
- references resolve case-insensitively;
- unknown references are compile errors;
- named-node identity is derived from canonical name, not authored list order.

The schema remains backward compatible for graph assets that do not declare named poses.

## 5. Dependency compilation and cycle validation

### 5.1 Dependency resolution

Compilation lowers every authored pose operation into a `CompiledPoseNode` and resolves all child references to `PoseNodeId` values. The compiler records direct dependencies for each node.

### 5.2 Cycle detection

The dependency graph must be acyclic. Compilation performs deterministic tri-color DFS:

```text
WHITE = unseen
GRAY  = active DFS stack
BLACK = validated
```

An edge to a GRAY node is a compile error. The error must include the resolved cycle path, for example:

```text
animation pose graph contains dependency cycle: locomotion -> weapon_ready -> locomotion
```

No runtime recursion guard is accepted as a substitute for compile-time cycle rejection.

### 5.3 Deterministic topology

The compiler emits a deterministic topological order independent of authored named-pose list order. Canonical node identity and dependency traversal order define the result. This topology is used for diagnostics, validation, and future optimization passes; frame evaluation remains demand-driven.

Consumer counts may be computed at compile time for diagnostics and later allocation/planning work.

## 6. Evaluation domains

### 6.1 Why node ID alone is insufficient

A pose node can be referenced from multiple temporal owners. State A at 0.35 seconds and State B at 1.20 seconds must not share a cached sample merely because they reference the same `PoseNodeId`.

The evaluation identity is therefore:

```text
PoseCacheKey = (PoseEvaluationDomain, PoseNodeId)
```

Initial domain variants:

```text
PoseEvaluationDomain
├── State(state_index)
└── Layer(layer_index)
```

Transitions continue to evaluate source and destination through their respective state domains. An interrupted frozen transition source remains a frozen base-pose snapshot and does not acquire a synthetic pose domain.

### 6.2 Domain ownership

A domain owns temporal/runtime state required by its node graph, including event cursors and any node-local playback state introduced by future nodes. Clocks remain at state/layer ownership boundaries unless a later feature explicitly introduces independent node clocks.

This preserves P2 Sync System V2 and existing transition/layer clock semantics.

## 7. Frame-local pose cache

### 7.1 Logical invalidation

`AnimationGraphInstance` owns reusable cache slots. A monotonically increasing frame generation marks validity:

```text
slot.generation == current_generation -> HIT
slot.generation != current_generation -> MISS
```

Advancing a frame invalidates logically by incrementing the generation; it must not clear/reallocate every cached pose vector.

Generation rollover must be handled deterministically by resetting slot generations before reuse rather than allowing stale equality.

### 7.2 Cache key layout

The runtime should use dense storage derived from compiled node count and domain count rather than a per-frame `HashMap`. A suitable representation is logically equivalent to:

```text
cache[domain_index][pose_node_id]
```

with each slot owning reusable pose/result buffers.

### 7.3 Demand-driven evaluation

Evaluation begins at a state or layer root and recursively/demand-evaluates only required dependencies:

```text
evaluate(domain, node)
├── cache HIT -> return cached result
└── cache MISS
    ├── evaluate required dependencies
    ├── evaluate current operation
    ├── materialize result in cache slot
    └── return slot
```

The compiler proves acyclicity, so runtime recursion cannot encounter a dependency cycle in a valid graph.

Selectors evaluate only the selected dependency branch. Unselected branches are not pose-sampled.

## 8. Cached result contract

Caching only `Vec<JointLocalPose>` is insufficient because cache hits must not change observable graph behavior.

A cached frame result must preserve at least:

```text
PoseNodeFrameResult
├── local_pose
├── motion/root metadata required for root extraction
└── event occurrence contribution metadata
```

The exact internal layout may be split across tightly coupled cache structures if that reduces copying, but the semantics are atomic: a cache hit must be behaviorally equivalent to one evaluation of that node in the same domain and frame.

### 8.1 Pose ownership

The cache owns its result storage for the frame generation. Consumers borrow/read cached pose data and blend/copy only when their operation requires a new output. A cache hit must not allocate.

### 8.2 Root-motion metadata

P3 established weighted root-motion sources for blend nodes. Cache hits must preserve the same weighted source metadata and playback times. Root-motion history remains owned at the root-owning state level; reading the same cached pose twice must not advance root history twice.

### 8.3 Event semantics

Node evaluation may advance domain-local cursors exactly once per frame. Reusing a cached result cannot advance them again.

If a cached node fans out to multiple consumers in one domain, a semantic event occurrence from that node is published at most once for the domain/frame. Fan-out is not an event multiplier.

Different evaluation domains remain independent and may legitimately produce equivalent occurrences according to their own clocks/cursors.

## 9. PoseRef and CachedPose

### 9.1 PoseRef

`PoseRef` references a named pose node and contributes no evaluation semantics of its own after compilation. The compiler should alias/canonicalize references so runtime does not pay for a chain of trivial reference nodes unless retaining one is useful for diagnostics.

### 9.2 CachedPose

`CachedPose` is an authored materialization/share boundary, not a second evaluator implementation. It references a source node and guarantees frame memoization at that node boundary in the current evaluation domain.

Because P4's evaluator already memoizes node results, `CachedPose` primarily provides explicit authored intent, stable tooling identity, diagnostics, and a future optimization boundary. Its semantics must not diverge from the generic frame cache.

Nested or repeated CachedPose references are legal if acyclic and resolve to the same deterministic evaluation result.

## 10. PoseSelector

### 10.1 Contract

A selector contains ordered conditional choices and a required deterministic default pose:

```text
PoseSelector
├── choice[0]: conditions[] -> pose
├── choice[1]: conditions[] -> pose
├── ...
└── default -> pose
```

### 10.2 Conditions and arbitration

Selectors reuse the existing compiled transition condition language for bool/float parameter predicates. No second predicate system is introduced in P4.

Rules:

- choices are tested in authored order;
- the first fully matching choice wins;
- only the winning branch is pose-evaluated;
- if no choice matches, default is evaluated;
- unknown parameters and type mismatches are compile errors;
- selector itself performs no crossfade.

Selector smoothing belongs to future inertialization or an explicit blend node, not to selection semantics.

### 10.3 Inactive branch runtime

Switching into a previously inactive branch must not replay historical events. The runtime must keep branch event state consistent with the existing inactive-sample policy: when a branch becomes selected, its event cursors are seeded/seeked to the current domain time before event ownership is enabled, unless that branch has already been evaluated in the same frame/domain.

The implementation may achieve this through domain node-runtime synchronization rather than evaluating the branch pose while inactive. The invariant is semantic, not an instruction to sample every branch.

## 11. State, transition, sync, and layer integration

### 11.1 States

Each compiled state owns a root `PoseNodeId` plus its existing speed/root-motion policy and state playback clock/runtime domain.

### 11.2 Transitions

P1 semantics remain unchanged. Source and destination state roots are evaluated through separate state domains. Interrupted transitions still freeze the evaluated base state-machine pose before layers and blend that frozen pose against the new destination root.

### 11.3 Sync System V2

P2 marker-aware synchronization remains the single timing substrate. Pose graph indirection must expose enough compiled timing metadata for state-to-state and base-to-layer sync matching to identify the effective sync leader of a root node.

`PoseRef` and `CachedPose` are transparent for sync identity. `PoseSelector` exposes the selected branch's sync identity for the current evaluation; compile-time validation should reject ambiguous cases only where an existing API requires one static sync identity and cannot safely resolve it dynamically.

The implementation plan must preserve normalized-phase fallback for graphs without authored marker contracts.

### 11.4 Blend1D and Blend2D

Existing blend math, dominant-event ownership, weighted root-motion metadata, inactive cursor behavior, and Sync V2 sample remapping remain unchanged when lowered into DAG nodes.

### 11.5 Layers

Layers retain independent `Layer` evaluation domains and clocks. A layer root can reference shared named poses but never shares cached results with a state domain or another layer domain.

Layer masks and additive/override application remain post-root evaluation operations unless later optimization demonstrates a need to represent layer composition itself as DAG nodes.

## 12. Runtime diagnostics

P4 adds per-frame pose graph statistics suitable for tests and optional diagnostics:

```text
PoseGraphFrameStats
├── node_evaluations
├── cache_hits
├── cache_misses
└── selector_branches_evaluated
```

Recommended additional debug-only counters include unique nodes touched and maximum dependency depth, but they are not closure requirements.

The counters must not require expensive logging or allocations when diagnostics are disabled.

## 13. Error handling

Compilation rejects malformed authoring before an instance is created. Required compile-time errors include:

- empty/duplicate named pose;
- unknown PoseRef/CachedPose/selector target;
- dependency cycle with path;
- invalid selector condition parameter/type;
- selector without a deterministic default;
- node shape or existing Blend1D/Blend2D/Sync validation errors.

Runtime errors are reserved for graph-instance/compiled-graph mismatches, corrupt internal shapes, invalid skeleton contracts, or other invariant violations that cannot originate from valid compiled authoring.

## 14. Acceptance invariants

### 14.1 DAG

- named refs resolve case-insensitively;
- unknown ref is rejected;
- dependency cycle is rejected;
- deterministic topological order is produced;
- authored named-node ordering does not alter canonical node identity semantics.

### 14.2 Cache

- a node evaluates at most once per `(domain, frame)`;
- next-frame invalidation is generation-based and reuses allocations;
- different state clocks never share a cached result;
- cache hit preserves pose exactly within normal floating-point determinism;
- cache hit preserves root-motion metadata;
- cached event occurrence does not duplicate due to fan-out.

### 14.3 Selector

- first matching authored branch wins;
- only selected branch is pose-evaluated;
- default behavior is deterministic;
- invalid predicates are compile errors;
- switching branches does not replay inactive event history.

### 14.4 Regression

The final P4 gate covers:

- Clip;
- Blend1D;
- Blend2D;
- Sync V2 marker remapping and normalized fallback;
- authored transitions and interruption/frozen-pose continuity;
- layers;
- timeline events;
- root motion, including weighted blend sources.

## 15. Test strategy

P4 implementation follows test-driven development. Tests should be added before or with each slice and must include behavior plus evaluation-count assertions.

Key dedicated tests:

1. named refs are case-insensitive and order-independent;
2. unknown refs fail compilation;
3. cycle reports the dependency path;
4. deterministic topology remains stable across authored permutations;
5. one shared subgraph requested three times yields one evaluation and two cache hits in one domain/frame;
6. same node in two state domains evaluates independently;
7. frame generation invalidates logically while retaining slot capacities;
8. cached pose preserves weighted root metadata;
9. cached event occurrence emits once under fan-out;
10. selector evaluates only the first matching branch;
11. selector default is used when no choice matches;
12. selector branch switch does not replay inactive events;
13. Clip/Blend1D/Blend2D output remains equivalent before and after DAG lowering;
14. Sync V2 marker timing remains equivalent through PoseRef/CachedPose;
15. transition interruption still freezes the base pose before layers;
16. GameReady regression remains green without requiring product content migration.

## 16. Quality gates

P4 is not closed until the following package gates are green:

```text
cargo check -p newengine-animation-runtime
cargo clippy -p newengine-animation-runtime -- -D warnings
cargo test -p newengine-animation-runtime
cargo fmt -p newengine-animation-runtime -p newengine-game-ready-world -- --check
cargo check -p newengine-game-ready-world
cargo test -p newengine-game-ready-world
```

Workspace-wide `-D warnings` is not a P4 closure criterion when failures are demonstrably unrelated to the animation subsystem.

## 17. Implementation sequence

### P4.1 Compiled dependency DAG

- introduce authored named pose registry and references;
- lower existing motions to stable pose nodes;
- resolve canonical references;
- detect cycles with path;
- emit deterministic topology and consumer counts;
- preserve old graph assets.

### P4.2 Domain-qualified frame cache

- introduce state/layer evaluation domains;
- add dense reusable cache slots and frame generation;
- route root evaluation through demand-driven node evaluation;
- add stats and single-evaluation tests.

### P4.3 PoseRef / CachedPose

- add authored variants;
- make PoseRef runtime-transparent after resolution;
- expose CachedPose as an explicit materialization/diagnostic boundary;
- prove fan-out reuse and domain separation.

### P4.4 PoseSelector

- compile ordered branches using existing condition predicates;
- demand-evaluate only the selected child;
- implement deterministic default;
- preserve inactive event history semantics.

### P4.5 Acceptance

- close event deduplication and root metadata cache semantics;
- run full animation regression;
- run GameReady regression;
- report P4 CLOSED only if every invariant in Section 14 is demonstrated.

## 18. Closure definition

P4 may be classified `CLOSED` only when the compiled animation graph has a validated acyclic pose dependency model, repeated pose subgraphs are evaluated at most once per domain/frame, authored cached poses and selectors are functional, and all existing transition/sync/blend/layer/event/root-motion semantics remain green under regression.
