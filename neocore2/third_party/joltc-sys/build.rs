use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

fn main() {
    println!("cargo:rerun-if-changed=src/bindings_static.rs");
    println!("cargo:rerun-if-env-changed=NEWENGINE_JOLTC_BINDGEN");
    println!("cargo:rerun-if-env-changed=JOLTC_SYS_BINDGEN");
    let flags = build_flags();
    let target = BuildTarget::from_env();

    build_joltc(&target);
    link(&target);
    emit_bindings(&target, &flags).unwrap();
}

fn build_joltc(target: &BuildTarget) {
    let mut config = cmake::Config::new("JoltC");

    if let Some(generator) = cmake_generator_for_target(target) {
        // Set the generator on cmake::Config directly. Setting only the
        // CMAKE_GENERATOR environment variable from the build script is too
        // late for cmake crate generator inference on some hosts.
        config.generator(generator);
    }

    config.profile("Release");

    match target.toolchain {
        WindowsToolchain::Msvc => {
            // MSVC accepts /EHsc. MinGW/GNU treats it as a missing input file and
            // fails CMake's CXX compiler probe before JoltC is even configured.
            config.cxxflag("/EHsc");
        }
        WindowsToolchain::Gnu => {
            // Keep the exception model explicit for Jolt while staying valid for
            // x86_64-pc-windows-gnu / MSYS2 MinGW.
            config.cxxflag("-fexceptions");
        }
        WindowsToolchain::Other => {}
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

fn link(target: &BuildTarget) {
    // GNU/MinGW resolves static archives left-to-right. `joltc` references
    // symbols from `Jolt`, and both use the C++ runtime. Emitting `Jolt`
    // before `joltc` leaves the linker with unresolved JPH::* / operator new /
    // __gxx_personality_seh0 symbols when the final artifact is a Rust cdylib.
    println!("cargo:rustc-link-lib=static=joltc");
    println!("cargo:rustc-link-lib=static=Jolt");

    match target.toolchain {
        WindowsToolchain::Gnu => {
            // Rust's windows-gnu linker invocation does not automatically add
            // the C++ standard library for C++ static archives pulled in by a
            // -sys crate. Keep this explicit and ordered after Jolt.
            println!("cargo:rustc-link-lib=dylib=stdc++");
            // MSYS2/MinGW C++ exceptions use the SEH personality runtime. Some
            // toolchains satisfy this through libgcc_eh, others through the
            // libgcc_s_seh import library. Emitting it explicitly makes the
            // vendored JoltC link deterministic for x86_64-pc-windows-gnu.
            println!("cargo:rustc-link-lib=dylib=gcc_s_seh");
        }
        WindowsToolchain::Other => {
            // Non-MSVC Unix-like targets also need the C++ runtime when linking
            // the static JoltC/Jolt archives into Rust artifacts.
            println!("cargo:rustc-link-lib=dylib=stdc++");
        }
        WindowsToolchain::Msvc => {}
    }
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

fn emit_bindings(
    target: &BuildTarget,
    flags: &[(&'static str, &'static str)],
) -> anyhow::Result<()> {
    // Runtime builds must not require LLVM/libclang to be installed. The C ABI
    // for the vendored JoltC 5.0.0 wrapper is stable inside this third_party
    // directory, so the default path uses a checked-in static binding snapshot.
    // Developers can opt into bindgen explicitly when updating JoltC headers.
    if env_flag("NEWENGINE_JOLTC_BINDGEN") || env_flag("JOLTC_SYS_BINDGEN") {
        return generate_bindings(target, flags);
    }

    copy_static_bindings()
}

fn copy_static_bindings() -> anyhow::Result<()> {
    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("bindings.rs");
    let static_path = Path::new("src").join("bindings_static.rs");
    let source = fs::read_to_string(&static_path).with_context(|| {
        format!(
            "failed to read static JoltC bindings from '{}'",
            static_path.display()
        )
    })?;

    // The static body is included from `src/generated.rs`. Inner crate/module
    // attributes are legal only at the beginning of the enclosing file/module;
    // after `include!` they would appear inside `generated.rs` after existing
    // attributes and produce E0753-style build errors. Keep the allow-list in
    // `generated.rs` and make the copied body pure items.
    let source = strip_leading_inner_allow_attributes(&source);

    fs::write(&out_path, source).with_context(|| {
        format!(
            "failed to write static JoltC bindings to '{}'",
            out_path.display()
        )
    })?;
    if std::env::var_os("NEWENGINE_JOLTC_VERBOSE").is_some() {
        println!("cargo:warning=joltc-sys: using checked-in static JoltC bindings; set NEWENGINE_JOLTC_BINDGEN=1 to regenerate with libclang");
    }
    Ok(())
}

fn strip_leading_inner_allow_attributes(source: &str) -> String {
    let mut output = String::new();
    let mut skipping_prelude = true;

    for line in source.lines() {
        let trimmed = line.trim();
        if skipping_prelude && trimmed.starts_with("#![allow(") {
            continue;
        }
        if skipping_prelude && trimmed.is_empty() {
            continue;
        }
        skipping_prelude = false;
        output.push_str(line);
        output.push('\n');
    }

    output
}

fn env_flag(key: &str) -> bool {
    match env::var(key) {
        Ok(value) => {
            let value = value.trim().to_ascii_lowercase();
            matches!(value.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

fn generate_bindings(
    target: &BuildTarget,
    flags: &[(&'static str, &'static str)],
) -> anyhow::Result<()> {
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

    match target.toolchain {
        // Critical: MSVC libclang needs MSVC + Windows SDK include paths explicitly.
        WindowsToolchain::Msvc => {
            for arg in windows_clang_args_for_msvc(target.clang_triple())? {
                builder = builder.clang_arg(arg);
            }
        }
        // MinGW/GNU must not receive MSVC include-path probing. The toolchain
        // already provides the Windows headers through GCC/Clang search paths.
        WindowsToolchain::Gnu => {
            builder = builder.clang_arg(format!("--target={}", target.clang_triple()));
        }
        WindowsToolchain::Other => {}
    }

    let bindings = builder
        .generate()
        .context("failed to generate JoltC bindings")?;

    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings
        .write_to_file(out_path)
        .context("Couldn't write bindings!")
}


fn cmake_generator_for_target(target: &BuildTarget) -> Option<String> {
    // Generator selection must follow the Rust target toolchain. A Visual Studio
    // generator is valid for MSVC, but invalid for x86_64-pc-windows-gnu: it
    // makes CMake look for Visual Studio even when the active Rust target is
    // MinGW/GNU. Keep this vendored build deterministic and fail with a clear
    // diagnostic instead of letting cmake crate panic with an opaque message.
    if let Some(generator) = explicit_cmake_generator(target) {
        return Some(generator);
    }

    if generator_env_is_set(target) {
        return None;
    }

    match target.toolchain {
        WindowsToolchain::Msvc => Some("Visual Studio 17 2022".to_owned()),
        WindowsToolchain::Gnu => detected_gnu_cmake_generator(),
        WindowsToolchain::Other => None,
    }
}

fn explicit_cmake_generator(target: &BuildTarget) -> Option<String> {
    let Ok(generator) = env::var("NEWENGINE_JOLTC_CMAKE_GENERATOR") else {
        return None;
    };
    let generator = generator.trim();
    if generator.is_empty() {
        return None;
    }

    if target.toolchain == WindowsToolchain::Gnu
        && generator.to_ascii_lowercase().contains("visual studio")
    {
        panic!(
            "NEWENGINE_JOLTC_CMAKE_GENERATOR='{}' is incompatible with target '{}'. \
             Use Ninja or MinGW Makefiles for windows-gnu, or build with a windows-msvc Rust toolchain.",
            generator, target.rust_triple
        );
    }

    Some(generator.to_owned())
}

fn detected_gnu_cmake_generator() -> Option<String> {
    if command_exists("ninja") || command_exists("ninja.exe") || command_exists("ninja-build") {
        return Some("Ninja".to_owned());
    }
    if command_exists("mingw32-make") || command_exists("mingw32-make.exe") {
        return Some("MinGW Makefiles".to_owned());
    }
    None
}

fn command_exists(command: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&paths).any(|dir| dir.join(command).is_file())
}

fn generator_env_is_set(target: &BuildTarget) -> bool {
    let triple_hyphen = target.rust_triple.as_str();
    let triple_underscore = triple_hyphen.replace('-', "_");
    [
        format!("CMAKE_GENERATOR_{triple_hyphen}"),
        format!("CMAKE_GENERATOR_{triple_underscore}"),
        "HOST_CMAKE_GENERATOR".to_owned(),
        "CMAKE_GENERATOR".to_owned(),
    ]
    .iter()
    .any(|key| env::var_os(key).is_some())
}

// ------------------------- Target/toolchain helpers -------------------------

#[derive(Debug, Clone)]
struct BuildTarget {
    rust_triple: String,
    toolchain: WindowsToolchain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsToolchain {
    Msvc,
    Gnu,
    Other,
}

impl BuildTarget {
    fn from_env() -> Self {
        let rust_triple =
            env::var("TARGET").unwrap_or_else(|_| String::from("x86_64-pc-windows-msvc"));

        let toolchain = if rust_triple.ends_with("windows-msvc") {
            WindowsToolchain::Msvc
        } else if rust_triple.ends_with("windows-gnu") || rust_triple.ends_with("windows-gnullvm") {
            WindowsToolchain::Gnu
        } else {
            WindowsToolchain::Other
        };

        Self {
            rust_triple,
            toolchain,
        }
    }

    fn clang_triple(&self) -> &str {
        // Rust's MinGW triple is accepted by many tools, but libclang/clang is
        // most reliable with the canonical MinGW vendor triple.
        match self.rust_triple.as_str() {
            "x86_64-pc-windows-gnu" => "x86_64-w64-windows-gnu",
            "i686-pc-windows-gnu" => "i686-w64-windows-gnu",
            other => other,
        }
    }
}

// ------------------------- Windows helpers -------------------------

fn windows_clang_args_for_msvc(target: &str) -> anyhow::Result<Vec<String>> {
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
