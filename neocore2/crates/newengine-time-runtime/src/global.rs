use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use crate::state::RuntimeHostedTimeState;

static TIME_GATEWAY: OnceLock<Arc<Mutex<RuntimeHostedTimeState>>> = OnceLock::new();

pub(crate) fn state() -> Arc<Mutex<RuntimeHostedTimeState>> {
    Arc::clone(TIME_GATEWAY.get_or_init(|| Arc::new(Mutex::new(RuntimeHostedTimeState::default()))))
}
