#[path = "screen_profile/editor.rs"]
mod editor;
#[path = "screen_profile/dock_toast.rs"]
mod dock_toast;
#[path = "screen_profile/presentation.rs"]
mod presentation;

pub use dock_toast::*;
pub use editor::*;
pub use presentation::*;

#[cfg(test)]
#[path = "screen_profile/tests.rs"]
mod presentation_flow_tests;
