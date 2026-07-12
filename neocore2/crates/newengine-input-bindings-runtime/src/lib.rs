#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted service for `engine.input.bindings`.

use abi_stable::std_types::{RResult, RString};
use newengine_input_actions_api::{
    InputActionDefinition, InputActionFrame, InputActionListenerRegistration, InputFrameSource,
};
use newengine_input_bindings_api::{
    InputBindingRegistration, InputBindingsManifest, InputBindingsProfile,
    InputBindingsServiceInfo, InputKeyRegistration, ENGINE_INPUT_BINDINGS_SERVICE_ID,
    INPUT_BINDINGS_BACKEND_CAPABILITY_ID, INPUT_BINDINGS_METHOD_ACTION_CATALOG_JSON_V1,
    INPUT_BINDINGS_METHOD_INVOKE, INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1,
    INPUT_BINDINGS_METHOD_PROFILE_JSON_V1, INPUT_BINDINGS_METHOD_REGISTER_ACTION_JSON_V1,
    INPUT_BINDINGS_METHOD_REGISTER_BINDING_JSON_V1, INPUT_BINDINGS_METHOD_REGISTER_KEY_JSON_V1,
    INPUT_BINDINGS_METHOD_REGISTER_LISTENER_JSON_V1,
    INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1, INPUT_BINDINGS_METHOD_RESET_PROFILE_JSON_V1,
    INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1, INPUT_BINDINGS_METHOD_SHUTDOWN_V1,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    decode_json_payload, engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

mod api;
mod persistence;
mod registration;
mod service;
mod state;

pub(crate) use api::mutate_profile_state_result;
pub use api::{
    input_bindings_profile_snapshot, register_input_action, register_input_binding,
    register_input_key, register_input_listener, register_input_manifest,
    reset_input_bindings_profile, resolve_input_actions, save_input_bindings_profile,
};
pub(crate) use persistence::{load_profile_from_config, profile_path, save_profile_to_config};
pub use registration::register_input_bindings_gateway_best_effort;
pub(crate) use service::{input_bindings_gateway_service, INPUT_BINDINGS_GATEWAY_OWNER};
pub(crate) use state::{
    gateway_state, install_or_update_default_profile, InputBindingsGatewayState,
    INPUT_BINDINGS_GATEWAY,
};

#[cfg(test)]
mod tests;
