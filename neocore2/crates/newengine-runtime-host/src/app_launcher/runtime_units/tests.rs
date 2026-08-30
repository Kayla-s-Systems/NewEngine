
use std::collections::BTreeSet;

use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_service_api::{
    EngineCompositionSpec, EngineRuntimeUnitSpec, RuntimeUnitDescriptor,
};

use super::catalog::{
    distribution_runtime_unit_catalog, RuntimeUnitCatalog, RuntimeUnitMaterializer,
};
use super::solver::select_runtime_unit_keys;
use super::super::types::RuntimeHostRuntimeUnitRegistration;
use newengine_service_api::{
    CapabilityId, CapabilityRequirement, EngineRuntimeUnitKind,
    RuntimeUnitRequirementDescriptor, RuntimeUnitRequirementSpec, SystemTag,
};

const RENDER: CapabilityId = CapabilityId::new("render.backend", "engine.render", "render");
const PHYSICS: CapabilityId = CapabilityId::new("physics.backend", "engine.physics", "physics");
const WORLD: CapabilityId = CapabilityId::new("world.backend", "engine.world", "world");
const CUSTOM: CapabilityId =
    CapabilityId::new("custom.runtime", "engine.custom", "runtime-unit");

const REQUIREMENTS: &[CapabilityRequirement] = &[
    CapabilityRequirement::required(RENDER),
    CapabilityRequirement::required(PHYSICS),
];
const WORLD_REQUIREMENTS: &[CapabilityRequirement] = &[CapabilityRequirement::required(WORLD)];
const CUSTOM_REQUIREMENTS: &[CapabilityRequirement] =
    &[CapabilityRequirement::required(CUSTOM)];
const RENDER_V1: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "render.bridge.v1",
    1,
    EngineRuntimeUnitKind::Adapter,
    &["engine.runtime.render-api"],
    &["render.backend"],
    &["engine.runtime-unit", "service-adapter"],
);
const RENDER_V2: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "render.bridge.v2",
    2,
    EngineRuntimeUnitKind::Adapter,
    &["engine.runtime.render-api"],
    &["render.backend"],
    &["engine.runtime-unit", "service-adapter"],
);
const PHYSICS_V1: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "physics.bridge.v1",
    1,
    EngineRuntimeUnitKind::Adapter,
    &["engine.runtime.physics-api"],
    &["physics.backend"],
    &["engine.runtime-unit", "service-adapter"],
);
const SCENE_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.scene.test",
    1,
    EngineRuntimeUnitKind::Provider,
    &["scene.backend"],
    &[],
    &["engine.runtime-unit", "static"],
);
const WORLD_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.world.test",
    1,
    EngineRuntimeUnitKind::Provider,
    &["world.backend"],
    &["scene.backend"],
    &["engine.runtime-unit", "static"],
);
const SCENE_HIGH_VERSION: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.scene.high-version",
    10,
    EngineRuntimeUnitKind::Provider,
    &["scene.backend"],
    &[],
    &["engine.runtime-unit", "static"],
);
const SCENE_PREFERRED: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.scene.preferred",
    1,
    EngineRuntimeUnitKind::Provider,
    &["scene.backend"],
    &[],
    &["engine.runtime-unit", "static", "runtime.preferred"],
);
const CLOCK_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.clock.test",
    1,
    EngineRuntimeUnitKind::Provider,
    &["clock.backend"],
    &[],
    &["engine.runtime-unit", "static"],
);
const SCENE_TRANSITIVE_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.scene.transitive",
    1,
    EngineRuntimeUnitKind::Provider,
    &["scene.backend"],
    &["clock.backend"],
    &["engine.runtime-unit", "static"],
);
const PREFERRED_RUNTIME: SystemTag = SystemTag::new("runtime.preferred");
const CUSTOM_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "profile.runtime.custom",
    1,
    EngineRuntimeUnitKind::ProductExtension,
    &["custom.runtime"],
    &[],
    &["engine.runtime-unit", "profile"],
);
const PROFILE_INVENTORY: &[EngineRuntimeUnitSpec] = &[CUSTOM_UNIT];

fn noop_runtime_unit(
    _engine: &mut Engine<()>,
    _startup: &StartupConfig,
) -> EngineResult<Option<Box<dyn newengine_core::Module<()>>>> {
    Ok(None)
}

#[test]
fn solver_selects_implicit_runtime_bridges() {
    let composition = EngineCompositionSpec::new("test.product", REQUIREMENTS);
    let descriptors = [RENDER_V1, PHYSICS_V1, RENDER_V2]
        .into_iter()
        .map(RuntimeUnitDescriptor::from_static)
        .collect::<Vec<_>>();
    let selected = select_runtime_unit_keys(composition, &descriptors, &[])
        .expect("runtime-unit composition");
    assert!(selected.contains(&"render.bridge.v2@2".to_owned()));
    assert!(selected.contains(&"physics.bridge.v1@1".to_owned()));
    assert!(!selected.contains(&"render.bridge.v1@1".to_owned()));
}

