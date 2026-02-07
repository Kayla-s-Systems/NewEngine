use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    let is_windows = target.contains("windows");
    let is_msvc = target.contains("msvc");

    let pkg_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "plugin".to_owned());
    let pkg_version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_owned());
    let pkg_desc = env::var("CARGO_PKG_DESCRIPTION").unwrap_or_else(|_| "NewEngine plugin".to_owned());
    let pkg_authors = env::var("CARGO_PKG_AUTHORS").unwrap_or_else(|_| "NewEngine".to_owned());

    // Normalize file stem: keep stable and filesystem-friendly.
    let stem = pkg_name.replace('-', "_");
    let dll_name = format!("{stem}-{pkg_version}.dll");

    if is_windows && is_msvc {
        // Produce only versioned DLL
        println!("cargo:warning=Setting DLL output name to {dll_name}");
        println!("cargo:rustc-cdylib-link-arg=/OUT:{dll_name}");

        // Do not generate .lib/.exp
        println!("cargo:rustc-link-arg=/NOIMPLIB");

        // Do not generate .pdb
        println!("cargo:rustc-link-arg=/DEBUG:NONE");

        // Optional link optimizations (safe)
        println!("cargo:rustc-link-arg=/OPT:REF");
        println!("cargo:rustc-link-arg=/OPT:ICF");
    } else if is_windows {
        // Non-MSVC (gnu) still set output name where applicable.
        println!("cargo:warning=Setting DLL output name to {dll_name}");
    }

    if is_windows {
        embed_windows_version_info(&pkg_name, &pkg_version, &pkg_desc, &pkg_authors);
    }
}

fn embed_windows_version_info(
    pkg_name: &str,
    pkg_version: &str,
    pkg_desc: &str,
    pkg_authors: &str,
) {
    let (maj, min, pat, bld) = parse_semver_4(pkg_version);

    let company = first_author_or(pkg_authors, "NewEngine");
    let product_name = "NewEngine";
    let file_desc = pkg_desc;
    let internal_name = pkg_name;
    let original_filename = format!("{}.dll", pkg_name.replace('-', "_"));

    // Windows resource strings must be UTF-16-ish in the PE; rc accepts quoted UTF-8 in practice.
    let rc = format!(
        r#"#include <windows.h>

#define VER_FILEVERSION             {maj},{min},{pat},{bld}
#define VER_FILEVERSION_STR         "{maj}.{min}.{pat}.{bld}\0"

#define VER_PRODUCTVERSION          {maj},{min},{pat},{bld}
#define VER_PRODUCTVERSION_STR      "{maj}.{min}.{pat}.{bld}\0"

VS_VERSION_INFO VERSIONINFO
 FILEVERSION     VER_FILEVERSION
 PRODUCTVERSION  VER_PRODUCTVERSION
 FILEFLAGSMASK   0x3fL
 FILEFLAGS       0x0L
 FILEOS          0x40004L
 FILETYPE        0x2L
 FILESUBTYPE     0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName",      "{company}\0"
            VALUE "FileDescription",  "{file_desc}\0"
            VALUE "FileVersion",      "{pkg_version}\0"
            VALUE "InternalName",     "{internal_name}\0"
            VALUE "OriginalFilename", "{original_filename}\0"
            VALUE "ProductName",      "{product_name}\0"
            VALUE "ProductVersion",   "{pkg_version}\0"
            VALUE "LegalCopyright",   "Copyright (c) {company}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#,
        maj = maj,
        min = min,
        pat = pat,
        bld = bld,
        company = escape_rc(&*company),
        file_desc = escape_rc(file_desc),
        pkg_version = escape_rc(pkg_version),
        internal_name = escape_rc(internal_name),
        original_filename = escape_rc(&original_filename),
        product_name = escape_rc(product_name),
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let rc_path = out_dir.join("plugin_versioninfo.rc");

    fs::write(&rc_path, rc).expect("failed to write rc");

    // Compile and embed into the resulting DLL.
    embed_resource::compile(rc_path.to_str().unwrap(), embed_resource::NONE);
}

fn parse_semver_4(v: &str) -> (u16, u16, u16, u16) {
    // Accept "x.y.z" or "x.y.z+build" or "x.y.z-bla"
    let mut core = v;
    if let Some(i) = core.find('+') {
        core = &core[..i];
    }
    if let Some(i) = core.find('-') {
        core = &core[..i];
    }

    let mut it = core.split('.');
    let a = it.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
    let b = it.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
    let c = it.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
    (a, b, c, 0)
}

fn first_author_or(authors: &str, fallback: &str) -> String {
    // CARGO_PKG_AUTHORS is "Name <mail>; Name2 <mail2>"
    let first = authors.split(';').next().unwrap_or("").trim();
    if first.is_empty() {
        fallback.to_owned()
    } else {
        // Remove email part for display
        match first.find('<') {
            Some(i) => first[..i].trim().to_owned(),
            None => first.to_owned(),
        }
    }
}

fn escape_rc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\"', "\\\"")
}