use std::sync::Arc;

use newengine_assets::AssetServiceClient;
use newengine_plugin_host::default_host_api;
use newengine_primitives::fnv1a_64;
use newengine_world_environment_api::authored_profile::{
    AuthoredTerrainHeightmapSpec, AuthoredTerrainSpec,
};

#[derive(Clone, Debug)]
pub struct TerrainHeightmapRuntime {
    pub width: u32,
    pub height: u32,
    samples: Arc<Vec<f32>>,
    spec: AuthoredTerrainHeightmapSpec,
    revision_key: u64,
}

impl TerrainHeightmapRuntime {
    #[inline]
    pub fn revision_key(&self) -> u64 {
        self.revision_key
    }

    pub fn apply_world_height(&self, world_x: f32, world_z: f32, procedural: f32) -> f32 {
        if self.width == 0 || self.height == 0 || self.samples.is_empty() {
            return procedural;
        }
        let u = repeat01(world_x * self.spec.tile_scale[0] + self.spec.tile_offset[0]);
        let v = repeat01(world_z * self.spec.tile_scale[1] + self.spec.tile_offset[1]);
        let mut h = self.sample_bilinear(u, v);
        if self.spec.invert {
            h = 1.0 - h;
        }
        let authored_height =
            self.spec.min_height + h * (self.spec.max_height - self.spec.min_height);
        match self.spec.mode.as_str() {
            "add" => procedural + authored_height * self.spec.strength,
            "replace" | "blend" => procedural + (authored_height - procedural) * self.spec.strength,
            _ => procedural,
        }
    }

    fn sample_bilinear(&self, u: f32, v: f32) -> f32 {
        let width = self.width as usize;
        let height = self.height as usize;
        if width == 0 || height == 0 {
            return 0.0;
        }
        let x = u * (width.saturating_sub(1) as f32);
        let y = v * (height.saturating_sub(1) as f32);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(width.saturating_sub(1));
        let y1 = (y0 + 1).min(height.saturating_sub(1));
        let tx = x - x0 as f32;
        let ty = y - y0 as f32;
        let h00 = self.sample_at(x0, y0);
        let h10 = self.sample_at(x1, y0);
        let h01 = self.sample_at(x0, y1);
        let h11 = self.sample_at(x1, y1);
        let hx0 = h00 + (h10 - h00) * tx;
        let hx1 = h01 + (h11 - h01) * tx;
        hx0 + (hx1 - hx0) * ty
    }

    #[inline]
    fn sample_at(&self, x: usize, y: usize) -> f32 {
        let width = self.width as usize;
        let i = y.saturating_mul(width).saturating_add(x);
        self.samples.get(i).copied().unwrap_or(0.0)
    }
}

