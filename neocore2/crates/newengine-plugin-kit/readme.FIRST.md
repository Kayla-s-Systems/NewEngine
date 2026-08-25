Even your tools designer can do it!

This template layer exists to make North Star plugin development as small,
boring and repeatable as possible.

A simple North Star provider plugin must not require the author to hand-write
ABI glue, gateway descriptors, service registration boilerplate, build-resource
setup, or copy-pasted descriptor builders.

For normal plugins, the author should edit only 2 files:

1. plugin_definition.rs
2. the plugin behavior file, for example commands.rs, runtime.rs, service.rs,
   or module.rs depending on the plugin kind

The goal is the same as the old two-file plugin templates, but adapted to
North Star:

1. Define the plugin identity in plugin_definition.rs
2. Define the provided service/backend route/capability list in plugin_definition.rs
3. Customize the exported commands, service methods or provider behavior in the
   behavior file
4. Implement the associated functions/method handlers

The engine owns ABI.
The gateway owns routing.
The plugin kit owns descriptor/build boilerplate.
The plugin author owns behavior.

Follow these 4 steps:

//-----------------------------------------//
//-- STEP 1. DEFINE YOUR PLUGIN IDENTITY --//
//-----------------------------------------//

Define the plugin id, display name, version and kind in plugin_definition.rs.
The version should normally be env!("CARGO_PKG_VERSION") so Cargo remains the
single version source.

Example:

```rust
pub(crate) const PLUGIN_ID: &str = "engine.example.my_provider";
pub(crate) const PLUGIN_NAME: &str = "My Provider";

const PLUGIN_DEFINITION: PluginDefinition = PluginDefinition {
    id: PLUGIN_ID,
    name: PLUGIN_NAME,
    version: env!("CARGO_PKG_VERSION"),
    kind: PluginKind::Runtime,
    services: MY_SERVICES,
    backend_routes: MY_BACKEND_ROUTES,
    capabilities: MY_CAPABILITIES,
};
```

//-----------------------------------------------------//
//-- STEP 2. DEFINE SERVICES, ROUTES AND CAPABILITIES --//
//-----------------------------------------------------//

Describe what the plugin exposes or requires through the declarative helper
API from newengine-plugin-kit / newengine-plugin-api prelude.

Examples:

```rust
const MY_SERVICES: &[PluginServiceDefinition] = &[
    plugin_service(MY_SERVICE_ID, 1, SERVICE_DESCRIPTION_JSON),
];

const MY_BACKEND_ROUTES: &[PluginBackendRouteDefinition] = &[optional_backend_route(
    MY_BACKEND_CAPABILITY_ID,
    BackendServiceSpec::new(
        "example",
        ENGINE_EXAMPLE_GATEWAY_ID,
        MY_SERVICE_ID,
        MY_BACKEND_CAPABILITY_ID,
    ),
    Some(MY_PROVIDER_ROUTE_ID),
    Some("my_backend"),
    None,
    100,
    &["feature.write", "feature.query"],
    &[],
    &[],
)];

const MY_CAPABILITIES: &[PluginCapabilityDefinition] = &[
    provided_capability("example.write", CapabilityKind::Other, 1, ""),
];

// Optional extension contracts are an overlay; they do not modify the
// normative Engine Contract Registry or the PluginDefinition source layout.
const MY_CONTRACTS: &[PluginContractDefinition] = &[
    plugin_contract(
        "example.streaming.protocol",
        ContractKind::Protocol,
        ContractVersion::major(1),
        ContractCompatibility::SameMajor,
        Some("example.streaming/v1"),
    ),
];

fn descriptor() -> PluginDescriptor {
    PLUGIN_DEFINITION.with_contracts(MY_CONTRACTS).descriptor()
}
```

//----------------------------------------------//
//-- STEP 3. CUSTOMIZE PLUGIN BEHAVIOR SURFACE --//
//----------------------------------------------//

Keep behavior in the normal runtime/service/commands file.
Do not mix descriptor declaration with runtime behavior.

Good:

```text
plugin_definition.rs    -> identity, services, routes, capabilities, optional contracts
service.rs              -> ServiceV1 method handling
commands.rs             -> command handlers
device.rs/runtime.rs    -> provider-owned state and backend implementation
module.rs/lib.rs        -> thin PluginModule bridge
```

Bad:

```text
module.rs contains a giant PluginDescriptor::builder(...) block
module.rs repeats build/resource/ABI boilerplate
plugin author manually constructs every CapabilityDesc unless infrastructure
work truly requires it
```

//----------------------------------------------//
//-- STEP 4. IMPLEMENT THE ASSOCIATED HANDLERS --//
//----------------------------------------------//

Implement only the behavior that belongs to the plugin:

- command handlers
- service methods
- provider runtime state
- config parsing/validation
- diagnostics and domain logic

The template layer should keep the rest boring.

The target rule:

A simple provider/tool plugin should be creatable by editing no more than
2 files.

No plugin author should manually write descriptor glue unless they are building
infrastructure.

Practical North Star layout:

```text
src/
  plugin_definition.rs    # identity, services, routes, capabilities, optional contracts
  module.rs or lib.rs      # thin PluginModule bridge
  service.rs               # ServiceV1 implementation
  commands.rs              # optional command handlers
  runtime.rs               # optional provider state
  config.rs                # optional config schema
```

Current template helpers live in:

```text
NewEngine/neocore2/crates/newengine-plugin-kit/src/definition.rs
```

Provider crates import them through the existing plugin workspace alias:

```rust
use newengine_plugin_api::prelude::*;
```

Keep this file close to the template kit. If a new plugin cannot follow this
file, the template is leaking too much engine complexity.
