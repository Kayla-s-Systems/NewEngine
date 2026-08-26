use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_DESCRIPTION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_AUTHORS");
    println!("cargo:rerun-if-env-changed=PROFILE");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        compile_windows_resources();
    }
}

fn compile_windows_resources() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let icon_path = manifest_dir
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join("Engine")
        .join("Content")
        .join("logo.ico");
    let icon_path = fs::canonicalize(&icon_path).unwrap_or_else(|_| {
        panic!(
            "NewEngine canonical Windows application icon is missing: {}",
            icon_path.display()
        )
    });
    println!("cargo:rerun-if-changed={}", icon_path.display());

    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_owned());
    let (major, minor, patch, build) = parse_version(&version);
    let description = env::var("CARGO_PKG_DESCRIPTION")
        .unwrap_or_else(|_| "North Star standalone engine/runtime launcher".to_owned());
    let company = env::var("CARGO_PKG_AUTHORS")
        .ok()
        .and_then(|authors| first_author(&authors))
        .unwrap_or_else(|| "Take Some()".to_owned());
    let debug_flags = if env::var("PROFILE").as_deref() == Ok("release") {
        "0x0L"
    } else {
        "0x1L"
    };

    let rc = format!(
        r#"#include <windows.h>

1 ICON "{icon}"

1 VERSIONINFO
 FILEVERSION {major},{minor},{patch},{build}
 PRODUCTVERSION {major},{minor},{patch},{build}
 FILEFLAGSMASK 0x3fL
 FILEFLAGS {debug_flags}
 FILEOS 0x40004L
 FILETYPE 0x1L
 FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "{company}\0"
            VALUE "FileDescription", "{description}\0"
            VALUE "FileVersion", "{version}\0"
            VALUE "InternalName", "NewEngine\0"
            VALUE "LegalCopyright", "Copyright (c) 2026 {company}\0"
            VALUE "OriginalFilename", "NewEngine.exe\0"
            VALUE "ProductName", "North Star NewEngine\0"
            VALUE "ProductVersion", "{version}\0"
            VALUE "Comments", "North Star modular game engine runtime and project launcher\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#,
        icon = rc_path(&icon_path),
        company = escape_rc(&company),
        description = escape_rc(&description),
        version = escape_rc(&version),
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let rc_path = out_dir.join("newengine_windows_resources.rc");
    fs::write(&rc_path, rc).expect("failed to write NewEngine Windows resource script");
    embed_resource::compile(rc_path, embed_resource::NONE);
}

fn parse_version(version: &str) -> (u16, u16, u16, u16) {
    let without_metadata = version.split_once('+').map_or(version, |(core, _)| core);
    let core = without_metadata
        .split_once('-')
        .map_or(without_metadata, |(core, _)| core);
    let mut parts = core.split('.').map(|part| part.parse::<u16>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn first_author(authors: &str) -> Option<String> {
    let author = authors.split(';').next()?.trim();
    if author.is_empty() {
        None
    } else {
        Some(
            author
                .split_once('<')
                .map_or(author, |(name, _)| name)
                .trim()
                .to_owned(),
        )
    }
}

fn rc_path(path: &Path) -> String {
    escape_rc(&path.to_string_lossy())
}

fn escape_rc(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
