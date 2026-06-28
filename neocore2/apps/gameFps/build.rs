#[path = "../../build-support/winres.rs"]
mod winres;

fn main() {
    winres::compile_windows_app_resources();
}
