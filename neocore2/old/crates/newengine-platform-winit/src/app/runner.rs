#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{Engine, EngineError, EngineResult};
use winit::event_loop::EventLoop;

use newengine_ui::UiBuildFn;

use crate::app::config::WinitAppConfig;
use crate::app::handler::App;

/// Runs winit host and starts the engine after the window is created.
pub fn run_winit_app<E, F>(engine: Engine<E>, after_window: F) -> EngineResult<()>
where
    E: Send + 'static,
    F: FnOnce(&mut Engine<E>) -> EngineResult<()> + 'static,
{
    run_winit_app_with_config(engine, WinitAppConfig::default(), None, after_window)
}

/// Runs winit host with the provided window configuration and starts the engine after the window is created.
pub fn run_winit_app_with_config<E, F>(
    engine: Engine<E>,
    config: WinitAppConfig,
    ui_build: Option<Box<dyn UiBuildFn>>,
    after_window: F,
) -> EngineResult<()>
where
    E: Send + 'static,
    F: FnOnce(&mut Engine<E>) -> EngineResult<()> + 'static,
{
    let event_loop = EventLoop::new().map_err(|e| EngineError::Other(e.to_string()))?;
    let mut app = App::new(engine, config, ui_build, after_window);

    event_loop
        .run_app(&mut app)
        .map_err(|e| EngineError::Other(e.to_string()))
}