# Common Pistol Native Master Rig Design

Date: 2026-08-28
Status: Approved design; implementation not started by this spec
Domain: NorthStar / NewEngine native Naughty Dog weapon import and animation pipeline

## 1. Decision

NorthStar will represent Naughty Dog pistol-family animation domain `E7898652` as one reusable 62-slot reconstructed native master rig:

`models/weapon/pistol/common-pistol.ymt@common_pistol`

This master rig is shared by the Glock/Military Pistol, Beretta/semi-auto, XCaliber/Velazquez, and revolver families that have been observed using the same native 62-slot geometry/animation domain.

`parts-pistl-base[35]` is not the animation master. It remains a separately preserved native 35-joint local hierarchy and is related to the 62-slot master only through an explicit, evidence-backed subset mapping.

No production asset may invent names, parents, bind transforms, or remaps merely to satisfy runtime admission.

## 2. Motivation and evidence

The original Military Pistol import used `parts-pistl-base.pak` as a 35-joint skeleton. That is sufficient for the existing parts geometry, but native pistol gameplay animation clips address a 62-channel domain.

Observed native animation records include:

- `ellie-pistol-fire-gun`
- `ellie-pistol-dry-fire-gun`
- `abby-pistol-reload-stand-tac-gun`
- Glock workbench object clips such as `abby-workbench-pistol-idle-on-table-glock--glock`

These clips address slots beyond joint 34. Directly binding them to the 35-joint parts skeleton is therefore invalid.

The 62-channel domain is not merely an animation scratch buffer. `pistol-military.pak` and other pistol variants contain geometry with `source_skin_joint_domain_size = 62` and native skin weights addressing the same slot range. The opaque native token `0xE7898652` is observed across pistol geometry/collision/animation resources and identifies the common pistol-family domain for the purposes of this design.

The 35-joint `parts-pistl-base` hierarchy is a subset/parts hierarchy, not a lossless representation of all animated common-pistol nodes. Common-only nodes include slide/action semantics that are required by native fire animation but do not exist as equivalent joints in the 35-joint hierarchy.

Therefore the correct architecture is 62-slot master ownership, not 62-to-35 animation compression.

## 3. Production asset graph

Military Pistol is composed around the common 62-slot master rig:

```text
common-pistol.ymt@common_pistol        # 62-slot master
        |
        +-- pistol-military.pak
        |     `-- native 62-domain Glock body
        |
        +-- parts-pistl-military-main.pak
        |     `-- 35-domain part -> common subset mapping
        |
        +-- parts-pistl-military-mag-base.pak
        |     `-- 35-domain part -> common subset mapping
        |
        `-- military-pistol.ycd
              +-- fire
              +-- dry-fire
              +-- reload
              `-- optional authored idle/spawn
```

The same `common-pistol.ymt@common_pistol` may later be reused by Beretta, XCaliber, and revolver variants without redefining the animation-domain identity.

## 4. Reconstruction artifacts

The reconstruction must remain auditable outside the final YMT. The source tree will contain:

```text
Source/models/weapon/pistol/
+-- common-pistol.reconstruction.json
+-- parts-pistl-base.subset.json
+-- common-pistol.sources.json
`-- military/
    `-- military-pistol.animation-catalog.json
```

### 4.1 `common-pistol.reconstruction.json`

Contains exactly 62 slot records, indexed `0..61`.

Each slot records:

```text
index
slot_id                  required structural identity, e.g. e7898652:00
semantic_name?           optional until confirmed
parent_slot?             optional until confirmed
bind_srt?                optional until confirmed

evidence[]
  source
  resource
  variant
  evidence_kind
  confidence
  observations

