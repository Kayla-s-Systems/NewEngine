#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NoiseAlgorithm {
    /// Smooth lattice value noise in `[-1, 1]`.
    #[default]
    Value,
    /// Absolute-value fractal shape, useful for cloud/blob masks.
    Billow,
    /// Inverted absolute-value ridges, useful for mountain creases.
    Ridged,
    /// Worley/cellular filled cell value.
    Cellular,
    /// Bright borders between Worley cells; good for energy-vein textures.
    CellularEdge,
    /// Soft Voronoi cell centers.
    VoronoiCells,
    /// Sine-banded warped value noise.
    Marble,
    /// High-contrast branching ridges for lightning/arc masks.
    Lightning,
    /// Cellular edges domain-warped by value ridges.
    Veins,
}

/// Parameterized deterministic 2D fractal noise.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FractalNoise2D {
    pub seed: u64,
    pub frequency: f32,
    pub octaves: u8,
    pub lacunarity: f32,
    pub gain: f32,
    pub amplitude: f32,
    pub algorithm: NoiseAlgorithm,
}

impl Default for FractalNoise2D {
    #[inline]
    fn default() -> Self {
        Self {
            seed: 0x4e45_4f43_4f52_4532,
            frequency: 0.075,
            octaves: 5,
            lacunarity: 2.0,
            gain: 0.5,
            amplitude: 1.0,
            algorithm: NoiseAlgorithm::Value,
        }
    }
}

impl FractalNoise2D {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            seed: self.seed,
            frequency: finite_or(self.frequency, 0.075).abs().max(1.0e-6),
            octaves: self.octaves.clamp(1, 12),
            lacunarity: finite_or(self.lacunarity, 2.0).abs().max(1.01),
            gain: finite_or(self.gain, 0.5).clamp(0.0, 0.99),
            amplitude: finite_or(self.amplitude, 1.0).abs(),
            algorithm: self.algorithm,
        }
    }

    #[inline]
    pub fn sample(self, x: f32, z: f32) -> f32 {
        ValueNoise2D::new(self).sample(x, z)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ValueNoise2D {
    settings: FractalNoise2D,
}

impl ValueNoise2D {
    #[inline]
    pub fn new(settings: FractalNoise2D) -> Self {
        Self {
            settings: settings.sanitized(),
        }
    }

    #[inline]
    pub const fn settings(&self) -> FractalNoise2D {
        self.settings
    }

    /// Returns normalized noise in approximately `[-amplitude, amplitude]`.
    #[inline]
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        sample_fractal(self.settings, x, z)
    }
}

