use std::path::Path;

use super::*;

#[test]
fn manifest_supports_an_ordered_logo_sequence() {
    let source = r#"
format_version = 1
schema = "newengine.startup_intro.v1"
enabled = true

[window]
mode = "fullscreen"

[[sequence]]
id = "northstar"
source = "logo.mp4"

[[sequence]]
id = "middleware"
source = "middleware.mp4"
skippable = false
volume = 0.5
"#;
    let manifest: StartupIntroManifest = toml::from_str(source).unwrap();
    crate::resolver::validate_manifest(&manifest).unwrap();
    assert_eq!(manifest.sequence.len(), 2);
    assert_eq!(manifest.sequence[0].id, "northstar");
    assert_eq!(manifest.sequence[1].volume, 0.5);
    assert!(!manifest.sequence[1].skippable);
}

#[test]
fn enabled_logo_ids_must_be_unique_and_non_empty() {
    let manifest = StartupIntroManifest {
        sequence: vec![
            StartupIntroEntry {
                id: "northstar".to_owned(),
                source: "logo.mp4".to_owned(),
                ..StartupIntroEntry::default()
            },
            StartupIntroEntry {
                id: " northstar ".to_owned(),
                source: "logo2.mp4".to_owned(),
                ..StartupIntroEntry::default()
            },
        ],
        ..StartupIntroManifest::default()
    };

    let error = crate::resolver::validate_manifest(&manifest).unwrap_err();
    assert!(error.contains("duplicated"));
}

#[test]
fn per_logo_timeout_must_be_positive() {
    let mut manifest = StartupIntroManifest::default();
    manifest.sequence.push(StartupIntroEntry {
        id: "northstar".to_owned(),
        source: "logo.mp4".to_owned(),
        max_duration_ms: Some(0),
        ..StartupIntroEntry::default()
    });

    let error = crate::resolver::validate_manifest(&manifest).unwrap_err();
    assert!(error.contains("max_duration_ms"));
}

#[test]
fn root_dir_token_is_data_driven() {
    let root = Path::new("C:/NorthStar");
    let runtime = Path::new("C:/NorthStar/NewEngine/neocore2/runtime.toml");
    let resolved = resolve_descriptor_path(
        "ROOT-DIR/Shared/Source/authoring/northstar/intro/intro.toml",
        runtime,
        root,
    );
    assert_eq!(
        resolved,
        root.join("Shared/Source/authoring/northstar/intro/intro.toml")
    );
}
