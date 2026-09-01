use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioClipRef {
    /// VFS logical path resolved by `engine.assets`. Physical filesystem paths are
    /// deliberately outside the audio provider contract.
    pub uri: String,
}

impl AudioClipRef {
    #[inline]
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioAttenuationCurve {
    Linear,
    Smoothstep,
    #[default]
    Inverse,
    Exponential,
    Custom,
}

/// Authored distance attenuation policy. Distances are engine world units and
/// custom points use normalized `[distance_fraction, gain]` coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioAttenuationSettings {
    pub min_distance: f32,
    pub max_distance: f32,
    pub curve: AudioAttenuationCurve,
    pub rolloff: f32,
    pub curve_points: Vec<[f32; 2]>,
}

impl Default for AudioAttenuationSettings {
    fn default() -> Self {
        Self {
            min_distance: 1.0,
            max_distance: 50.0,
            curve: AudioAttenuationCurve::Inverse,
            rolloff: 1.0,
            curve_points: Vec::new(),
        }
    }
}

impl AudioAttenuationSettings {
    pub fn sanitized(mut self) -> Self {
        self.min_distance = if self.min_distance.is_finite() {
            self.min_distance.clamp(0.0, 1_000_000.0)
        } else {
            1.0
        };
        self.max_distance = if self.max_distance.is_finite() {
            self.max_distance.clamp(0.01, 1_000_000.0)
        } else {
            50.0
        };
        if self.max_distance <= self.min_distance {
            self.max_distance = (self.min_distance + 0.01).min(1_000_000.0);
            if self.max_distance <= self.min_distance {
                self.min_distance = (self.max_distance - 0.01).max(0.0);
            }
        }
        self.rolloff = if self.rolloff.is_finite() {
            self.rolloff.clamp(0.1, 8.0)
        } else {
            1.0
        };
        self.curve_points
            .retain(|point| point[0].is_finite() && point[1].is_finite());
        for point in &mut self.curve_points {
            point[0] = point[0].clamp(0.0, 1.0);
            point[1] = point[1].clamp(0.0, 1.0);
        }
        self.curve_points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        self.curve_points
            .dedup_by(|a, b| (a[0] - b[0]).abs() <= 1.0e-6);
        if self.curve == AudioAttenuationCurve::Custom {
            if self.curve_points.first().is_none_or(|point| point[0] > 0.0) {
                self.curve_points.insert(0, [0.0, 1.0]);
            }
            if self.curve_points.last().is_none_or(|point| point[0] < 1.0) {
                self.curve_points.push([1.0, 0.0]);
            }
        }
        self
    }

    #[inline]
    pub fn gain_at_distance(&self, distance: f32) -> f32 {
        let min_distance = if self.min_distance.is_finite() {
            self.min_distance.max(0.0)
        } else {
            1.0
        };
        let mut max_distance = if self.max_distance.is_finite() {
            self.max_distance.max(0.01)
        } else {
            50.0
        };
        if max_distance <= min_distance {
            max_distance = min_distance + 0.01;
        }
        let rolloff = if self.rolloff.is_finite() {
            self.rolloff.clamp(0.1, 8.0)
        } else {
            1.0
        };
        let distance = if distance.is_finite() {
            distance.max(0.0)
        } else {
            max_distance
        };
        if distance <= min_distance {
            return 1.0;
        }
        if distance >= max_distance {
            return 0.0;
        }
        let t = ((distance - min_distance) / (max_distance - min_distance)).clamp(0.0, 1.0);
        match self.curve {
            AudioAttenuationCurve::Linear => 1.0 - t,
            AudioAttenuationCurve::Smoothstep => 1.0 - t * t * (3.0 - 2.0 * t),
            AudioAttenuationCurve::Exponential => (1.0 - t).powf(rolloff),
            AudioAttenuationCurve::Inverse => {
                let scale = 4.0 * rolloff;
                let raw = 1.0 / (1.0 + scale * t);
                let end = 1.0 / (1.0 + scale);
                ((raw - end) / (1.0 - end)).clamp(0.0, 1.0)
            }
            AudioAttenuationCurve::Custom => sample_custom_attenuation(&self.curve_points, t),
        }
    }
}

