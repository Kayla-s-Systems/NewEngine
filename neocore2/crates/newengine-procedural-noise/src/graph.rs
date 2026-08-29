#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use newengine_math::hash_combine_u64;

use crate::noise::{sample_algorithm, FractalNoise2D, NoiseAlgorithm};

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NoiseDomain2D {
    pub seed: u64,
    pub frequency: f32,
    pub offset_x: f32,
    pub offset_z: f32,
    pub warp: Option<DomainWarp2D>,
}

impl Default for NoiseDomain2D {
    #[inline]
    fn default() -> Self {
        Self {
            seed: 0x4e45_5745_4e47_494e,
            frequency: 1.0,
            offset_x: 0.0,
            offset_z: 0.0,
            warp: None,
        }
    }
}

impl NoiseDomain2D {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            seed: self.seed,
            frequency: finite_or(self.frequency, 1.0).abs().max(1.0e-6),
            offset_x: finite_or(self.offset_x, 0.0),
            offset_z: finite_or(self.offset_z, 0.0),
            warp: self.warp.map(DomainWarp2D::sanitized),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DomainWarp2D {
    pub seed_offset: u64,
    pub frequency: f32,
    pub strength: f32,
    pub octaves: u8,
}

impl Default for DomainWarp2D {
    #[inline]
    fn default() -> Self {
        Self {
            seed_offset: 0x5741_5250,
            frequency: 0.75,
            strength: 1.0,
            octaves: 3,
        }
    }
}

impl DomainWarp2D {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            seed_offset: self.seed_offset,
            frequency: finite_or(self.frequency, 0.75).abs().max(1.0e-6),
            strength: finite_or(self.strength, 1.0).clamp(0.0, 64.0),
            octaves: self.octaves.clamp(1, 8),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NoiseCombineMode {
    Replace,
    Add,
    Multiply,
    Max,
    Min,
    Screen,
    Difference,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NoiseShape {
    Identity,
    Abs,
    Invert,
    Power { exponent: f32 },
    SmoothStep { edge0: f32, edge1: f32 },
    Threshold { threshold: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NoiseLayer2D {
    pub algorithm: NoiseAlgorithm,
    pub combine: NoiseCombineMode,
    pub seed_offset: u64,
    pub frequency: f32,
    pub amplitude: f32,
    pub bias: f32,
    pub shape: NoiseShape,
}

impl NoiseLayer2D {
    #[inline]
    pub const fn new(algorithm: NoiseAlgorithm) -> Self {
        Self {
            algorithm,
            combine: NoiseCombineMode::Add,
            seed_offset: 0,
            frequency: 1.0,
            amplitude: 1.0,
            bias: 0.0,
            shape: NoiseShape::Identity,
        }
    }

    #[inline]
    pub const fn combine(mut self, combine: NoiseCombineMode) -> Self {
        self.combine = combine;
        self
    }

    #[inline]
    pub const fn seed_offset(mut self, seed_offset: u64) -> Self {
        self.seed_offset = seed_offset;
        self
    }

    #[inline]
    pub const fn frequency(mut self, frequency: f32) -> Self {
        self.frequency = frequency;
        self
    }

    #[inline]
    pub const fn amplitude(mut self, amplitude: f32) -> Self {
        self.amplitude = amplitude;
        self
    }

    #[inline]
    pub const fn bias(mut self, bias: f32) -> Self {
        self.bias = bias;
        self
    }

    #[inline]
    pub const fn shape(mut self, shape: NoiseShape) -> Self {
        self.shape = shape;
        self
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            algorithm: self.algorithm,
            combine: self.combine,
            seed_offset: self.seed_offset,
            frequency: finite_or(self.frequency, 1.0).abs().max(1.0e-6),
            amplitude: finite_or(self.amplitude, 1.0),
            bias: finite_or(self.bias, 0.0),
            shape: self.shape.sanitized(),
        }
    }
}

impl Default for NoiseLayer2D {
    #[inline]
    fn default() -> Self {
        Self::new(NoiseAlgorithm::Value)
    }
}

impl NoiseShape {
    #[inline]
    fn sanitized(self) -> Self {
        match self {
            Self::Power { exponent } => Self::Power {
                exponent: finite_or(exponent, 1.0).abs().max(0.001),
            },
            Self::SmoothStep { edge0, edge1 } => Self::SmoothStep {
                edge0: finite_or(edge0, 0.0),
                edge1: finite_or(edge1, 1.0),
            },
            Self::Threshold { threshold } => Self::Threshold {
                threshold: finite_or(threshold, 0.0).clamp(-1.0, 1.0),
            },
            other => other,
        }
    }

    #[inline]
    fn apply(self, v: f32) -> f32 {
        match self.sanitized() {
            Self::Identity => v,
            Self::Abs => v.abs() * 2.0 - 1.0,
            Self::Invert => -v,
            Self::Power { exponent } => v.signum() * v.abs().powf(exponent),
            Self::SmoothStep { edge0, edge1 } => smoothstep(edge0, edge1, v),
            Self::Threshold { threshold } => {
                if v >= threshold {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NoiseRemap {
    pub input_min: f32,
    pub input_max: f32,
    pub output_min: f32,
    pub output_max: f32,
    pub clamp: bool,
}

impl Default for NoiseRemap {
    #[inline]
    fn default() -> Self {
        Self {
            input_min: -1.0,
            input_max: 1.0,
            output_min: -1.0,
            output_max: 1.0,
            clamp: true,
        }
    }
}

impl NoiseRemap {
    #[inline]
    pub fn normalized_01() -> Self {
        Self {
            output_min: 0.0,
            output_max: 1.0,
            ..Self::default()
        }
    }

    #[inline]
    fn apply(self, v: f32) -> f32 {
        let input_min = finite_or(self.input_min, -1.0);
        let input_max = finite_or(self.input_max, 1.0);
        let output_min = finite_or(self.output_min, -1.0);
        let output_max = finite_or(self.output_max, 1.0);
        let t = ((v - input_min) / (input_max - input_min).max(1.0e-6)).clamp(0.0, 1.0);
        let out = output_min + (output_max - output_min) * t;
        if self.clamp {
            out.clamp(output_min.min(output_max), output_min.max(output_max))
        } else {
            out
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NoiseGraph2D {
    pub domain: NoiseDomain2D,
    pub layers: Vec<NoiseLayer2D>,
    pub remap: NoiseRemap,
}

impl Default for NoiseGraph2D {
    #[inline]
    fn default() -> Self {
        Self::from_fractal(FractalNoise2D::default())
    }
}

impl NoiseGraph2D {
    #[inline]
    pub fn new(domain: NoiseDomain2D) -> Self {
        Self {
            domain,
            layers: Vec::new(),
            remap: NoiseRemap::default(),
        }
    }

    #[inline]
    pub fn from_fractal(fractal: FractalNoise2D) -> Self {
        let fractal = fractal.sanitized();
        let mut graph = Self::new(NoiseDomain2D {
            seed: fractal.seed,
            frequency: fractal.frequency,
            ..NoiseDomain2D::default()
        });
        let mut frequency = 1.0_f32;
        let mut amplitude = 1.0_f32;
        for octave in 0..fractal.octaves {
            graph.layers.push(
                NoiseLayer2D::new(fractal.algorithm)
                    .seed_offset((octave as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))
                    .frequency(frequency)
                    .amplitude(amplitude),
            );
            frequency *= fractal.lacunarity;
            amplitude *= fractal.gain;
        }
        graph
    }

    #[inline]
    pub fn with_layer(mut self, layer: NoiseLayer2D) -> Self {
        self.layers.push(layer);
        self
    }

    #[inline]
    pub fn with_remap(mut self, remap: NoiseRemap) -> Self {
        self.remap = remap;
        self
    }

    pub fn electric_veins(seed: u64) -> Self {
        Self::new(NoiseDomain2D {
            seed,
            frequency: 0.09,
            warp: Some(DomainWarp2D {
                frequency: 0.28,
                strength: 1.85,
                octaves: 4,
                ..DomainWarp2D::default()
            }),
            ..NoiseDomain2D::default()
        })
        .with_layer(
            NoiseLayer2D::new(NoiseAlgorithm::Veins)
                .combine(NoiseCombineMode::Replace)
                .frequency(1.0)
                .amplitude(1.4)
                .shape(NoiseShape::SmoothStep {
                    edge0: 0.18,
                    edge1: 0.98,
                }),
        )
        .with_layer(
            NoiseLayer2D::new(NoiseAlgorithm::Lightning)
                .seed_offset(0x1a7e_51de)
                .frequency(2.25)
                .amplitude(0.72)
                .shape(NoiseShape::SmoothStep {
                    edge0: 0.25,
                    edge1: 1.0,
                })
                .combine(NoiseCombineMode::Screen),
        )
        .with_layer(
            NoiseLayer2D::new(NoiseAlgorithm::Value)
                .seed_offset(0x50f7_610f)
                .frequency(0.65)
                .amplitude(0.14)
                .combine(NoiseCombineMode::Add),
        )
        .with_remap(NoiseRemap::normalized_01())
    }

    pub fn soft_cells(seed: u64) -> Self {
        Self::new(NoiseDomain2D {
            seed,
            frequency: 0.12,
            warp: Some(DomainWarp2D {
                frequency: 0.18,
                strength: 0.85,
                octaves: 3,
                ..DomainWarp2D::default()
            }),
            ..NoiseDomain2D::default()
        })
        .with_layer(
            NoiseLayer2D::new(NoiseAlgorithm::VoronoiCells)
                .combine(NoiseCombineMode::Replace)
                .amplitude(1.0)
                .shape(NoiseShape::SmoothStep {
                    edge0: -0.25,
                    edge1: 0.9,
                }),
        )
        .with_layer(
            NoiseLayer2D::new(NoiseAlgorithm::Billow)
                .seed_offset(0x5110_0001)
                .frequency(2.2)
                .amplitude(0.32)
                .combine(NoiseCombineMode::Screen),
        )
        .with_remap(NoiseRemap::normalized_01())
    }

    pub fn marble_energy(seed: u64) -> Self {
        Self::new(NoiseDomain2D {
            seed,
            frequency: 0.075,
            warp: Some(DomainWarp2D {
                frequency: 0.22,
                strength: 2.6,
                octaves: 5,
                ..DomainWarp2D::default()
            }),
            ..NoiseDomain2D::default()
        })
        .with_layer(
            NoiseLayer2D::new(NoiseAlgorithm::Marble)
                .combine(NoiseCombineMode::Replace)
                .frequency(1.0)
                .amplitude(0.85),
        )
        .with_layer(
            NoiseLayer2D::new(NoiseAlgorithm::CellularEdge)
                .seed_offset(0xed9e_0001)
                .frequency(0.8)
                .amplitude(0.45)
                .shape(NoiseShape::SmoothStep {
                    edge0: 0.15,
                    edge1: 1.0,
                })
                .combine(NoiseCombineMode::Screen),
        )
        .with_remap(NoiseRemap::normalized_01())
    }

    #[inline]
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        let domain = self.domain.sanitized();
        let (x, z) = apply_domain(domain, x, z);
        let mut acc = 0.0_f32;
        let mut first = true;

        for layer in &self.layers {
            let layer = layer.sanitized();
            let sample = sample_algorithm(
                domain.seed ^ layer.seed_offset,
                layer.algorithm,
                x * layer.frequency,
                z * layer.frequency,
            );
            let shaped = layer.shape.apply(sample) * layer.amplitude + layer.bias;
            if first || matches!(layer.combine, NoiseCombineMode::Replace) {
                acc = shaped;
                first = false;
                continue;
            }
            acc = combine(acc, shaped, layer.combine);
        }

        self.remap.apply(acc)
    }

    #[inline]
    pub fn sample_uv(&self, u: f32, v: f32) -> f32 {
        self.sample(u, v)
    }

    pub fn revision_key(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        let domain = self.domain.sanitized();
        h = hash_combine_u64(h, domain.seed);
        h = hash_combine_u64(h, domain.frequency.to_bits() as u64);
        h = hash_combine_u64(h, domain.offset_x.to_bits() as u64);
        h = hash_combine_u64(h, domain.offset_z.to_bits() as u64);
        if let Some(warp) = domain.warp {
            h = hash_combine_u64(h, warp.seed_offset);
            h = hash_combine_u64(h, warp.frequency.to_bits() as u64);
            h = hash_combine_u64(h, warp.strength.to_bits() as u64);
            h = hash_combine_u64(h, warp.octaves as u64);
        }
        h = hash_combine_u64(h, self.remap.input_min.to_bits() as u64);
        h = hash_combine_u64(h, self.remap.input_max.to_bits() as u64);
        h = hash_combine_u64(h, self.remap.output_min.to_bits() as u64);
        h = hash_combine_u64(h, self.remap.output_max.to_bits() as u64);
        h = hash_combine_u64(h, self.remap.clamp as u64);
        for layer in &self.layers {
            let layer = layer.sanitized();
            h = hash_combine_u64(h, layer.algorithm as u64);
            h = hash_combine_u64(h, layer.combine as u64);
            h = hash_combine_u64(h, layer.seed_offset);
            h = hash_combine_u64(h, layer.frequency.to_bits() as u64);
            h = hash_combine_u64(h, layer.amplitude.to_bits() as u64);
            h = hash_combine_u64(h, layer.bias.to_bits() as u64);
            h = hash_shape(h, layer.shape);
        }
        h
    }
}

#[inline]
fn hash_shape(mut h: u64, shape: NoiseShape) -> u64 {
    match shape.sanitized() {
        NoiseShape::Identity => hash_combine_u64(h, 0),
        NoiseShape::Abs => hash_combine_u64(h, 1),
        NoiseShape::Invert => hash_combine_u64(h, 2),
        NoiseShape::Power { exponent } => {
            h = hash_combine_u64(h, 3);
            hash_combine_u64(h, exponent.to_bits() as u64)
        }
        NoiseShape::SmoothStep { edge0, edge1 } => {
            h = hash_combine_u64(h, 4);
            h = hash_combine_u64(h, edge0.to_bits() as u64);
            hash_combine_u64(h, edge1.to_bits() as u64)
        }
        NoiseShape::Threshold { threshold } => {
            h = hash_combine_u64(h, 5);
            hash_combine_u64(h, threshold.to_bits() as u64)
        }
    }
}

#[inline]
fn apply_domain(domain: NoiseDomain2D, x: f32, z: f32) -> (f32, f32) {
    let mut x = x * domain.frequency + domain.offset_x;
    let mut z = z * domain.frequency + domain.offset_z;

    if let Some(warp) = domain.warp {
        let warp = warp.sanitized();
        let fx = FractalNoise2D {
            seed: domain.seed ^ warp.seed_offset,
            frequency: warp.frequency,
            octaves: warp.octaves,
            lacunarity: 2.03,
            gain: 0.5,
            amplitude: warp.strength,
            algorithm: NoiseAlgorithm::Value,
        };
        let fz = FractalNoise2D {
            seed: domain.seed ^ warp.seed_offset ^ 0xa5a5_5a5a_c3c3_3c3c,
            ..fx
        };
        x += crate::noise::sample_fractal(fx, x, z);
        z += crate::noise::sample_fractal(fz, x + 19.0, z - 11.0);
    }

    (x, z)
}

#[inline]
fn combine(a: f32, b: f32, mode: NoiseCombineMode) -> f32 {
    match mode {
        NoiseCombineMode::Replace => b,
        NoiseCombineMode::Add => a + b,
        NoiseCombineMode::Multiply => a * b,
        NoiseCombineMode::Max => a.max(b),
        NoiseCombineMode::Min => a.min(b),
        NoiseCombineMode::Screen => 1.0 - (1.0 - a.clamp(0.0, 1.0)) * (1.0 - b.clamp(0.0, 1.0)),
        NoiseCombineMode::Difference => (a - b).abs() * 2.0 - 1.0,
    }
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1.0e-6)).clamp(0.0, 1.0);
    (t * t * (3.0 - 2.0 * t)) * 2.0 - 1.0
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
    fn electric_preset_is_deterministic() {
        let g = NoiseGraph2D::electric_veins(7);
        assert_eq!(g.sample(0.25, 0.75), g.sample(0.25, 0.75));
    }
}
