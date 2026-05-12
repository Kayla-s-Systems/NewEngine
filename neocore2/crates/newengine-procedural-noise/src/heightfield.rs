#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use newengine_bounds::Aabb;
use newengine_math::Vec3;

use crate::graph::NoiseGraph2D;
use crate::noise::{FractalNoise2D, ValueNoise2D};

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TerrainHeightfieldSettings {
    /// Number of quads along local X.
    pub cells_x: u32,
    /// Number of quads along local Z.
    pub cells_z: u32,
    pub size_x: f32,
    pub size_z: f32,
    pub base_height: f32,
    pub height_scale: f32,
    pub noise: FractalNoise2D,
}

impl Default for TerrainHeightfieldSettings {
    #[inline]
    fn default() -> Self {
        Self {
            cells_x: 96,
            cells_z: 96,
            size_x: 44.0,
            size_z: 44.0,
            base_height: 0.0,
            height_scale: 3.4,
            noise: FractalNoise2D::default(),
        }
    }
}

impl TerrainHeightfieldSettings {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            cells_x: self.cells_x.clamp(2, 512),
            cells_z: self.cells_z.clamp(2, 512),
            size_x: finite_or(self.size_x, 44.0).abs().max(1.0),
            size_z: finite_or(self.size_z, 44.0).abs().max(1.0),
            base_height: finite_or(self.base_height, 0.0),
            height_scale: finite_or(self.height_scale, 3.4).abs(),
            noise: self.noise.sanitized(),
        }
    }

    #[inline]
    pub fn vertex_count_x(self) -> usize {
        self.sanitized().cells_x as usize + 1
    }

    #[inline]
    pub fn vertex_count_z(self) -> usize {
        self.sanitized().cells_z as usize + 1
    }
}

/// Fully declarative heightfield source.
///
/// `TerrainHeightfieldSettings` remains as the compact backwards-compatible
/// fractal form. New code should prefer this descriptor when the terrain is
/// assembled from multiple layers, cellular edges, domain warps, or texture-like
/// masks.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TerrainHeightfieldDescriptor {
    pub cells_x: u32,
    pub cells_z: u32,
    pub size_x: f32,
    pub size_z: f32,
    pub base_height: f32,
    pub height_scale: f32,
    pub graph: NoiseGraph2D,
    /// Number of deterministic post-filter smoothing passes applied to generated heights.
    #[cfg_attr(feature = "serde", serde(default))]
    pub smoothing_passes: u32,
    /// Blend factor per smoothing pass. 0 keeps raw noise, 1 fully averages neighbours.
    #[cfg_attr(feature = "serde", serde(default))]
    pub smoothing_strength: f32,
}

impl Default for TerrainHeightfieldDescriptor {
    #[inline]
    fn default() -> Self {
        TerrainHeightfieldSettings::default().into()
    }
}

impl From<TerrainHeightfieldSettings> for TerrainHeightfieldDescriptor {
    #[inline]
    fn from(settings: TerrainHeightfieldSettings) -> Self {
        let settings = settings.sanitized();
        Self {
            cells_x: settings.cells_x,
            cells_z: settings.cells_z,
            size_x: settings.size_x,
            size_z: settings.size_z,
            base_height: settings.base_height,
            height_scale: settings.height_scale,
            graph: NoiseGraph2D::from_fractal(settings.noise),
            smoothing_passes: 0,
            smoothing_strength: 0.0,
        }
    }
}

impl TerrainHeightfieldDescriptor {
    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.cells_x = self.cells_x.clamp(2, 512);
        self.cells_z = self.cells_z.clamp(2, 512);
        self.size_x = finite_or(self.size_x, 44.0).abs().max(1.0);
        self.size_z = finite_or(self.size_z, 44.0).abs().max(1.0);
        self.base_height = finite_or(self.base_height, 0.0);
        self.height_scale = finite_or(self.height_scale, 3.4).abs();
        self.smoothing_passes = self.smoothing_passes.min(16);
        self.smoothing_strength = finite_or(self.smoothing_strength, 0.0).clamp(0.0, 1.0);
        self
    }

    #[inline]
    pub fn compact_settings(&self) -> TerrainHeightfieldSettings {
        TerrainHeightfieldSettings {
            cells_x: self.cells_x,
            cells_z: self.cells_z,
            size_x: self.size_x,
            size_z: self.size_z,
            base_height: self.base_height,
            height_scale: self.height_scale,
            noise: FractalNoise2D::default(),
        }
        .sanitized()
    }
}

