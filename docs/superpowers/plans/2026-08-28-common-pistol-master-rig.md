# Common Pistol Native Master Rig Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconstruct the reusable `E7898652` 62-slot common-pistol master rig, compile the Military Pistol as native 62-domain geometry plus explicitly mapped `parts-pistl-base[35]` parts, compile native fire/dry-fire/reload YCD without retargeting, and pass ForestRoad live sidearm acceptance.

**Architecture:** `common-pistol.ymt@common_pistol` owns the 62-slot skin/animation domain. Native 62-domain pistol geometry preserves its indices and weights verbatim; 35-domain `parts-pistl-base` geometry is admitted only through an explicit local-joint-to-common-slot subset contract. Native ANIM destination slots remain direct common-pistol slots, keeping `YDD domain == YMT domain == YCD domain == 62`.

**Tech Stack:** Rust 2024 workspace, Python 3 maintenance tooling, NorthStar Naughty Dog PAK/OrbisAnim decoders, NEF8 YMT/YDD/YCD assets, JSON evidence contracts, ForestRoad TypeScript gameplay authoring.

**Spec:** `docs/superpowers/specs/2026-08-28-common-pistol-master-rig-design.md`

## Global Constraints

- `E7898652` is one reusable 62-slot common-pistol domain for Glock/Military, Beretta, XCaliber, and revolver families.
- `parts-pistl-base[35]` remains a separate native local hierarchy and is never the animation master.
- No invented native semantic names, parents, bind transforms, procedural animations, proximity skin fallback, root fallback, or implicit dense-index remap.
- Every released common-pistol slot must have confirmed parent and bind S/R/T; unknown semantics use deterministic structural `slot_id`, not a fabricated bone name.
- Native 62-domain skin indices/weights must be preserved; 35-domain indices may change only through the explicit subset contract and weights must remain unchanged.
- Native fire, dry-fire, and reload remain direct `E7898652` channel data; no retargeting.
- Current ForestRoad rifle-primary authored policy remains unchanged; Military Pistol is admitted through the existing sidearm slot / `Digit2` path.
- Existing unrelated dirty worktree changes must not be reverted or folded into task commits.

---

### Task 1: Reconstruction Contract and Fail-Closed Validator

**Files:**
- Create: `tools/maintenance/common_pistol_master_rig.py`
- Create: `tools/maintenance/test_common_pistol_master_rig.py`
- Create: `Shared/Source/models/weapon/pistol/common-pistol.sources.json`
- Create: `Shared/Source/models/weapon/pistol/common-pistol.reconstruction.json`
- Create: `Shared/Source/models/weapon/pistol/parts-pistl-base.subset.json`

**Interfaces:**
- Produces: `load_reconstruction(path) -> CommonPistolReconstruction`
- Produces: `validate_reconstruction(reconstruction, subset) -> ValidationReport`
- Produces: `emit_common_pistol_ymt_xml(reconstruction) -> bytes`, callable only after all 62 parent/bind gates pass.
- Produces JSON schemas `northstar.common-pistol.reconstruction.v1`, `northstar.common-pistol.subset.v1`, and `northstar.common-pistol.sources.v1`.

- [ ] **Step 1: Write failing Python tests for structural validation**

```python
class CommonPistolContractTests(unittest.TestCase):
    def test_requires_exactly_62_dense_slots(self):
        reconstruction = fixture_reconstruction(slot_count=61)
        with self.assertRaisesRegex(ValueError, "exactly 62"):
            validate_reconstruction(reconstruction, fixture_subset())

    def test_unconfirmed_parent_or_bind_blocks_ymt(self):
        reconstruction = fixture_reconstruction()
        reconstruction["slots"][29]["parent_confirmed"] = False
        with self.assertRaisesRegex(ValueError, "parent.*29"):
            emit_common_pistol_ymt_xml(reconstruction)

    def test_subset_source_joint_is_unique_and_target_below_62(self):
        subset = fixture_subset()
        subset["mappings"].append(dict(subset["mappings"][0]))
        with self.assertRaisesRegex(ValueError, "duplicate local joint"):
            validate_reconstruction(fixture_reconstruction(), subset)
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```powershell
python tools/maintenance/test_common_pistol_master_rig.py
```

Expected: import/function failures because the validator does not exist.

- [ ] **Step 3: Implement typed validation and deterministic structural labels**

Core behavior:

```python
def structural_slot_id(index: int) -> str:
    return f"e7898652:{index:02d}"