fn sample_custom_attenuation(points: &[[f32; 2]], t: f32) -> f32 {
    let Some(first) = points.first() else {
        return 1.0 - t;
    };
    if t <= first[0] {
        return first[1];
    }
    for pair in points.windows(2) {
        let a = pair[0];
        let b = pair[1];
        if t <= b[0] {
            let width = (b[0] - a[0]).max(1.0e-6);
            let local = ((t - a[0]) / width).clamp(0.0, 1.0);
            return (a[1] + (b[1] - a[1]) * local).clamp(0.0, 1.0);
        }
    }
    points.last().map(|point| point[1]).unwrap_or(0.0)
}

/// Material-domain acoustic response resolved from a blocker surface. The physics
/// backend never owns this data; engine/audio semantics map stable surface ids to
/// these coefficients after a provider-neutral query hit is returned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AcousticMaterialProfile {
    /// Broadband energy transmitted through the material when a ray is blocked.
    pub transmission_gain: f32,
    /// Broadband energy returned by a visible first-order reflection. Transmission, reflection
    /// and absorption are independent authored channels; runtime never derives this as `1-T`.
    pub reflection_gain: f32,
    /// Fraction of high-frequency energy absorbed by the material in `[0,1]`.
    pub high_frequency_absorption: f32,
    /// Nominal low-pass cutoff for a fully blocked path.
    pub low_pass_hz: f32,
}

impl Default for AcousticMaterialProfile {
    #[inline]
    fn default() -> Self {
        Self {
            transmission_gain: 0.35,
            reflection_gain: 0.50,
            high_frequency_absorption: 0.65,
            low_pass_hz: 3_500.0,
        }
    }
}

impl AcousticMaterialProfile {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            transmission_gain: finite_clamped(self.transmission_gain, 0.35, 0.0, 1.0),
            reflection_gain: finite_clamped(self.reflection_gain, 0.50, 0.0, 1.0),
            high_frequency_absorption: finite_clamped(
                self.high_frequency_absorption,
                0.65,
                0.0,
                1.0,
            ),
            low_pass_hz: finite_clamped(self.low_pass_hz, 3_500.0, 80.0, 20_000.0),
        }
    }

    #[inline]
    pub fn high_frequency_gain(self) -> f32 {
        1.0 - self.sanitized().high_frequency_absorption
    }

    /// Acoustically transparent propagation fallback. This is deliberately distinct from
    /// `Default`: an unmapped physics surface must not invent a concrete wall response.
    #[inline]
    pub const fn transparent() -> Self {
        Self {
            transmission_gain: 1.0,
            reflection_gain: 0.0,
            high_frequency_absorption: 0.0,
            low_pass_hz: 20_000.0,
        }
    }
}

pub const ACOUSTIC_MATERIAL_LIBRARY_SCHEMA: &str = "newengine.audio.acoustic-material-library.v2";
pub const ACOUSTIC_MATERIAL_LIBRARY_VERSION: u32 = 2;

/// One authored mapping rule between provider-neutral `PhysicsSurface.id` semantics and
/// an audio-domain material response. Concrete names/presets belong to Shared or projects;
/// the engine only performs deterministic rule matching.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AcousticMaterialRule {
    pub material_id: String,
    pub surface_matches: Vec<String>,
    pub profile: AcousticMaterialProfile,
}

impl AcousticMaterialRule {
    pub fn sanitized(mut self) -> Self {
        self.material_id = self.material_id.trim().to_owned();
        self.surface_matches = self
            .surface_matches
            .into_iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect();
        self.surface_matches.sort();
        self.surface_matches.dedup();
        self.profile = self.profile.sanitized();
        self
    }
}

/// Data-authored acoustic material library installed as an ECS world resource by the
/// active product/profile composition. Project libraries can replace/extend Shared without
/// teaching engine-runtime concrete material names.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AcousticMaterialLibrary {
    pub schema: String,
    pub version: u32,
    pub rules: Vec<AcousticMaterialRule>,
}

impl Default for AcousticMaterialLibrary {
    fn default() -> Self {
        Self {
            schema: ACOUSTIC_MATERIAL_LIBRARY_SCHEMA.to_owned(),
            version: ACOUSTIC_MATERIAL_LIBRARY_VERSION,
            rules: Vec::new(),
        }
    }
}