#[test]
fn profile_runtime_units_are_inventory_candidates_not_implicit_roots() {
    let no_requirement =
        EngineCompositionSpec::new("test.profile", &[]).with_runtime_units(PROFILE_INVENTORY);
    let descriptors = PROFILE_INVENTORY
        .iter()
        .copied()
        .map(RuntimeUnitDescriptor::from_static)
        .collect::<Vec<_>>();
    assert!(select_runtime_unit_keys(no_requirement, &descriptors, &[])
        .unwrap()
        .is_empty());

    let required = EngineCompositionSpec::new("test.profile", CUSTOM_REQUIREMENTS)
        .with_runtime_units(PROFILE_INVENTORY);
    assert_eq!(
        select_runtime_unit_keys(required, &descriptors, &[]).unwrap(),
        vec!["profile.runtime.custom@1".to_owned()]
    );
}

#[test]
fn runtime_unit_only_requirement_selects_from_profile_inventory() {
    const ROOTS: &[RuntimeUnitRequirementSpec] =
        &[RuntimeUnitRequirementSpec::required("custom.runtime")];
    let composition = EngineCompositionSpec::new("test.profile-roots", &[])
        .with_runtime_units(PROFILE_INVENTORY)
        .with_runtime_unit_requirements(ROOTS);
    let descriptors = PROFILE_INVENTORY
        .iter()
        .copied()
        .map(RuntimeUnitDescriptor::from_static)
        .collect::<Vec<_>>();
    assert_eq!(
        select_runtime_unit_keys(composition, &descriptors, &[]).unwrap(),
        vec!["profile.runtime.custom@1".to_owned()]
    );
}

#[test]
fn missing_runtime_unit_root_is_a_hard_error() {
    const ROOTS: &[RuntimeUnitRequirementSpec] =
        &[RuntimeUnitRequirementSpec::required("missing.runtime")];
    let composition = EngineCompositionSpec::new("test.missing-root", &[])
        .with_runtime_unit_requirements(ROOTS);
    let error = select_runtime_unit_keys(composition, &[], &[])
        .expect_err("missing runtime-unit root must fail composition");
    assert!(error.contains("missing.runtime"));
}

#[test]
fn dependency_closure_uses_combined_inventory() {
    let composition = EngineCompositionSpec::new("test.static", WORLD_REQUIREMENTS)
        .with_runtime_units(&[WORLD_UNIT, SCENE_UNIT]);
    let descriptors = [WORLD_UNIT, SCENE_UNIT]
        .into_iter()
        .map(RuntimeUnitDescriptor::from_static)
        .collect::<Vec<_>>();
    let selected = select_runtime_unit_keys(composition, &descriptors, &[]).unwrap();
    assert!(selected.contains(&"engine.runtime.world.test@1".to_owned()));
    assert!(selected.contains(&"engine.runtime.scene.test@1".to_owned()));
}

#[test]
fn dependency_closure_provider_choice_is_owned_by_composition_solver() {
    let composition =
        EngineCompositionSpec::new("test.fixed-point-preference", WORLD_REQUIREMENTS)
            .with_runtime_units(&[WORLD_UNIT, SCENE_HIGH_VERSION, SCENE_PREFERRED])
            .with_preferred_tags(&[PREFERRED_RUNTIME]);
    let descriptors = [WORLD_UNIT, SCENE_HIGH_VERSION, SCENE_PREFERRED]
        .into_iter()
        .map(RuntimeUnitDescriptor::from_static)
        .collect::<Vec<_>>();

    let selected = select_runtime_unit_keys(composition, &descriptors, &[])
        .expect("dependency provider must resolve through shared solver");

    assert!(selected.contains(&"engine.runtime.world.test@1".to_owned()));
    assert!(selected.contains(&"engine.runtime.scene.preferred@1".to_owned()));
    assert!(!selected.contains(&"engine.runtime.scene.high-version@10".to_owned()));
}

#[test]
fn dependency_closure_reaches_transitive_fixed_point() {
    let composition =
        EngineCompositionSpec::new("test.fixed-point-transitive", WORLD_REQUIREMENTS)
            .with_runtime_units(&[WORLD_UNIT, SCENE_TRANSITIVE_UNIT, CLOCK_UNIT]);
    let descriptors = [WORLD_UNIT, SCENE_TRANSITIVE_UNIT, CLOCK_UNIT]
        .into_iter()
        .map(RuntimeUnitDescriptor::from_static)
        .collect::<Vec<_>>();

    let selected = select_runtime_unit_keys(composition, &descriptors, &[])
        .expect("transitive dependency closure");

    assert_eq!(
        selected,
        vec![
            "engine.runtime.clock.test@1".to_owned(),
            "engine.runtime.scene.transitive@1".to_owned(),
            "engine.runtime.world.test@1".to_owned(),
        ]
    );
}

