#![allow(dead_code)]

use northstar_gui_editor_gateway::registry::ProviderDescriptor;
use northstar_gui_editor_assets::workspace::AssetRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorModel {
    pub provider_id: String,
    pub asset_label: String,
    pub schema: InspectorSchemaDto,
    pub transaction: InspectorTransactionDto,
    pub validation: ValidationReportDto,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorSchemaDto {
    pub id: String,
    pub title: String,
    pub sections: Vec<InspectorSectionDto>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorSectionDto {
    pub id: String,
    pub title: String,
    pub fields: Vec<PropertyFieldDto>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyFieldDto {
    pub id: String,
    pub label: String,
    pub value_kind: PropertyValueKind,
    pub editable: bool,
    pub source: PropertySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyValueKind {
    Text,
    Bool,
    Number,
    EnumChoice,
    Path,
    JsonTree,
    BinarySize,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertySource {
    AssetRef,
    ProviderManifest,
    ProviderSchema,
    Validation,
    RuntimeGenerated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorTransactionDto {
    pub undo_stack_depth: u32,
    pub redo_stack_depth: u32,
    pub dirty: bool,
    pub write_back_allowed: bool,
    pub write_capability: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReportDto {
    pub status: ValidationStatus,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStatus {
    NotRun,
    Passed,
    Warning,
    Failed,
}

impl InspectorModel {
    pub fn for_provider(provider: &ProviderDescriptor, asset: &AssetRef) -> Self {
        let write_capability = provider
            .capabilities
            .iter()
            .find(|capability| capability.ends_with(".write") || capability.contains("_pack") || capability.contains("write"))
            .cloned();

        let has_edit_schema = provider
            .capabilities
            .iter()
            .any(|capability| capability.contains("edit_schema") || capability.contains("editor."));

        let schema = InspectorSchemaDto {
            id: format!("inspector.{}", provider.id),
            title: format!("Inspector generated from {}", provider.id),
            sections: vec![
                InspectorSectionDto {
                    id: "asset".to_owned(),
                    title: "Asset".to_owned(),
                    fields: vec![
                        PropertyFieldDto::readonly("asset.logical_path", "Logical path", PropertyValueKind::Path, PropertySource::AssetRef),
                        PropertyFieldDto::readonly("asset.absolute_path", "Absolute path", PropertyValueKind::Path, PropertySource::AssetRef),
                        PropertyFieldDto::readonly("asset.extension", "Extension token", PropertyValueKind::Text, PropertySource::AssetRef),
                    ],
                },
                InspectorSectionDto {
                    id: "provider".to_owned(),
                    title: "Provider".to_owned(),
                    fields: vec![
                        PropertyFieldDto::readonly("provider.id", "Provider id", PropertyValueKind::Text, PropertySource::ProviderManifest),
                        PropertyFieldDto::readonly("provider.kind", "Provider kind", PropertyValueKind::Text, PropertySource::ProviderManifest),
                        PropertyFieldDto::readonly("provider.capabilities", "Capabilities", PropertyValueKind::JsonTree, PropertySource::ProviderManifest),
                    ],
                },
                InspectorSectionDto {
                    id: "schema".to_owned(),
                    title: "Edit Schema".to_owned(),
                    fields: vec![PropertyFieldDto {
                        id: "schema.generated".to_owned(),
                        label: "Provider-declared edit schema".to_owned(),
                        value_kind: PropertyValueKind::JsonTree,
                        editable: has_edit_schema,
                        source: PropertySource::ProviderSchema,
                    }],
                },
            ],
        };

        let validation = if provider
            .capabilities
            .iter()
            .any(|capability| capability.ends_with(".validate") || capability.contains("validation"))
        {
            ValidationReportDto {
                status: ValidationStatus::NotRun,
                messages: vec!["validation provider is available but has not been invoked in the shell".to_owned()],
            }
        } else {
            ValidationReportDto {
                status: ValidationStatus::Warning,
                messages: vec!["no validation capability declared by selected provider".to_owned()],
            }
        };

        Self {
            provider_id: provider.id.clone(),
            asset_label: asset.logical_path.display().to_string(),
            schema,
            transaction: InspectorTransactionDto {
                undo_stack_depth: 0,
                redo_stack_depth: 0,
                dirty: false,
                write_back_allowed: write_capability.is_some(),
                write_capability,
            },
            validation,
        }
    }
}

impl PropertyFieldDto {
    fn readonly(id: &str, label: &str, value_kind: PropertyValueKind, source: PropertySource) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
            value_kind,
            editable: false,
            source,
        }
    }
}
