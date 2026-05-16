#![forbid(unsafe_op_in_unsafe_fn)]

//! Deterministic procedural noise and terrain data for NewEngine.
//!
//! The crate is engine-foundation level: it does not talk to render backends,
//! plugin hosts, filesystems, clocks, or global RNG. Callers provide explicit
//! settings and receive deterministic CPU data that can be rendered, cooked,
//! streamed, or consumed by the physics layer through heightfield residency.

mod graph;
mod heightfield;
mod mesh;
mod noise;
mod terrain;
mod texture;

pub use graph::{
    DomainWarp2D, NoiseCombineMode, NoiseDomain2D, NoiseGraph2D, NoiseLayer2D, NoiseRemap,
    NoiseShape,
};
pub use heightfield::{HeightField, TerrainHeightfieldDescriptor, TerrainHeightfieldSettings};
pub use noise::{sample_algorithm, sample_fractal, FractalNoise2D, NoiseAlgorithm, ValueNoise2D};
pub use terrain::ProceduralTerrain;
pub use texture::{NoiseTexture2D, NoiseTextureDescriptor, NoiseTexturePreset};
