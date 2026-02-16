#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetState {
    Unloaded,
    Loading,
    Ready,
    Failed,
}

impl AssetState {
    #[inline]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "unloaded" => Some(Self::Unloaded),
            "loading" => Some(Self::Loading),
            "ready" => Some(Self::Ready),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

pub trait AssetAccess {
    fn load(&self, logical_path: &str) -> Result<String, String>;
    fn state(&self, id_u128_hex32: &str) -> Result<AssetState, String>;
    fn blob_wire_v1(&self, id_u128_hex32: &str) -> Result<(String, Vec<u8>), String>;
    fn pump(&self);
}

#[derive(Debug)]
pub enum WaitReadyError {
    Timeout,
    Failed(String),
    Backend(String),
}

pub fn wait_ready<A: AssetAccess>(
    assets: &A,
    id_hex32: &str,
    timeout: Duration,
) -> Result<(), WaitReadyError> {
    let t0 = Instant::now();
    let mut spin: u32 = 0;

    loop {
        assets.pump();

        let st = assets.state(id_hex32).map_err(WaitReadyError::Backend)?;
        match st {
            AssetState::Ready => return Ok(()),
            AssetState::Failed => return Err(WaitReadyError::Failed("asset failed".to_string())),
            AssetState::Loading | AssetState::Unloaded => {}
        }

        if t0.elapsed() >= timeout {
            return Err(WaitReadyError::Timeout);
        }

        spin = spin.saturating_add(1);
        if spin < 32 {
            std::thread::yield_now();
        } else if spin < 128 {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            std::thread::sleep(Duration::from_millis(3));
        }
    }
}
