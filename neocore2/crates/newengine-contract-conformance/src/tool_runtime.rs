use std::collections::BTreeSet;

use newengine_contract_api::{ContractKind, ContractSpec};

pub const TOOL_RUNTIME_CONFORMANCE_REGISTRY_SCHEMA: &str = "northstar.tool_runtime_conformance.v1";
pub const TOOL_RUNTIME_CONFORMANCE_REGISTRY_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolRuntimeFixtureKind {
    File,
    GeneratedDirectory,
}

impl ToolRuntimeFixtureKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::GeneratedDirectory => "generated_directory",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolRuntimeFixtureSpec {
    pub kind: ToolRuntimeFixtureKind,
    pub testdata_name: Option<&'static str>,
    pub source_relative: &'static str,
}

impl ToolRuntimeFixtureSpec {
    pub const fn file(testdata_name: &'static str, source_relative: &'static str) -> Self {
        Self {
            kind: ToolRuntimeFixtureKind::File,
            testdata_name: Some(testdata_name),
            source_relative,
        }
    }

    pub const fn generated_directory(source_relative: &'static str) -> Self {
        Self {
            kind: ToolRuntimeFixtureKind::GeneratedDirectory,
            testdata_name: None,
            source_relative,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolRuntimeCommandPhase {
    Prepare,
    Produce,
    Validate,
}

impl ToolRuntimeCommandPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::Produce => "produce",
            Self::Validate => "validate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolRuntimeCommandSpec {
    pub phase: ToolRuntimeCommandPhase,
    pub args: &'static [&'static str],
}

impl ToolRuntimeCommandSpec {
    pub const fn prepare(args: &'static [&'static str]) -> Self {
        Self {
            phase: ToolRuntimeCommandPhase::Prepare,
            args,
        }
    }

