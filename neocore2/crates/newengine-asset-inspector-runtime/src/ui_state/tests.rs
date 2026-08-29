use super::preview::entry_size_label;
use super::shell::display_path;

#[test]
fn preview_entry_size_label_is_compact() {
    assert_eq!(entry_size_label(Some(512)), "512 B");
    assert_eq!(entry_size_label(Some(2048)), "2.0 KB");
    assert_eq!(entry_size_label(Some(2 * 1024 * 1024)), "2.0 MB");
    assert_eq!(entry_size_label(None), "-");
}

#[test]
fn container_location_uses_provider_entry_suffix() {
    assert_eq!(
        display_path("textures/characters.ytd", true),
        "engine.assets:/textures/characters.ytd#entries"
    );
    assert_eq!(display_path("textures", false), "engine.assets:/textures");
}
