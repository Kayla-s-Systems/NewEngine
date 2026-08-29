/// Human-readable entity name for tools and authoring workflows.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct EntityName(String);

impl EntityName {
    #[inline]
    pub fn new(value: impl AsRef<str>) -> Self {
        Self(value.as_ref().to_owned())
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_exposes_borrowed_text() {
        let name = EntityName::new("player");
        assert_eq!(name.as_str(), "player");
    }
}