#[derive(Clone, Debug)]
pub struct HeightField {
    settings: TerrainHeightfieldSettings,
    heights: Vec<f32>,
    min_height: f32,
    max_height: f32,
    revision_key: u64,
}

impl HeightField {
    pub fn generate(settings: TerrainHeightfieldSettings) -> Self {
        let settings = settings.sanitized();
        let noise = ValueNoise2D::new(settings.noise);
        Self::generate_impl(settings, settings.noise.seed, 0, 0.0, |local_x, local_z| {
            noise.sample(local_x, local_z)
        })
    }

    pub fn generate_descriptor(descriptor: TerrainHeightfieldDescriptor) -> Self {
        let descriptor = descriptor.sanitized();
        let settings = descriptor.compact_settings();
        let graph = descriptor.graph.clone();
        let graph_key = graph.revision_key();
        Self::generate_impl(
            settings,
            graph_key,
            descriptor.smoothing_passes,
            descriptor.smoothing_strength,
            move |local_x, local_z| graph.sample(local_x, local_z),
        )
    }

    pub fn generate_with_graph(settings: TerrainHeightfieldSettings, graph: NoiseGraph2D) -> Self {
        Self::generate_descriptor(TerrainHeightfieldDescriptor {
            graph,
            ..TerrainHeightfieldDescriptor::from(settings)
        })
    }

    fn generate_impl(
        settings: TerrainHeightfieldSettings,
        source_key: u64,
        smoothing_passes: u32,
        smoothing_strength: f32,
        mut sample: impl FnMut(f32, f32) -> f32,
    ) -> Self {
        let settings = settings.sanitized();
        let vx = settings.vertex_count_x();
        let vz = settings.vertex_count_z();
        let mut heights = Vec::with_capacity(vx * vz);
        let mut min_height = f32::INFINITY;
        let mut max_height = f32::NEG_INFINITY;

        for z in 0..vz {
            for x in 0..vx {
                let u = x as f32 / settings.cells_x as f32;
                let v = z as f32 / settings.cells_z as f32;
                let local_x = (u - 0.5) * settings.size_x;
                let local_z = (v - 0.5) * settings.size_z;
                let h = settings.base_height + sample(local_x, local_z) * settings.height_scale;
                min_height = min_height.min(h);
                max_height = max_height.max(h);
                heights.push(h);
            }
        }

        apply_height_smoothing(
            &mut heights,
            vx,
            vz,
            smoothing_passes.min(16),
            finite_or(smoothing_strength, 0.0).clamp(0.0, 1.0),
        );

        min_height = f32::INFINITY;
        max_height = f32::NEG_INFINITY;
        for h in &heights {
            min_height = min_height.min(*h);
            max_height = max_height.max(*h);
        }

        let revision_key = hash_settings_and_heights(
            settings,
            source_key,
            min_height,
            max_height,
            smoothing_passes.min(16),
            finite_or(smoothing_strength, 0.0).clamp(0.0, 1.0),
        );

        Self {
            settings,
            heights,
            min_height,
            max_height,
            revision_key,
        }
    }

    #[inline]
    pub const fn settings(&self) -> TerrainHeightfieldSettings {
        self.settings
    }

    #[inline]
    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    #[inline]
    pub const fn min_height(&self) -> f32 {
        self.min_height
    }

    #[inline]
    pub const fn max_height(&self) -> f32 {
        self.max_height
    }

    #[inline]
    pub const fn revision_key(&self) -> u64 {
        self.revision_key
    }

    #[inline]
    pub fn vertex_count_x(&self) -> usize {
        self.settings.cells_x as usize + 1
    }

    #[inline]
    pub fn vertex_count_z(&self) -> usize {
        self.settings.cells_z as usize + 1
    }

    #[inline]
    pub fn index(&self, x: usize, z: usize) -> usize {
        z * self.vertex_count_x() + x
    }

    #[inline]
    pub fn height_at_grid(&self, x: usize, z: usize) -> f32 {
        let xx = x.min(self.vertex_count_x().saturating_sub(1));
        let zz = z.min(self.vertex_count_z().saturating_sub(1));
        self.heights[self.index(xx, zz)]
    }

    #[inline]
    pub fn local_position_at_grid(&self, x: usize, z: usize) -> Vec3 {
        let u = x as f32 / self.settings.cells_x as f32;
        let v = z as f32 / self.settings.cells_z as f32;
        Vec3::new(
            (u - 0.5) * self.settings.size_x,
            self.height_at_grid(x, z),
            (v - 0.5) * self.settings.size_z,
        )
    }