#[test]
fn catalog_merges_distribution_descriptor_and_profile_inventory() {
    let mut catalog = RuntimeUnitCatalog::default();
    catalog
        .register_static(CUSTOM_UNIT, "distribution", noop_runtime_unit)
        .unwrap();
    catalog
        .register_descriptor(
            RuntimeUnitDescriptor::from_static(CUSTOM_UNIT),
            "profile",
            None,
        )
        .unwrap();
    let entry = catalog.registration("profile.runtime.custom@1").unwrap();
    assert_eq!(entry.sources.len(), 2);
    assert!(matches!(
        entry.materializer,
        Some(RuntimeUnitMaterializer::Static(_))
    ));
}

#[test]
fn catalog_allows_multiple_versions_of_same_unit_id() {
    let v2 = EngineRuntimeUnitSpec::new(
        CUSTOM_UNIT.id,
        2,
        CUSTOM_UNIT.kind,
        CUSTOM_UNIT.provides,
        CUSTOM_UNIT.requires,
        CUSTOM_UNIT.tags,
    );
    let mut catalog = RuntimeUnitCatalog::default();
    catalog
        .register_static(CUSTOM_UNIT, "v1", noop_runtime_unit)
        .unwrap();
    catalog
        .register_static(v2, "v2", noop_runtime_unit)
        .unwrap();
    assert_eq!(catalog.registrations.len(), 2);
}

#[test]
fn product_supplied_distribution_catalog_is_the_only_static_distribution_inventory() {
    const DISTRIBUTION: &[RuntimeHostRuntimeUnitRegistration] = &[
        RuntimeHostRuntimeUnitRegistration::new(SCENE_UNIT, noop_runtime_unit),
        RuntimeHostRuntimeUnitRegistration::new(WORLD_UNIT, noop_runtime_unit),
    ];
    let catalog = distribution_runtime_unit_catalog(DISTRIBUTION)
        .expect("product-supplied distribution catalog");
    let ids = catalog
        .descriptors()
        .into_iter()
        .map(|descriptor| descriptor.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids,
        BTreeSet::from([
            "engine.runtime.scene.test".to_owned(),
            "engine.runtime.world.test".to_owned(),
        ])
    );
}

#[test]
fn product_supplied_distribution_units_participate_in_generic_solver() {
    const DISTRIBUTION: &[RuntimeHostRuntimeUnitRegistration] = &[
        RuntimeHostRuntimeUnitRegistration::new(SCENE_UNIT, noop_runtime_unit),
        RuntimeHostRuntimeUnitRegistration::new(WORLD_UNIT, noop_runtime_unit),
    ];
    const ROOTS: &[RuntimeUnitRequirementSpec] =
        &[RuntimeUnitRequirementSpec::required("world.backend")];
    let composition = EngineCompositionSpec::new("test.product-distribution", &[])
        .with_runtime_unit_requirements(ROOTS);
    let catalog = distribution_runtime_unit_catalog(DISTRIBUTION)
        .expect("product-supplied distribution catalog");
    let selected = select_runtime_unit_keys(composition, &catalog.descriptors(), &[])
        .expect("generic runtime-unit selection");
    assert_eq!(
        selected,
        vec![
            "engine.runtime.scene.test@1".to_owned(),
            "engine.runtime.world.test@1".to_owned(),
        ]
    );
}
#[test]
fn runtime_overlay_many_selects_all_compatible_units() {
    const FEATURE_A: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "game.render.feature.a",
        1,
        EngineRuntimeUnitKind::ProductExtension,
        &["render.feature"],
        &[],
        &["engine.runtime-unit"],
    );
    const FEATURE_B: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "game.render.feature.b",
        1,
        EngineRuntimeUnitKind::ProductExtension,
        &["render.feature"],
        &[],
        &["engine.runtime-unit"],
    );
    let descriptors = [FEATURE_A, FEATURE_B]
        .into_iter()
        .map(RuntimeUnitDescriptor::from_static)
        .collect::<Vec<_>>();
    let overlay = [RuntimeUnitRequirementDescriptor::required("render.feature")
        .with_cardinality(newengine_service_api::Cardinality::Many)];
    let selected = select_runtime_unit_keys(
        EngineCompositionSpec::new("test.game-module-overlay", &[]),
        &descriptors,
        &overlay,
    )
    .expect("many runtime-unit overlay must resolve");
    assert_eq!(selected.len(), 2);
    assert!(selected.contains(&"game.render.feature.a@1".to_owned()));
    assert!(selected.contains(&"game.render.feature.b@1".to_owned()));
}