    pub const fn produce(args: &'static [&'static str]) -> Self {
        Self {
            phase: ToolRuntimeCommandPhase::Produce,
            args,
        }
    }

    pub const fn validate(args: &'static [&'static str]) -> Self {
        Self {
            phase: ToolRuntimeCommandPhase::Validate,
            args,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConformanceWorkspace {
    NeoCore,
    AssetManager,
}

impl ConformanceWorkspace {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeoCore => "neocore",
            Self::AssetManager => "asset_manager",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetManagerDecodeSpec {
    pub workspace: ConformanceWorkspace,
    pub package: &'static str,
    pub example: &'static str,
    pub output_kind: &'static str,
}

impl AssetManagerDecodeSpec {
    pub const fn new(
        package: &'static str,
        example: &'static str,
        output_kind: &'static str,
    ) -> Self {
        Self {
            workspace: ConformanceWorkspace::AssetManager,
            package,
            example,
            output_kind,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeDecodeSpec {
    pub workspace: ConformanceWorkspace,
    pub package: &'static str,
    pub example: &'static str,
}

impl RuntimeDecodeSpec {
    pub const fn new(
        workspace: ConformanceWorkspace,
        package: &'static str,
        example: &'static str,
    ) -> Self {
        Self {
            workspace,
            package,
            example,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonicalProjection {
    YtypDefinitionEntriesV1,
    YddDrawableDictionaryV1,
    YtdTextureDictionaryV1,
    NematMaterialLibraryV1,
    NeuiSelectorSurfaceV1,
}

impl CanonicalProjection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::YtypDefinitionEntriesV1 => "ytyp_definition_entries_v1",
            Self::YddDrawableDictionaryV1 => "ydd_drawable_dictionary_v1",
            Self::YtdTextureDictionaryV1 => "ytd_texture_dictionary_v1",
            Self::NematMaterialLibraryV1 => "nemat_material_library_v1",
            Self::NeuiSelectorSurfaceV1 => "neui_selector_surface_v1",
        }
    }
}

/// Declarative producer -> artifact -> runtime-contract conformance entry.
///
/// Tool paths are intentionally identified by logical `tool_key` only. The Python
/// executor resolves that key through `northstar_native_assets.tool_paths()`, so no
/// username, suite location or executable path is owned by this registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolRuntimeConformanceSpec {
    pub id: &'static str,
    pub tool_key: &'static str,
    pub fixture: ToolRuntimeFixtureSpec,
    pub output_relative: &'static str,
    pub content_kind: u32,
    pub schema_contract_key: &'static str,
    pub readable_legacy_schema_versions: &'static [u16],
    pub commands: &'static [ToolRuntimeCommandSpec],
    pub asset_manager_decode: Option<AssetManagerDecodeSpec>,
    pub runtime_decode: Option<RuntimeDecodeSpec>,
    pub canonical_projection: Option<CanonicalProjection>,
}

const YTYP_COMMANDS: &[ToolRuntimeCommandSpec] = &[
    ToolRuntimeCommandSpec::produce(&[
        "compile",
        "--root",
        "{root}",
        "--input",
        "{source_rel}",
        "--output",
        "{output_rel}",
    ]),
    ToolRuntimeCommandSpec::validate(&["validate", "--root", "{root}", "--all"]),
];

const YDD_COMMANDS: &[ToolRuntimeCommandSpec] = &[
    ToolRuntimeCommandSpec::produce(&["pack", "--input", "{source}", "--output", "{output}"]),
    ToolRuntimeCommandSpec::validate(&["validate", "--input", "{output}"]),
];

const YTD_COMMANDS: &[ToolRuntimeCommandSpec] = &[
    ToolRuntimeCommandSpec::prepare(&["write-smoke-fixtures", "--output", "{source}"]),
    ToolRuntimeCommandSpec::produce(&["pack", "--input-dir", "{source}", "--output", "{output}"]),
    ToolRuntimeCommandSpec::validate(&["validate", "--input", "{output}"]),
];

const NEMAT_COMMANDS: &[ToolRuntimeCommandSpec] = &[
    ToolRuntimeCommandSpec::produce(&["pack", "--input", "{source}", "--output", "{output}"]),
    ToolRuntimeCommandSpec::validate(&["validate", "--input", "{output}"]),
];

const NEUI_COMMANDS: &[ToolRuntimeCommandSpec] = &[
    ToolRuntimeCommandSpec::produce(&[
        "compile", "--root", "{root}", "--input", "{source}", "--output", "{output}",
    ]),
    ToolRuntimeCommandSpec::validate(&["validate", "--root", "{root}", "--input", "{output}"]),
];

pub const TOOL_RUNTIME_CONFORMANCE_SPECS: &[ToolRuntimeConformanceSpec] = &[
    ToolRuntimeConformanceSpec {
        id: "ytyp",
        tool_key: "ytyp",
        fixture: ToolRuntimeFixtureSpec::file(
            "p3_ytyp_fixture.ytyp.xml",
            "Source/p3_ytyp_fixture.ytyp.xml",
        ),
        output_relative: "Content/p3_ytyp_fixture.ytyp",
        content_kind: newengine_asset_format_nef8::ytyp::CONTENT_KIND,
        schema_contract_key: newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_CONTRACT_SPEC.key,
        readable_legacy_schema_versions: &[],
        commands: YTYP_COMMANDS,
        asset_manager_decode: Some(AssetManagerDecodeSpec::new(
            "newengine-codec-listfile",
            "p4_decode_assetmanager_native_dto",
            newengine_assets_api::ASSET_LIST_FILE_MANIFEST_OUTPUT,
        )),
        runtime_decode: Some(RuntimeDecodeSpec::new(
            ConformanceWorkspace::NeoCore,
            "newengine-definitions-runtime",
            "p4_decode_ytyp_native_dto",
        )),
        canonical_projection: Some(CanonicalProjection::YtypDefinitionEntriesV1),
    },
    ToolRuntimeConformanceSpec {
        id: "ydd",
        tool_key: "ydd",
        fixture: ToolRuntimeFixtureSpec::file("p3_ydd_fixture.obj", "Source/p3_ydd_fixture.obj"),
        output_relative: "Content/p3_ydd_fixture.ydd",
        content_kind: newengine_asset_format_nef8::ydd::CONTENT_KIND,
        schema_contract_key: newengine_asset_format_nef8::YDD_BINARY_CONTRACT_SPEC.key,
        readable_legacy_schema_versions:
            newengine_asset_format_nef8::ydd::READABLE_CONTENT_SCHEMA_VERSIONS,
        commands: YDD_COMMANDS,
        asset_manager_decode: Some(AssetManagerDecodeSpec::new(
            "newengine-codec-listfile",
            "p4_decode_assetmanager_native_dto",
            "asset.drawable_manifest_v1",
        )),
        runtime_decode: Some(RuntimeDecodeSpec::new(
            ConformanceWorkspace::NeoCore,
            "newengine-asset-format-nef8",
            "p4_decode_ydd_native_dto",
        )),
        canonical_projection: Some(CanonicalProjection::YddDrawableDictionaryV1),
    },
    ToolRuntimeConformanceSpec {
        id: "ytd",
        tool_key: "ytd",
        fixture: ToolRuntimeFixtureSpec::generated_directory("Source/textures"),
        output_relative: "Content/p3_ytd_fixture.ytd",
        content_kind: newengine_asset_format_nef8::ytd::CONTENT_KIND,
        schema_contract_key: newengine_asset_format_nef8::ytd::CONTENT_SCHEMA_CONTRACT_SPEC.key,
        readable_legacy_schema_versions:
            newengine_asset_format_nef8::ytd::READABLE_CONTENT_SCHEMA_VERSIONS,
        commands: YTD_COMMANDS,
        asset_manager_decode: Some(AssetManagerDecodeSpec::new(
            "newengine-codec-listfile",
            "p4_decode_assetmanager_native_dto",
            "asset.texture_dictionary_manifest_v1",
        )),
        runtime_decode: Some(RuntimeDecodeSpec::new(
            ConformanceWorkspace::NeoCore,
            "newengine-texture-container",
            "p4_decode_ytd_native_dto",
        )),
        canonical_projection: Some(CanonicalProjection::YtdTextureDictionaryV1),
    },
    ToolRuntimeConformanceSpec {
        id: "nemat",
        tool_key: "nemat",
        fixture: ToolRuntimeFixtureSpec::file(
            "p3_nemat_fixture.nemat.xml",
            "Source/p3_nemat_fixture.nemat.xml",
        ),
        output_relative: "Content/p3_nemat_fixture.nemat",
        content_kind: newengine_asset_format_nef8::nemat::CONTENT_KIND,
        schema_contract_key: newengine_asset_format_nef8::nemat::CONTENT_SCHEMA_CONTRACT_SPEC.key,
        readable_legacy_schema_versions: &[],
        commands: NEMAT_COMMANDS,
        asset_manager_decode: Some(AssetManagerDecodeSpec::new(
            "newengine-codec-listfile",
            "p4_decode_assetmanager_native_dto",
            newengine_assets_api::ASSET_LIST_FILE_MANIFEST_OUTPUT,
        )),
        runtime_decode: Some(RuntimeDecodeSpec::new(
            ConformanceWorkspace::NeoCore,
            "newengine-material-runtime",
            "p4_decode_nemat_native_dto",
        )),
        canonical_projection: Some(CanonicalProjection::NematMaterialLibraryV1),
    },
    ToolRuntimeConformanceSpec {
        id: "neui",
        tool_key: "neui",
        fixture: ToolRuntimeFixtureSpec::file(
            "p3_neui_fixture.neui.xml",
            "Source/p3_neui_fixture.neui.xml",
        ),
        output_relative: "Content/p3_neui_fixture.neui",
        content_kind: newengine_asset_format_nef8::neui::CONTENT_KIND,
        schema_contract_key: newengine_asset_format_nef8::neui::CONTENT_SCHEMA_CONTRACT_SPEC.key,
        readable_legacy_schema_versions: &[],
        commands: NEUI_COMMANDS,
        asset_manager_decode: Some(AssetManagerDecodeSpec::new(
            "newengine-codec-listfile",
            "p4_decode_assetmanager_native_dto",
            newengine_assets_api::ASSET_LIST_FILE_MANIFEST_OUTPUT,
        )),
        runtime_decode: Some(RuntimeDecodeSpec::new(
            ConformanceWorkspace::NeoCore,
            "newengine-assets-ui-runtime",
            "p4_decode_neui_native_dto",
        )),
        canonical_projection: Some(CanonicalProjection::NeuiSelectorSurfaceV1),
    },
];

#[inline]
pub const fn tool_runtime_conformance_specs() -> &'static [ToolRuntimeConformanceSpec] {
    TOOL_RUNTIME_CONFORMANCE_SPECS
}

pub fn tool_runtime_conformance_spec(id: &str) -> Option<&'static ToolRuntimeConformanceSpec> {
    TOOL_RUNTIME_CONFORMANCE_SPECS
        .iter()
        .find(|spec| spec.id == id)
}

pub fn validate_tool_runtime_registry() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    let mut schema_keys = BTreeSet::new();
    const ALLOWED_PLACEHOLDERS: &[&str] = &[
        "{root}",
        "{source}",
        "{source_rel}",
        "{output}",
        "{output_rel}",
    ];

    for spec in TOOL_RUNTIME_CONFORMANCE_SPECS {
        if spec.id.trim().is_empty() {
            errors.push("tool/runtime registry contains empty id".to_owned());
        } else if !ids.insert(spec.id) {
            errors.push(format!("duplicate tool/runtime id '{}'", spec.id));
        }
        if spec.tool_key.trim().is_empty() {
            errors.push(format!("tool/runtime '{}' has empty tool_key", spec.id));
        }
        if spec.fixture.source_relative.trim().is_empty() {
            errors.push(format!("tool/runtime '{}' has empty source path", spec.id));
        }
        match spec.fixture.kind {
            ToolRuntimeFixtureKind::File if spec.fixture.testdata_name.is_none() => {
                errors.push(format!(
                    "tool/runtime '{}' file fixture has no testdata_name",
                    spec.id
                ))
            }
            ToolRuntimeFixtureKind::GeneratedDirectory if spec.fixture.testdata_name.is_some() => {
                errors.push(format!(
                    "tool/runtime '{}' generated fixture must not name testdata",
                    spec.id
                ))
            }
            _ => {}
        }
        if spec.output_relative.trim().is_empty() {
            errors.push(format!("tool/runtime '{}' has empty output path", spec.id));
        }
        let Some(contract) = newengine_contract_registry::contract(spec.schema_contract_key) else {
            errors.push(format!(
                "tool/runtime '{}' references unregistered schema contract '{}'",
                spec.id, spec.schema_contract_key
            ));
            continue;
        };
        if contract.kind != ContractKind::Schema {
            errors.push(format!(
                "tool/runtime '{}' contract '{}' is kind '{}', expected schema",
                spec.id,
                contract.key,
                contract.kind.as_str()
            ));
        }
        if !schema_keys.insert(spec.schema_contract_key) {
            errors.push(format!(
                "schema contract '{}' has multiple tool/runtime producers",
                spec.schema_contract_key
            ));
        }
        let has_produce = spec
            .commands
            .iter()
            .any(|command| command.phase == ToolRuntimeCommandPhase::Produce);
        let has_validate = spec
            .commands
            .iter()
            .any(|command| command.phase == ToolRuntimeCommandPhase::Validate);
        if !has_produce {
            errors.push(format!("tool/runtime '{}' has no produce command", spec.id));
        }
        if !has_validate {
            errors.push(format!(
                "tool/runtime '{}' has no validate command",
                spec.id
            ));
        }
        match (
            spec.asset_manager_decode,
            spec.runtime_decode,
            spec.canonical_projection,
        ) {
            (Some(asset_manager), Some(runtime), Some(_)) => {
                if asset_manager.workspace != ConformanceWorkspace::AssetManager {
                    errors.push(format!(
                        "tool/runtime '{}' AssetManager decoder must use asset_manager workspace",
                        spec.id
                    ));
                }
                if asset_manager.package.trim().is_empty()
                    || asset_manager.example.trim().is_empty()
                    || asset_manager.output_kind.trim().is_empty()
                {
                    errors.push(format!(
                        "tool/runtime '{}' has incomplete AssetManager decode spec",
                        spec.id
                    ));
                }
                if runtime.package.trim().is_empty() || runtime.example.trim().is_empty() {
                    errors.push(format!(
                        "tool/runtime '{}' has incomplete runtime decode spec",
                        spec.id
                    ));
                }
            }
            (None, None, None) => {}
            _ => errors.push(format!(
                "tool/runtime '{}' DTO parity requires asset_manager_decode + runtime_decode + canonical_projection together",
                spec.id
            )),
        }
        for command in spec.commands {
            if command.args.is_empty() {
                errors.push(format!(
                    "tool/runtime '{}' phase '{}' has empty command",
                    spec.id,
                    command.phase.as_str()
                ));
            }
            for arg in command.args {
                let mut rest = *arg;
                while let Some(start) = rest.find('{') {
                    let Some(end_offset) = rest[start..].find('}') else {
                        errors.push(format!(
                            "tool/runtime '{}' has unterminated placeholder in '{}'",
                            spec.id, arg
                        ));
                        break;
                    };
                    let end = start + end_offset + 1;
                    let placeholder = &rest[start..end];
                    if !ALLOWED_PLACEHOLDERS.contains(&placeholder) {
                        errors.push(format!(
                            "tool/runtime '{}' uses unsupported placeholder '{}'",
                            spec.id, placeholder
                        ));
                    }
                    rest = &rest[end..];
                }
            }
        }
    }

    // P4 rule: every registered first-party asset Schema contract must have one
    // canonical producer fixture. NEF8 itself is Wire, so it is intentionally not
    // part of this schema-producer coverage set.
    for contract in newengine_contract_registry::contracts()
        .iter()
        .filter(|contract| {
            contract.kind == ContractKind::Schema && contract.key.starts_with("asset.")
        })
    {
        if !schema_keys.contains(contract.key) {
            errors.push(format!(
                "registered asset schema '{}' has no ToolRuntimeConformanceSpec",
                contract.key
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn validate_tool_runtime_artifact(
    spec: &ToolRuntimeConformanceSpec,
    bytes: &[u8],
) -> Result<super::ListFileContractConformance, Vec<String>> {
    let schema: ContractSpec = newengine_contract_registry::contract(spec.schema_contract_key)
        .copied()
        .ok_or_else(|| {
            vec![format!(
                "tool/runtime '{}' schema contract '{}' is not registered",
                spec.id, spec.schema_contract_key
            )]
        })?;
    super::validate_list_file_contract_with_read_compatibility(
        bytes,
        spec.content_kind,
        schema,
        spec.readable_legacy_schema_versions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_runtime_registry_is_well_formed_and_complete() {
        if let Err(errors) = validate_tool_runtime_registry() {
            panic!("tool/runtime registry invalid: {}", errors.join("; "));
        }
    }

    #[test]
    fn current_first_party_asset_schema_contracts_have_exactly_one_producer_spec() {
        let expected = newengine_contract_registry::contracts()
            .iter()
            .filter(|contract| {
                contract.kind == ContractKind::Schema && contract.key.starts_with("asset.")
            })
            .map(|contract| contract.key)
            .collect::<BTreeSet<_>>();
        let actual = TOOL_RUNTIME_CONFORMANCE_SPECS
            .iter()
            .map(|spec| spec.schema_contract_key)
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn p4_2_dto_parity_is_declared_for_all_first_party_formats() {
        let parity = TOOL_RUNTIME_CONFORMANCE_SPECS
            .iter()
            .filter(|spec| spec.canonical_projection.is_some())
            .map(|spec| spec.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            parity,
            BTreeSet::from(["nemat", "neui", "ydd", "ytd", "ytyp"])
        );
    }

    #[test]
    fn tool_runtime_lookup_is_stable_by_format_id() {
        for id in ["ytyp", "ydd", "ytd", "nemat", "neui"] {
            let spec = tool_runtime_conformance_spec(id).expect("registered tool/runtime spec");
            assert_eq!(spec.id, id);
            assert!(!spec.commands.is_empty());
        }
    }
}