impl AcousticMaterialLibrary {
    pub fn new(rules: Vec<AcousticMaterialRule>) -> Self {
        Self {
            rules,
            ..Self::default()
        }
        .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.schema = ACOUSTIC_MATERIAL_LIBRARY_SCHEMA.to_owned();
        self.version = ACOUSTIC_MATERIAL_LIBRARY_VERSION;
        self.rules = self
            .rules
            .into_iter()
            .map(AcousticMaterialRule::sanitized)
            .filter(|rule| !rule.material_id.is_empty() && !rule.surface_matches.is_empty())
            .collect();
        self.rules.sort_by(|a, b| a.material_id.cmp(&b.material_id));
        self
    }

    /// Resolve the strongest authored match. Longer match expressions win; material id is
    /// the deterministic tie breaker. Unknown surfaces intentionally return `None`.
    pub fn resolve(&self, surface_id: &str) -> Option<AcousticSurface> {
        let surface = surface_id.trim().to_ascii_lowercase();
        if surface.is_empty() {
            return None;
        }
        let mut best: Option<(usize, &AcousticMaterialRule)> = None;
        for rule in &self.rules {
            for pattern in &rule.surface_matches {
                if !surface.contains(pattern) {
                    continue;
                }
                let replace = best.is_none_or(|(best_len, best_rule)| {
                    pattern.len() > best_len
                        || (pattern.len() == best_len && rule.material_id < best_rule.material_id)
                });
                if replace {
                    best = Some((pattern.len(), rule));
                }
            }
        }
        best.map(|(_, rule)| AcousticSurface::new(rule.material_id.clone(), rule.profile))
    }
}

/// Durable authored acoustic material override attached to a collidable ECS entity.
/// The physics backend remains unaware of this component; engine-runtime resolves it
/// from the stable blocker entity returned by `engine.physics`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AcousticSurface {
    pub material_id: String,
    pub profile: AcousticMaterialProfile,
}

impl Default for AcousticSurface {
    fn default() -> Self {
        Self {
            material_id: "material.default".to_owned(),
            profile: AcousticMaterialProfile::default(),
        }
    }
}

impl AcousticSurface {
    pub fn new(material_id: impl Into<String>, profile: AcousticMaterialProfile) -> Self {
        Self {
            material_id: material_id.into(),
            profile,
        }
        .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.material_id = self.material_id.trim().to_owned();
        if self.material_id.is_empty() {
            self.material_id = "material.default".to_owned();
        }
        self.profile = self.profile.sanitized();
        self
    }
}

/// Authored spatial occlusion policy for ECS-owned audio emitters. The physics
/// transport stays provider-neutral: engine-runtime turns this policy into a
/// bounded batch of `PhysicsQueryDto::Ray` probes through `engine.physics`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioOcclusionSettings {
    pub enabled: bool,
    pub max_distance: f32,
    pub ray_count: u8,
    pub probe_radius: f32,
    pub obstruction_gain: f32,
    pub occlusion_gain: f32,
    pub attack_seconds: f32,
    pub release_seconds: f32,
}

impl Default for AudioOcclusionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_distance: 80.0,
            ray_count: 3,
            probe_radius: 0.35,
            obstruction_gain: 0.65,
            occlusion_gain: 0.22,
            attack_seconds: 0.06,
            release_seconds: 0.22,
        }
    }
}

