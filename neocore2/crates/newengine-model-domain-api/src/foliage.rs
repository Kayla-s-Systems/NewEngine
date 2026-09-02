use std::collections::BTreeMap;

use newengine_materials::MaterialId;
use serde::{Deserialize, Serialize};

include!("foliage/settings.rs");
include!("foliage/asset.rs");
include!("foliage/extraction_types.rs");
include!("foliage/extraction_plan.rs");
include!("foliage/util.rs");

#[cfg(test)]
#[path = "foliage/tests.rs"]
mod tests;