#[inline]
pub fn sample_fractal(settings: FractalNoise2D, x: f32, z: f32) -> f32 {
    let settings = settings.sanitized();
    let mut frequency = settings.frequency;
    let mut amplitude = 1.0_f32;
    let mut weighted = 0.0_f32;
    let mut normalizer = 0.0_f32;

    for octave in 0..settings.octaves {
        let seed = settings.seed ^ ((octave as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let raw = sample_algorithm(seed, settings.algorithm, x * frequency, z * frequency);
        weighted += raw * amplitude;
        normalizer += amplitude;
        frequency *= settings.lacunarity;
        amplitude *= settings.gain;
    }

    if normalizer <= 1.0e-6 {
        0.0
    } else {
        ((weighted / normalizer) * settings.amplitude)
            .clamp(-settings.amplitude, settings.amplitude)
    }
}

#[inline]
pub fn sample_algorithm(seed: u64, algorithm: NoiseAlgorithm, x: f32, z: f32) -> f32 {
    match algorithm {
        NoiseAlgorithm::Value => value_noise(seed, x, z),
        NoiseAlgorithm::Billow => value_noise(seed, x, z).abs() * 2.0 - 1.0,
        NoiseAlgorithm::Ridged => 1.0 - value_noise(seed, x, z).abs() * 2.0,
        NoiseAlgorithm::Cellular => cellular_value(seed, x, z),
        NoiseAlgorithm::CellularEdge => cellular_edge(seed, x, z),
        NoiseAlgorithm::VoronoiCells => cellular_center(seed, x, z),
        NoiseAlgorithm::Marble => marble(seed, x, z),
        NoiseAlgorithm::Lightning => lightning(seed, x, z),
        NoiseAlgorithm::Veins => veins(seed, x, z),
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

#[inline]
fn fast_floor(v: f32) -> i32 {
    let i = v as i32;
    if (i as f32) > v {
        i - 1
    } else {
        i
    }
}

#[inline]
fn fade(t: f32) -> f32 {
    // Perlin fade curve, deterministic and branch-free for sanitized interpolation domain.
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1.0e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub(crate) fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[inline]
fn lattice(seed: u64, x: i32, z: i32) -> f32 {
    let key = seed
        ^ ((x as i64 as u64).wrapping_mul(0x632b_e59b_d9b4_e019))
        ^ ((z as i64 as u64).wrapping_mul(0x8515_7af5_5d10_839d));
    let h = splitmix64(key);
    let unit = ((h >> 40) as u32) as f32 / ((1_u32 << 24) as f32);
    unit * 2.0 - 1.0
}

#[inline]
fn value_noise(seed: u64, x: f32, z: f32) -> f32 {
    let x0 = fast_floor(x);
    let z0 = fast_floor(z);
    let x1 = x0 + 1;
    let z1 = z0 + 1;

    let tx = fade(x - x0 as f32);
    let tz = fade(z - z0 as f32);

    let v00 = lattice(seed, x0, z0);
    let v10 = lattice(seed, x1, z0);
    let v01 = lattice(seed, x0, z1);
    let v11 = lattice(seed, x1, z1);

    lerp(lerp(v00, v10, tx), lerp(v01, v11, tx), tz).clamp(-1.0, 1.0)
}

#[inline]
fn cell_point(seed: u64, x: i32, z: i32) -> (f32, f32, f32) {
    let key = seed
        ^ ((x as i64 as u64).wrapping_mul(0xd6e8_feb8_6659_fd93))
        ^ ((z as i64 as u64).wrapping_mul(0xa076_1d64_78bd_642f));
    let h0 = splitmix64(key);
    let h1 = splitmix64(h0);
    let h2 = splitmix64(h1);
    let ux = ((h0 >> 40) as u32) as f32 / ((1_u32 << 24) as f32);
    let uz = ((h1 >> 40) as u32) as f32 / ((1_u32 << 24) as f32);
    let cv = ((h2 >> 40) as u32) as f32 / ((1_u32 << 24) as f32);
    (x as f32 + ux, z as f32 + uz, cv * 2.0 - 1.0)
}

#[inline]
fn cellular_distances(seed: u64, x: f32, z: f32) -> (f32, f32, f32) {
    let cx = fast_floor(x);
    let cz = fast_floor(z);
    let mut d1 = f32::INFINITY;
    let mut d2 = f32::INFINITY;
    let mut cell_value = 0.0;

    for dz in -1..=1 {
        for dx in -1..=1 {
            let (px, pz, cv) = cell_point(seed, cx + dx, cz + dz);
            let ddx = px - x;
            let ddz = pz - z;
            let d = ddx * ddx + ddz * ddz;
            if d < d1 {
                d2 = d1;
                d1 = d;
                cell_value = cv;
            } else if d < d2 {
                d2 = d;
            }
        }
    }

    (d1.sqrt(), d2.sqrt(), cell_value)
}

#[inline]
fn cellular_value(seed: u64, x: f32, z: f32) -> f32 {
    let (d1, _, _) = cellular_distances(seed, x, z);
    (1.0 - d1.clamp(0.0, 1.0)) * 2.0 - 1.0
}

#[inline]
fn cellular_center(seed: u64, x: f32, z: f32) -> f32 {
    let (d1, _, cell) = cellular_distances(seed, x, z);
    let center_mask = 1.0 - smoothstep(0.12, 0.55, d1);
    (cell * 0.35 + center_mask * 0.95).clamp(-1.0, 1.0)
}

#[inline]
fn cellular_edge(seed: u64, x: f32, z: f32) -> f32 {
    let (d1, d2, _) = cellular_distances(seed, x, z);
    let gap = (d2 - d1).abs();
    let edge = 1.0 - smoothstep(0.015, 0.18, gap);
    (edge * 2.0 - 1.0).clamp(-1.0, 1.0)
}

#[inline]
fn marble(seed: u64, x: f32, z: f32) -> f32 {
    let warp = value_noise(seed ^ 0x51f1_5eed, x * 0.85, z * 0.85) * 2.25;
    let bands = ((x + z * 0.35 + warp) * core::f32::consts::TAU).sin();
    (bands * 0.82 + value_noise(seed ^ 0xa11c_e001, x * 2.3, z * 2.3) * 0.18).clamp(-1.0, 1.0)
}

#[inline]
fn lightning(seed: u64, x: f32, z: f32) -> f32 {
    let warp_x = value_noise(seed ^ 0x1234_9876, x * 0.7, z * 0.7) * 1.65;
    let warp_z = value_noise(seed ^ 0x9876_1234, x * 0.7 + 41.0, z * 0.7 - 19.0) * 1.65;
    let ridge = 1.0 - value_noise(seed ^ 0xf17e_1eaf, (x + warp_x) * 1.9, (z + warp_z) * 1.9).abs();
    let filament = smoothstep(0.78, 0.98, ridge);
    let branches = smoothstep(
        0.74,
        0.96,
        cellular_edge(seed ^ 0xb01d_aced, x * 0.72 + warp_x, z * 0.72 + warp_z) * 0.5 + 0.5,
    );
    ((filament.max(branches * 0.78)) * 2.0 - 1.0).clamp(-1.0, 1.0)
}

#[inline]
fn veins(seed: u64, x: f32, z: f32) -> f32 {
    let wx = value_noise(seed ^ 0x0ddc_0ffe, x * 0.42, z * 0.42) * 1.15;
    let wz = value_noise(seed ^ 0xfeed_5eed, x * 0.42 + 17.0, z * 0.42 - 23.0) * 1.15;
    let edge = cellular_edge(seed ^ 0xce11_5eed, x + wx, z + wz);
    let glow = smoothstep(0.42, 1.0, edge * 0.5 + 0.5);
    let haze = value_noise(seed ^ 0xda7a_cafe, x * 1.35, z * 1.35) * 0.22;
    ((glow + haze).clamp(0.0, 1.0) * 2.0 - 1.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_seed() {
        let n = ValueNoise2D::new(FractalNoise2D::default());
        assert_eq!(n.sample(10.25, -3.75), n.sample(10.25, -3.75));
    }

    #[test]
    fn different_seeds_change_output() {
        let a = ValueNoise2D::new(FractalNoise2D {
            seed: 1,
            ..Default::default()
        });
        let b = ValueNoise2D::new(FractalNoise2D {
            seed: 2,
            ..Default::default()
        });
        assert_ne!(a.sample(5.0, 7.0), b.sample(5.0, 7.0));
    }

    #[test]
    fn algorithms_stay_finite() {
        for algorithm in [
            NoiseAlgorithm::Value,
            NoiseAlgorithm::Billow,
            NoiseAlgorithm::Ridged,
            NoiseAlgorithm::Cellular,
            NoiseAlgorithm::CellularEdge,
            NoiseAlgorithm::VoronoiCells,
            NoiseAlgorithm::Marble,
            NoiseAlgorithm::Lightning,
            NoiseAlgorithm::Veins,
        ] {
            let v = sample_algorithm(42, algorithm, 1.25, -7.5);
            assert!(v.is_finite());
            assert!((-1.0..=1.0).contains(&v));
        }
    }
}