# YMT emission gate:
if len(slots) != 62 or any(slot["index"] != i for i, slot in enumerate(slots)):
    raise ValueError("common-pistol reconstruction requires exactly 62 dense slots")
for slot in slots:
    if not slot["parent_confirmed"]:
        raise ValueError(f"parent is not confirmed slot={slot['index']}")
    if not slot["bind_confirmed"]:
        raise ValueError(f"bind S/R/T is not confirmed slot={slot['index']}")
```

Emit semantic names only when `name_confirmed`; otherwise emit `slot_id` as structural YMT label and retain `semantic_name = null` in provenance.

- [ ] **Step 4: Seed evidence artifacts from already proven data**

`common-pistol.sources.json` records `E7898652`, the four common pistol model variants, `pistol-military.pak`, `parts-pistl-base.pak`, and the three animation banks with source hashes/resource offsets when available.

`parts-pistl-base.subset.json` initially records only confirmed/evidence-backed correspondences, including:

```json
{"local_joint":5,"local_name":"hammer","common_slot":29,"confirmed":true},
{"local_joint":6,"local_name":"trigger","common_slot":32,"confirmed":true},
{"local_joint":22,"local_name":"grip","common_slot":53,"confirmed":true},
{"local_joint":24,"local_name":"scope","common_slot":40,"confirmed":true},
{"local_joint":33,"local_name":"xcaliber_barrel_base","common_slot":43,"confirmed":true}
```

Do not mark unresolved mappings confirmed.

- [ ] **Step 5: Run tests and validate seed artifacts**

Run:

```powershell
python tools/maintenance/test_common_pistol_master_rig.py
python tools/maintenance/common_pistol_master_rig.py validate --reconstruction Shared/Source/models/weapon/pistol/common-pistol.reconstruction.json --subset Shared/Source/models/weapon/pistol/parts-pistl-base.subset.json
```

Expected: unit tests PASS; reconstruction validation reports unresolved master hierarchy and refuses YMT emission until Task 2 completes it.

### Task 2: Reconstruct the Full 62-Slot Native Hierarchy from Evidence

**Files:**
- Modify: `tools/maintenance/common_pistol_master_rig.py`
- Modify: `tools/maintenance/northstar_pak.py` only if a reusable PAK structure primitive is missing.
- Modify: `Shared/Source/models/weapon/pistol/common-pistol.reconstruction.json`
- Modify: `Shared/Source/models/weapon/pistol/parts-pistl-base.subset.json`
- Test: `tools/maintenance/test_common_pistol_master_rig.py`

**Interfaces:**
- Consumes: `PakFile`, `RawChannelsClip`, native geometry skin-domain data, `parts-pistl-base` JOINT_HIERARCHY.
- Produces: fully confirmed 62-slot `parent_slot` + bind S/R/T reconstruction and a complete subset mapping for every Military-Pistol-used 35-domain joint.
- Produces: evidence rows carrying exact source PAK/resource/offset and evidence kind; no confidence-only promotion.

- [ ] **Step 1: Add failing tests for evidence promotion rules**

```python
def test_confidence_does_not_confirm_parent(self):
    slot = fixture_slot(confidence=1.0, parent_confirmed=False)
    self.assertFalse(can_emit_slot(slot))

def test_cross_source_parent_requires_matching_native_observations(self):
    evidence = [parent_evidence(29, 0, "pistol-military"), parent_evidence(29, 4, "pistol-a-semi-auto")]
    with self.assertRaisesRegex(ValueError, "conflicting parent evidence"):
        resolve_parent(29, evidence)
