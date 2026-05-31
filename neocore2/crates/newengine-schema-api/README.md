# newengine-schema-api

Engine-facing schema/property/transaction contract for North Star Engine.

`engine.schema` owns reflection-style metadata for editor, asset, component,
settings and scripting surfaces. Providers still own concrete semantics; this
crate only defines DTOs and method names so consumers can render properties,
validate patches and build scripting bindings without hardcoded type branches.
