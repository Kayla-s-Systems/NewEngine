use super::*;
use newengine_materials::api::material_id_from_name;

fn settings() -> FoliageSettings {
    FoliageSettings {
        canonical_path: "Source/foliage/oak.srt".to_owned(),
        density: 1.0,
        seed: 91,
        ..FoliageSettings::default()
    }
}

fn runtime_asset() -> FoliageRuntimeAssetV1 {
    FoliageRuntimeAssetV1 {
        source_ref: "Source/foliage/oak.srt".to_owned(),
        lods: vec![
            FoliageLodAssetV1 {
                lod_index: 0,
                min_distance: 0.0,
                max_distance: 28.0,
                drawable_ref: "Content/foliage/oak.ydd@lod0".to_owned(),
                impostor: false,
            },
            FoliageLodAssetV1 {
                lod_index: 1,
                min_distance: 28.0,
                max_distance: 64.0,
                drawable_ref: "Content/foliage/oak.ydd@lod1".to_owned(),
                impostor: false,
            },
            FoliageLodAssetV1 {
                lod_index: 2,
                min_distance: 64.0,
                max_distance: 300.0,
                drawable_ref: "Content/foliage/oak.ydd@billboard".to_owned(),
                impostor: true,
            },
        ],
        materials: vec![FoliageMaterialBindingV1 {
            variant: "default".to_owned(),
            material: material_id_from_name("foliage.oak.default"),
        }],
        ..FoliageRuntimeAssetV1::default()
    }
}

fn instance(id: u64, x: f32) -> FoliageInstanceInputV1 {
    let mut value = FoliageInstanceInputV1 {
        stable_id: id,
        ..FoliageInstanceInputV1::default()
    };
    value.transform_cols[3][0] = x;
    value
}

#[test]
fn settings_accept_speedtree_sources_and_derive_runtime_ref() {
    let clean = settings().sanitized().expect("valid foliage settings");
    assert_eq!(clean.canonical_path, "Source/foliage/oak.srt");
    assert_eq!(clean.importer_id().unwrap(), FOLIAGE_SRT_IMPORTER_ID);
    assert_eq!(
        clean.runtime_asset_ref().unwrap(),
        "Source/foliage/oak.nefoliage"
    );

    let spm = FoliageSettings {
        canonical_path: "Source/foliage/speedtree/oak/Oak_Hero_Forest.spm".to_owned(),
        ..FoliageSettings::default()
    }
    .sanitized()
    .expect("valid SPM foliage settings");
    assert_eq!(spm.importer_id().unwrap(), FOLIAGE_SPM_IMPORTER_ID);
    assert_eq!(
        spm.runtime_asset_ref().unwrap(),
        "Source/foliage/speedtree/oak/Oak_Hero_Forest.nefoliage"
    );

    let invalid = FoliageSettings {
        canonical_path: "Source/foliage/oak.fbx".to_owned(),
        ..FoliageSettings::default()
    };
    assert!(invalid.sanitized().is_err());
}

#[test]
fn cpu_fallback_is_deterministic_and_selects_lods() {
    let request = FoliageExtractionRequestV1 {
        settings: settings(),
        runtime_asset: runtime_asset(),
        instances: vec![instance(3, 80.0), instance(1, 10.0), instance(2, 40.0)],
        ..FoliageExtractionRequestV1::default()
    };
    let a = build_foliage_extraction_plan_v1(request.clone()).unwrap();
    let b = build_foliage_extraction_plan_v1(request).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.path, FoliageExtractionPathV1::CpuFallback);
    assert_eq!(a.batches.len(), 3);
    assert_eq!(
        a.batches
            .iter()
            .map(|batch| batch.instances[0].stable_id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(a.batches.iter().all(|batch| batch.material.is_valid()));
}

#[test]
fn gpu_path_is_capability_gated_and_cpu_always_available() {
    let base = FoliageExtractionRequestV1 {
        settings: settings(),
        runtime_asset: runtime_asset(),
        instances: vec![instance(1, 10.0)],
        capabilities: FoliageExtractionCapabilitiesV1 {
            gpu_culling: true,
            indirect_draw: false,
        },
        ..FoliageExtractionRequestV1::default()
    };
    let cpu = build_foliage_extraction_plan_v1(base.clone()).unwrap();
    assert_eq!(cpu.path, FoliageExtractionPathV1::CpuFallback);
    assert_eq!(cpu.batches.len(), 1);
    assert!(cpu.gpu_work.is_none());

    let mut gpu_request = base;
    gpu_request.capabilities.indirect_draw = true;
    let gpu = build_foliage_extraction_plan_v1(gpu_request).unwrap();
    assert_eq!(gpu.path, FoliageExtractionPathV1::GpuIndirect);
    assert!(gpu.batches.is_empty());
    let work = gpu.gpu_work.expect("capability-gated GPU work");
    assert_eq!(work.candidates.len(), 1);
    assert_eq!(work.lods.len(), 3);
    assert_eq!(work.settings.wind, settings().sanitized().unwrap().wind);
}

#[test]
fn instance_runtime_uses_distance_only_and_does_not_camera_cone_cull() {
    let settings = settings();
    let runtime = FoliageInstanceRuntime::new(&settings, 0, 1);
    assert!(runtime.is_visible(299.0, false));
    assert!(!runtime.is_visible(301.0, false));
    assert!(runtime.is_visible(179.0, true));
    assert!(!runtime.is_visible(181.0, true));
}

#[test]
fn compact_runtime_component_selects_only_its_lod_range() {
    let settings = settings();
    let runtime = FoliageInstanceRuntime::new(&settings, 1, 3);
    assert!(!runtime.is_visible(10.0, false));
    assert!(runtime.is_visible(40.0, false));
    assert!(!runtime.is_visible(80.0, false));
}

#[test]
fn runtime_manifest_rejects_non_contiguous_lods() {
    let mut asset = runtime_asset();
    asset.lods[1].lod_index = 7;
    assert!(asset.validate().is_err());
}
