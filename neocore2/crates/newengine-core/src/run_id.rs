#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static RUN_ID: OnceLock<String> = OnceLock::new();
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Initializes and returns a process-unique Run ID.
///
/// The value is also stored globally and can be retrieved later via [`run_id`].
/// The format is a 32-char lowercase hex string (16 bytes).
///
/// Layout (16 bytes total):
/// - 8 bytes: unix time in milliseconds (big-endian)
/// - 4 bytes: process id (big-endian)
/// - 4 bytes: per-process sequence (big-endian)
pub fn init_run_id() -> &'static str {
    RUN_ID.get_or_init(|| generate_run_id_hex()).as_str()
}

/// Returns the Run ID if it has been initialized via [`init_run_id`].
pub fn run_id() -> Option<&'static str> {
    RUN_ID.get().map(|s| s.as_str())
}

fn generate_run_id_hex() -> String {
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

    hex_32(bytes)
}

fn hex_32(bytes16: [u8; 16]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 32];

    for (i, b) in bytes16.iter().enumerate() {
        let hi = (b >> 4) as usize;
        let lo = (b & 0x0F) as usize;
        out[i * 2] = LUT[hi];
        out[i * 2 + 1] = LUT[lo];
    }

    // Safety: we only write ASCII hex chars.
    unsafe { String::from_utf8_unchecked(out.to_vec()) }
}