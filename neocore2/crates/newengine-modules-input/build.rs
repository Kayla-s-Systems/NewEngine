use std::env;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();

    // Only for MSVC toolchain
    if target.contains("msvc") {
        // Disable import lib (.lib) and export file (.exp)
        println!("cargo:rustc-link-arg=/NOIMPLIB");

        // Disable PDB generation
        println!("cargo:rustc-link-arg=/DEBUG:NONE");

        // Optional: strip unused sections harder
        println!("cargo:rustc-link-arg=/OPT:REF");
        println!("cargo:rustc-link-arg=/OPT:ICF");
    }

    // Versioned DLL name (as you already do)
    let name = env::var("CARGO_PKG_NAME").expect("CARGO_PKG_NAME not set");
    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION not set");

    let stem = name.replace('-', "_");
    let dll_name = format!("{stem}-{version}.dll");

    println!("cargo:warning=Setting DLL output name to {dll_name}");
    println!("cargo:rustc-cdylib-link-arg=/OUT:{dll_name}");
}