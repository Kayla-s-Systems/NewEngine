use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

fn main() {
    let flags = build_flags();

    build_joltc();
    link();
    generate_bindings(&flags).unwrap();
}

fn build_joltc() {
    let mut config = cmake::Config::new("JoltC");

    config.profile("Release");

    if cfg!(windows) {
        config.cxxflag("/EHsc");
    }

    config.configure_arg("-DINTERPROCEDURAL_OPTIMIZATION=OFF");

    if cfg!(feature = "double-precision") {
        config.configure_arg("-DDOUBLE_PRECISION=ON");
    }
    if cfg!(feature = "object-layer-u32") {
        config.configure_arg("-DOBJECT_LAYER_BITS=32");
    }

    let mut dst = config.build();
    dst.push("lib");

    println!("cargo:rustc-link-search=native={}", dst.display());
}

fn link() {
    println!("cargo:rustc-link-lib=Jolt");
    println!("cargo:rustc-link-lib=joltc");
}

fn build_flags() -> Vec<(&'static str, &'static str)> {
    let mut flags = Vec::new();

    flags.push(("JPH_DEBUG_RENDERER", "ON"));

    if cfg!(feature = "double-precision") {
        flags.push(("JPC_DOUBLE_PRECISION", "ON"));
        flags.push(("JPH_DOUBLE_PRECISION", "ON"));
    }

    if cfg!(feature = "object-layer-u32") {
        flags.push(("JPC_OBJECT_LAYER_BITS", "32"));
        flags.push(("JPH_OBJECT_LAYER_BITS", "32"));
    }

    flags
}

fn generate_bindings(flags: &[(&'static str, &'static str)]) -> anyhow::Result<()> {
    let mut builder = bindgen::Builder::default()
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .header("JoltC/JoltC/JoltC.h")
        .clang_arg("-IJoltC")
        .allowlist_item("JPC_.*")
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false);

    for (key, value) in flags {
        builder = builder.clang_arg(format!("-D{key}={value}"));
    }

    // Critical: on Windows, libclang needs MSVC + Windows SDK include paths explicitly.
    if cfg!(windows) {
        for arg in windows_clang_args_for_msvc()? {
            builder = builder.clang_arg(arg);
        }
    }

    let bindings = builder
        .generate()
        .context("failed to generate JoltC bindings")?;

    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings
        .write_to_file(out_path)
        .context("Couldn't write bindings!")
}

// ------------------------- Windows helpers -------------------------

fn windows_clang_args_for_msvc() -> anyhow::Result<Vec<String>> {
    let target = env::var("TARGET").unwrap_or_else(|_| "x86_64-pc-windows-msvc".to_string());

    let mut args = Vec::<String>::new();
    args.push(format!("--target={target}"));
    args.push("-fms-compatibility".to_string());
    args.push("-fms-extensions".to_string());
    args.push("-D_CRT_SECURE_NO_WARNINGS".to_string());

    let (msvc_include, sdk_includes) = windows_include_paths()?;

    // MSVC STL headers
    args.push(format!("-isystem{}", msvc_include.display()));

    // Windows SDK headers (ucrt/shared/um/winrt)
    for p in sdk_includes {
        args.push(format!("-isystem{}", p.display()));
    }

    Ok(args)
}

fn windows_include_paths() -> anyhow::Result<(PathBuf, Vec<PathBuf>)> {
    // 1) MSVC include: derive from the actual cl.exe that cc crate finds.
    let tool = cc::Build::new().get_compiler();
    let cl = tool.path();

    let msvc_ver_dir = cl
        .parent() // .../x64
        .and_then(|p| p.parent()) // .../HostX64
        .and_then(|p| p.parent()) // .../bin
        .and_then(|p| p.parent()) // .../14.xx.xxxxx
        .context("failed to derive MSVC toolchain root from cl.exe path")?;

    let msvc_include = msvc_ver_dir.join("include");
    if !msvc_include.exists() {
        anyhow::bail!(
            "MSVC include dir not found at '{}'",
            msvc_include.display()
        );
    }

    // 2) Windows SDK include: prefer env, fallback to scanning default Windows Kits path.
    let sdk_include_base = if let (Ok(root), Ok(ver)) = (env::var("WindowsSdkDir"), windows_sdk_version_env()) {
        PathBuf::from(root).join("Include").join(ver)
    } else {
        // Typical fallback: C:\Program Files (x86)\Windows Kits\10\Include\<version>
        let root = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\Include");
        let ver = pick_latest_windows_kit_version(&root)
            .with_context(|| format!("failed to find Windows Kits version under '{}'", root.display()))?;
        root.join(ver)
    };

    let mut sdk_includes = Vec::new();
    for sub in ["ucrt", "shared", "um", "winrt"] {
        let p = sdk_include_base.join(sub);
        if p.exists() {
            sdk_includes.push(p);
        }
    }

    if sdk_includes.is_empty() {
        anyhow::bail!(
            "Windows SDK include dirs not found under '{}'",
            sdk_include_base.display()
        );
    }

    Ok((msvc_include, sdk_includes))
}

fn windows_sdk_version_env() -> anyhow::Result<String> {
    // VS Dev Shell usually sets WindowsSDKVersion like "10.0.22621.0\"
    // Normalize by trimming trailing slashes.
    let v = env::var("WindowsSDKVersion")
        .or_else(|_| env::var("WindowsSdkVersion"))
        .context("Windows SDK version env var not set")?;
    Ok(v.trim_end_matches(['\\', '/']).to_string())
}

fn pick_latest_windows_kit_version(include_root: &Path) -> anyhow::Result<String> {
    let mut best: Option<(Vec<u32>, String)> = None;

    for e in fs::read_dir(include_root)? {
        let e = e?;
        if !e.file_type()?.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let parsed = parse_version(&name);
        if parsed.is_empty() {
            continue;
        }

        match &best {
            None => best = Some((parsed, name)),
            Some((bver, _)) => {
                if parsed > *bver {
                    best = Some((parsed, name));
                }
            }
        }
    }

    best.map(|(_, n)| n)
        .context("no version-like directories found")
}

fn parse_version(s: &str) -> Vec<u32> {
    s.split('.').filter_map(|x| x.parse::<u32>().ok()).collect()
}
