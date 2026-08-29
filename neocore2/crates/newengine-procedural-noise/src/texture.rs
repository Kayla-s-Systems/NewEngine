#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::graph::NoiseGraph2D;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NoiseTexturePreset {
    ElectricVeins,
    MarbleEnergy,
    SoftCells,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NoiseTextureDescriptor {
    pub width: u32,
    pub height: u32,
    pub world_size_x: f32,
    pub world_size_z: f32,
    pub contrast: f32,
    pub brightness: f32,
    pub graph: NoiseGraph2D,
}

impl NoiseTextureDescriptor {
    #[inline]
    pub fn preset(preset: NoiseTexturePreset, seed: u64, width: u32, height: u32) -> Self {
        let graph = match preset {
            NoiseTexturePreset::ElectricVeins => NoiseGraph2D::electric_veins(seed),
            NoiseTexturePreset::MarbleEnergy => NoiseGraph2D::marble_energy(seed),
            NoiseTexturePreset::SoftCells => NoiseGraph2D::soft_cells(seed),
        };
        Self {
            width,
            height,
            world_size_x: 16.0,
            world_size_z: 16.0,
            contrast: 1.35,
            brightness: 0.0,
            graph,
        }
        .sanitized()
    }

    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.width = self.width.clamp(1, 8192);
        self.height = self.height.clamp(1, 8192);
        self.world_size_x = finite_or(self.world_size_x, 16.0).abs().max(1.0e-6);
        self.world_size_z = finite_or(self.world_size_z, 16.0).abs().max(1.0e-6);
        self.contrast = finite_or(self.contrast, 1.0).clamp(0.0, 16.0);
        self.brightness = finite_or(self.brightness, 0.0).clamp(-1.0, 1.0);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoiseTexture2D {
    pub width: u32,
    pub height: u32,
    /// Linear 8-bit grayscale pixels. Renderer/importer layers can upload this as R8/linear.
    pub pixels: Vec<u8>,
}

impl NoiseTexture2D {
    pub fn generate(desc: NoiseTextureDescriptor) -> Self {
        let desc = desc.sanitized();
        let mut pixels = Vec::with_capacity(desc.width as usize * desc.height as usize);
        let denom_x = desc.width.saturating_sub(1).max(1) as f32;
        let denom_z = desc.height.saturating_sub(1).max(1) as f32;

        for y in 0..desc.height {
            for x in 0..desc.width {
                let u = x as f32 / denom_x;
                let v = y as f32 / denom_z;
                let sx = (u - 0.5) * desc.world_size_x;
                let sz = (v - 0.5) * desc.world_size_z;
                let mut n = desc.graph.sample(sx, sz);
                n = ((n - 0.5) * desc.contrast + 0.5 + desc.brightness).clamp(0.0, 1.0);
                pixels.push((n * 255.0).round() as u8);
            }
        }

        Self {
            width: desc.width,
            height: desc.height,
            pixels,
        }
    }

    /// Tiny debug/export path without pulling image encoders into the foundation crate.
    pub fn to_pgm_bytes(&self) -> Vec<u8> {
        let mut out = format!("P5\n{} {}\n255\n", self.width, self.height).into_bytes();
        out.extend_from_slice(&self.pixels);
        out
    }
}

#[inline]
fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn electric_texture_has_expected_size() {
        let tex = NoiseTexture2D::generate(NoiseTextureDescriptor::preset(
            NoiseTexturePreset::ElectricVeins,
            11,
            16,
            8,
        ));
        assert_eq!(tex.pixels.len(), 128);
    }
}