impl AudioOcclusionSettings {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            enabled: self.enabled,
            max_distance: finite_clamped(self.max_distance, 80.0, 0.5, 10_000.0),
            ray_count: self.ray_count.clamp(1, 5),
            probe_radius: finite_clamped(self.probe_radius, 0.35, 0.0, 4.0),
            obstruction_gain: finite_clamped(self.obstruction_gain, 0.65, 0.0, 1.0),
            occlusion_gain: finite_clamped(self.occlusion_gain, 0.22, 0.0, 1.0),
            attack_seconds: finite_clamped(self.attack_seconds, 0.06, 0.005, 5.0),
            release_seconds: finite_clamped(self.release_seconds, 0.22, 0.005, 5.0),
        }
    }

    #[inline]
    pub fn transmission_gain(self, obstruction: f32, occlusion: f32) -> f32 {
        let settings = self.sanitized();
        let obstruction = finite_clamped(obstruction, 0.0, 0.0, 1.0);
        let occlusion = finite_clamped(occlusion, 0.0, 0.0, 1.0);
        let obstructed = 1.0 - obstruction * (1.0 - settings.obstruction_gain);
        (obstructed + (settings.occlusion_gain - obstructed) * occlusion).clamp(0.0, 1.0)
    }

    #[inline]
    pub fn acoustic_state(self, obstruction: f32, occlusion: f32) -> AudioAcousticState {
        self.acoustic_state_with_material(
            obstruction,
            occlusion,
            AcousticMaterialProfile::transparent(),
        )
    }

    /// Combines geometric blockage with the material-domain spectral response.
    /// Clear rays contribute unity; blocked rays contribute the material profile.
    #[inline]
    pub fn acoustic_state_with_material(
        self,
        obstruction: f32,
        occlusion: f32,
        material: AcousticMaterialProfile,
    ) -> AudioAcousticState {
        let obstruction = finite_clamped(obstruction, 0.0, 0.0, 1.0);
        let occlusion = finite_clamped(occlusion, 0.0, 0.0, 1.0);
        let material = material.sanitized();
        let geometry_gain = self.transmission_gain(obstruction, occlusion);
        let material_gain = lerp(1.0, material.transmission_gain, obstruction);
        let spectral_weight = obstruction.max(occlusion);
        AudioAcousticState {
            obstruction,
            occlusion,
            transmission_gain: (geometry_gain * material_gain).clamp(0.0, 1.0),
            high_frequency_gain: lerp(1.0, material.high_frequency_gain(), spectral_weight),
            low_pass_hz: lerp(20_000.0, material.low_pass_hz, spectral_weight),
        }
        .sanitized()
    }
}

/// Smoothed acoustic result applied to a logical voice after distance attenuation.
/// `transmission_gain` is part of audibility ranking, while `high_frequency_gain`
/// and `low_pass_hz` drive the provider-neutral spectral transmission controls.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioAcousticState {
    pub obstruction: f32,
    pub occlusion: f32,
    pub transmission_gain: f32,
    pub high_frequency_gain: f32,
    pub low_pass_hz: f32,
}

impl Default for AudioAcousticState {
    #[inline]
    fn default() -> Self {
        Self::clear()
    }
}

impl AudioAcousticState {
    #[inline]
    pub const fn clear() -> Self {
        Self {
            obstruction: 0.0,
            occlusion: 0.0,
            transmission_gain: 1.0,
            high_frequency_gain: 1.0,
            low_pass_hz: 20_000.0,
        }
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            obstruction: finite_clamped(self.obstruction, 0.0, 0.0, 1.0),
            occlusion: finite_clamped(self.occlusion, 0.0, 0.0, 1.0),
            transmission_gain: finite_clamped(self.transmission_gain, 1.0, 0.0, 1.0),
            high_frequency_gain: finite_clamped(self.high_frequency_gain, 1.0, 0.0, 1.0),
            low_pass_hz: finite_clamped(self.low_pass_hz, 20_000.0, 80.0, 20_000.0),
        }
    }

    pub fn smoothed_towards(
        self,
        target: Self,
        dt: f32,
        attack_seconds: f32,
        release_seconds: f32,
    ) -> Self {
        let current = self.sanitized();
        let target = target.sanitized();
        let dt = finite_clamped(dt, 1.0 / 60.0, 0.0, 0.25);
        let closing = target.transmission_gain < current.transmission_gain
            || target.high_frequency_gain < current.high_frequency_gain
            || target.low_pass_hz < current.low_pass_hz;
        let time = if closing {
            finite_clamped(attack_seconds, 0.06, 0.005, 5.0)
        } else {
            finite_clamped(release_seconds, 0.22, 0.005, 5.0)
        };
        let alpha = if dt <= 0.0 {
            0.0
        } else {
            1.0 - (-dt / time).exp()
        };
        Self {
            obstruction: lerp(current.obstruction, target.obstruction, alpha),
            occlusion: lerp(current.occlusion, target.occlusion, alpha),
            transmission_gain: lerp(current.transmission_gain, target.transmission_gain, alpha),
            high_frequency_gain: lerp(
                current.high_frequency_gain,
                target.high_frequency_gain,
                alpha,
            ),
            low_pass_hz: lerp(current.low_pass_hz, target.low_pass_hz, alpha),
        }
        .sanitized()
    }
}