```

- [ ] **Step 2: Build an evidence extractor for all `E7898652` model/animation resources**

The extractor must gather, for slots `0..61`:

```text
native skin usage: package, mesh, vertex counts, weighted slot
native animation usage: package, clip, active slot, constant/dynamic S/R/T
cross-variant observations: Military/Beretta/XCaliber/Revolver
parts subset bind data: local name/parent/bind SRT and proven common correspondence
collision/domain token occurrences
serialized descriptor/fixup records associated with E7898652
```

Run against:

```text
pistol-military.pak
pistol-a-semi-auto.pak
pistol-velazquez.pak
pistol-revolver-357.pak
anim-player-gun-shared.pak
anim-player-gun-abby.pak
anim-abby-workbench-military.pak
parts-pistl-base.pak
```

- [ ] **Step 3: Decode/reconstruct parent and bind evidence; fail rather than fabricate**

Priority order:

```text
serialized parent/bind descriptor
> exact cross-domain subset correspondence
> exact repeated static native ANIM S/R/T reference evidence
> cross-variant native skin/animation topology evidence
```

A parent or bind may be promoted to confirmed only when its evidence rule is deterministic and conflict-free. Do not use visual intuition, generic weapon topology, or `parent=root` fallback.

- [ ] **Step 4: Prove all 62 parent/bind records and complete required subset mappings**

Run the extractor repeatedly until:

```text
slots_total=62
parent_confirmed=62
bind_confirmed=62
```

and every joint used by `parts-pistl-military-main.pak` / `parts-pistl-military-mag-base.pak` has a confirmed subset target.

If native corpus evidence cannot prove one or more required fields, keep the task RED and continue format reverse engineering; do not proceed to YMT production with gaps.

- [ ] **Step 5: Emit and validate `common-pistol.ymt` source XML/body**

Run:

```powershell
python tools/maintenance/common_pistol_master_rig.py emit-ymt --reconstruction Shared/Source/models/weapon/pistol/common-pistol.reconstruction.json --output Shared/Source/models/weapon/pistol/common-pistol.ymt.xml
```

Expected: exactly 62 joints, dense indices, deterministic structural labels for unnamed slots, fully confirmed parents/bind SRT.

- [ ] **Step 6: Run validator tests**

Expected: all reconstruction tests PASS and emission gate PASS.

### Task 3: Generic `northstar_native_master_rig` Model Import Path

**Files:**
- Modify: `NewEngine/neocore2/crates/newengine-model-import-northstar/src/compile.rs`
- Modify: `NewEngine/neocore2/crates/newengine-model-import-northstar/src/main.rs` or actual CLI argument module discovered in the crate.
- Add tests in the existing `newengine-model-import-northstar` test module(s).

**Interfaces:**
- Produces Rust DTO `MasterRigSubsetMapping { source_domain_size: usize, target_domain_size: usize, local_to_master: Vec<u16> }`.
- Produces compile behavior: master-domain passthrough or explicit subset index rewrite; all other domains reject.
- Does not call `rebind_mesh_skin_to_master_joints`.

- [ ] **Step 1: Write failing Rust unit tests**

```rust
#[test]
fn master_domain_skin_is_preserved_exactly() {
    let input = skin_vertex([53, 29, 0, 0], [0.7, 0.3, 0.0, 0.0]);
    assert_eq!(remap_skin_to_master(input, 62, 62, None).unwrap(), input);
}

#[test]
fn subset_rewrites_indices_but_preserves_weights() {
    let map = subset_map(&[(22, 53), (5, 29)]);
    let input = skin_vertex([22, 5, 0, 0], [0.75, 0.25, 0.0, 0.0]);
    let output = remap_skin_to_master(input, 35, 62, Some(&map)).unwrap();
    assert_eq!(output.joints[..2], [53, 29]);
    assert_eq!(output.weights, input.weights);
}

