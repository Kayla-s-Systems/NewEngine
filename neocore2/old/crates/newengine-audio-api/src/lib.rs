#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(rust_2018_idioms)]
#![allow(clippy::needless_return)]

pub mod ids;
pub mod math;
pub mod types;

pub mod ambience;
pub mod environment;
pub mod mixer;
pub mod music;
pub mod system;
pub mod vehicle;
pub mod voice;
pub mod protocol;

pub mod audio_api;
pub mod capability;
mod occlusion;

pub mod prelude {
    pub use crate::ambience::*;
    pub use crate::audio_api::*;
    pub use crate::capability::*;
    pub use crate::environment::*;
    pub use crate::ids::*;
    pub use crate::math::*;
    pub use crate::mixer::*;
    pub use crate::music::*;
    pub use crate::system::*;
    pub use crate::types::*;
    pub use crate::vehicle::*;
    pub use crate::voice::*;
}