name_confirmed
parent_confirmed
bind_confirmed
subset_confirmed
```

Allowed evidence kinds include at least:

- `native-skin`
- `native-animation`
- `parts-subset`
- `cross-variant`
- `collision-domain`
- `serialized-descriptor`

`confidence` is diagnostic metadata only. A high score never authorizes a production semantic name, parent, bind transform, or subset correspondence by itself. Authorization is explicit through the corresponding `*_confirmed` fields and validation policy.

`slot_id` is not a claimed native bone name. It is NorthStar's deterministic address for an otherwise unnamed native slot and may be used as the YMT joint label until a semantic native name is confirmed. This preserves a stable 62-slot runtime identity without fabricating semantics.

### 4.2 `common-pistol.sources.json`

Records the source corpus and native identities used to reconstruct the rig, including source package names, hashes where available, domain token observations, resource offsets, and variant coverage.

### 4.3 `military-pistol.animation-catalog.json`

Records the exact native ANIM sources selected for the Military Pistol YCD, including authored clip name, PAK, resource offset, animation group/domain identity, sample count/rate, active destination slots, and decoder profile.

No renamed synthetic `idle`, `fire`, or `reload` clip may hide its original authored identity. Runtime-friendly aliases, if ever added, must be explicit metadata referencing the preserved authored clip.

## 5. `parts-pistl-base[35]` subset contract

`parts-pistl-base.pak` remains preserved as the native 35-joint local hierarchy.

The subset mapping is directed:

```text
parts-pistl-base local joint -> common-pistol slot
```

not the inverse.

Core invariant:

```text
one local joint -> exactly one common slot
```

The inverse map is allowed to be incomplete because the 62-slot master contains common-only nodes that do not exist in the 35-joint parts hierarchy.

Previously observed correspondences may be recorded as evidence but are production-authorized only after validation policy accepts them. Examples of established evidence include:

```text
hammer               -> common slot 29
trigger              -> common slot 32
grip                 -> common slot 53
scope                -> common slot 40
xcaliber_barrel_base -> common slot 43
```

The subset file must preserve the local joint index, local joint name, common slot, evidence records, and confirmation state.

No nearest-joint, geometric-proximity, root fallback, or dense-index assumption is permitted in this subset contract.

## 6. Reconstruction acceptance policy

`common-pistol.ymt` must not be emitted merely because 62 slot indices are known.

For every slot required by the Military Pistol production path, the runtime-critical hierarchy information must be established sufficiently to evaluate skinning correctly.

A slot is required when at least one of the following is true:

1. native Military Pistol geometry has non-zero skin weight for that slot;
2. a selected production fire/dry-fire/reload/idle/spawn clip addresses that slot;
3. a mapped Military Pistol 35-domain part depends on that slot.

For every required slot, the production reconstruction must have confirmed bind transform and confirmed parent relationship. A semantic native name is required only where a subset/tooling/runtime contract refers to that semantic; otherwise the deterministic `slot_id` is the emitted structural label and must not be presented as a recovered native name.

The current `AnimationSkeletonRuntime` compiles the complete skeleton hierarchy and bind pose, so **all 62 slots must have confirmed parent relationships and confirmed bind transforms before `common-pistol.ymt` can be released**. Unresolved semantic names are permitted only as unresolved semantics; unresolved parent/bind data is not. No dummy root parenting, identity bind substitution, or fabricated hierarchy is allowed.

The emitted YMT must produce an identity bind-pose skin palette under `AnimationSkeletonRuntime::compile`.

## 7. YDD importer architecture

A dedicated master-rig compile path is required. It must not reuse the existing character `package_skin_fallback_joints` approximation behavior.

Recommended compiler mode:

`northstar_native_master_rig`

This mode is generic rather than pistol-specific so that future native assets with the same master/subset pattern can reuse the mechanism.

### 7.1 Inputs

The compile request includes:

- master rig definition / reconstructed YMT source
- master domain size (`62` for `common-pistol`)
- master domain identity (`E7898652` as recorded provenance token)
- one or more native geometry PAKs
- zero or more explicit subset contracts for non-master skin domains
- material bindings and ordinary model compile options

### 7.2 Skin-domain behavior

```text
native package domain == master domain (62)
    -> preserve native skin joint indices and weights verbatim

native package domain == known subset domain (35)
    -> require explicit subset contract
    -> rewrite only joint indices 35 -> common 62
    -> preserve weights verbatim

unknown source skin domain
    -> hard reject
```

Forbidden in this pipeline:

- proximity-weighted fallback
- nearest-joint assignment
- root fallback
- silent skin stripping
- dense-index equivalence assumptions between unrelated domains
- procedural reconstruction of missing joints

### 7.3 Military Pistol model composition

The production Military Pistol YDD must include the actual `pistol-military.pak` Glock body because that package carries the native 62-domain skin required for slide/action animation.

The existing 35-only composition using only:

- `parts-pistl-base.pak`
- `parts-pistl-military-main.pak`
- `parts-pistl-military-mag-base.pak`

is not sufficient as the final animated model composition.

The 35-domain military part packages are retained and remapped into the 62-slot master only through the explicit subset contract.

## 8. YCD compiler architecture

Native animation remains in the `E7898652` common-pistol destination-slot domain.

No retarget is performed.

```text
ANIM_GROUP E7898652 slot N
        -> YCD joint/tag N
        -> common-pistol master slot N
