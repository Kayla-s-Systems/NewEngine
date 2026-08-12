#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.assets.definitions` runtime service.
//!
//! `.ytyp` ownership lives here. The service uses `engine.assets` only as the
//! VFS/raw-bytes owner and returns single-asset Properties DTOs to tools,
//! scene/map placement loaders and the asset graph resolver.
use std::collections::{BTreeMap, BTreeSet};

use abi_stable::std_types::{RResult, RString};
use newengine_assets::AssetServiceClient;
use newengine_assets_api::{
    definitions_method, stable_hash_from_text, AssetDecodeRequest, AssetDependencyRecord,
    AssetReference, ASSET_LIST_FILE_BODY_OUTPUT, DEFINITIONS_BACKEND_CAPABILITY_ID,
    DEFINITIONS_RUNTIME_CONTRACT, DEFINITIONS_SERVICE_ID, DEFINITIONS_SERVICE_METHODS,
    ENGINE_ASSETS_DEFINITIONS_SERVICE_ID, ENGINE_ASSETS_GRAPH_SERVICE_ID, ENGINE_ASSET_SERVICE_ID,
};
use newengine_authored_xml as authored_xml;
use newengine_model_domain_api::{MaterialBindingRef, MeshRenderOptions, MeshShadowPolicy};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};
use serde::{Deserialize, Serialize};

mod loading;
mod model;
mod parsing;
mod projection;
mod request;
mod service;

use loading::*;
pub use model::{
    DefinitionEntryV1, DefinitionIdentityV1, DefinitionManifestEntryV1, DefinitionManifestRequest,
    DefinitionManifestV1, DefinitionRefRequest, DefinitionRefResolutionV1, DefinitionRefsV1,
    DefinitionSideEffectV1, DefinitionValidationV1, DefinitionsServiceInfo, ModelExplanationV1,
};
use model::{RawDefinitionEntryV1, StableDiagnostic};
use parsing::*;
use projection::*;
use request::*;
pub use service::{
    definitions_gateway_service, definitions_service_info,
    register_definitions_gateway_best_effort, DefinitionsRuntimeState,
};

pub const DEFINITIONS_GATEWAY_OWNER: &str = "newengine-definitions-runtime.engine-runtime-provider";

#[cfg(test)]
mod tests;
