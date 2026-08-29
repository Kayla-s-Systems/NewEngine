/// Entity kind is a stable, engine-wide category tag.
///
/// The core crate intentionally provides only the type; project-specific meaning
/// should live in plugins/tools to avoid hard-coded enums in the engine host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct EntityKind(pub u32);

impl EntityKind {
    #[inline]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn value(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trips_value() {
        assert_eq!(EntityKind::new(17).value(), 17);
    }
}
