#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimStage {
    Startup,
    PreUpdate,
    Update,
    PostUpdate,
    Shutdown,
}

impl SimStage {
    #[inline]
    pub const fn as_usize(self) -> usize {
        match self {
            Self::Startup => 0,
            Self::PreUpdate => 1,
            Self::Update => 2,
            Self::PostUpdate => 3,
            Self::Shutdown => 4,
        }
    }

    pub const COUNT: usize = 5;
}