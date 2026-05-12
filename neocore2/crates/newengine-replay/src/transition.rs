use serde::{Deserialize, Serialize};

use crate::ReplayCoordinatorState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCoordinatorTransition {
    pub previous: ReplayCoordinatorState,
    pub next: ReplayCoordinatorState,
    pub changed: bool,
    pub valid: bool,
}
