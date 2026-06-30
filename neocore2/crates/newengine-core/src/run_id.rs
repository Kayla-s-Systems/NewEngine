#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static RUN_IDS: OnceLock<RunIds> = OnceLock::new();
static SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
struct RunIds {
    run_id: String,  // 32 hex (16 bytes)
    run_tag: String, // 4-4-4-4 uppercase hex (8 bytes)
}

/// Initializes and returns the process-unique Run ID (primary key).
///
/// - `run_id`: 32-char lowercase hex string (16 bytes)
/// - `run_tag`: 4-4-4-4 uppercase hex string (8 bytes), derived from run_id with avalanche mixing
///
/// Layout for `run_id` (16 bytes total):
/// - 8 bytes: unix time in milliseconds (big-endian)
/// - 4 bytes: process id (big-endian)
/// - 4 bytes: per-process sequence (big-endian)
pub fn init_run_id() -> &'static str {
    RUN_IDS.get_or_init(generate_run_ids).run_id.as_str()
}

/// Returns the Run ID if it has been initialized via [`init_run_id`].
pub fn run_id() -> Option<&'static str> {
    RUN_IDS.get().map(|v| v.run_id.as_str())
}

/// Initializes and returns the human-readable Run Tag (NaughtyDog-style).
///
/// Format: `FFFF-FFFF-FFFF-FFFF` (uppercase hex).
pub fn init_run_tag() -> &'static str {
    RUN_IDS.get_or_init(generate_run_ids).run_tag.as_str()
}

/// Returns the Run Tag if it has been initialized.
pub fn run_tag() -> Option<&'static str> {
    RUN_IDS.get().map(|v| v.run_tag.as_str())
}

fn generate_run_ids() -> RunIds {
    let ts_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let pid: u32 = std::process::id();
    let seq: u32 = (SEQ.fetch_add(1, Ordering::Relaxed) & 0xFFFF_FFFF) as u32;

    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&ts_ms.to_be_bytes());
    bytes[8..12].copy_from_slice(&pid.to_be_bytes());
    bytes[12..16].copy_from_slice(&seq.to_be_bytes());

    let run_id = hex_lower_32(bytes);

    // Derive a readable tag from run_id with avalanche mixing, so it doesn't
    // visually "leak" parts of the primary key.
    let hi = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
    let lo = u64::from_be_bytes(bytes[8..16].try_into().unwrap());
    let mixed = splitmix64(hi ^ lo ^ 0x9E37_79B9_7F4A_7C15u64);
    let run_tag = hex_upper_4x4_u64(mixed);

    RunIds { run_id, run_tag }
}

#[inline]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15u64);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9u64);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EBu64);
    z ^ (z >> 31)
}

fn hex_lower_32(bytes16: [u8; 16]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 32];

    for (i, b) in bytes16.iter().enumerate() {
        let hi = (b >> 4) as usize;
        let lo = (b & 0x0F) as usize;
        out[i * 2] = LUT[hi];
        out[i * 2 + 1] = LUT[lo];
    }

    String::from_utf8(out.to_vec()).expect("hex_lower_32: ascii")
}

fn hex_upper_4x4_u64(v: u64) -> String {
    const LUT: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = [0u8; 19]; // 16 hex + 3 dashes

    let bytes = v.to_be_bytes();

    let mut w = 0usize;
    for (i, b) in bytes.iter().copied().enumerate() {
        if i == 2 || i == 4 || i == 6 {
            out[w] = b'-';
            w += 1;
        }
        let hi = (b >> 4) as usize;
        let lo = (b & 0x0F) as usize;
        out[w] = LUT[hi];
        out[w + 1] = LUT[lo];
        w += 2;
    }

    String::from_utf8(out.to_vec()).expect("hex_upper_4x4_u64: ascii")
}