#[test]
fn unknown_skin_domain_is_rejected() {
    assert!(remap_skin_to_master(skin_vertex(...), 41, 62, None).is_err());
}
```

- [ ] **Step 2: Run focused crate tests and verify RED**

Run:

```powershell
cargo test -p newengine-model-import-northstar master_rig -- --nocapture
```

- [ ] **Step 3: Implement a separate master-rig skin path**

Add exact index remapping for all 8 possible skin influences. Reject missing subset entries for any nonzero-weight influence. Preserve all weights and geometry data unchanged.

- [ ] **Step 4: Add compile request fields and CLI inputs**

Required logical inputs:

```text
--master-rig-ymt <path>
--master-domain-size 62
--master-domain-token E7898652
--subset-contract <path>
```

Use existing importer material/mesh options; do not duplicate the entire character compiler.

- [ ] **Step 5: Run crate tests and a dry Military Pistol import**

Expected: `pistol-military.pak` 62-domain meshes pass with no skin mutation; 35-domain parts pass only when subset entries cover their weighted joints; no fallback report is generated.

### Task 4: Asset Builder Integration

**Files:**
- Modify: `tools/maintenance/northstar_native_assets.py`
- Add/extend Python tests in `tools/maintenance/test_common_pistol_master_rig.py` or a focused `test_northstar_native_assets_master_rig.py`.
- Modify later: `Shared/asset.build.json` only after offline importer PASS.

**Interfaces:**
- Adds compiler key `northstar_native_master_rig`.
- Consumes reconstruction/master YMT, domain size/token, subset contracts, package list, material contract.

- [ ] **Step 1: Write failing plan-validation tests**

Verify the builder rejects missing `master_domain_size`, missing subset contract for a 35-domain package, malformed mapping targets, or master domain different from 62 for the common-pistol record.

- [ ] **Step 2: Implement command construction and dependency tracking**

The builder must include reconstruction/subset/source files in incremental dependency hashes so evidence changes invalidate the generated YDD/YMT/YCD.

- [ ] **Step 3: Run Python tests and `--mode production` dry/build gate**

Expected: master-rig record is reproducible and fails closed on missing evidence.

### Task 5: Produce the 62-Domain Military Pistol YDD/YMT

**Files:**
- Copy/provenance source: `Shared/Source/models/weapon/pistol/military/pistol-military.pak`
- Existing: `Shared/Source/models/weapon/pistol/parts-pistl-military-main.pak`
- Existing: `Shared/Source/models/weapon/pistol/parts-pistl-military-mag-base.pak`
- Create: `Shared/Content/models/weapon/pistol/common-pistol.ymt`
- Replace after successful offline build: `Shared/Content/models/weapon/pistol/pistol.ydd`
- Modify: `Shared/asset.build.json`

**Interfaces:**
- `pistol.ydd` emitted skin domain is 62.
- `common-pistol.ymt` contains 62 fully confirmed joints.

- [ ] **Step 1: Add `pistol-military.pak` to preserved source provenance and verify hash**

Do not mutate the source PAK.

- [ ] **Step 2: Update asset plan to use `northstar_native_master_rig`**

Geometry packages:

```text
pistol-military.pak                    domain 62 -> passthrough
parts-pistl-military-main.pak          domain 35 -> subset
parts-pistl-military-mag-base.pak      domain 35 -> subset
```

Skeleton output/reference is `common-pistol.ymt@common_pistol`.

- [ ] **Step 3: Build model and validate skin statistics**

Expected gates:

```text
master joints = 62
all emitted skin indices < 62
62-domain weight/index preservation = exact
35-domain weights = exact, indices = subset-translated only
skin fallback count = 0
```

- [ ] **Step 4: Run bind-pose identity validation using runtime-equivalent skeleton compilation**

A small focused test/example may decode the emitted YMT metadata and call `AnimationSkeletonRuntime::compile`; bind palette must remain identity within the runtime tolerance.

### Task 6: Native Military Pistol YCD Against `common-pistol`

**Files:**
- Modify: `tools/maintenance/compile_northstar_native_pak_animations.py`
- Modify: `tools/maintenance/northstar_pak.py` only if ANIM_GROUP identity is not exposed cleanly.
- Create: `Shared/Source/models/weapon/pistol/military/military-pistol.animation-catalog.json`
- Create: `Shared/Content/animations/weapon/pistol/military-pistol.ycd`
- Modify: `Shared/asset.build.json`

**Interfaces:**
- Adds `--expected-group E7898652` and `--joint-domain-size 62` validation.
- YCD skeleton ref: `shared/models/weapon/pistol/common-pistol.ymt@common_pistol`.

- [ ] **Step 1: Write failing Python tests for group/domain gates**

```python
def test_wrong_anim_group_is_rejected(): ...
def test_destination_slot_62_or_above_is_rejected(): ...
def test_common_pistol_clip_keeps_native_destination_indices(): ...
```

- [ ] **Step 2: Implement group/domain validation without remapping**

Compile exact native authored records for fire, dry-fire, reload. Keep destination indices unchanged.

- [ ] **Step 3: Compile native YCD and validate NEF8/catalog**

Expected: all selected clips decode, max active slot `<62`, no unsupported/procedural fallback, skeleton ref matches `common-pistol`.

### Task 7: Author Military Pistol Animation Definition and Runtime Admission

**Files:**
- Modify: `Projects/ForestRoad/Source/scripts/fps_gameplay.ts` only where the existing `weapon.pistol.standard` definition needs skeleton/YCD fields.
- Rebuild: `Projects/ForestRoad/Scripts/fps_gameplay.ysc` from authored TS through the canonical class-5/schema-v3 path.
- Modify runtime code only if an actual 62-domain admission bug is exposed; do not redesign runtime preemptively.

**Interfaces:**
- `WeaponAnimationDefinition.skeleton = shared/models/weapon/pistol/common-pistol.ymt@common_pistol`
- `fire`, `reload`, and optional native spawn/idle point to exact YCD entries.
- Rifle remains authored primary; pistol remains sidearm.

- [ ] **Step 1: Add/adjust runtime binding tests**

Verify mount-prefix-normalized skeleton refs bind and 62-joint weapon palette publishes without truncation.

- [ ] **Step 2: Add authored Military Pistol skeleton/YCD references**

Do not change `primary_weapon` or `primary_ammo`.

- [ ] **Step 3: Rebuild canonical YSC and round-trip compare decompressed body to authored TS**

Expected: byte-for-byte equality of decompressed YSC body and `Source/scripts/fps_gameplay.ts`.

### Task 8: Full Verification and Live `Digit2` Acceptance

**Files:**
- No new production code unless a verified defect is found.
- Logs under `Intermediate/` or `tmp/` only.

**Interfaces:**
- Validates the entire slice from source evidence to live equipped weapon.

- [ ] **Step 1: Run focused Python and Rust tests**

```powershell
python tools/maintenance/test_common_pistol_master_rig.py
python -m py_compile tools/maintenance/common_pistol_master_rig.py tools/maintenance/compile_northstar_native_pak_animations.py tools/maintenance/northstar_native_assets.py
cargo test -p newengine-model-import-northstar
cargo test -p newengine-animation-runtime
cargo test -p newengine-game-ready-world weapon
```

- [ ] **Step 2: Run Shared production asset build/validation for pistol records**

Verify YTD/NEMAT remain green, YDD/YMT/YCD all validate, and no skin fallback is reported.

- [ ] **Step 3: Launch ForestRoad with authored rifle-primary configuration**

Equip the Military Pistol using `Digit2`, then trigger fire and reload.

- [ ] **Step 4: Inspect logs and visual result**

Required live gates:

```text
YDD/YMT admission PASS
YCD skeleton binding PASS
common-pistol joints=62
slide/action visibly moves under native fire
hammer/trigger visibly moves under native fire
magazine subtree behaves under native reload
no joint-out-of-range
no skeleton mismatch
no proximity/root/procedural fallback
```

- [ ] **Step 5: Run regression gate and report unrelated blockers separately**

Do not modify unrelated audio/render/animation work merely to make the global repository green. Distinguish feature-local PASS from external compile barriers.

- [ ] **Step 6: Final verification before completion claim**

Confirm generated assets exist, source provenance hashes are recorded, authored TS/YSC are synchronized, and `git diff` contains only intended NewEngine code/docs changes in task commits plus separately managed Shared/ForestRoad artifacts.