```

The compiler must support exact native clip selection and must additionally validate:

- selected record belongs to the expected common-pistol animation group/domain;
- maximum destination slot is `< 62`;
- every active destination slot exists in the master rig;
- clip skeleton reference is `shared/models/weapon/pistol/common-pistol.ymt@common_pistol` (mount-prefix normalization allowed by runtime policy);
- no procedural pose or RAGE/ONIM retarget fallback is substituted for unsupported native data.

For Military Pistol, the initial required native clip set is:

- fire
- dry-fire
- reload

An authored idle/spawn pose may be included only when a native compatible clip is identified and validated. Absence of idle/spawn does not authorize generation of a fake one.

## 9. Runtime contract

The existing runtime model remains conceptually unchanged. The target invariant is:

```text
YDD skin domain == YMT skeleton domain == YCD channel domain
62              == 62                  == 62
```

`WeaponAnimationDefinition.skeleton` must reference:

`shared/models/weapon/pistol/common-pistol.ymt@common_pistol`

Fire/reload references must point at the Military Pistol native YCD dictionary entries.

The existing weapon-animation runtime may continue to compile the skeleton, bind clips, sample local poses, build the skin palette, and publish the palette to the weapon root. The design specifically avoids adding a multi-rig runtime or runtime retarget layer.

## 10. Validation and failure policy

The pipeline fails closed.

### Master rig gates

- exactly 62 slots;
- `E7898652` domain token recorded in provenance;
- no invented native semantic names; unconfirmed semantics use deterministic structural `slot_id` labels only;
- no invented production parents;
- bind transforms confirmed for all 62 slots;
- parent relationships confirmed for all 62 slots;
- bind-pose skin palette identity passes runtime-equivalent validation;
- reconstruction provenance complete for every production-required field.

### Subset gates

- local `parts-pistl-base` count is exactly 35;
- every local joint appears at most once as a mapping source;
- every mapped common target is `< 62`;
- all Military Pistol part joints used by emitted geometry are mapped;
- each production mapping is evidence-backed and explicitly confirmed;
- no fallback assignment is permitted.

### Geometry gates

- `pistol-military.pak` native 62-domain indices and weights are preserved;
- 35-domain military parts are converted only through the subset contract;
- no proximity/root/dense-domain fallback occurs;
- all emitted skin indices are `< 62`;
- material and texture contracts remain valid.

### Animation gates

- native fire decodes successfully;
- native dry-fire decodes successfully;
- native reload decodes successfully;
- no active channel is outside `0..61`;
- clips bind directly to `common-pistol`;
- no procedural or retarget fallback is used.

### Runtime gates

- YDD/YMT admission passes;
- YCD skeleton binding passes;
- slide/action motion is visibly driven by native animation;
- hammer and trigger motion is visibly driven by native animation;
- reload magazine subtree motion/visibility behaves correctly;
- switching to the Military Pistol through the authored sidearm input (`Digit2`) passes live smoke;
- rifle remains the authored primary weapon unless separately changed by authored gameplay content.

## 11. Testing strategy

Testing is layered so reconstruction errors are caught before live runtime smoke.

1. Reconstruction validator tests: schema, 62 slots, confirmation policy, evidence integrity, duplicate mappings, required-field closure.
2. Importer unit tests: 62-domain pass-through, 35-to-62 index rewrite, unknown-domain rejection, weight preservation, fallback prohibition.
3. YCD compiler tests: exact clip selection, group/domain validation, destination range validation, skeleton-ref metadata, no fallback.
4. Asset-build validation: YMT/YDD/YCD NEF8 validation, model skin-domain validation, material/YTD dependencies.
5. Runtime unit/integration tests: skeleton compilation, identity bind palette, clip binding, fire/reload sampling.
6. ForestRoad live acceptance: equip with `Digit2`, fire, dry-fire where practical, reload, inspect visual movement and logs.

Tests must compare skin indices/weights before and after 62-domain pass-through to ensure the master-domain body is not silently altered.

## 12. Migration strategy

Migration occurs in explicit stages:

1. preserve current working static/35-domain Military Pistol assets until the 62-master path passes offline validation;
2. add reconstruction/subset source artifacts and validators;
3. add generic `northstar_native_master_rig` importer mode;
4. reconstruct and validate `common-pistol` master data;
5. compile Military Pistol model using native 62-domain body plus mapped 35-domain parts;
6. compile native Military Pistol YCD against `common-pistol`;
7. update the authored Military Pistol definition to use the common-pistol skeleton/YCD;
8. run runtime/live acceptance;
9. only after acceptance, retire the 35-only animated-path assumption.

No migration stage may replace the current production asset with an unvalidated 62-slot reconstruction merely to make later stages testable.

## 13. Non-goals

This slice does not:

- invent a complete pistol hierarchy from visual intuition;
- create procedural slide, hammer, trigger, or magazine animation;
- retarget common-pistol animation into the 35-joint parts skeleton;
- introduce a general runtime multi-skeleton weapon system;
- redesign player hand/IK animation;
- add handgun Wwise audio extraction;
- change ForestRoad primary-weapon policy;
- integrate Beretta/XCaliber/Revolver gameplay definitions beyond ensuring the master rig can be reused by them later.

## 14. Completion definition

The implementation slice is complete only when the Military Pistol is admitted and rendered through a validated 62-slot `common-pistol` master rig, uses native 62-domain Glock body skinning, maps required 35-domain military parts through an explicit evidence-backed subset, binds native fire/dry-fire/reload YCD clips without retargeting, and passes the full release gates above including the live `Digit2` sidearm smoke.
