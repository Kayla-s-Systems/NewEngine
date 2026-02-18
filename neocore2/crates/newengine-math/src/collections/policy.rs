//! Collections policies.
//!
//! This module defines engine-wide policies for container hashing:
//! - FAST/INTERNAL: deterministic + fast, not DoS-safe
//! - SECURE/UNTRUSTED: randomized + DoS-resistant for external inputs

/// Fast (internal) hash builder.
///
/// Current choice: FxHasher via `fxhash`.
///
/// ⚠️ Not DoS-resistant; must NOT be used for untrusted inputs.
pub type FastBuildHasher = fxhash::FxBuildHasher;

/// Secure (untrusted) hash builder.
///
/// Use this for any data influenced by user input / file formats / network payloads / arbitrary strings.
///
/// Current choice: stdlib RandomState (SipHash).
pub type SecureBuildHasher = std::collections::hash_map::RandomState;
