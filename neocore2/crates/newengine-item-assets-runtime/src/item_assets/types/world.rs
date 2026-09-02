#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredUseEffect {
    pub kind: String,
    pub amount: f32,
}

impl Default for AuthoredUseEffect {
    fn default() -> Self {
        Self {
            kind: "none".to_owned(),
            amount: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWorldItemDefinition {
    pub model: String,
    pub material_library: String,
    pub fallback_primitive: String,
    pub scale: [f32; 3],
    pub color_rgba: [f32; 4],
    pub pickup_half_extents: [f32; 3],
    pub respawn_seconds: f32,
}

impl Default for AuthoredWorldItemDefinition {
    fn default() -> Self {
        Self {
            model: String::new(),
            material_library: String::new(),
            fallback_primitive: "cube".to_owned(),
            scale: [0.2, 0.2, 0.2],
            color_rgba: [0.55, 0.60, 0.68, 1.0],
            pickup_half_extents: [0.2, 0.2, 0.2],
            respawn_seconds: 0.0,
        }
    }
}

impl AuthoredWorldItemDefinition {
    pub(super) fn compile(&self, kind: ItemKind) -> Result<WorldItemDefinition, String> {
        let fallback_primitive = match self.fallback_primitive.trim().to_ascii_lowercase().as_str()
        {
            "" | "cube" => primitive_builtins::ID_CUBE,
            "sphere" | "sphere_uv" => primitive_builtins::ID_SPHERE_UV,
            "cylinder" => primitive_builtins::ID_CYLINDER,
            "capsule" => primitive_builtins::ID_CAPSULE,
            "cone" => primitive_builtins::ID_CONE,
            "torus" => primitive_builtins::ID_TORUS,
            "disc" => primitive_builtins::ID_DISC,
            other => return Err(format!("unsupported world fallback primitive '{other}'")),
        };
        let mut definition = WorldItemDefinition::for_kind(kind);
        definition.model_ref =
            (!self.model.trim().is_empty()).then(|| self.model.trim().to_owned());
        definition.material_library_ref = (!self.material_library.trim().is_empty())
            .then(|| self.material_library.trim().to_owned());
        definition.fallback_primitive = fallback_primitive;
        definition.scale = self.scale;
        definition.color = self.color_rgba;
        definition.pickup_half_extents = self.pickup_half_extents;
        definition.respawn_seconds = self.respawn_seconds;
        Ok(definition.sanitized())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredLoadoutDefinition {
    pub id: String,
    pub display_name: String,
    pub clear_existing: bool,
    pub entries: Vec<AuthoredLoadoutEntry>,
}

impl Default for AuthoredLoadoutDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            clear_existing: true,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredLoadoutEntry {
    pub item: String,
    pub quantity: u32,
    pub equip: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledItemPackage {
    pub catalog: ItemCatalog,
    pub loadouts: InventoryLoadoutCatalog,
}