pub fn load_terrain_heightmap(spec: &AuthoredTerrainSpec) -> Option<Arc<TerrainHeightmapRuntime>> {
    let heightmap = &spec.heightmap;
    if !heightmap.enabled {
        newengine_ulog_api::ulog::info!(
            "game-ready terrain heightmap: disabled source='{}' mode='{}' strength={} range=[{},{}] tile_scale=[{},{}] tile_offset=[{},{}] invert={} reason='profile_disabled' action='continue_procedural'",
            heightmap.source,
            heightmap.mode,
            heightmap.strength,
            heightmap.min_height,
            heightmap.max_height,
            heightmap.tile_scale[0],
            heightmap.tile_scale[1],
            heightmap.tile_offset[0],
            heightmap.tile_offset[1],
            heightmap.invert,
        );
        return None;
    }
    if !heightmap.source.to_ascii_lowercase().contains(".ytd@") {
        newengine_ulog_api::ulog::warn!(
            "game-ready terrain heightmap: rejected source='{}' expected='.ytd@entry' mode='{}' strength={} policy='heightmaps load through engine.assets.textures only' action='continue_procedural'",
            heightmap.source,
            heightmap.mode,
            heightmap.strength,
        );
        return None;
    }

    newengine_ulog_api::ulog::info!(
        "game-ready terrain heightmap: resolve begin source='{}' mode='{}' strength={} range=[{},{}] tile_scale=[{},{}] tile_offset=[{},{}] invert={} policy='engine.assets asset.decode_v1 + StarVault ytd format module'",
        heightmap.source,
        heightmap.mode,
        heightmap.strength,
        heightmap.min_height,
        heightmap.max_height,
        heightmap.tile_scale[0],
        heightmap.tile_scale[1],
        heightmap.tile_offset[0],
        heightmap.tile_offset[1],
        heightmap.invert,
    );

    let assets = AssetServiceClient::new(default_host_api());
    let texture = match assets.textures_entry_rgba8_ref_v1_typed(&heightmap.source) {
        Ok(texture) => texture,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready terrain heightmap: texture decode failed source='{}' err='{}' policy='engine.assets asset.decode_v1' action='continue_procedural'",
                heightmap.source,
                error
            );
            return None;
        }
    };

    match decode_rgba8_height_samples(texture.width, texture.height, &texture.rgba) {
        Ok((width, height, samples)) => {
            let revision_key = terrain_heightmap_revision_key(heightmap, width, height, &samples);
            let sample_count = samples.len();
            newengine_ulog_api::ulog::info!(
                "game-ready terrain heightmap: resolved source='{}' size={}x{} samples={} revision_key={} mode='{}' strength={} range=[{},{}] tile_scale=[{},{}] tile_offset=[{},{}] invert={} policy='engine.assets.asset.decode_v1/ytd.entry_rgba8_v1'",
                heightmap.source,
                width,
                height,
                sample_count,
                revision_key,
                heightmap.mode,
                heightmap.strength,
                heightmap.min_height,
                heightmap.max_height,
                heightmap.tile_scale[0],
                heightmap.tile_scale[1],
                heightmap.tile_offset[0],
                heightmap.tile_offset[1],
                heightmap.invert,
            );
            Some(Arc::new(TerrainHeightmapRuntime {
                width,
                height,
                samples: Arc::new(samples),
                spec: heightmap.clone(),
                revision_key,
            }))
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready terrain heightmap: decode failed source='{}' err='{}' action='continue_procedural'",
                heightmap.source,
                e
            );
            None
        }
    }
}

fn decode_rgba8_height_samples(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(u32, u32, Vec<f32>), String> {
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "heightmap dimensions overflow".to_owned())?;
    if rgba.len() != expected_len {
        return Err(format!(
            "RGBA payload length mismatch got={} expected={} size={}x{}",
            rgba.len(),
            expected_len,
            width,
            height
        ));
    }
    let mut samples = Vec::with_capacity((width as usize).saturating_mul(height as usize));
    for px in rgba.as_chunks::<4>().0 {
        let luma =
            (0.2126 * f32::from(px[0]) + 0.7152 * f32::from(px[1]) + 0.0722 * f32::from(px[2]))
                / 255.0;
        samples.push(luma.clamp(0.0, 1.0));
    }
    Ok((width, height, samples))
}

#[inline]
fn repeat01(value: f32) -> f32 {
    let value = value.fract();
    if value < 0.0 {
        value + 1.0
    } else {
        value
    }
}

fn terrain_heightmap_revision_key(
    spec: &AuthoredTerrainHeightmapSpec,
    width: u32,
    height: u32,
    samples: &[f32],
) -> u64 {
    let mut h = fnv1a_64(&spec.source);
    h = mix_heightmap_key(h, u64::from(width));
    h = mix_heightmap_key(h, u64::from(height));
    h = mix_heightmap_key(h, spec.strength.to_bits() as u64);
    h = mix_heightmap_key(h, spec.min_height.to_bits() as u64);
    h = mix_heightmap_key(h, spec.max_height.to_bits() as u64);
    h = mix_heightmap_key(h, spec.tile_scale[0].to_bits() as u64);
    h = mix_heightmap_key(h, spec.tile_scale[1].to_bits() as u64);
    h = mix_heightmap_key(h, spec.invert as u64);
    if let Some(first) = samples.first() {
        h = mix_heightmap_key(h, first.to_bits() as u64);
    }
    if let Some(last) = samples.last() {
        h = mix_heightmap_key(h, last.to_bits() as u64);
    }
    h
}

#[inline]
fn mix_heightmap_key(mut h: u64, v: u64) -> u64 {
    h ^= v
        .wrapping_add(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(h << 6)
        .wrapping_add(h >> 2);
    h
}
