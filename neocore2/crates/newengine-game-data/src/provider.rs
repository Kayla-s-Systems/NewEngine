use std::sync::{Arc, OnceLock};

use super::*;

static RUST_DEFAULT_GAME_DATA: OnceLock<GameData> = OnceLock::new();
static RUST_DEFAULT_GAME_DATA_SHARED: OnceLock<Arc<GameData>> = OnceLock::new();

/// Immutable process-wide Rust fallback snapshot.
///
/// Future Lua integration should replace the provider that creates the active snapshot, not the
/// systems that consume these fields.
#[inline]
pub fn default_game_data() -> &'static GameData {
    RUST_DEFAULT_GAME_DATA.get_or_init(GameData::default)
}

/// Immutable runtime snapshot installed once during scene bootstrap.
/// Gameplay systems consume this resource without invoking the source provider in hot loops.
#[derive(Clone, Debug)]
pub struct GameDataSnapshot {
    source_id: String,
    data: Arc<GameData>,
}

impl GameDataSnapshot {
    #[inline]
    pub fn new(source_id: impl Into<String>, data: GameData) -> Self {
        Self {
            source_id: source_id.into(),
            data: Arc::new(data),
        }
    }

    #[inline]
    pub fn rust_defaults() -> Self {
        let data =
            RUST_DEFAULT_GAME_DATA_SHARED.get_or_init(|| Arc::new(default_game_data().clone()));
        Self {
            source_id: "newengine.game_data.rust_defaults".to_owned(),
            data: Arc::clone(data),
        }
    }

    #[inline]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[inline]
    pub fn data(&self) -> &GameData {
        self.data.as_ref()
    }

    #[inline]
    pub fn shared(&self) -> Arc<GameData> {
        Arc::clone(&self.data)
    }
}

pub trait GameDataProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn load(&self) -> Result<GameData, String>;

    #[inline]
    fn load_snapshot(&self) -> Result<GameDataSnapshot, String> {
        self.load()
            .map(|data| GameDataSnapshot::new(self.id(), data))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RustGameDataProvider;

impl GameDataProvider for RustGameDataProvider {
    fn id(&self) -> &'static str {
        "newengine.game_data.rust_defaults"
    }

    fn load(&self) -> Result<GameData, String> {
        Ok(default_game_data().clone())
    }

    #[inline]
    fn load_snapshot(&self) -> Result<GameDataSnapshot, String> {
        Ok(GameDataSnapshot::rust_defaults())
    }
}
