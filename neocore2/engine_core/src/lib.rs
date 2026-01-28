//! NEOCORE2 Engine Core
//!
//! Публичный фасад движка.
//! Всё, что используется приложениями и играми — экспортируется здесь.
//! Внутренние детали остаются скрыты.

pub mod engine;
pub mod config;
pub mod frame;
pub mod module;
pub mod phase;
pub mod schedule;
pub mod telemetry;
pub mod time;
pub mod signals;
pub mod log;

// ===============================
// 🎯 PUBLIC ENGINE SDK FACADE
// ===============================

// Главные типы, которые видит пользователь движка
pub use engine::Engine;
pub use config::EngineConfig;

// ❌ НЕ ре-экспортируем:
// - ModuleConfig
// - FrameSchedule
// - Telemetry internals
// - Time internals
// - signals / log / schedule
//
// Это внутренности движка.