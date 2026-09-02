use super::*;

impl GameData {
    /// Validates only project-authored GameData. Character model/locomotion fields are deliberately
    /// excluded: they are runtime-resolved from the selected character YTYP and are not serialized.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != GAME_DATA_SCHEMA {
            return Err(format!(
                "unsupported game-data schema '{}' expected '{}'",
                self.schema, GAME_DATA_SCHEMA
            ));
        }
        if self.version != GAME_DATA_VERSION {
            return Err(format!(
                "unsupported game-data version {} expected {}",
                self.version, GAME_DATA_VERSION
            ));
        }
        self.audio
            .mix_graph
            .validate()
            .map_err(|error| format!("audio.mix_graph invalid: {error}"))?;
        if self.audio.mix_graph.buses.is_empty() {
            return Err(
                "audio.mix_graph must declare at least one project-authored route".to_owned(),
            );
        }
        if self.runtime.fixed_dt_ms == 0 {
            return Err("runtime.fixed_dt_ms must be greater than zero".to_owned());
        }
        for (name, value) in [
            ("runtime.app_name", self.runtime.app_name.as_str()),
            ("runtime.app_dir_name", self.runtime.app_dir_name.as_str()),
            ("runtime.window_title", self.runtime.window_title.as_str()),
            (
                "runtime.default_profile_asset",
                self.runtime.default_profile_asset.as_str(),
            ),
            ("player.character_ref", self.player.character_ref.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{name} must be project-authored and non-empty"));
            }
        }
        if !self
            .player
            .character_ref
            .to_ascii_lowercase()
            .contains(".ytyp@")
        {
            return Err(format!(
                "player.character_ref '{}' must be a selector-qualified .ytyp reference",
                self.player.character_ref
            ));
        }
        if self.player.spawn.iter().any(|value| !value.is_finite())
            || !self.player.yaw.is_finite()
            || !self.player.look_sensitivity.is_finite()
            || self.player.look_sensitivity <= 0.0
        {
            return Err("player authored spawn/yaw/look_sensitivity must be finite and look_sensitivity > 0".to_owned());
        }

        if self.world.terrain.enabled {
            if self.world.terrain.cells == 0
                || !self.world.terrain.size.is_finite()
                || self.world.terrain.size <= 0.0
                || !self.world.terrain.base_height.is_finite()
                || !self.world.terrain.height_scale.is_finite()
            {
                return Err(
                    "enabled world.terrain requires positive cells/size and finite height values"
                        .to_owned(),
                );
            }
            if self.world.terrain.generator.id.trim().is_empty() {
                return Err(
                    "enabled world.terrain requires project-authored generator.id".to_owned(),
                );
            }
        }

        if !self.world.sky.radius.is_finite() || self.world.sky.radius <= 0.0 {
            return Err("world.sky.radius must be finite and greater than zero".to_owned());
        }
        if self.world.sky.definition_ref.trim().is_empty() {
            return Err("world.sky.definition_ref must be project-authored".to_owned());
        }

        let mut finite = Vec::new();
        finite.extend_from_slice(&self.world.lighting.ambient_color);
        finite.push(self.world.lighting.ambient_intensity);
        finite.extend_from_slice(&self.world.lighting.sun_direction);
        finite.extend_from_slice(&self.world.lighting.sun_color);
        finite.push(self.world.lighting.sun_intensity);
        finite.push(self.world.shadows.max_distance);
        finite.push(self.world.shadows.softness);
        finite.push(self.world.shadows.bias);
        finite.push(self.world.shadows.normal_bias);
        finite.push(self.world.shadows.contact_strength);
        finite.push(self.world.day_night.time_of_day_hours);
        finite.push(self.world.day_night.day_length_seconds);
        finite.push(self.world.day_night.latitude_degrees);
        finite.push(self.world.day_night.axial_tilt_degrees);
        finite.push(self.gameplay.projectile.radius);
        finite.push(self.gameplay.projectile.speed);
        finite.push(self.gameplay.projectile.lifetime_seconds);
        finite.push(self.gameplay.projectile.spawn_clearance);
        finite.push(self.gameplay.projectile.restitution);
        finite.push(self.gameplay.projectile.friction);
        finite.push(self.gameplay.projectile.density);
        finite.extend_from_slice(&self.gameplay.projectile.angular_velocity);
        finite.extend_from_slice(&self.gameplay.projectile.color);
        if finite.iter().any(|value| !value.is_finite()) {
            return Err("game-data contains non-finite project-authored numeric values".to_owned());
        }
        if self.gameplay.projectile.radius <= 0.0
            || self.gameplay.projectile.speed < 0.0
            || self.gameplay.projectile.lifetime_seconds <= 0.0
        {
            return Err(
                "gameplay.projectile requires radius/lifetime > 0 and speed >= 0".to_owned(),
            );
        }
        if !(1..=256).contains(&self.gameplay.inventory.hud_slots) {
            return Err("gameplay.inventory.hud_slots must be in [1, 256]".to_owned());
        }
        match self
            .world
            .shadows
            .filter
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "hard" | "none" | "pcf" | "pcss" => {}
            other => {
                return Err(format!(
                    "world.shadows.filter '{}' is not supported; expected hard/none/pcf/pcss",
                    other
                ));
            }
        }
        Ok(())
    }
}

/// `Default` is an intentionally invalid, project-neutral sentinel. It exists for Rust struct
/// construction/tests only and must never be used as a shipping game profile.
impl Default for GameData {
    fn default() -> Self {
        Self {
            schema: GAME_DATA_SCHEMA.to_owned(),
            version: GAME_DATA_VERSION,
            runtime: RuntimeData::default(),
            audio: AudioProjectData::default(),
            world: WorldData::default(),
            player: PlayerData::default(),
            gameplay: GameplayData::default(),
        }
    }
}