    #[inline]
    pub fn contains_local_xz(&self, x: f32, z: f32, skin: f32) -> bool {
        let skin = finite_or(skin, 0.0).abs();
        let hx = self.settings.size_x * 0.5 + skin;
        let hz = self.settings.size_z * 0.5 + skin;
        x.is_finite() && z.is_finite() && x >= -hx && x <= hx && z >= -hz && z <= hz
    }

    pub fn sample_height_local(&self, x: f32, z: f32) -> f32 {
        let sx = self.settings.size_x.max(1.0e-6);
        let sz = self.settings.size_z.max(1.0e-6);
        let u = ((x / sx) + 0.5).clamp(0.0, 1.0) * self.settings.cells_x as f32;
        let v = ((z / sz) + 0.5).clamp(0.0, 1.0) * self.settings.cells_z as f32;

        let x0 = u.floor() as usize;
        let z0 = v.floor() as usize;
        let x1 = (x0 + 1).min(self.vertex_count_x().saturating_sub(1));
        let z1 = (z0 + 1).min(self.vertex_count_z().saturating_sub(1));
        let tx = u - x0 as f32;
        let tz = v - z0 as f32;

        let h00 = self.height_at_grid(x0, z0);
        let h10 = self.height_at_grid(x1, z0);
        let h01 = self.height_at_grid(x0, z1);
        let h11 = self.height_at_grid(x1, z1);
        let hx0 = h00 + (h10 - h00) * tx;
        let hx1 = h01 + (h11 - h01) * tx;
        hx0 + (hx1 - hx0) * tz
    }

    #[inline]
    pub fn sample_height_local_checked(&self, x: f32, z: f32, skin: f32) -> Option<f32> {
        self.contains_local_xz(x, z, skin)
            .then(|| self.sample_height_local(x, z))
    }

    #[inline]
    pub fn local_bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(-self.settings.size_x * 0.5, self.min_height, -self.settings.size_z * 0.5),
            Vec3::new(self.settings.size_x * 0.5, self.max_height, self.settings.size_z * 0.5),
        )
    }
}

#[inline]
fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}

#[inline]
fn mix_u64(mut h: u64, v: u64) -> u64 {
    h ^= v.wrapping_add(0x9e37_79b9_7f4a_7c15).wrapping_add(h << 6).wrapping_add(h >> 2);
    h
}

fn hash_settings_and_heights(
    settings: TerrainHeightfieldSettings,
    source_key: u64,
    min_height: f32,
    max_height: f32,
    smoothing_passes: u32,
    smoothing_strength: f32,
) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    h = mix_u64(h, settings.cells_x as u64);
    h = mix_u64(h, settings.cells_z as u64);
    h = mix_u64(h, settings.size_x.to_bits() as u64);
    h = mix_u64(h, settings.size_z.to_bits() as u64);
    h = mix_u64(h, settings.base_height.to_bits() as u64);
    h = mix_u64(h, settings.height_scale.to_bits() as u64);
    h = mix_u64(h, source_key);
    h = mix_u64(h, min_height.to_bits() as u64);
    h = mix_u64(h, max_height.to_bits() as u64);
    h = mix_u64(h, smoothing_passes as u64);
    h = mix_u64(h, smoothing_strength.to_bits() as u64);
    h
}

fn apply_height_smoothing(
    heights: &mut [f32],
    width: usize,
    height: usize,
    passes: u32,
    strength: f32,
) {
    if passes == 0 || strength <= 0.0 || width < 3 || height < 3 {
        return;
    }

    let mut scratch = heights.to_vec();
    for _ in 0..passes {
        scratch.copy_from_slice(heights);
        for z in 1..height - 1 {
            for x in 1..width - 1 {
                let i = z * width + x;
                let center = scratch[i];
                let avg = (
                    scratch[i - 1]
                    + scratch[i + 1]
                    + scratch[i - width]
                    + scratch[i + width]
                    + center * 2.0
                ) / 6.0;
                heights[i] = center + (avg - center) * strength;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heightfield_has_expected_vertex_count() {
        let hf = HeightField::generate(TerrainHeightfieldSettings { cells_x: 8, cells_z: 4, ..Default::default() });
        assert_eq!(hf.heights().len(), 9 * 5);
    }

    #[test]
    fn graph_descriptor_generates_heightfield() {
        let hf = HeightField::generate_descriptor(TerrainHeightfieldDescriptor {
            cells_x: 8,
            cells_z: 8,
            graph: NoiseGraph2D::electric_veins(42),
            ..Default::default()
        });
        assert_eq!(hf.heights().len(), 81);
    }
}
